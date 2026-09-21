//! Discord Rich Presence via Discord's local IPC socket.
//!
//! Mirrors `media.rs`: the IPC client is blocking and stateful, so it gets an owner thread fed by a
//! channel. Everything here is best-effort — Discord not running, or quitting mid-song, is a
//! `debug!` line, never an error the user sees (context/16 fail-soft, same as the OS media widget).
//!
//! Presence is shown **only while actually playing**, unless the user turns on "show when paused".
//! Even then, a card needs playback to have started at least once this session: the queue restored on
//! launch is paused too, and a frozen "Listening to…" for a track nobody pressed play on is worse
//! than no card. A paused card also carries no timeline, because Discord has no paused state and its
//! progress bar would keep running.
//!
//! Timeline correctness hinges on three rules:
//! 1. A track change resets the thread's position to 0 — the app only ever starts tracks from the
//!    top, and trusting the previous track's stale position put every new song minutes in.
//! 2. Position messages carry the `Instant` they were *sent*, and the thread age-corrects — the
//!    event pump can lag seconds behind mpv (gapless advance resolves the next track over the
//!    network before draining more events), and an un-aged position "corrects" the timeline
//!    backwards.
//! 3. Only real mpv pause-flag transitions change `playing` — mpv fires `time-pos` on seeks while
//!    paused, so a position tick must never be treated as proof of playback.
//!
//! The socket is opened as soon as presence is enabled, not lazily on the first card — connecting
//! on demand meant the first song of a session waited out a connect round-trip (and a single failed
//! attempt stalled it for the whole retry interval) before anything showed up.
//!
//! Sending is rate-limited, because Discord silently drops presence updates that arrive too close
//! together — and a dropped update is invisible (the socket ACKs it). Two rules keep state from
//! getting stranded behind that: a **trailing-edge floor** (when a push is due but too soon, the
//! loop sleeps until the floor expires and sends whatever is current — never discards it),
//! and a short **grace** on a brand-new track so its length can land before the first push. Without
//! the grace, a track change pushed a bar-less card and needed a second push milliseconds later,
//! which Discord dropped — leaving the card stuck as an elapsed counter with no progress bar.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use innertube::SongItem;

/// Discord application id (a snowflake — digits only). Registered app named "Nocturne".
const APP_ID: &str = "1543224160166092828";

const SONG_URL: &str = "https://music.youtube.com/watch?v=";
const ARTIST_URL: &str = "https://music.youtube.com/channel/";
const ALBUM_URL: &str = "https://music.youtube.com/browse/";
const REPO_URL: &str = "https://github.com/Neo-XD/nocturne-music";
const LT_ACTION_URL: &str = "https://github.com/Neo-XD/nocturne-music?action=listen-together";
/// The small "provider badge" Discord draws in the corner of the artwork.
const BADGE_URL: &str =
    "https://raw.githubusercontent.com/Neo-XD/nocturne-music/master/src-tauri/icons/128x128.png";

/// Reconnect backoff while enabled but unconnected (Discord not running, or it quit).
const CONNECT_RETRY_MIN: Duration = Duration::from_secs(1);
const CONNECT_RETRY_MAX: Duration = Duration::from_secs(15);
/// How long to park when nothing is pending. Any message wakes the thread anyway.
const IDLE_TICK: Duration = Duration::from_secs(15);
/// A position this far off the timeline we last pushed means the user seeked, so the progress bar
/// needs re-pushing. Anything smaller is just clock/tick noise.
const SEEK_DRIFT_MS: i64 = 2_000;
/// Never send two updates inside this window — Discord drops the second silently.
const SEND_FLOOR: Duration = Duration::from_millis(1_500);
/// How long a new track waits for mpv to report its length before we give up and push a card with
/// no progress bar. Collapses the track-change burst (track + length + play state) into one push.
const DURATION_GRACE: Duration = Duration::from_millis(800);
/// Floor for any computed wait — handing `recv_timeout` a zero duration would spin.
const MIN_WAIT: Duration = Duration::from_millis(10);
/// Discord rejects `details`/`state`/`large_text` outside 2–128 characters.
const MAX_FIELD: usize = 128;
/// Discord rejects asset URLs longer than this.
const MAX_ASSET_URL: usize = 256;
/// Discord rejects a button whose URL is longer than this, and with it the whole payload.
const MAX_BUTTON_URL: usize = 512;

