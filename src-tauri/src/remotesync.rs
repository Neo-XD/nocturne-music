//! Remote Device Playback Sync (Direct IP / Tailscale with PIN).
//!
//! Embedded WebSocket server that allows Nocturne Mobile (or remote clients) to securely
//! pair and control desktop playback over local network or Tailscale.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use listen_protocol::{PlaybackKind, RoomState, Track};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncWireMessage {
    AuthChallenge { nonce: String, host_device_name: String },
    AuthResponse { client_device_name: String, pin_hash: String },
    AuthResult { success: bool, session_token: Option<String>, message: Option<String> },
    SyncState { state: RoomState },
    PlaybackAction { action: RemotePlaybackAction },
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePlaybackAction {
    pub kind: PlaybackKind,
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

pub struct RemoteSyncController {
    running: Arc<AtomicBool>,
    broadcast_tx: broadcast::Sender<String>,
    state_ref: Arc<RwLock<Option<Arc<AppState>>>>,
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl RemoteSyncController {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(64);
        Self {
            running: Arc::new(AtomicBool::new(false)),
            broadcast_tx,
            state_ref: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_app_state(&self, app_state: Arc<AppState>) {
        let mut w = self.state_ref.write().await;
        *w = Some(app_state);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub async fn start(&self, port: u16, pin: String) -> Result<(), String> {
        if self.is_running() {
            self.stop().await;
        }

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr).await.map_err(|e| e.to_string())?;

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.lock().await = Some(shutdown_tx);
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let state_ref = self.state_ref.clone();
        let pin = Arc::new(pin);

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
                                let p = pin.clone();
                                tokio::spawn(async move {
                                    handle_connection(stream, peer_addr, b_tx, b_rx, st_ref, p).await;
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

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    _b_tx: broadcast::Sender<String>,
    mut b_rx: broadcast::Receiver<String>,
    state_ref: Arc<RwLock<Option<Arc<AppState>>>>,
    configured_pin: Arc<String>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::debug!(error = %e, "WS handshake failed for {peer_addr}");
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Authenticate
    let mut authenticated = false;
    let target_sha256 = sha256_hex(&configured_pin);

    while let Some(msg_res) = ws_receiver.next().await {
        let msg = match msg_res {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(data)) => {
                let _ = ws_sender.send(Message::Pong(data)).await;
                continue;
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        };

        if let Ok(wire_msg) = serde_json::from_str::<SyncWireMessage>(&msg) {
            match wire_msg {
                SyncWireMessage::AuthResponse { pin_hash, .. } => {
                    if pin_hash.eq_ignore_ascii_case(&target_sha256) || pin_hash == *configured_pin
                    {
                        authenticated = true;
                        let res = SyncWireMessage::AuthResult {
                            success: true,
                            session_token: Some("token_ok".into()),
                            message: None,
                        };
                        let _ = ws_sender
                            .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                            .await;

                        // Send current state
                        if let Some(app_state) = state_ref.read().await.as_ref() {
                            let snapshot = app_state.playback_snapshot().await;
                            let state = room_state_from_snapshot(&snapshot);
                            let state_msg = SyncWireMessage::SyncState { state };
                            let _ = ws_sender
                                .send(Message::Text(
                                    serde_json::to_string(&state_msg).unwrap().into(),
                                ))
                                .await;
                        }
                        break;
                    } else {
                        let res = SyncWireMessage::AuthResult {
                            success: false,
                            session_token: None,
                            message: Some("Invalid PIN".into()),
                        };
                        let _ = ws_sender
                            .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                            .await;
                        return;
                    }
                }
                _ => continue,
            }
        }
    }

    if !authenticated {
        return;
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
                        if let Ok(SyncWireMessage::PlaybackAction { action }) = serde_json::from_str::<SyncWireMessage>(&t) {
                            if let Some(app_state) = state_ref.read().await.as_ref() {
                                apply_remote_action(app_state, action).await;
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
}

async fn apply_remote_action(state: &Arc<AppState>, action: RemotePlaybackAction) {
    match action.kind {
        PlaybackKind::Play => {
            state.resume_or_toggle().await;
        }
        PlaybackKind::Pause => {
            state.resume_or_toggle().await;
        }
        PlaybackKind::Seek => {
            let _ = state.user_seek(action.position_ms as f64 / 1000.0).await;
        }
        PlaybackKind::ChangeTrack => {
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
        PlaybackKind::SetVolume => {
            let vol = (action.volume * 100.0) as i64;
            let _ = state.player.set_volume(vol);
        }
        _ => {}
    }
}

fn room_state_from_snapshot(v: &Value) -> RoomState {
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

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}
