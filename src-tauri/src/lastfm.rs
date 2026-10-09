//! Last.fm scrobbling. A second consumer of the same track/duration/position stream that feeds
//! `discord.rs` — but simpler: Last.fm doesn't care about live position, only two moments per
//! track. `track.updateNowPlaying` when a track starts, and one `track.scrobble` once the track
//! has played half its length or 4 minutes, whichever comes first (Last.fm's official rule;
//! tracks under 30s never scrobble). Both numbers are the user's to change in the Scrobbling
//! settings tab, along with what gets sent: see [`ScrobbleConfig`] and [`resolve`].
//!
//! Everything is best-effort: a failed scrobble is a `debug!` line, never a user-facing error.
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

/// How scrobbling behaves, from the Scrobbling settings tab. One JSON blob in the
/// `lastfm_config` setting.
#[derive(Clone, serde::Deserialize)]
#[serde(default)]
pub struct ScrobbleConfig {
    /// Scrobble at all. Off pauses scrobbling without signing out of Last.fm.
    pub enabled: bool,
    /// Send `track.updateNowPlaying` when a track starts.
    pub now_playing: bool,
    /// Scrobble once this share of the track has played, 1 to 100. Last.fm's rule is 50.
    pub percent: u32,
    /// ...or once this many minutes have played, whichever comes first. 0 turns the cap off.
    pub minutes: u32,
    /// Split video titles like "Artist - Title" into artist and song.
    pub split_video_titles: bool,
    pub rules: Vec<Rule>,
    pub edits: Vec<Edit>,
    pub primary_artist: bool,
    pub primary_strict: bool,
}

impl Default for ScrobbleConfig {
    fn default() -> Self {
        ScrobbleConfig {
            enabled: true,
            now_playing: true,
            percent: 50,
            minutes: 4,
            split_video_titles: false,
            rules: Vec::new(),
            edits: Vec::new(),
            primary_artist: false,
            primary_strict: false,
        }
    }
}

impl ScrobbleConfig {
    /// Anything unparseable is the default, never an error: a corrupt blob must not stop scrobbling.
    pub fn parse(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }

    /// The stored config, with the two settings that live in rows of their own.
    pub fn load(db: &crate::db::Db) -> Self {
        let mut cfg = Self::parse(&db.get_setting("lastfm_config").unwrap_or_default());
        cfg.primary_artist = db.get_setting("lastfm_primary_artist").as_deref() == Some("true");
        cfg.primary_strict = db.get_setting("lastfm_primary_strict").as_deref() == Some("true");
        cfg
    }
}

/// Find and replace on one field, applied to every track in list order.
#[derive(Clone, serde::Deserialize)]
#[serde(default)]
pub struct Rule {
    pub enabled: bool,
    /// `title` | `artist` | `album`.
    pub field: String,
    pub find: String,
    pub replace: String,
}

impl Default for Rule {
    fn default() -> Self {
        Rule { enabled: true, field: "title".into(), find: String::new(), replace: String::new() }
    }
}

/// A fix for one track, applied every time it plays.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct Edit {
    pub key: String,
    pub from_artist: String,
    pub from_title: String,
    pub from_album: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub skip: bool,
}

impl Edit {
    fn matches(&self, t: &Track) -> bool {
        if !self.key.is_empty() {
            return self.key == t.video_id;
        }
        let same = |want: &str, have: &str| {
            want.trim().is_empty() || want.trim().to_lowercase() == have.trim().to_lowercase()
        };
        let any = [&self.from_artist, &self.from_title, &self.from_album]
            .iter()
            .any(|f| !f.trim().is_empty());
        any && same(&self.from_artist, &t.artists)
            && same(&self.from_title, &t.title)
            && same(&self.from_album, t.album.as_deref().unwrap_or(""))
    }
}

enum Msg {
    Track(Box<Track>),
    Album { video_id: String, album: String },
    Duration(f64),
    Position(f64),
    Session(Option<String>),
    Config(Box<ScrobbleConfig>),
    Keys { key: String, secret: String },
}