/// How the card is assembled, from the user's Discord settings tab. Persisted as one JSON blob in
/// the `discord_rpc_config` setting.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RpcConfig {
    /// Which slot Discord renders after "Listening to": `app` | `line1` | `line2`.
    pub status_line: String,
    /// Replaces the registered application name in that slot. Empty keeps "Nocturne".
    pub app_name: String,
    /// What the card's first line (`details`) carries: `title` | `artist` | `album` or template.
    pub line1: String,
    /// The second line (`state`): `title` | `artist` | `album` | `artist_album` | `off` or template.
    pub line2: String,
    /// Show the album artwork at all. Also gates [`Self::line3`].
    pub cover: bool,
    /// The card's third line: `album` | `title` | `artist` | `off`.
    pub line3: String,
    /// Make line 1 / line 2 / the artwork open the thing they name.
    pub link_line1: bool,
    pub link_line2: bool,
    pub link_cover: bool,
    /// Show the elapsed/remaining progress bar.
    pub timestamps: bool,
    /// Draw the Nocturne badge in the corner of the artwork.
    pub badge: bool,
    /// Keep the card up while paused.
    pub show_paused: bool,
    /// Say only that music is playing: no title, artist, album, artwork, bar or buttons.
    pub hide_details: bool,
    /// The two buttons, in card order: `listen` | `album` | `artist` | `app` | `off`.
    pub button1: String,
    pub button2: String,
    /// Optional custom registered Discord Application ID (snowflake digits only).
    pub app_id: String,
    /// Optional custom label for the interactive button.
    pub custom_button_label: String,
}

impl Default for RpcConfig {
    fn default() -> Self {
        RpcConfig {
            status_line: "line2".into(),
            app_name: String::new(),
            line1: "title".into(),
            line2: "artist".into(),
            cover: true,
            line3: "album".into(),
            link_line1: false,
            link_line2: false,
            link_cover: false,
            timestamps: true,
            badge: true,
            show_paused: false,
            hide_details: false,
            button1: "listen".into(),
            button2: "app".into(),
            app_id: String::new(),
            custom_button_label: "Listen on Nocturne".into(),
        }
    }
}

impl RpcConfig {
    /// Parse the stored blob. Anything unparseable is the default card.
    pub fn parse(json: Option<&str>) -> Self {
        json.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
    }
}

pub fn load_discord_config(db: &crate::db::Db) -> RpcConfig {
    let mut cfg = RpcConfig::parse(db.get_setting("discord_rpc_config").as_deref());
    if let Some(details) = db.get_setting("discord_rpc_details") {
        if !details.is_empty() {
            cfg.line1 = details;
        }
    }
    if let Some(state) = db.get_setting("discord_rpc_state") {
        if !state.is_empty() {
            cfg.line2 = state;
        }
    }
    if let Some(show_time) = db.get_setting("discord_rpc_show_time") {
        cfg.timestamps = show_time != "false";
    }
    if let Some(show_pause) = db.get_setting("discord_rpc_show_pause") {
        cfg.show_paused = show_pause != "false";
    }
    if let Some(show_btn) = db.get_setting("discord_rpc_show_button") {
        cfg.button1 = if show_btn != "false" { "listen".into() } else { "off".into() };
    }
    if let Some(btn_label) = db.get_setting("discord_rpc_button_label") {
        if !btn_label.is_empty() {
            cfg.custom_button_label = btn_label;
        }
    }
    if let Some(app_id) = db.get_setting("discord_rpc_app_id") {
        if app_id != "1525891596804161727" && !app_id.is_empty() {
            cfg.app_id = app_id;
        } else {
            cfg.app_id = APP_ID.to_string();
        }
    }
    cfg
}

