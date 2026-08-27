//! Last.fm scrobbling. A second consumer of the same track/duration/position stream that feeds
//! `discord.rs` — but simpler: Last.fm doesn't care about live position, only two moments per
//! track. `track.updateNowPlaying` when a track starts, and one `track.scrobble` once the track
//! has played half its length or 4 minutes, whichever comes first (Last.fm's official rule;
//! tracks under 30s never scrobble).
//!
//! Everything is best-effort (fail-soft, same as Discord/media): a failed scrobble is
//! a `debug!` line, never a user-facing error.
//!
//! Auth is the desktop flow: `auth.getToken` → open the user's browser on the authorize page →
//! poll `auth.getSession` until they approve. Session keys never expire, so the key + username in
//! settings (`lastfm_session_key` / `lastfm_username`) are the whole persistent state.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use innertube::SongItem;
use md5::{Digest, Md5};
use tauri::Emitter;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::state::AppState;

/// Last.fm API credentials baked in at compile time from `lastfm.keys` / `.env`.
const COMPILED_API_KEY: &str = match option_env!("NOCTURNE_LASTFM_API_KEY") {
    Some(v) => v,
    None => match option_env!("LIMUSIC_LASTFM_API_KEY") {
        Some(v) => v,
        None => match option_env!("LASTFM_API_KEY") {
            Some(v) => v,
            None => "",
        },
    },
};

const COMPILED_API_SECRET: &str = match option_env!("NOCTURNE_LASTFM_API_SECRET") {
    Some(v) => v,
    None => match option_env!("LIMUSIC_LASTFM_API_SECRET") {
        Some(v) => v,
        None => match option_env!("LASTFM_API_SECRET") {
            Some(v) => v,
            None => "",
        },
    },
};

const API_ROOT: &str = "https://ws.audioscrobbler.com/2.0/";
const AUTH_URL: &str = "https://www.last.fm/api/auth/";

/// How long the user gets to approve the app in their browser: 60 polls × 5s = 5 minutes.
const AUTH_POLL_EVERY: Duration = Duration::from_secs(5);
const AUTH_POLL_TRIES: u32 = 60;

/// Last.fm error 14: "token has not been authorized" — the user hasn't clicked Allow yet.
const ERR_TOKEN_PENDING: i64 = 14;
/// Last.fm error 16: service temporarily unavailable — retryable, same as pending.
const ERR_TEMP_UNAVAILABLE: i64 = 16;

pub fn resolve_api_key(db: &crate::db::Db) -> String {
    if let Some(k) = db.get_setting("lastfm_api_key").filter(|s| !s.trim().is_empty()) {
        return k.trim().to_string();
    }
    COMPILED_API_KEY.to_string()
}

pub fn resolve_api_secret(db: &crate::db::Db) -> String {
    if let Some(s) = db.get_setting("lastfm_api_secret").filter(|s| !s.trim().is_empty()) {
        return s.trim().to_string();
    }
    COMPILED_API_SECRET.to_string()
}

enum Msg {
    Track(Box<Track>),
    Duration(f64),
    Position(f64),
    Session(Option<String>),
    Keys { key: String, secret: String },
}

struct Track {
    title: String,
    artists: String,
    album: Option<String>,
}

/// App-side handle to the scrobbler task.
pub struct LastfmHandle {
    tx: UnboundedSender<Msg>,
    auth_gen: AtomicU64,
}

impl LastfmHandle {
    pub fn set_track(&self, item: &SongItem) {
        let _ = self.tx.send(Msg::Track(Box::new(Track {
            title: item.title.clone(),
            artists: item.artists.clone(),
            album: item.album.clone(),
        })));
    }

    pub fn set_duration(&self, secs: f64) {
        let _ = self.tx.send(Msg::Duration(secs));
    }

    pub fn set_position(&self, pos: f64) {
        let _ = self.tx.send(Msg::Position(pos));
    }

    pub fn set_session(&self, key: Option<String>) {
        let _ = self.tx.send(Msg::Session(key));
    }

    pub fn set_keys(&self, key: String, secret: String) {
        let _ = self.tx.send(Msg::Keys { key, secret });
    }