/// The track as YouTube describes it.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub artists: String,
    pub album: Option<String>,
    pub is_video: bool,
}

impl From<&SongItem> for Track {
    fn from(item: &SongItem) -> Self {
        Track {
            video_id: item.video_id.clone(),
            title: item.title.clone(),
            artists: item.artists.clone(),
            album: item.album.clone(),
            is_video: item.is_video,
        }
    }
}

/// App-side handle to the scrobbler task.
pub struct LastfmHandle {
    tx: UnboundedSender<Msg>,
    auth_gen: AtomicU64,
}

impl LastfmHandle {
    pub fn set_track(&self, item: &SongItem) {
        let _ = self.tx.send(Msg::Track(Box::new(Track::from(item))));
    }

    /// Update album learned mid-track without restarting scrobble clock.
    pub fn set_album(&self, video_id: &str, album: &str) {
        let _ = self.tx.send(Msg::Album { video_id: video_id.to_owned(), album: album.to_owned() });
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

    pub fn set_config(&self, cfg: ScrobbleConfig) {
        let _ = self.tx.send(Msg::Config(Box::new(cfg)));
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
    let cfg = ScrobbleConfig::load(db);
    let api_key = resolve_api_key(db);
    let api_secret = resolve_api_secret(db);
    let (tx, mut rx) = unbounded_channel::<Msg>();
    tauri::async_runtime::spawn(async move {
        let mut s = Scrobbler::new(session_key, cfg, api_key, api_secret);
        while let Some(msg) = rx.recv().await {
            s.apply(msg).await;
        }
    });
    LastfmHandle { tx, auth_gen: AtomicU64::new(0) }
}

struct Scrobbler {
    session: Option<String>,
    cfg: ScrobbleConfig,
    api_key: String,
    api_secret: String,
    track: Option<Track>,
    started_at: u64,
    duration: f64,
    scrobbled: bool,
}

impl Scrobbler {
    fn new(
        session: Option<String>,
        cfg: ScrobbleConfig,
        api_key: String,
        api_secret: String,
    ) -> Self {
        Scrobbler {
            session,
            cfg,
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
            Msg::Album { video_id, album } => {
                let Some(t) = self.track.as_mut().filter(|t| t.video_id == video_id) else {
                    return;
                };
                t.album = Some(album);
                self.now_playing().await;
            }
            Msg::Duration(secs) => self.duration = secs,
            Msg::Position(pos) => {
                if !self.scrobbled && crosses_threshold(pos, self.duration, &self.cfg) {
                    self.scrobbled = true;
                    self.scrobble().await;
                }
            }
            Msg::Session(key) => self.session = key,
            Msg::Config(cfg) => self.cfg = *cfg,
            Msg::Keys { key, secret } => {
                self.api_key = key;
                self.api_secret = secret;
            }
        }
    }

    fn outgoing(&self) -> Option<(&str, Resolved)> {
        let (Some(sk), Some(t)) = (&self.session, &self.track) else { return None };
        if !self.cfg.enabled || self.api_key.is_empty() || self.api_secret.is_empty() {
            return None;
        }
        let r = resolve(t, &self.cfg);
        r.skip.is_none().then_some((sk.as_str(), r))
    }

    async fn now_playing(&self) {
        if !self.cfg.now_playing {
            return;
        }
        let Some((sk, r)) = self.outgoing() else { return };
        let mut params = vec![
            ("artist".to_string(), r.artist),
            ("track".to_string(), r.title.clone()),
            ("sk".to_string(), sk.to_string()),
        ];
        if !r.album.is_empty() {
            params.push(("album".to_string(), r.album));
        }
        match call("track.updateNowPlaying", params, true, &self.api_key, &self.api_secret).await {
            Ok(_) => tracing::debug!(track = %r.title, "last.fm now playing sent"),
            Err(e) => tracing::debug!(error = %e.message, "last.fm now playing failed"),
        }
    }

    async fn scrobble(&self) {
        let Some((sk, r)) = self.outgoing() else { return };
        let mut params = vec![
            ("artist".to_string(), r.artist),
            ("track".to_string(), r.title.clone()),
            ("timestamp".to_string(), self.started_at.to_string()),
            ("sk".to_string(), sk.to_string()),
        ];
        if !r.album.is_empty() {
            params.push(("album".to_string(), r.album));
        }
        if self.duration > 0.0 {
            params.push(("duration".to_string(), (self.duration as i64).to_string()));
        }
        match call("track.scrobble", params, true, &self.api_key, &self.api_secret).await {
            Ok(_) => tracing::info!(track = %r.title, "scrobbled to last.fm"),
            Err(e) => tracing::warn!(error = %e.message, "last.fm scrobble failed"),
        }
    }
}

/// What a track scrobbles as, and why.
#[derive(Clone, serde::Serialize)]
pub struct Resolved {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub skip: Option<&'static str>,
    pub edit: Option<usize>,
    pub split: bool,
    pub rules: Vec<usize>,
    pub errors: Vec<(usize, String)>,
}

impl Resolved {
    fn field(&mut self, name: &str) -> Option<&mut String> {
        match name {
            "artist" => Some(&mut self.artist),
            "title" => Some(&mut self.title),
            "album" => Some(&mut self.album),
            _ => None,
        }
    }
}

/// Apply the user's scrobbling settings to a track.
pub fn resolve(t: &Track, cfg: &ScrobbleConfig) -> Resolved {
    let mut r = Resolved {
        artist: t.artists.clone(),
        title: t.title.clone(),
        album: t.album.clone().unwrap_or_default(),
        skip: None,
        edit: None,
        split: false,
        rules: Vec::new(),
        errors: Vec::new(),
    };
    let compiled: Vec<Option<regex::Regex>> = cfg
        .rules
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            if !rule.enabled || rule.find.is_empty() {
                return None;
            }
            regex::RegexBuilder::new(&rule.find)
                .case_insensitive(true)
                .build()
                .map_err(|e| r.errors.push((i, e.to_string())))
                .ok()
        })
        .collect();