enum Msg {
    Track(Box<Track>),
    Duration(f64),
    /// A position tick. `at` is when the value was read — the thread ages it before use.
    Position {
        pos: f64,
        at: Instant,
    },
    /// A real play/pause transition (mpv's pause flag), never inferred from position ticks.
    Playing(bool),
    Enabled(bool),
    Config(Box<RpcConfig>),
}

#[derive(Clone)]
struct Track {
    video_id: String,
    title: String,
    artists: String,
    artist_id: Option<String>,
    album: Option<String>,
    album_id: Option<String>,
    thumbnail: Option<String>,
    /// A file on this machine: it has no YouTube page, and its "id" is a path.
    local: bool,
}

/// App-side handle to the presence thread. `None` when the thread couldn't be spawned; every push
/// is then a no-op.
pub struct DiscordHandle {
    tx: Sender<Msg>,
}

impl DiscordHandle {
    pub fn set_track(&self, item: &SongItem) {
        let local = crate::local::is_local_song(&item.video_id);
        let _ = self.tx.send(Msg::Track(Box::new(Track {
            video_id: item.video_id.clone(),
            title: item.title.clone(),
            artists: item.artists.clone(),
            artist_id: item.artist_id.clone(),
            album: item.album.clone(),
            album_id: item.album_id.clone(),
            thumbnail: item.thumbnail.as_deref().filter(|_| !local).and_then(discord_thumb),
            local,
        })));
    }

    /// mpv's reported track length.
    pub fn set_duration(&self, secs: f64) {
        let _ = self.tx.send(Msg::Duration(secs));
    }

    /// A raw position tick.
    pub fn set_position(&self, pos: f64) {
        let _ = self.tx.send(Msg::Position { pos, at: Instant::now() });
    }

    pub fn set_playing(&self, playing: bool) {
        let _ = self.tx.send(Msg::Playing(playing));
    }

    pub fn set_enabled(&self, on: bool) {
        let _ = self.tx.send(Msg::Enabled(on));
    }

    /// Apply a new card layout.
    pub fn set_config(&self, cfg: RpcConfig) {
        let _ = self.tx.send(Msg::Config(Box::new(cfg)));
    }

    /// Backwards compatibility alias for `set_config`.
    pub fn update_config(&self, cfg: RpcConfig) {
        self.set_config(cfg);
    }
}

/// Spawn the presence owner thread.
pub fn spawn(enabled: bool, cfg: RpcConfig) -> Option<DiscordHandle> {
    if APP_ID.is_empty() || !APP_ID.bytes().all(|b| b.is_ascii_digit()) {
        tracing::warn!(APP_ID, "discord rich presence disabled: APP_ID is not a Discord app id");
        return None;
    }
    let (tx, rx) = channel::<Msg>();
    match std::thread::Builder::new()
        .name("discord-rpc".into())
        .spawn(move || run(rx, Presence::new(enabled, cfg)))
    {
        Ok(_) => Some(DiscordHandle { tx }),
        Err(e) => {
            tracing::warn!(error = %e, "discord-rpc thread spawn failed");
            None
        }
    }
}