    fn bump_gen(&self) -> u64 {
        self.auth_gen.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn gen(&self) -> u64 {
        self.auth_gen.load(Ordering::SeqCst)
    }
}

/// Spawn the scrobbler task.
pub fn spawn(db: &crate::db::Db) -> LastfmHandle {
    let session_key = db.get_setting("lastfm_session_key").filter(|s| !s.is_empty());
    let api_key = resolve_api_key(db);
    let api_secret = resolve_api_secret(db);
    let (tx, mut rx) = unbounded_channel::<Msg>();
    tauri::async_runtime::spawn(async move {
        let mut s = Scrobbler::new(session_key, api_key, api_secret);
        while let Some(msg) = rx.recv().await {
            s.apply(msg).await;
        }
    });
    LastfmHandle { tx, auth_gen: AtomicU64::new(0) }
}

struct Scrobbler {
    session: Option<String>,
    api_key: String,
    api_secret: String,
    track: Option<Track>,
    started_at: u64,
    duration: f64,
    scrobbled: bool,
}

impl Scrobbler {
    fn new(session: Option<String>, api_key: String, api_secret: String) -> Self {
        Scrobbler {
            session,
            api_key,
            api_secret,
            track: None,
            started_at: 0,
            duration: 0.0,
            scrobbled: false,
        }
    }

    async fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Track(t) => {
                self.track = Some(*t);
                self.started_at = now_secs();
                self.duration = 0.0;
                self.scrobbled = false;
                self.now_playing().await;
            }
            Msg::Duration(secs) => self.duration = secs,
            Msg::Position(pos) => {
                if !self.scrobbled && crosses_threshold(pos, self.duration) {
                    self.scrobbled = true;
                    self.scrobble().await;
                }
            }
            Msg::Session(key) => self.session = key,
            Msg::Keys { key, secret } => {
                self.api_key = key;
                self.api_secret = secret;
            }
        }
    }

    async fn now_playing(&self) {
        let (Some(sk), Some(t)) = (&self.session, &self.track) else { return };
        if !scrobbleable(t) || self.api_key.is_empty() || self.api_secret.is_empty() {
            return;
        }
        let mut params = vec![
            ("artist".to_string(), t.artists.clone()),
            ("track".to_string(), t.title.clone()),
            ("sk".to_string(), sk.clone()),
        ];
        if let Some(album) = t.album.as_ref().filter(|a| !a.is_empty()) {
            params.push(("album".to_string(), album.clone()));
        }
        match call("track.updateNowPlaying", params, true, &self.api_key, &self.api_secret).await {
            Ok(_) => tracing::debug!(track = %t.title, "last.fm now playing sent"),
            Err(e) => tracing::debug!(error = %e.message, "last.fm now playing failed"),
        }
    }

    async fn scrobble(&self) {
        let (Some(sk), Some(t)) = (&self.session, &self.track) else { return };
        if !scrobbleable(t) || self.api_key.is_empty() || self.api_secret.is_empty() {
            return;
        }
        let mut params = vec![
            ("artist".to_string(), t.artists.clone()),
            ("track".to_string(), t.title.clone()),
            ("timestamp".to_string(), self.started_at.to_string()),
            ("sk".to_string(), sk.clone()),
        ];
        if let Some(album) = t.album.as_ref().filter(|a| !a.is_empty()) {
            params.push(("album".to_string(), album.clone()));
        }
        if self.duration > 0.0 {
            params.push(("duration".to_string(), (self.duration as i64).to_string()));
        }
        match call("track.scrobble", params, true, &self.api_key, &self.api_secret).await {
            Ok(_) => tracing::info!(track = %t.title, "scrobbled to last.fm"),
            Err(e) => tracing::warn!(error = %e.message, "last.fm scrobble failed"),
        }
    }
}

fn scrobbleable(t: &Track) -> bool {
    !t.title.trim().is_empty()
        && !t.artists.trim().is_empty()
        && t.artists != crate::local::UNKNOWN_ARTIST
}

