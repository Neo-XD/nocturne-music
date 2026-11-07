use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use innertube::SongItem;
use librespot::connect::config::ConnectConfig;
use librespot::connect::spirc::Spirc;
use librespot::core::authentication::Credentials;
use librespot::core::config::{DeviceType, SessionConfig};
use librespot::core::session::Session;
use librespot::discovery::Discovery;
use librespot::metadata::{Metadata, Track as SpTrack};
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::softmixer::SoftMixer;
use librespot::playback::mixer::{Mixer, MixerConfig};
use librespot::playback::player::{Player, PlayerEvent};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

const DEFAULT_SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TrackItem {
    pub video_id: String,
    pub title: String,
    pub artists: String,
    pub album: Option<String>,
    pub is_video: bool,
}

impl From<&SongItem> for TrackItem {
    fn from(item: &SongItem) -> Self {
        TrackItem {
            video_id: item.video_id.clone(),
            title: item.title.clone(),
            artists: item.artists.clone(),
            album: item.album.clone(),
            is_video: item.is_video,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SpotifyConnectStatus {
    pub enabled: bool,
    pub device_name: String,
    pub device_id: String,
    pub active: bool,
    pub scrobble_enabled: bool,
    pub total_scrobbles: u64,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SpotifyConnectStateEvent {
    pub active: bool,
    pub is_playing: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover_url: Option<String>,
    pub duration_ms: u64,
    pub position_ms: u64,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SpotifyScrobbleEvent {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub spotify_id: Option<String>,
    pub scrobbled_at: i64,
}

enum ConnectMsg {
    Track(Box<TrackItem>),
    Duration(f64),
    Position(f64),
    SetEnabled(bool),
    SetDeviceName(String),
    SetScrobbleEnabled(bool),
}

#[derive(Clone)]
struct InternalConnectState {
    event: SpotifyConnectStateEvent,
    anchor: Option<std::time::Instant>,
}

#[derive(Clone)]
pub struct SpotifyConnectHandle {
    tx: UnboundedSender<ConnectMsg>,
    active: Arc<AtomicBool>,
    current_state: Arc<std::sync::Mutex<Option<InternalConnectState>>>,
}

impl SpotifyConnectHandle {
    pub fn set_track(&self, item: &SongItem) {
        let _ = self.tx.send(ConnectMsg::Track(Box::new(TrackItem::from(item))));
    }

    pub fn set_duration(&self, secs: f64) {
        let _ = self.tx.send(ConnectMsg::Duration(secs));
    }

    pub fn set_position(&self, pos: f64) {
        let _ = self.tx.send(ConnectMsg::Position(pos));
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.tx.send(ConnectMsg::SetEnabled(enabled));
    }

    pub fn set_device_name(&self, name: String) {
        let _ = self.tx.send(ConnectMsg::SetDeviceName(name));
    }

    pub fn set_scrobble_enabled(&self, enabled: bool) {
        let _ = self.tx.send(ConnectMsg::SetScrobbleEnabled(enabled));
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn get_current_state(&self) -> Option<SpotifyConnectStateEvent> {
        self.current_state.lock().ok().and_then(|g| {
            g.as_ref().map(|s| {
                let mut ev = s.event.clone();
                if let Some(anchor) = s.anchor {
                    let elapsed = anchor.elapsed().as_millis() as u64;
                    if ev.duration_ms > 0 {
                        ev.position_ms = (ev.position_ms + elapsed).min(ev.duration_ms);
                    } else {
                        ev.position_ms += elapsed;
                    }
                }
                ev
            })
        })
    }
}

pub fn spawn(db: &Arc<crate::db::Db>, app: AppHandle) -> SpotifyConnectHandle {
    let (tx, mut rx) = unbounded_channel();
    let active = Arc::new(AtomicBool::new(false));
    let current_state = Arc::new(std::sync::Mutex::new(None));

    let mut enabled = db.get_setting("spotify_connect_enabled").as_deref() != Some("false");
    let mut scrobble_enabled =
        db.get_setting("spotify_scrobble_enabled").as_deref() != Some("false");
    let mut device_name = db
        .get_setting("spotify_connect_device_name")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Nocturne".to_string());

    let device_id = match db.get_setting("spotify_connect_device_id") {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            let mut bytes = [0u8; 20];
            rand::Rng::fill(&mut rand::thread_rng(), &mut bytes[..]);
            let id = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            db.set_setting("spotify_connect_device_id", &id);
            id
        }
    };

    let client_id = db
        .get_setting("spotify_client_id")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SPOTIFY_CLIENT_ID.to_string());

    let db_clone = db.clone();
    let app_clone = app.clone();
    let active_clone = active.clone();
    let cur_state_clone = current_state.clone();

    tauri::async_runtime::spawn(async move {
        let mut cur_track: Option<Box<TrackItem>> = None;
        let mut cur_dur: f64 = 0.0;
        let mut scrobbled_current = false;

        let mut discovery_session: Option<Discovery> = None;
        let mut spirc_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>> = None;

        if enabled {
            discovery_session = start_discovery(&device_id, &client_id, &device_name);
        }

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    match msg {
                        ConnectMsg::Track(item) => {
                            cur_track = Some(item);
                            cur_dur = 0.0;
                            scrobbled_current = false;
                        }
                        ConnectMsg::Duration(secs) => {
                            cur_dur = secs;
                        }
                        ConnectMsg::Position(pos) => {
                            if !scrobble_enabled || scrobbled_current || cur_dur < 30.0 {
                                continue;
                            }
                            let threshold = (cur_dur * 0.5).min(240.0);
                            if pos >= threshold {
                                scrobbled_current = true;
                                if let Some(ref track) = cur_track {
                                    let t = track.clone();
                                    let db_arc = db_clone.clone();
                                    let app_h = app_clone.clone();
                                    tauri::async_runtime::spawn(async move {
                                        execute_scrobble(&db_arc, &app_h, &t).await;
                                    });
                                }
                            }
                        }
                        ConnectMsg::SetEnabled(on) => {
                            enabled = on;
                            db_clone.set_setting("spotify_connect_enabled", if on { "true" } else { "false" });
                            if on {
                                if discovery_session.is_none() {
                                    discovery_session = start_discovery(&device_id, &client_id, &device_name);
                                }
                            } else {
                                if let Some(disc) = discovery_session.take() {
                                    disc.shutdown().await;
                                }
                                if let Some(tx) = spirc_shutdown_tx.take() {
                                    let _ = tx.send(());
                                }
                                active_clone.store(false, Ordering::Relaxed);
                                if let Ok(mut lk) = cur_state_clone.lock() {
                                    *lk = None;
                                }
                            }
                        }
                        ConnectMsg::SetDeviceName(name) => {
                            let clean = name.trim().to_string();
                            if !clean.is_empty() {
                                device_name = clean;
                                db_clone.set_setting("spotify_connect_device_name", &device_name);
                                if enabled {
                                    if let Some(disc) = discovery_session.take() {
                                        disc.shutdown().await;
                                    }
                                    discovery_session = start_discovery(&device_id, &client_id, &device_name);
                                }
                            }
                        }
                        ConnectMsg::SetScrobbleEnabled(on) => {
                            scrobble_enabled = on;
                            db_clone.set_setting("spotify_scrobble_enabled", if on { "true" } else { "false" });
                        }
                    }
                }
                Some(creds) = async {
                    match discovery_session.as_mut() {
                        Some(d) => d.next().await,
                        None => std::future::pending().await,
                    }
                } => {
                    tracing::info!("Received Spotify Connect credentials for user: {:?}", creds.username);
                    if let Some(tx) = spirc_shutdown_tx.take() {
                        let _ = tx.send(());
                    }

                    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                    spirc_shutdown_tx = Some(shutdown_tx);

                    let dev_id = device_id.clone();
                    let dev_name = device_name.clone();
                    let app_h = app_clone.clone();
                    let act = active_clone.clone();
                    let cs = cur_state_clone.clone();

                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = run_connect_session(
                            dev_id,
                            dev_name,
                            creds,
                            app_h,
                            act,
                            cs,
                            shutdown_rx,
                        )
                        .await
                        {
                            tracing::warn!("Spotify Connect session terminated: {e}");
                        }
                    });
                }
                else => break,
            }
        }
    });

    SpotifyConnectHandle { tx, active, current_state }
}