fn run(rx: Receiver<Msg>, mut p: Presence) {
    let mut wait = p.sync();
    loop {
        match rx.recv_timeout(wait) {
            Ok(msg) => {
                p.apply(msg);
                loop {
                    match rx.try_recv() {
                        Ok(msg) => p.apply(msg),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            p.disconnect();
                            return;
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                p.disconnect();
                return;
            }
        }
        wait = p.sync();
    }
}

#[derive(Debug, PartialEq)]
enum Act {
    Idle,
    Wait(Duration),
    Push,
    Clear,
}

struct Presence {
    enabled: bool,
    cfg: RpcConfig,
    client: Option<DiscordIpcClient>,
    last_connect_try: Option<Instant>,
    connect_backoff: Duration,
    track: Option<Track>,
    track_at: Instant,
    duration: f64,
    playing: bool,
    played: bool,
    pos: f64,
    pos_at: Instant,
    sent: Option<Sent>,
    cfg_dirty: bool,
    last_send: Option<Instant>,
}

struct Sent {
    video_id: String,
    playing: bool,
    start_ms: i64,
    duration: f64,
}

impl Presence {
    fn new(enabled: bool, cfg: RpcConfig) -> Self {
        Presence {
            enabled,
            cfg,
            client: None,
            last_connect_try: None,
            connect_backoff: CONNECT_RETRY_MIN,
            track: None,
            track_at: Instant::now(),
            duration: 0.0,
            playing: false,
            played: false,
            pos: 0.0,
            pos_at: Instant::now(),
            sent: None,
            cfg_dirty: false,
            last_send: None,
        }
    }

    fn estimate(&self) -> f64 {
        if self.playing {
            self.pos + self.pos_at.elapsed().as_secs_f64()
        } else {
            self.pos
        }
    }

    fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Track(t) => {
                self.track = Some(*t);
                self.track_at = Instant::now();
                self.pos = 0.0;
                self.pos_at = Instant::now();
                self.duration = 0.0;
            }
            Msg::Duration(secs) => self.duration = secs,
            Msg::Position { pos, at } => {
                self.pos = pos;
                self.pos_at = at;
            }
            Msg::Playing(on) => {
                self.played |= on;
                if self.playing != on {
                    self.pos = self.estimate();
                    self.pos_at = Instant::now();
                    self.playing = on;
                }
            }
            Msg::Enabled(on) => {
                if self.enabled != on {
                    self.enabled = on;
                    if !on {
                        self.clear_shown();
                        self.client = None;
                    } else {
                        self.sent = None;
                    }
                }
            }
            Msg::Config(cfg) => {
                if self.cfg != *cfg {
                    self.cfg = *cfg;
                    self.cfg_dirty = true;
                }
            }
        }
    }

    fn sync(&mut self) -> Duration {
        let wait = match self.plan() {
            Act::Idle => IDLE_TICK,
            Act::Wait(d) => d,
            Act::Push => {
                if self.ensure_connected() {
                    self.push_card();
                }
                IDLE_TICK
            }
            Act::Clear => {
                self.clear_shown();
                IDLE_TICK
            }
        };
        match self.connect_backoff_remaining() {
            Some(rem) if self.client.is_none() => wait.min(rem.max(MIN_WAIT)),
            _ => wait,
        }
    }

    fn plan(&self) -> Act {
        if !self.enabled {
            return Act::Idle;
        }
        let want_card =
            self.track.is_some() && (self.playing || (self.cfg.show_paused && self.played));
        if want_card {
            if !self.wants_push() {
                return Act::Idle;
            }
            if self.duration <= 0.0 && self.is_new_card() {
                if let Some(rem) = DURATION_GRACE.checked_sub(self.track_at.elapsed()) {
                    return Act::Wait(rem.max(MIN_WAIT));
                }
            }
        } else if self.sent.is_none() {
            return Act::Idle;
        }
        if let Some(rem) = self.floor_remaining() {
            return Act::Wait(rem.max(MIN_WAIT));
        }
        if want_card {
            Act::Push
        } else {
            Act::Clear
        }
    }

    fn wants_push(&self) -> bool {
        let Some(track) = &self.track else { return false };
        let Some(sent) = &self.sent else { return true };
        if self.cfg_dirty {
            return true;
        }
        if sent.video_id != track.video_id {
            return true;
        }
        if sent.playing != self.playing {
            return true;
        }
        if (self.duration - sent.duration).abs() > 1.0 {
            return true;
        }
        if !self.playing {
            return false;
        }
        let drift_ms = (self.estimate() * 1000.0) as i64 - (now_ms() - sent.start_ms);
        drift_ms.abs() > SEEK_DRIFT_MS
    }

    fn is_new_card(&self) -> bool {
        match (&self.sent, &self.track) {
            (Some(sent), Some(track)) => sent.video_id != track.video_id,
            _ => true,
        }
    }

    fn floor_remaining(&self) -> Option<Duration> {
        SEND_FLOOR.checked_sub(self.last_send?.elapsed())
    }

    fn push_card(&mut self) {
        let (Some(track), Some(mut client)) = (self.track.clone(), self.client.take()) else {
            return;
        };

        let pos = self.estimate().max(0.0);
        let start_ms = now_ms() - (pos * 1000.0) as i64;
        let end_ms = (self.duration > 0.0).then(|| start_ms + (self.duration * 1000.0) as i64);

        let mut ts = activity::Timestamps::new().start(start_ms);
        if let Some(end) = end_ms {
            ts = ts.end(end);
        }
        let cfg = self.cfg.clone();
        let mut act = activity::Activity::new().activity_type(activity::ActivityType::Listening);
        if !cfg.app_name.is_empty() {
            act = act.name(field(&cfg.app_name));
        }

        if cfg.hide_details {
            act = act.status_display_type(activity::StatusDisplayType::Name);
            self.last_send = Some(Instant::now());
            self.cfg_dirty = false;
            if client.set_activity(act).is_ok() && check_response(&mut client, "set_activity") {
                self.sent = Some(Sent {
                    video_id: track.video_id,
                    playing: self.playing,
                    start_ms,
                    duration: self.duration,
                });
                self.client = Some(client);
            } else {
                self.sent = None;
            }
            return;
        }

        let line1 = text_for(&cfg.line1, &track);
        let line1_slot = if line1.is_some() { cfg.line1.as_str() } else { "title" };
        act = act.details(field(&line1.unwrap_or_else(|| track.title.clone())));
        if cfg.timestamps && self.playing {
            act = act.timestamps(ts);
        }
        act = act.status_display_type(match cfg.status_line.as_str() {
            "app" => activity::StatusDisplayType::Name,
            "line1" => activity::StatusDisplayType::Details,
            _ => activity::StatusDisplayType::State,
        });
        if let Some(line2) = text_for(&cfg.line2, &track) {
            act = act.state(field(&line2));
        }
        if let Some(url) = track.thumbnail.clone().filter(|_| cfg.cover) {
            let mut assets = activity::Assets::new().large_image(url);
            if let Some(line3) = text_for(&cfg.line3, &track) {
                assets = assets.large_text(field(&line3));
            }
            if cfg.badge {
                let name = if cfg.app_name.is_empty() { "Nocturne" } else { &cfg.app_name };
                assets = assets.small_image(BADGE_URL).small_text(field(name));
            }
            act = act.assets(assets);
        }
        let mut buttons: Vec<activity::Button<'static>> = Vec::new();
        // 1. Listen Together (always present)
        buttons.push(activity::Button::new("Listen Together", LT_ACTION_URL));
        // 2. Secondary action button
        let secondary = match cfg.button1.as_str() {
            "album" => button_for("album", &track),
            "artist" => button_for("artist", &track),
            "app" => button_for("app", &track),
            "off" => None,
            _ => {
                if !crate::local::is_local_song(&track.video_id) {
                    let label = if !cfg.custom_button_label.is_empty() {
                        cfg.custom_button_label.clone()
                    } else {
                        "Listen on YouTube Music".to_string()
                    };
                    Some(activity::Button::new(label, format!("{SONG_URL}{}", track.video_id)))
                } else {
                    Some(activity::Button::new("Get Nocturne", REPO_URL))
                }
            }
        };
        if let Some(btn) = secondary {
            buttons.push(btn);
        }
        act = act.buttons(buttons);

        self.last_send = Some(Instant::now());
        self.cfg_dirty = false;
        if client.set_activity(act).is_ok() && check_response(&mut client, "set_activity") {
            self.sent = Some(Sent {
                video_id: track.video_id,
                playing: self.playing,
                start_ms,
                duration: self.duration,
            });
            self.client = Some(client);
        } else {
            self.sent = None;
        }
    }

    fn clear_shown(&mut self) {
        if self.sent.take().is_none() {
            return;
        }
        if let Some(mut client) = self.client.take() {
            self.last_send = Some(Instant::now());
            if client.clear_activity().is_ok() && check_response(&mut client, "clear_activity") {
                self.client = Some(client);
            }
        }
    }

    fn connect_backoff_remaining(&self) -> Option<Duration> {
        self.connect_backoff.checked_sub(self.last_connect_try?.elapsed())
    }

    fn ensure_connected(&mut self) -> bool {
        if self.client.is_some() {
            return true;
        }
        if self.connect_backoff_remaining().is_some() {
            return false;
        }
        self.last_connect_try = Some(Instant::now());
        let app_id = if !self.cfg.app_id.is_empty()
            && self.cfg.app_id != "1525891596804161727"
            && self.cfg.app_id.bytes().all(|b| b.is_ascii_digit())
        {
            self.cfg.app_id.as_str()
        } else {
            APP_ID
        };
        let mut client = DiscordIpcClient::new(app_id);
        match client.connect() {
            Ok(()) => {
                tracing::info!(app_id, "discord rich presence connected");
                self.client = Some(client);
                self.connect_backoff = CONNECT_RETRY_MIN;
                self.sent = None;
                true
            }
            Err(e) => {
                tracing::debug!(error = %e, backoff = ?self.connect_backoff, "discord not available");
                self.connect_backoff = (self.connect_backoff * 2).min(CONNECT_RETRY_MAX);
                false
            }
        }
    }

    fn disconnect(&mut self) {
        self.clear_shown();
        if let Some(mut client) = self.client.take() {
            let _ = client.close();
        }
        self.sent = None;
        self.last_connect_try = None;
        self.connect_backoff = CONNECT_RETRY_MIN;
    }
}