    if let Some((i, e)) = cfg.edits.iter().enumerate().find(|(_, e)| e.matches(t)) {
        r.edit = Some(i);
        if e.skip {
            r.skip = Some("edit");
            return r;
        }
        for (have, want) in
            [(&mut r.artist, &e.artist), (&mut r.title, &e.title), (&mut r.album, &e.album)]
        {
            if !want.trim().is_empty() {
                *have = want.trim().to_owned();
            }
        }
        return finish(r);
    }

    if cfg.split_video_titles && t.is_video {
        if let Some((artist, title)) = split_title(&t.title) {
            r.artist = artist;
            r.title = title;
            r.split = true;
        }
    }
    if cfg.primary_artist {
        r.artist = primary_artist(&r.artist, cfg.primary_strict);
    }
    for (i, (rule, re)) in cfg.rules.iter().zip(&compiled).enumerate() {
        if let Some(re) = re {
            if apply_rule(re, &rule.field, &rule.replace, &mut r) {
                r.rules.push(i);
            }
        }
    }
    finish(r)
}

fn finish(mut r: Resolved) -> Resolved {
    for f in [&mut r.artist, &mut r.title, &mut r.album] {
        *f = f.trim().to_owned();
    }
    if r.title.is_empty() || r.artist.is_empty() || r.artist == crate::local::UNKNOWN_ARTIST {
        r.skip = Some("incomplete");
    }
    r
}

const GROUPS: [&str; 3] = ["artist", "title", "album"];

fn apply_rule(re: &regex::Regex, field: &str, replace: &str, r: &mut Resolved) -> bool {
    let Some(value) = r.field(field) else { return false };
    let named: Vec<&str> = re.capture_names().flatten().filter(|n| GROUPS.contains(n)).collect();
    if named.is_empty() {
        let out = re.replace_all(value, replace);
        if out == value.as_str() {
            return false;
        }
        *value = out.into_owned();
        return true;
    }
    let Some(caps) = re.captures(value) else { return false };
    let found: Vec<(&str, String)> = named
        .iter()
        .filter_map(|n| caps.name(n).map(|m| (*n, m.as_str().trim().to_owned())))
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let mut changed = false;
    for (name, v) in found {
        let f = r.field(name).expect("GROUPS are all fields");
        changed |= *f != v;
        *f = v;
    }
    changed
}