fn crosses_threshold(pos: f64, duration: f64) -> bool {
    if duration > 0.0 && duration < 30.0 {
        return false;
    }
    let half = if duration > 0.0 { duration / 2.0 } else { f64::INFINITY };
    pos >= half.min(240.0)
}

// --- API plumbing ---

struct ApiError {
    code: Option<i64>,
    message: String,
}

impl ApiError {
    fn transport(e: impl std::fmt::Display) -> Self {
        ApiError { code: None, message: e.to_string() }
    }
    fn retryable(&self) -> bool {
        matches!(self.code, Some(ERR_TOKEN_PENDING) | Some(ERR_TEMP_UNAVAILABLE) | None)
    }
}

async fn call(
    method: &str,
    mut params: Vec<(String, String)>,
    post: bool,
    api_key: &str,
    api_secret: &str,
) -> Result<serde_json::Value, ApiError> {
    params.push(("api_key".to_string(), api_key.to_string()));
    params.push(("method".to_string(), method.to_string()));
    params.push(("api_sig".to_string(), sign(&params, api_secret)));
    params.push(("format".to_string(), "json".to_string()));

    let http = crate::http::client();
    let req =
        if post { http.post(API_ROOT).form(&params) } else { http.get(API_ROOT).query(&params) };
    let resp = req.timeout(Duration::from_secs(15)).send().await.map_err(ApiError::transport)?;
    let body: serde_json::Value = resp.json().await.map_err(ApiError::transport)?;
    if let Some(code) = body.get("error").and_then(|v| v.as_i64()) {
        let message = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown last.fm error")
            .to_string();
        return Err(ApiError { code: Some(code), message });
    }
    Ok(body)
}

pub fn sign(params: &[(String, String)], secret: &str) -> String {
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut s = String::new();
    for (k, v) in sorted {
        s.push_str(k);
        s.push_str(v);
    }
    s.push_str(secret);
    format!("{:x}", Md5::digest(s.as_bytes()))
}

// --- auth flow (connect / disconnect / status) ---

fn emit_state(
    app: &tauri::AppHandle,
    connected: bool,
    username: Option<&str>,
    error: Option<&str>,
) {
    let _ = app.emit(
        "lastfm-state",
        serde_json::json!({ "connected": connected, "username": username, "error": error }),
    );
}

pub async fn connect(state: Arc<AppState>) -> Result<(), String> {
    let api_key = resolve_api_key(&state.db);
    let api_secret = resolve_api_secret(&state.db);
    if api_key.is_empty() || api_secret.is_empty() {
        return Err("Last.fm API key or secret not found. Add them in lastfm.keys or Settings > Last.fm (https://www.last.fm/api/account/create).".into());
    }

    let gen = state.lastfm.bump_gen();
    let token = call("auth.getToken", vec![], false, &api_key, &api_secret)
        .await
        .map_err(|e| format!("Last.fm: {}", e.message))?
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or("Last.fm returned no token")?
        .to_string();

    open_browser(&format!("{AUTH_URL}?api_key={api_key}&token={token}"))?;

    let key_clone = api_key.clone();
    let secret_clone = api_secret.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..AUTH_POLL_TRIES {
            tokio::time::sleep(AUTH_POLL_EVERY).await;
            if state.lastfm.gen() != gen {
                return;
            }
            let params = vec![("token".to_string(), token.clone())];
            match call("auth.getSession", params, false, &key_clone, &secret_clone).await {
                Ok(body) => {
                    let name = body.pointer("/session/name").and_then(|v| v.as_str());
                    let key = body.pointer("/session/key").and_then(|v| v.as_str());
                    let (Some(name), Some(key)) = (name, key) else {
                        emit_state(
                            &state.app,
                            false,
                            None,
                            Some("Last.fm sent a malformed session"),
                        );
                        return;
                    };
                    state.db.set_setting("lastfm_session_key", key);
                    state.db.set_setting("lastfm_username", name);
                    state.lastfm.set_session(Some(key.to_string()));
                    state.lastfm.set_keys(key_clone.clone(), secret_clone.clone());
                    tracing::info!(user = name, "last.fm connected");
                    emit_state(&state.app, true, Some(name), None);
                    return;
                }
                Err(e) if e.retryable() => continue,
                Err(e) => {
                    emit_state(&state.app, false, None, Some(&format!("Last.fm: {}", e.message)));
                    return;
                }
            }
        }
        emit_state(&state.app, false, None, Some("Last.fm authorization timed out — try again"));
    });
    Ok(())
}