fn check_response(client: &mut DiscordIpcClient, what: &str) -> bool {
    match client.recv() {
        Ok((_, resp)) => {
            if resp.get("evt").and_then(|v| v.as_str()) == Some("ERROR") {
                let msg =
                    resp.pointer("/data/message").and_then(|v| v.as_str()).unwrap_or("unknown");
                tracing::warn!(what, error = msg, "discord rejected the payload");
            }
            true
        }
        Err(e) => {
            tracing::debug!(what, error = %e, "discord response read failed — dropping socket");
            false
        }
    }
}

fn text_for(slot: &str, t: &Track) -> Option<String> {
    let trimmed = slot.trim();
    if trimmed.is_empty() || trimmed == "off" {
        return None;
    }
    let artist = (!t.artists.is_empty()).then(|| t.artists.clone());
    let album = t.album.clone().filter(|a| !a.is_empty());
    match trimmed {
        "title" => (!t.title.is_empty()).then(|| t.title.clone()),
        "artist" => artist,
        "album" => album,
        "artist_album" => match (artist, album) {
            (Some(a), Some(b)) => Some(format!("{a} — {b}")),
            (a, b) => a.or(b),
        },
        custom => {
            let replaced = custom
                .replace("{title}", &t.title)
                .replace("{artist}", &t.artists)
                .replace("{album}", t.album.as_deref().unwrap_or(""));
            let r = replaced.trim();
            (!r.is_empty()).then(|| r.to_string())
        }
    }
}