fn start_discovery(device_id: &str, client_id: &str, device_name: &str) -> Option<Discovery> {
    match Discovery::builder(device_id, client_id)
        .name(device_name.to_string())
        .device_type(DeviceType::Computer)
        .launch()
    {
        Ok(d) => {
            tracing::info!("Spotify Connect discovery launched as '{device_name}'");
            Some(d)
        }
        Err(e) => {
            tracing::warn!("Failed to launch Spotify Connect discovery: {e}");
            None
        }
    }
}

async fn run_connect_session(
    device_id: String,
    device_name: String,
    creds: Credentials,
    app: AppHandle,
    active: Arc<AtomicBool>,
    current_state: Arc<std::sync::Mutex<Option<InternalConnectState>>>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    let session_config = SessionConfig { device_id: device_id.clone(), ..Default::default() };

    let session = Session::new(session_config, None);
    session
        .connect(creds.clone(), true)
        .await
        .map_err(|e| format!("Failed to connect Spotify session: {e}"))?;

    let player_config = PlayerConfig::default();
    let mixer_config = MixerConfig::default();
    let mixer = Arc::new(SoftMixer::open(mixer_config));
    let volume_getter = mixer.get_soft_volume();

    let sink_builder = audio_backend::find(None)
        .ok_or_else(|| "No audio backend found for Spotify Connect".to_string())?;

    let player = Player::new(player_config, session.clone(), volume_getter, move || {
        sink_builder(None, AudioFormat::default())
    });

    let connect_config = ConnectConfig {
        name: device_name.clone(),
        device_type: DeviceType::Computer,
        ..Default::default()
    };

    let (_spirc, spirc_task) =
        Spirc::new(connect_config, session.clone(), creds, player.clone(), mixer)
            .await
            .map_err(|e| format!("Failed to initialize Spirc: {e}"))?;

    active.store(true, Ordering::Relaxed);
    let initial_ev = SpotifyConnectStateEvent {
        active: true,
        is_playing: false,
        title: None,
        artist: None,
        album: None,
        cover_url: None,
        duration_ms: 0,
        position_ms: 0,
    };
    if let Ok(mut lk) = current_state.lock() {
        *lk = Some(InternalConnectState { event: initial_ev.clone(), anchor: None });
    }
    let _ = app.emit("spotify-connect-state", initial_ev);

    let mut event_rx = player.get_player_event_channel();
    let session_clone = session.clone();
    let app_clone = app.clone();
    let cur_state_clone = current_state.clone();

    let event_loop = async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                PlayerEvent::Playing { track_id, position_ms, .. } => {
                    let mut title = None;
                    let mut artist = None;
                    let mut album = None;
                    let mut duration_ms = 0u64;

                    if let Ok(meta) = SpTrack::get(&session_clone, &track_id).await {
                        title = Some(meta.name);
                        duration_ms = meta.duration as u64;
                        album = Some(meta.album.name);
                        artist = Some(
                            meta.artists
                                .iter()
                                .map(|a| a.name.clone())
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                    }

                    let ev = SpotifyConnectStateEvent {
                        active: true,
                        is_playing: true,
                        title,
                        artist,
                        album,
                        cover_url: None,
                        duration_ms,
                        position_ms: position_ms as u64,
                    };
                    if let Ok(mut lk) = cur_state_clone.lock() {
                        *lk = Some(InternalConnectState {
                            event: ev.clone(),
                            anchor: Some(std::time::Instant::now()),
                        });
                    }
                    let _ = app_clone.emit("spotify-connect-state", ev);
                }
                PlayerEvent::Paused { position_ms, .. } => {
                    let ev = SpotifyConnectStateEvent {
                        active: true,
                        is_playing: false,
                        title: None,
                        artist: None,
                        album: None,
                        cover_url: None,
                        duration_ms: 0,
                        position_ms: position_ms as u64,
                    };
                    if let Ok(mut lk) = cur_state_clone.lock() {
                        *lk = Some(InternalConnectState { event: ev.clone(), anchor: None });
                    }
                    let _ = app_clone.emit("spotify-connect-state", ev);
                }
                PlayerEvent::Stopped { .. } => {
                    let ev = SpotifyConnectStateEvent {
                        active: true,
                        is_playing: false,
                        title: None,
                        artist: None,
                        album: None,
                        cover_url: None,
                        duration_ms: 0,
                        position_ms: 0,
                    };
                    if let Ok(mut lk) = cur_state_clone.lock() {
                        *lk = None;
                    }
                    let _ = app_clone.emit("spotify-connect-state", ev);
                }
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = spirc_task => {},
        _ = event_loop => {},
        _ = &mut shutdown_rx => {},
    }

    active.store(false, Ordering::Relaxed);
    if let Ok(mut lk) = current_state.lock() {
        *lk = None;
    }
    let _ = app.emit(
        "spotify-connect-state",
        SpotifyConnectStateEvent {
            active: false,
            is_playing: false,
            title: None,
            artist: None,
            album: None,
            cover_url: None,
            duration_ms: 0,
            position_ms: 0,
        },
    );

    Ok(())
}

async fn execute_scrobble(db: &crate::db::Db, app: &AppHandle, track: &TrackItem) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    let mut spotify_id = None;

    if let Ok(token) = crate::commands::get_spotify_token_from_db(db).await {
        let clean_artist = track.artists.split(',').next().unwrap_or(&track.artists).trim();
        let query = format!("artist:{clean_artist} track:{}", track.title);
        let client = crate::http::client();
        if let Ok(res) = client
            .get("https://api.spotify.com/v1/search")
            .header("Authorization", format!("Bearer {token}"))
            .query(&[("q", query.as_str()), ("type", "track"), ("limit", "1")])
            .send()
            .await
        {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    spotify_id = json["tracks"]["items"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|item| item["id"].as_str())
                        .map(|s| s.to_string());
                }
            }
        }
    }

    let _ = db.record_spotify_scrobble(
        &track.title,
        &track.artists,
        track.album.as_deref(),
        spotify_id.as_deref(),
    );

    let _ = app.emit(
        "spotify-scrobbled",
        SpotifyScrobbleEvent {
            title: track.title.clone(),
            artist: track.artists.clone(),
            album: track.album.clone(),
            spotify_id,
            scrobbled_at: now,
        },
    );
}
