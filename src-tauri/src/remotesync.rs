//! Remote Device Playback Sync (Direct IP / Tailscale - Orchard inspired zero-friction pairing).
//!
//! Embedded WebSocket server + UDP LAN discovery that allows Nocturne Mobile to automatically
//! discover, pair, and control desktop playback with instant bidirectional state syncing.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use listen_protocol::{RoomState, Track};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Emitter;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncWireMessage {
    AuthChallenge { nonce: String, host_device_name: String },
    AuthResponse { client_device_name: String, pin_hash: Option<String> },
    AuthResult { success: bool, session_token: Option<String>, message: Option<String> },
    SyncState { state: RoomState },
    PlaybackAction { action: RemotePlaybackAction },
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePlaybackAction {
    pub kind: String,
    #[serde(default)]
    pub position_ms: i64,
    #[serde(default)]
    pub track: Option<Track>,
    #[serde(default)]
    pub queue: Option<Vec<Track>>,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedClient {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub connected_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSyncInfo {
    pub is_running: bool,
    pub port: u16,
    pub local_ip: String,
    pub device_name: String,
    pub connected_clients: Vec<ConnectedClient>,
}

pub struct RemoteSyncController {
    running: Arc<AtomicBool>,
    broadcast_tx: broadcast::Sender<String>,
    state_ref: Arc<RwLock<Option<Arc<AppState>>>>,
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    connected_clients: Arc<RwLock<Vec<ConnectedClient>>>,
    current_port: Arc<RwLock<u16>>,
}

impl RemoteSyncController {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(128);
        Self {
            running: Arc::new(AtomicBool::new(false)),
            broadcast_tx,
            state_ref: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            connected_clients: Arc::new(RwLock::new(Vec::new())),
            current_port: Arc::new(RwLock::new(8080)),
        }
    }

    pub async fn set_app_state(&self, app_state: Arc<AppState>) {
        let mut w = self.state_ref.write().await;
        *w = Some(app_state);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub async fn get_info(&self) -> RemoteSyncInfo {
        let port = *self.current_port.read().await;
        let clients = self.connected_clients.read().await.clone();
        let local_ip = get_local_ip();
        let device_name = get_device_name();

        RemoteSyncInfo {
            is_running: self.is_running(),
            port,
            local_ip,
            device_name,
            connected_clients: clients,
        }
    }

    pub async fn start(&self, port: u16) -> Result<(), String> {
        if self.is_running() {
            self.stop().await;
        }

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr).await.map_err(|e| e.to_string())?;
        *self.current_port.write().await = port;

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.lock().await = Some(shutdown_tx);
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let state_ref = self.state_ref.clone();
        let connected_clients = self.connected_clients.clone();

        // Spawn UDP Discovery Beacon and Responder (Port 8081)
        tokio::spawn(async move {
            run_udp_discovery(port).await;
        });

        // Spawn WebSocket Server
        tokio::spawn(async move {
            tracing::info!(port, "Remote sync server started on 0.0.0.0:{port}");
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, peer_addr)) => {
                                let b_tx = broadcast_tx.clone();
                                let b_rx = broadcast_tx.subscribe();
                                let st_ref = state_ref.clone();
                                let clients_ref = connected_clients.clone();
                                tokio::spawn(async move {
                                    handle_connection(stream, peer_addr, b_tx, b_rx, st_ref, clients_ref).await;
                                });
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Accept error in remote sync server");
                            }
                        }
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            tracing::info!("Remote sync server stopped");
        });

        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(());
        }
        self.connected_clients.write().await.clear();
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn broadcast_state(&self, state: RoomState) {
        if !self.is_running() {
            return;
        }
        let msg = SyncWireMessage::SyncState { state };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.broadcast_tx.send(json);
        }
    }
}