fn link_for(slot: &str, t: &Track) -> Option<String> {
    if t.local {
        return None;
    }
    match slot {
        "title" => (!t.video_id.is_empty()).then(|| format!("{SONG_URL}{}", t.video_id)),
        "artist" | "artist_album" => t.artist_id.as_ref().map(|id| format!("{ARTIST_URL}{id}")),
        "album" => t.album_id.as_ref().map(|id| format!("{ALBUM_URL}{id}")),
        _ => None,
    }
}

fn button_for(kind: &str, t: &Track) -> Option<activity::Button<'static>> {
    let (label, url) = match kind {
        "listen_together" => ("Listen Together", LT_ACTION_URL.to_owned()),
        "listen" => ("Listen on YouTube Music", link_for("title", t)?),
        "album" => ("View album", link_for("album", t)?),
        "artist" => ("View artist", link_for("artist", t)?),
        "app" => ("Get Nocturne", REPO_URL.to_owned()),
        _ => return None,
    };
    (url.len() <= MAX_BUTTON_URL).then(|| activity::Button::new(label, url))
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

fn field(s: &str) -> String {
    let mut out: String = match s.char_indices().nth(MAX_FIELD) {
        Some(_) => s.chars().take(MAX_FIELD - 1).chain(['…']).collect(),
        None => s.to_owned(),
    };
    while out.chars().count() < 2 {
        out.push('\u{2800}');
    }
    out
}