pub fn disconnect(state: &AppState) {
    state.lastfm.bump_gen();
    state.db.set_setting("lastfm_session_key", "");
    state.db.set_setting("lastfm_username", "");
    state.lastfm.set_session(None);
    emit_state(&state.app, false, None, None);
}

pub fn status(state: &AppState) -> serde_json::Value {
    let key = state.db.get_setting("lastfm_session_key").filter(|s| !s.is_empty());
    let username = state.db.get_setting("lastfm_username").filter(|s| !s.is_empty());
    let has_keys =
        !resolve_api_key(&state.db).is_empty() && !resolve_api_secret(&state.db).is_empty();
    serde_json::json!({
        "connected": key.is_some(),
        "username": username,
        "configured": has_keys
    })
}

pub(crate) fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let cmd = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        unappimage(&mut cmd);
        cmd.spawn()
    };
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let cmd = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd").raw_arg(format!("/C start \"\" \"{url}\"")).spawn()
    };
    cmd.map(|_| ()).map_err(|e| format!("Couldn't open the browser: {e}"))
}

#[cfg(target_os = "linux")]
fn unappimage(cmd: &mut std::process::Command) {
    let Some(appdir) = std::env::var("APPDIR").ok().filter(|d| !d.is_empty()) else { return };
    for (key, value) in std::env::vars_os() {
        let (Some(key), Some(value)) = (key.to_str(), value.to_str()) else { continue };
        if !value.contains(&appdir) {
            continue;
        }
        match strip_appdir(value, &appdir) {
            Some(kept) => cmd.env(key, kept),
            None => cmd.env_remove(key),
        };
    }
}

#[cfg(target_os = "linux")]
fn strip_appdir(value: &str, appdir: &str) -> Option<String> {
    let kept: Vec<&str> =
        value.split(':').filter(|p| !p.starts_with(appdir) && !p.is_empty()).collect();
    (!kept.is_empty()).then(|| kept.join(":"))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_sig_is_sorted_concat_md5() {
        let params = vec![
            ("method".to_string(), "auth.getSession".to_string()),
            ("api_key".to_string(), "abc".to_string()),
            ("token".to_string(), "xyz".to_string()),
        ];
        let secret = "secret123";
        let expected = format!(
            "{:x}",
            Md5::digest(format!("api_keyabcmethodauth.getSessiontokenxyz{secret}").as_bytes())
        );
        assert_eq!(sign(&params, secret), expected);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn strip_appdir_keeps_host_paths_and_drops_the_var_when_nothing_is_left() {
        let dir = "/tmp/.mount_limusiNnkaBP";
        assert_eq!(
            strip_appdir(&format!("{dir}/usr/lib/:{dir}/usr/lib64/:/opt/mine/lib"), dir),
            Some("/opt/mine/lib".to_string())
        );
        assert_eq!(strip_appdir(&format!("{dir}/usr/lib:{dir}/usr/lib64"), dir), None);
        assert_eq!(strip_appdir(dir, dir), None);
        assert_eq!(
            strip_appdir(&format!("{dir}/usr/share:/usr/share:/usr/local/share"), dir),
            Some("/usr/share:/usr/local/share".to_string())
        );
    }

    #[test]
    fn scrobble_threshold_follows_lastfm_rules() {
        assert!(!crosses_threshold(89.0, 180.0));
        assert!(crosses_threshold(90.0, 180.0));
        assert!(!crosses_threshold(239.0, 1200.0));
        assert!(crosses_threshold(240.0, 1200.0));
        assert!(!crosses_threshold(29.0, 20.0));
        assert!(!crosses_threshold(120.0, 0.0));
        assert!(crosses_threshold(240.0, 0.0));
    }
}