async fn run_udp_discovery(ws_port: u16) {
    let socket = match UdpSocket::bind("0.0.0.0:8081").await {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("UDP discovery bind on 8081 skipped/busy: {e}");
            return;
        }
    };
    let _ = socket.set_broadcast(true);

    let mut buf = [0u8; 1024];
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let local_ip = get_local_ip();
                let device_name = get_device_name();
                let beacon = serde_json::json!({
                    "service": "nocturne-sync",
                    "device_name": device_name,
                    "port": ws_port,
                    "ip": local_ip
                });
                let bytes = beacon.to_string().into_bytes();
                let broadcast_addr: SocketAddr = "255.255.255.255:8081".parse().unwrap();
                let _ = socket.send_to(&bytes, broadcast_addr).await;
            }
            recv_res = socket.recv_from(&mut buf) => {
                if let Ok((len, peer)) = recv_res {
                    let msg = String::from_utf8_lossy(&buf[..len]);
                    if msg.contains("nocturne-discover") || msg.contains("discover") {
                        let local_ip = get_local_ip();
                        let device_name = get_device_name();
                        let response = serde_json::json!({
                            "service": "nocturne-sync",
                            "device_name": device_name,
                            "port": ws_port,
                            "ip": local_ip
                        });
                        let bytes = response.to_string().into_bytes();
                        let _ = socket.send_to(&bytes, peer).await;
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    b_tx: broadcast::Sender<String>,
    mut b_rx: broadcast::Receiver<String>,
    state_ref: Arc<RwLock<Option<Arc<AppState>>>>,
    connected_clients: Arc<RwLock<Vec<ConnectedClient>>>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::debug!(error = %e, "WS handshake failed for {peer_addr}");
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let client_id = format!("{peer_addr}");
    let client_ip = peer_addr.ip().to_string();

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut client_info = ConnectedClient {
        id: client_id.clone(),
        name: format!("Mobile ({client_ip})"),
        ip: client_ip.clone(),
        connected_at: now_secs,
    };

    {
        let mut clients = connected_clients.write().await;
        clients.push(client_info.clone());
    }

    // Send immediate Auth OK & initial state on connect
    let auth_ok = SyncWireMessage::AuthResult {
        success: true,
        session_token: Some("token_ok".into()),
        message: None,
    };
    let _ = ws_sender
        .send(Message::Text(serde_json::to_string(&auth_ok).unwrap().into()))
        .await;

    if let Some(app_state) = state_ref.read().await.as_ref() {
        let snapshot = app_state.playback_snapshot().await;
        let state = room_state_from_snapshot(&snapshot);
        let state_msg = SyncWireMessage::SyncState { state };
        let _ = ws_sender
            .send(Message::Text(serde_json::to_string(&state_msg).unwrap().into()))
            .await;
    }

    // Active session loop
    loop {
        tokio::select! {
            broadcast_msg = b_rx.recv() => {
                if let Ok(text) = broadcast_msg {
                    if ws_sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }
            client_msg = ws_receiver.next() => {
                match client_msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(wire_msg) = serde_json::from_str::<SyncWireMessage>(&t) {
                            match wire_msg {
                                SyncWireMessage::AuthResponse { client_device_name, .. } => {
                                    client_info.name = client_device_name;
                                    let mut clients = connected_clients.write().await;
                                    if let Some(c) = clients.iter_mut().find(|c| c.id == client_id) {
                                        c.name = client_info.name.clone();
                                    }
                                }
                                SyncWireMessage::PlaybackAction { action } => {
                                    if let Some(app_state) = state_ref.read().await.as_ref() {
                                        apply_remote_action(app_state, action).await;
                                        // Broadcast updated state immediately
                                        let snapshot = app_state.playback_snapshot().await;
                                        let state = room_state_from_snapshot(&snapshot);
                                        let msg = SyncWireMessage::SyncState { state };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            let _ = b_tx.send(json);
                                        }
                                    }
                                }
                                SyncWireMessage::Ping => {
                                    let _ = ws_sender.send(Message::Text(r#"{"type":"pong"}"#.into())).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Ping(d))) => {
                        let _ = ws_sender.send(Message::Pong(d)).await;
                    }
                    _ => break,
                }
            }
        }
    }

    // Cleanup client on disconnect
    {
        let mut clients = connected_clients.write().await;
        clients.retain(|c| c.id != client_id);
    }
}

async fn apply_remote_action(state: &Arc<AppState>, action: RemotePlaybackAction) {
    match action.kind.as_str() {
        "play" | "toggle" => {
            state.resume_or_toggle().await;
        }
        "pause" => {
            let _ = state.player.pause();
        }
        "seek" => {
            let _ = state.user_seek(action.position_ms as f64 / 1000.0).await;
        }
        "next_track" | "next" => {
            state.next_in_queue().await;
        }
        "previous_track" | "prev" | "previous" => {
            state.prev_in_queue().await;
        }
        "change_track" => {
            if let Some(track) = action.track {
                let song = innertube::SongItem {
                    video_id: track.id,
                    title: track.title,
                    artists: track.artist,
                    thumbnail: track.thumbnail,
                    duration: None,
                    ..Default::default()
                };
                state.play_song(song).await;
            }
        }
        "set_volume" => {
            let vol = (action.volume * 100.0).clamp(0.0, 100.0) as i64;
            let _ = state.player.set_volume(vol);
            let _ = state.app.emit("volume", vol);
        }
        _ => {}
    }
}

pub fn room_state_from_snapshot(v: &Value) -> RoomState {
    let title = v.get("title").and_then(Value::as_str).unwrap_or_default().to_string();
    let artists = v.get("artists").and_then(Value::as_str).unwrap_or_default().to_string();
    let video_id = v.get("videoId").and_then(Value::as_str).unwrap_or_default().to_string();
    let thumbnail = v.get("thumbnail").and_then(Value::as_str).map(str::to_string);
    let playing = v.get("playing").and_then(Value::as_bool).unwrap_or(false);
    let position = v.get("position").and_then(Value::as_f64).unwrap_or(0.0);
    let duration = v.get("duration").and_then(Value::as_f64).unwrap_or(0.0);

    let track = if !video_id.is_empty() {
        Some(Track {
            id: video_id,
            title,
            artist: artists,
            thumbnail,
            duration_ms: (duration * 1000.0) as i64,
            queued_by: None,
        })
    } else {
        None
    };

    RoomState {
        room_code: "DIRECT".into(),
        host_id: "desktop".into(),
        users: vec![],
        current_track: track,
        is_playing: playing,
        position_ms: (position * 1000.0) as i64,
        last_update_ms: crate::db::now_secs() * 1000,
        volume: 1.0,
        queue: vec![],
    }
}

pub fn get_local_ip() -> String {
    use std::net::UdpSocket as StdUdpSocket;
    if let Ok(socket) = StdUdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".into()
}

pub fn get_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Nocturne PC".into())
}