fn split_title(title: &str) -> Option<(String, String)> {
    let (at, sep) =
        [" - ", " – ", " — "].iter().filter_map(|s| title.find(s).map(|i| (i, *s))).min()?;
    let (artist, song) = (title[..at].trim(), title[at + sep.len()..].trim());
    (!artist.is_empty() && !song.is_empty()).then(|| (artist.to_owned(), song.to_owned()))
}

fn primary_artist(artists: &str, strict: bool) -> String {
    let end = artists.find(|c: char| c == ',' || (strict && c == '&')).unwrap_or(artists.len());
    let first = artists[..end].trim();
    if first.is_empty() {
        artists.to_owned()
    } else {
        first.to_owned()
    }
}

fn scrobble_at(duration: f64, cfg: &ScrobbleConfig) -> Option<f64> {
    if duration > 0.0 && duration < 30.0 {
        return None;
    }
    let share = if duration > 0.0 {
        duration * f64::from(cfg.percent.clamp(1, 100)) / 100.0
    } else {
        f64::INFINITY
    };
    let cap = if cfg.minutes > 0 { f64::from(cfg.minutes) * 60.0 } else { f64::INFINITY };
    Some(share.min(cap)).filter(|at| at.is_finite())
}

fn crosses_threshold(pos: f64, duration: f64, cfg: &ScrobbleConfig) -> bool {
    scrobble_at(duration, cfg).is_some_and(|at| pos >= at)
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

fn sign(params: &[(String, String)], api_secret: &str) -> String {
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut s = String::new();
    for (k, v) in sorted {
        s.push_str(k);
        s.push_str(v);
    }
    s.push_str(api_secret);
    format!("{:x}", Md5::digest(s.as_bytes()))
}

// --- Auth flow ---

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
        return Err(
            "Last.fm isn't configured in this build — paste an API key and secret into Settings."
                .into(),
        );
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

    let api_key_poll = api_key.clone();
    let api_secret_poll = api_secret.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..AUTH_POLL_TRIES {
            tokio::time::sleep(AUTH_POLL_EVERY).await;
            if state.lastfm.gen() != gen {
                return;
            }
            let params = vec![("token".to_string(), token.clone())];
            match call("auth.getSession", params, false, &api_key_poll, &api_secret_poll).await {
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
    serde_json::json!({ "connected": key.is_some(), "username": username })
}

pub async fn profile(state: &AppState) -> Option<Profile> {
    let name = state.db.get_setting("lastfm_username").filter(|s| !s.is_empty())?;
    let api_key = resolve_api_key(&state.db);
    let api_secret = resolve_api_secret(&state.db);
    if api_key.is_empty() || api_secret.is_empty() {
        return None;
    }
    let body = call("user.getInfo", vec![("user".to_string(), name)], false, &api_key, &api_secret)
        .await
        .ok()?;
    parse_profile(&body)
}

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Profile {
    pub image: Option<String>,
    pub url: Option<String>,
    pub scrobbles: u64,
    pub artists: u64,
    pub tracks: u64,
    pub since: u64,
}

fn parse_profile(body: &serde_json::Value) -> Option<Profile> {
    let u = body.get("user")?;
    let num = |v: Option<&serde_json::Value>| {
        v.and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0)
    };
    let images = u.get("image").and_then(|v| v.as_array());
    let image = ["large", "extralarge", "medium"].iter().find_map(|size| {
        images?
            .iter()
            .find(|i| i.get("size").and_then(|s| s.as_str()) == Some(size))?
            .get("#text")?
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    });
    Some(Profile {
        image,
        url: u.get("url").and_then(|v| v.as_str()).map(str::to_owned),
        scrobbles: num(u.get("playcount")),
        artists: num(u.get("artist_count")),
        tracks: num(u.get("track_count")),
        since: num(u.pointer("/registered/unixtime")),
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
        let cfg = ScrobbleConfig::default();
        assert!(!crosses_threshold(89.0, 180.0, &cfg));
        assert!(crosses_threshold(90.0, 180.0, &cfg));
        assert!(!crosses_threshold(239.0, 1200.0, &cfg));
        assert!(crosses_threshold(240.0, 1200.0, &cfg));
        assert!(!crosses_threshold(29.0, 20.0, &cfg));
        assert!(!crosses_threshold(120.0, 0.0, &cfg));
        assert!(crosses_threshold(240.0, 0.0, &cfg));
    }

    #[test]
    fn scrobble_threshold_follows_the_users_numbers() {
        let cfg = ScrobbleConfig { percent: 90, minutes: 0, ..Default::default() };
        assert!(!crosses_threshold(1000.0, 1200.0, &cfg));
        assert!(crosses_threshold(1080.0, 1200.0, &cfg));
        assert_eq!(scrobble_at(0.0, &cfg), None);
        let cfg = ScrobbleConfig { percent: 100, minutes: 2, ..Default::default() };
        assert_eq!(scrobble_at(200.0, &cfg), Some(120.0));
        let cfg = ScrobbleConfig { percent: 0, minutes: 0, ..Default::default() };
        assert_eq!(scrobble_at(200.0, &cfg), Some(2.0));
    }

    fn test_track(title: &str, artists: &str, is_video: bool) -> Track {
        Track {
            video_id: "T1Vkc5w79-M".into(),
            title: title.into(),
            artists: artists.into(),
            album: None,
            is_video,
        }
    }

    fn test_rule(field: &str, find: &str, replace: &str) -> Rule {
        Rule {
            field: field.into(),
            find: find.into(),
            replace: replace.into(),
            ..Default::default()
        }
    }

    #[test]
    fn default_config_changes_nothing() {
        let t = Track { album: Some("Album".into()), ..test_track("Song - Live", "A, B", true) };
        let r = resolve(&t, &ScrobbleConfig::default());
        assert_eq!(
            (r.artist.as_str(), r.title.as_str(), r.album.as_str()),
            ("A, B", "Song - Live", "Album")
        );
        assert!(r.skip.is_none() && r.edit.is_none() && !r.split && r.rules.is_empty());
        assert!(ScrobbleConfig::parse("{oops").enabled);
    }

    #[test]
    fn video_titles_split_into_artist_and_song_when_asked() {
        let t = test_track(
            "Ruelle - Fire Meets Fate - Shadowhunters 3x10 Music (Mid Season Finale)",
            "alexias788",
            true,
        );
        let cfg = ScrobbleConfig { split_video_titles: true, ..Default::default() };
        let r = resolve(&t, &cfg);
        assert!(r.split);
        assert_eq!(r.artist, "Ruelle");
        assert_eq!(r.title, "Fire Meets Fate - Shadowhunters 3x10 Music (Mid Season Finale)");
        let r = resolve(&test_track("Song - Remastered 2011", "Band", false), &cfg);
        assert!(!r.split && r.artist == "Band");
        assert_eq!(
            split_title("Ruelle – War of Hearts"),
            Some(("Ruelle".into(), "War of Hearts".into()))
        );
        assert_eq!(split_title(" - Intro"), None);
        assert_eq!(split_title("Re-Run"), None);
    }

    #[test]
    fn rules_replace_in_order_and_ignore_case() {
        let cfg = ScrobbleConfig {
            rules: vec![
                test_rule("title", r"\s*[(\[](?:official\s+)?(?:music\s+)?video[)\]]", ""),
                test_rule("artist", "VEVO$", ""),
                test_rule("album", "x", "y"),
            ],
            ..Default::default()
        };
        let r =
            resolve(&test_track("Higher Ground [OFFICIAL Music Video]", "NovaSkyVEVO", true), &cfg);
        assert_eq!((r.title.as_str(), r.artist.as_str()), ("Higher Ground", "NovaSky"));
        assert_eq!(r.rules, vec![0, 1]);
        let cfg = ScrobbleConfig {
            rules: vec![test_rule("artist", "(?-i)vevo$", "")],
            ..Default::default()
        };
        assert!(resolve(&test_track("x", "NovaSkyVEVO", false), &cfg).rules.is_empty());
    }

    #[test]
    fn named_groups_move_text_between_fields() {
        let cfg = ScrobbleConfig {
            rules: vec![test_rule("title", r"^(?<artist>.+?) - (?<title>.+)$", "ignored")],
            ..Default::default()
        };
        let r = resolve(&test_track("Ruelle - Fire Meets Fate", "alexias788", false), &cfg);
        assert_eq!((r.artist.as_str(), r.title.as_str()), ("Ruelle", "Fire Meets Fate"));
        let r = resolve(&test_track("Fire Meets Fate", "Ruelle", false), &cfg);
        assert!(r.rules.is_empty() && r.title == "Fire Meets Fate");
    }

    #[test]
    fn a_broken_rule_is_reported_and_skipped() {
        let cfg = ScrobbleConfig {
            rules: vec![test_rule("title", "(unclosed", ""), test_rule("title", "Song", "Track")],
            ..Default::default()
        };
        let r = resolve(&test_track("Song", "A", false), &cfg);
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.errors[0].0, 0);
        assert_eq!(r.title, "Track");
        let cfg =
            ScrobbleConfig { rules: vec![test_rule("artist", ".*", "")], ..Default::default() };
        assert_eq!(resolve(&test_track("Song", "A", false), &cfg).skip, Some("incomplete"));
    }

    #[test]
    fn an_edit_is_final_and_can_skip() {
        let edit = Edit {
            key: "T1Vkc5w79-M".into(),
            artist: "Ruelle".into(),
            title: "Fire Meets Fate".into(),
            ..Default::default()
        };
        let cfg = ScrobbleConfig {
            edits: vec![edit.clone()],
            rules: vec![test_rule("artist", "Ruelle", "Nope")],
            split_video_titles: true,
            ..Default::default()
        };
        let t = Track {
            album: Some("Shadowhunters".into()),
            ..test_track("whatever", "alexias788", true)
        };
        let r = resolve(&t, &cfg);
        assert_eq!(r.edit, Some(0));
        assert_eq!(
            (r.artist.as_str(), r.title.as_str(), r.album.as_str()),
            ("Ruelle", "Fire Meets Fate", "Shadowhunters")
        );
        assert!(!r.split && r.rules.is_empty());
        let other = Track { video_id: "dQw4w9WgXcQ".into(), ..test_track("x", "y", false) };
        assert!(resolve(&other, &cfg).edit.is_none());
        let cfg = ScrobbleConfig { edits: vec![Edit { skip: true, ..edit }], ..Default::default() };
        assert_eq!(resolve(&t, &cfg).skip, Some("edit"));
    }

    #[test]
    fn keyless_edits_match_on_metadata() {
        let block = Edit { from_artist: "white noise co".into(), skip: true, ..Default::default() };
        let cfg = ScrobbleConfig { edits: vec![block], ..Default::default() };
        assert_eq!(
            resolve(&test_track("Rain 10h", "White Noise Co", false), &cfg).skip,
            Some("edit")
        );
        assert!(resolve(&test_track("Rain 10h", "Someone", false), &cfg).skip.is_none());
        let cfg = ScrobbleConfig {
            edits: vec![Edit { skip: true, ..Default::default() }],
            ..Default::default()
        };
        assert!(resolve(&test_track("a", "b", false), &cfg).skip.is_none());
    }

    #[test]
    fn untagged_local_files_never_scrobble() {
        let r = resolve(
            &test_track("01 - track", crate::local::UNKNOWN_ARTIST, false),
            &ScrobbleConfig::default(),
        );
        assert_eq!(r.skip, Some("incomplete"));
    }
}