fn discord_thumb(url: &str) -> Option<String> {
    static WH: OnceLock<regex::Regex> = OnceLock::new();
    static S: OnceLock<regex::Regex> = OnceLock::new();
    let wh = WH.get_or_init(|| regex::Regex::new(r"=w\d+-h\d+").expect("static regex"));
    let s = S.get_or_init(|| regex::Regex::new(r"=s\d+").expect("static regex"));
    let sized = if wh.is_match(url) {
        wh.replace(url, "=w512-h512").into_owned()
    } else if s.is_match(url) {
        s.replace(url, "=s512").into_owned()
    } else {
        url.to_owned()
    };
    (sized.len() <= MAX_ASSET_URL).then_some(sized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_is_a_snowflake() {
        assert!(
            !APP_ID.is_empty() && APP_ID.bytes().all(|b| b.is_ascii_digit()),
            "APP_ID must be a Discord application id (digits only) — got {APP_ID:?}"
        );
    }

    fn track(id: &str) -> Box<Track> {
        Box::new(Track {
            video_id: id.into(),
            title: "t".into(),
            artists: "a".into(),
            artist_id: None,
            album: None,
            album_id: None,
            thumbnail: None,
            local: false,
        })
    }

    fn playing(id: &str, pos: f64) -> Presence {
        let mut p = Presence::new(true, RpcConfig::default());
        p.apply(Msg::Track(track(id)));
        p.apply(Msg::Playing(true));
        p.apply(Msg::Position { pos, at: Instant::now() });
        p
    }

    fn sent_now(p: &mut Presence, pos_secs: i64) {
        p.sent = Some(Sent {
            video_id: p.track.as_ref().unwrap().video_id.clone(),
            playing: p.playing,
            start_ms: now_ms() - pos_secs * 1000,
            duration: p.duration,
        });
        p.last_send = Some(Instant::now() - Duration::from_secs(60));
        p.cfg_dirty = false;
    }

    #[test]
    fn a_new_track_waits_for_its_length_then_pushes_once() {
        let mut p = playing("old", 200.0);
        p.duration = 210.0;
        sent_now(&mut p, 200);

        p.apply(Msg::Track(track("new")));
        assert!(
            matches!(p.plan(), Act::Wait(_)),
            "must hold the push while the length is unknown, got {:?}",
            p.plan()
        );

        p.apply(Msg::Duration(185.0));
        assert_eq!(p.plan(), Act::Push, "with the length known, push immediately");

        p.sent = Some(Sent {
            video_id: "new".into(),
            playing: true,
            start_ms: now_ms(),
            duration: 185.0,
        });
        p.last_send = Some(Instant::now());
        assert_eq!(p.plan(), Act::Idle, "one push per track change, not two");
    }

    #[test]
    fn the_grace_expires_rather_than_hanging() {
        let mut p = playing("abc", 0.0);
        p.track_at = Instant::now() - DURATION_GRACE - Duration::from_millis(50);
        assert_eq!(p.plan(), Act::Push, "grace expired — push a bar-less card");
    }

    #[test]
    fn a_late_length_repushes_the_bar() {
        let mut p = playing("abc", 1.0);
        sent_now(&mut p, 1);
        p.apply(Msg::Duration(185.0));
        assert!(p.wants_push(), "the bar's end appeared — push it");
        assert_eq!(p.plan(), Act::Push);
    }

    #[test]
    fn the_send_floor_defers_rather_than_discards() {
        let mut p = playing("abc", 1.0);
        p.duration = 185.0;
        sent_now(&mut p, 1);
        p.last_send = Some(Instant::now());
        p.apply(Msg::Position { pos: 120.0, at: Instant::now() });

        match p.plan() {
            Act::Wait(d) => assert!(d <= SEND_FLOOR, "waits out the floor, got {d:?}"),
            other => panic!("expected a deferred push, got {other:?}"),
        }
        p.last_send = Some(Instant::now() - SEND_FLOOR - Duration::from_millis(10));
        assert_eq!(p.plan(), Act::Push, "the deferred update must not be lost");
    }

    #[test]
    fn track_change_resets_the_timeline() {
        let mut p = playing("old", 187.0);
        sent_now(&mut p, 187);
        p.apply(Msg::Track(track("new")));
        assert!(p.estimate() < 0.5, "new track starts at 0, got {}", p.estimate());
        assert!(p.wants_push(), "track change must push");
    }

    #[test]
    fn steady_playback_does_not_push() {
        let mut p = playing("abc", 30.0);
        assert!(p.wants_push(), "first track must push");
        sent_now(&mut p, 30);

        p.apply(Msg::Position { pos: 31.0, at: Instant::now() });
        assert!(!p.wants_push(), "a tick on the pushed timeline must not push");
        assert_eq!(p.plan(), Act::Idle);

        p.apply(Msg::Position { pos: 120.0, at: Instant::now() });
        assert!(p.wants_push(), "a scrub must push");
    }

    #[test]
    fn aged_positions_are_corrected() {
        let mut p = playing("abc", 0.0);
        sent_now(&mut p, 13);
        p.apply(Msg::Position { pos: 10.0, at: Instant::now() - Duration::from_secs(3) });
        let est = p.estimate();
        assert!((est - 13.0).abs() < 0.2, "expected ~13, got {est}");
        assert!(!p.wants_push(), "an aged-but-on-timeline tick must not push");
    }

    #[test]
    fn pause_freezes_and_hides() {
        let mut p = playing("abc", 10.0);
        p.pos_at = Instant::now() - Duration::from_secs(3);
        p.apply(Msg::Playing(false));
        let frozen = p.estimate();
        assert!((frozen - 13.0).abs() < 0.2, "expected ~13 frozen, got {frozen}");
        assert!(!p.playing, "paused");
    }

    #[test]
    fn dropping_show_paused_while_paused_clears_the_card() {
        let mut p = playing("abc", 30.0);
        p.duration = 185.0;
        p.apply(Msg::Config(Box::new(RpcConfig { show_paused: true, ..Default::default() })));
        p.apply(Msg::Playing(false));
        sent_now(&mut p, 30);
        assert_eq!(p.plan(), Act::Idle, "the paused card is up and current");

        p.apply(Msg::Config(Box::new(RpcConfig::default())));
        assert_eq!(p.plan(), Act::Clear);
    }

    #[test]
    fn field_clamps_to_discords_window() {
        assert_eq!(field("hello"), "hello");
        assert_eq!(field("a"), "a\u{2800}");
        assert_eq!(field(""), "\u{2800}\u{2800}");
        let long = "x".repeat(200);
        let out = field(&long);
        assert_eq!(out.chars().count(), 128);
        assert!(out.ends_with('…'));
    }
}
