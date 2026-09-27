//! libmpv wrapper. context/14. YouTube-agnostic: takes a fully-resolved URL + headers, never
//! a videoId. Gapless via mpv's internal playlist or true dual-deck overlapping crossfade.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use libmpv2::events::{Event, EventContext, PropertyData};
use libmpv2::{Format, Mpv};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mpv: {0}")]
    Mpv(#[from] libmpv2::Error),
    /// mpv refused a chain carrying the pitch filter, which means this libmpv was built without
    /// librubberband. Its own answer is `Raw(-9)`, so it needs saying in words: this one reaches
    /// the user as a toast.
    #[error("Pitch shifting isn't available in this build")]
    NoPitchFilter,
    #[error("Failed to parse json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Output audio device description from mpv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

/// Events pumped from mpv's event thread. context/14 §player surface.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Position(f64),
    Duration(f64),
    /// Playback started or stopped, emitted only on a real change.
    ///
    /// Derived from mpv's `pause` **and** `idle-active`, because `pause` alone is a trap: it starts
    /// out `false` and a `loadfile` doesn't touch it, so starting a track sets `false` → `false`
    /// and fires **no** property event at all. `idle-active` is the one that actually flips when a
    /// file starts (and when the playlist runs dry). Anything reading playback state off `pause`
    /// alone never hears that a track began, and only recovers on a manual pause/unpause.
    Playing(bool),
    /// One track finished normally (EOF) — orchestrator advances the queue.
    TrackEnded,
    /// One track died (end-file with error, e.g. its URL 403'd). mpv may have auto-advanced
    /// into the next playlist entry or gone idle — the orchestrator asks [`Player::is_idle`].
    TrackFailed(String),
    Error(String),
}

/// mpv end-file reasons (from `mpv_end_file_reason`).
const EOF: i32 = 0;

/// User-facing message for a failed track — raw mpv codes ("Raw(-13)") mean nothing to users.
fn friendly_error(e: &libmpv2::Error) -> String {
    use libmpv2::mpv_error;
    match e {
        libmpv2::Error::Loadfile { error } => friendly_error(error),
        libmpv2::Error::Raw(code) => match *code {
            mpv_error::LoadingFailed => {
                "Couldn't load this track — YouTube rejected the stream link".to_owned()
            }
            mpv_error::NothingToPlay => "This stream contains no playable audio".to_owned(),
            mpv_error::UnknownFormat => "Unrecognized audio format".to_owned(),
            mpv_error::AoInitFailed => "Couldn't start audio output".to_owned(),
            other => format!("Playback failed (mpv error {other})"),
        },
        other => format!("Playback failed ({other})"),
    }
}

/// Single band in a parametric equalizer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EqBand {
    pub freq: f64,
    pub gain: f64,
    pub q: f64,
}

#[derive(Debug, Clone, Default)]
struct AudioFilters {
    gain_db: Option<f64>,
    semitones: i32,
    crossfade_secs: f64,
    skip_fade_in: bool,
    track_duration: Option<f64>,
    eq_enabled: bool,
    eq_preamp_db: f64,
    eq_bands: Vec<EqBand>,
}

/// Internal events from each deck's event loop to the central arbiter.
enum InternalDeckEvent {
    Position { deck_id: u8, pos: f64 },
    Duration { deck_id: u8, dur: f64 },
    Playing { deck_id: u8, playing: bool },
    TrackEnded { deck_id: u8 },
    TrackFailed { deck_id: u8, error: String },
}

/// Represents one playback deck (an independent libmpv instance).
pub struct Deck {
    pub id: u8,
    pub mpv: Arc<Mpv>,
    af: std::sync::Mutex<AudioFilters>,
}

impl Deck {
    fn new(id: u8, cache_dir: &str) -> Result<Self, Error> {
        let mpv = Mpv::new()?;
        mpv.set_property("vid", "no")?; // audio only
        mpv.set_property("ytdl", "no")?;
        mpv.set_property("gapless-audio", "yes")?;
        mpv.set_property("demuxer", "lavf")?;
        mpv.set_property("prefetch-playlist", "yes")?;
        mpv.set_property("cache", "yes")?;
        mpv.set_property("cache-on-disk", "yes")?;
        let deck_cache = format!("{cache_dir}/deck_{id}");
        let _ = std::fs::create_dir_all(&deck_cache);
        mpv.set_property("demuxer-cache-dir", deck_cache.as_str())?;
        mpv.set_property("demuxer-max-bytes", 32 * 1024 * 1024_i64)?;
        mpv.set_property("demuxer-max-back-bytes", 32 * 1024 * 1024_i64)?;
        mpv.set_property("demuxer-readahead-secs", 120.0_f64)?;
        let mpv = Arc::new(mpv);

        Ok(Deck {
            id,
            mpv,
            af: std::sync::Mutex::new(AudioFilters::default()),
        })
    }

    fn apply_af(&self) -> Result<(), Error> {
        let filters = self.af.lock().unwrap().clone();
        self.mpv.set_property("af", af_chain(&filters).as_str())?;
        Ok(())
    }

    fn apply_headers(&self, headers: &HashMap<String, String>) -> Result<(), Error> {
        if let Some(ua) = headers.get("User-Agent").or_else(|| headers.get("user-agent")) {
            self.mpv.set_property("user-agent", ua.as_str())?;
        }
        let fields: String = headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("user-agent"))
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(",");
        self.mpv.set_property("http-header-fields", fields.as_str())?;
        Ok(())
    }

    fn stop(&self) {
        let _ = self.mpv.command("stop", &[]);
    }

    fn is_idle(&self) -> bool {
        self.mpv.get_property::<bool>("idle-active").unwrap_or(true)
    }
}

/// The player. Wraps two libmpv instances (`deck_a` and `deck_b`) to support gapless audio as well
/// as true overlapping dual-deck crossfade playback where the outgoing track fades out while the
/// incoming track fades in concurrently.
pub struct Player {
    deck_a: Arc<Deck>,
    deck_b: Arc<Deck>,
    active_deck: Arc<AtomicU8>,
    fading_deck: Arc<AtomicI8>,
    primed_deck: Arc<AtomicI8>,
    crossfade_secs: Arc<AtomicU64>,
    skip_fade_in: Arc<AtomicBool>,
    events: Option<UnboundedReceiver<PlayerEvent>>,
    shared_af: Arc<std::sync::Mutex<AudioFilters>>,
    volume: Arc<AtomicI64>,
    speed: Arc<AtomicU64>,
    current_device: Arc<std::sync::Mutex<String>>,
}

impl Player {
    /// Create a dual-deck player with disk audio cache under `cache_dir`.
    pub fn new(cache_dir: &str) -> Result<Self, Error> {
        #[cfg(unix)]
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }

        let deck_a = Arc::new(Deck::new(0, cache_dir)?);
        let deck_b = Arc::new(Deck::new(1, cache_dir)?);

        let active_deck = Arc::new(AtomicU8::new(0));
        let fading_deck = Arc::new(AtomicI8::new(-1));
        let primed_deck = Arc::new(AtomicI8::new(-1));
        let crossfade_secs = Arc::new(AtomicU64::new(0f64.to_bits()));
        let skip_fade_in = Arc::new(AtomicBool::new(false));
        let shared_af = Arc::new(std::sync::Mutex::new(AudioFilters::default()));
        let volume = Arc::new(AtomicI64::new(100));
        let speed = Arc::new(AtomicU64::new(1.0f64.to_bits()));
        let current_device = Arc::new(std::sync::Mutex::new("auto".to_string()));

        let (internal_tx, internal_rx) = std::sync::mpsc::channel();
        let (external_tx, external_rx) = unbounded_channel();

        // Spawn event context thread for deck 0
        {
            let ev_a = EventContext::new(deck_a.mpv.ctx);
            ev_a.disable_deprecated_events().ok();
            ev_a.observe_property("time-pos", Format::Double, 0)?;
            ev_a.observe_property("duration", Format::Double, 1)?;
            ev_a.observe_property("pause", Format::Flag, 2)?;
            ev_a.observe_property("idle-active", Format::Flag, 3)?;

            let tx0 = internal_tx.clone();
            std::thread::Builder::new()
                .name("mpv-events-0".into())
                .spawn(move || deck_event_loop(0, ev_a, tx0))
                .expect("spawn mpv deck 0 event thread");
        }

        // Spawn event context thread for deck 1
        {
            let ev_b = EventContext::new(deck_b.mpv.ctx);
            ev_b.disable_deprecated_events().ok();
            ev_b.observe_property("time-pos", Format::Double, 0)?;
            ev_b.observe_property("duration", Format::Double, 1)?;
            ev_b.observe_property("pause", Format::Flag, 2)?;
            ev_b.observe_property("idle-active", Format::Flag, 3)?;

            let tx1 = internal_tx;
            std::thread::Builder::new()
                .name("mpv-events-1".into())
                .spawn(move || deck_event_loop(1, ev_b, tx1))
                .expect("spawn mpv deck 1 event thread");
        }

        // Spawn central event arbiter thread
        {
            let act = active_deck.clone();
            let fad = fading_deck.clone();
            let prm = primed_deck.clone();
            let da = deck_a.clone();
            let db = deck_b.clone();
            std::thread::Builder::new()
                .name("mpv-arbiter".into())
                .spawn(move || {
                    arbitrate_events(act, fad, prm, da, db, internal_rx, external_tx);
                })
                .expect("spawn mpv arbiter thread");
        }

        Ok(Player {
            deck_a,
            deck_b,
            active_deck,
            fading_deck,
            primed_deck,
            crossfade_secs,
            skip_fade_in,
            events: Some(external_rx),
            shared_af,
            volume,
            speed,
            current_device,
        })
    }

    /// Access the active deck helper.
    pub fn deck(&self, id: u8) -> &Arc<Deck> {
        if id == 0 {
            &self.deck_a
        } else {
            &self.deck_b
        }
    }

    pub fn active_deck(&self) -> &Arc<Deck> {
        self.deck(self.active_deck.load(Ordering::SeqCst))
    }

    pub fn fading_deck(&self) -> Option<&Arc<Deck>> {
        let f = self.fading_deck.load(Ordering::SeqCst);
        if f >= 0 {
            Some(self.deck(f as u8))
        } else {
            None
        }
    }

    pub fn stop_fading_deck(&self) {
        let f = self.fading_deck.swap(-1, Ordering::SeqCst);
        if f >= 0 {
            self.deck(f as u8).stop();
        }
    }

    pub fn stop_primed_deck(&self) {
        let p = self.primed_deck.swap(-1, Ordering::SeqCst);
        if p >= 0 {
            self.deck(p as u8).stop();
        }
    }

    #[cfg(test)]
    pub fn active_mpv(&self) -> &Arc<Mpv> {
        &self.active_deck().mpv
    }

    /// Take the event receiver (once).
    pub fn take_events(&mut self) -> Option<UnboundedReceiver<PlayerEvent>> {
        self.events.take()
    }

    /// Load and play a fresh URL, replacing the active deck's playlist.
    pub fn load(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        gain_db: Option<f64>,
    ) -> Result<(), Error> {
        self.stop_fading_deck();
        self.stop_primed_deck();

        let active = self.active_deck();
        active.apply_headers(headers)?;
        {
            let mut af = active.af.lock().unwrap();
            let shared = self.shared_af.lock().unwrap();
            af.gain_db = gain_db;
            af.semitones = shared.semitones;
            af.crossfade_secs = shared.crossfade_secs;
            af.skip_fade_in = self.skip_fade_in.swap(false, Ordering::SeqCst);
            af.track_duration = None;
            af.eq_enabled = shared.eq_enabled;
            af.eq_preamp_db = shared.eq_preamp_db;
            af.eq_bands = shared.eq_bands.clone();
        }
        active.apply_af()?;
        active.mpv.command("loadfile", &[&quoted(url), "replace"])?;
        Ok(())
    }

    /// Enqueue a URL for gapless transition (single deck fallback).
    pub fn enqueue(&self, url: &str) -> Result<(), Error> {
        self.enqueue_track(url, &HashMap::new(), None)
    }

    /// Enqueue the next track for either gapless transition (when crossfade is 0)
    /// or true dual-deck overlapping crossfade (when crossfade > 0).
    pub fn enqueue_track(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        gain_db: Option<f64>,
    ) -> Result<(), Error> {
        let xf = self.crossfade_secs();
        if xf <= 0.0 {
            // Gapless single deck
            self.active_deck().apply_headers(headers)?;
            self.active_deck().mpv.command("loadfile", &[&quoted(url), "append"])?;
            self.primed_deck.store(-1, Ordering::SeqCst);
            return Ok(());
        }

        // Dual-deck overlapping crossfade: prime the standby deck
        let standby_id = 1 - self.active_deck.load(Ordering::SeqCst);
        let standby = self.deck(standby_id);

        standby.apply_headers(headers)?;
        {
            let mut af = standby.af.lock().unwrap();
            let shared = self.shared_af.lock().unwrap();
            af.gain_db = gain_db;
            af.semitones = shared.semitones;
            af.crossfade_secs = xf;
            af.skip_fade_in = false; // Primed lookahead track fades in
            af.track_duration = None;
            af.eq_enabled = shared.eq_enabled;
            af.eq_preamp_db = shared.eq_preamp_db;
            af.eq_bands = shared.eq_bands.clone();
        }
        standby.apply_af()?;
        // Pre-load paused so it buffers and is ready to start playing instantly with 0 latency
        standby.mpv.set_property("pause", true)?;
        standby.mpv.command("loadfile", &[&quoted(url), "replace"])?;

        self.primed_deck.store(standby_id as i8, Ordering::SeqCst);

        // Also ensure active deck has fade-out configured if duration is known
        let _ = self.active_deck().apply_af();

        Ok(())
    }

    /// True if crossfade is configured (> 0) and the secondary deck is primed and ready.
    pub fn can_crossfade(&self) -> bool {
        self.crossfade_secs() > 0.0 && self.primed_deck.load(Ordering::SeqCst) >= 0
    }

    /// Trigger the overlapping crossfade handoff: starts the primed deck immediately with fade-in,
    /// marks the current deck as fading-out, and flips active decks.
    pub fn start_crossfade(&self) -> Result<(), Error> {
        let primed = self.primed_deck.load(Ordering::SeqCst);
        if primed < 0 {
            return Ok(());
        }
        let incoming_id = primed as u8;
        let outgoing_id = self.active_deck.load(Ordering::SeqCst);

        let incoming = self.deck(incoming_id);
        let outgoing = self.deck(outgoing_id);

        // Start incoming deck playback immediately
        incoming.mpv.set_property("pause", false)?;

        // Switch active deck to incoming, outgoing becomes fading
        self.fading_deck.store(outgoing_id as i8, Ordering::SeqCst);
        self.active_deck.store(incoming_id, Ordering::SeqCst);
        self.primed_deck.store(-1, Ordering::SeqCst);

        // Stop outgoing deck after crossfade completes (with 0.5s safety margin)
        let xf_secs = self.crossfade_secs();
        let out_mpv = outgoing.mpv.clone();
        let fading_ref = self.fading_deck.clone();
        std::thread::Builder::new()
            .name("crossfade-reaper".into())
            .spawn(move || {
                let wait_ms = ((xf_secs + 0.5) * 1000.0) as u64;
                std::thread::sleep(std::time::Duration::from_millis(wait_ms));
                if fading_ref.load(Ordering::SeqCst) == outgoing_id as i8 {
                    let _ = out_mpv.command("stop", &[]);
                    fading_ref.store(-1, Ordering::SeqCst);
                }
            })
            .ok();

        Ok(())
    }

    /// Clear the mpv playlist.
    pub fn clear_playlist(&self) -> Result<(), Error> {
        self.stop_fading_deck();
        self.stop_primed_deck();
        self.active_deck().mpv.command("playlist-clear", &[])?;
        Ok(())
    }

    /// Stop playback outright and empty the playlist: mpv goes idle and stays there.
    pub fn stop(&self) -> Result<(), Error> {
        self.stop_fading_deck();
        self.stop_primed_deck();
        self.active_deck().stop();
        Ok(())
    }

    /// True when active deck has nothing loaded (playlist exhausted or the last load failed).
    pub fn is_idle(&self) -> bool {
        self.active_deck().is_idle() && self.fading_deck.load(Ordering::SeqCst) < 0
    }

    pub fn play(&self) -> Result<(), Error> {
        self.active_deck().mpv.set_property("pause", false)?;
        if let Some(fading) = self.fading_deck() {
            let _ = fading.mpv.set_property("pause", false);
        }
        Ok(())
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.active_deck().mpv.set_property("pause", true)?;
        if let Some(fading) = self.fading_deck() {
            let _ = fading.mpv.set_property("pause", true);
        }
        Ok(())
    }

    pub fn toggle(&self) -> Result<(), Error> {
        let is_paused = self.active_deck().mpv.get_property::<bool>("pause").unwrap_or(false);
        if is_paused {
            self.play()
        } else {
            self.pause()
        }
    }

    /// Loop the current file seamlessly (repeat-one).
    pub fn set_loop_file(&self, on: bool) -> Result<(), Error> {
        self.deck_a.mpv.set_property("loop-file", if on { "inf" } else { "no" })?;
        self.deck_b.mpv.set_property("loop-file", if on { "inf" } else { "no" })?;
        Ok(())
    }

    /// Absolute seek in seconds. Immediately terminates any fading-out track.
    pub fn seek(&self, position_secs: f64) -> Result<(), Error> {
        self.stop_fading_deck();
        self.active_deck().mpv.command("seek", &[&position_secs.to_string(), "absolute"])?;
        Ok(())
    }

    /// Set output volume (0–100) mapped to perceptual scale across both decks.
    pub fn set_volume(&self, volume: i64) -> Result<(), Error> {
        self.volume.store(volume, Ordering::SeqCst);
        let mpv_vol = perceptual_to_mpv(volume);
        self.deck_a.mpv.set_property("volume", mpv_vol)?;
        self.deck_b.mpv.set_property("volume", mpv_vol)?;
        Ok(())
    }

    /// Apply a per-track loudness gain (dB) to active deck.
    pub fn set_gain(&self, gain_db: Option<f64>) -> Result<(), Error> {
        let active = self.active_deck();
        active.af.lock().unwrap().gain_db = gain_db;
        active.apply_af()
    }

    /// Tempo, 0.25–2.0 across both decks.
    pub fn set_speed(&self, speed: f64) -> Result<(), Error> {
        let clamped = speed.clamp(0.25, 2.0);
        self.speed.store(clamped.to_bits(), Ordering::SeqCst);
        self.deck_a.mpv.set_property("speed", clamped)?;
        self.deck_b.mpv.set_property("speed", clamped)?;
        Ok(())
    }

    /// Pitch shift in semitones, −12..=12 across both decks.
    pub fn set_pitch(&self, semitones: i32) -> Result<(), Error> {
        let wanted = semitones.clamp(-12, 12);
        let prev = {
            let mut shared = self.shared_af.lock().unwrap();
            let p = shared.semitones;
            shared.semitones = wanted;
            p
        };
        let active = self.active_deck();
        active.af.lock().unwrap().semitones = wanted;
        if let Err(e) = active.apply_af() {
            self.shared_af.lock().unwrap().semitones = prev;
            active.af.lock().unwrap().semitones = prev;
            let _ = active.apply_af();
            return Err(if wanted == 0 { e } else { Error::NoPitchFilter });
        }
        let standby_id = 1 - self.active_deck.load(Ordering::SeqCst);
        let standby = self.deck(standby_id);
        standby.af.lock().unwrap().semitones = wanted;
        let _ = standby.apply_af();
        Ok(())
    }

    /// Crossfade duration in seconds (0.0 = disabled).
    pub fn set_crossfade(&self, secs: f64) -> Result<(), Error> {
        let secs = secs.max(0.0);
        self.crossfade_secs.store(secs.to_bits(), Ordering::SeqCst);
        {
            let mut shared = self.shared_af.lock().unwrap();
            shared.crossfade_secs = secs;
        }
        let active = self.active_deck();
        active.af.lock().unwrap().crossfade_secs = secs;
        active.apply_af()?;

        let standby_id = 1 - self.active_deck.load(Ordering::SeqCst);
        let standby = self.deck(standby_id);
        standby.af.lock().unwrap().crossfade_secs = secs;
        let _ = standby.apply_af();

        Ok(())
    }

    /// Read the configured crossfade duration in seconds.
    pub fn crossfade_secs(&self) -> f64 {
        f64::from_bits(self.crossfade_secs.load(Ordering::SeqCst))
    }

    /// Set whether the incoming track should bypass the crossfade fade-in (e.g. on manual skip/jump).
    pub fn set_skip_fade_in(&self, skip: bool) -> Result<(), Error> {
        self.skip_fade_in.store(skip, Ordering::SeqCst);
        let active = self.active_deck();
        active.af.lock().unwrap().skip_fade_in = skip;
        active.apply_af()
    }

    /// Update current track duration for crossfade out timing.
    pub fn set_track_duration(&self, dur: Option<f64>) -> Result<(), Error> {
        let active = self.active_deck();
        active.af.lock().unwrap().track_duration = dur;
        active.apply_af()
    }

    /// Apply parametric equalizer bands and preamp across both decks.
    pub fn set_equalizer(
        &self,
        enabled: bool,
        preamp_db: f64,
        bands: Vec<EqBand>,
    ) -> Result<(), Error> {
        {
            let mut shared = self.shared_af.lock().unwrap();
            shared.eq_enabled = enabled;
            shared.eq_preamp_db = preamp_db;
            shared.eq_bands = bands.clone();
        }
        for deck in [&self.deck_a, &self.deck_b] {
            let mut af = deck.af.lock().unwrap();
            af.eq_enabled = enabled;
            af.eq_preamp_db = preamp_db;
            af.eq_bands = bands.clone();
            drop(af);
            let _ = deck.apply_af();
        }
        Ok(())
    }

    /// List output audio devices discovered by mpv.
    pub fn get_audio_devices(&self) -> Result<Vec<AudioDevice>, Error> {
        let json_str = self.active_deck().mpv.get_property::<String>("audio-device-list")?;
        let devices: Vec<AudioDevice> = serde_json::from_str(&json_str)?;
        Ok(devices)
    }

    /// Current active output audio device name ("auto" or driver/device identifier).
    pub fn get_current_audio_device(&self) -> Result<String, Error> {
        let dev = self.active_deck().mpv.get_property::<String>("audio-device")?;
        Ok(dev)
    }

    /// Set output audio device ("auto" or specific device name) across both decks.
    pub fn set_audio_device(&self, device: &str) -> Result<(), Error> {
        *self.current_device.lock().unwrap() = device.to_owned();
        self.deck_a.mpv.set_property("audio-device", device)?;
        self.deck_b.mpv.set_property("audio-device", device)?;
        Ok(())
    }
}

/// The whole `af` chain: loudness gain, pitch, crossfade, and parametric equalizer.
fn af_chain(af: &AudioFilters) -> String {
    let mut chain = Vec::new();

    // Volume / Preamp / ReplayGain
    let total_gain = match (af.gain_db, af.eq_enabled && af.eq_preamp_db.abs() > 0.01) {
        (Some(g), true) => Some(g + af.eq_preamp_db),
        (Some(g), false) => Some(g),
        (None, true) => Some(af.eq_preamp_db),
        (None, false) => None,
    };
    if let Some(g) = total_gain {
        chain.push(format!("lavfi=[volume={g:.2}dB]"));
    }

    // Parametric Equalizer bands (using FFmpeg's biquad peaking equalizer filter)
    if af.eq_enabled && !af.eq_bands.is_empty() {
        let active_bands: Vec<String> = af
            .eq_bands
            .iter()
            .filter(|b| b.gain.abs() > 0.05)
            .map(|b| {
                format!(
                    "equalizer=f={:.1}:t=q:w={:.2}:g={:.2}",
                    b.freq,
                    b.q.clamp(0.1, 10.0),
                    b.gain.clamp(-24.0, 24.0)
                )
            })
            .collect();
        if !active_bands.is_empty() {
            chain.push(format!("lavfi=[{}]", active_bands.join(",")));
        }
    }

    if af.semitones != 0 {
        // Semitones → frequency multiplier (equal temperament).
        chain.push(format!(
            "{}=pitch-scale={}",
            pitch_filter(),
            2f64.powf(af.semitones as f64 / 12.0)
        ));
    }
    if af.crossfade_secs > 0.0 {
        let xf = af.crossfade_secs;
        if !af.skip_fade_in {
            chain.push(format!("lavfi=[afade=t=in:ss=0:d={xf:.2}]"));
        }
        if let Some(dur) = af.track_duration {
            if dur > xf * 2.0 {
                let start_out = dur - xf;
                chain.push(format!("lavfi=[afade=t=out:st={start_out:.2}:d={xf:.2}]"));
            }
        }
    }
    chain.join(",")
}

#[cfg(test)]
static NO_RUBBERBAND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn pitch_filter() -> &'static str {
    #[cfg(test)]
    if NO_RUBBERBAND.load(std::sync::atomic::Ordering::Relaxed) {
        return "rubberband_this_build_does_not_have";
    }
    "rubberband"
}

fn deck_event_loop(
    deck_id: u8,
    mut ev: EventContext,
    tx: std::sync::mpsc::Sender<InternalDeckEvent>,
) {
    let mut paused = false;
    let mut idle = true;
    let mut playing = false;
    loop {
        match ev.wait_event(1.0) {
            Some(Ok(event)) => {
                let out = match event {
                    Event::PropertyChange {
                        name: "time-pos",
                        change: PropertyData::Double(p),
                        ..
                    } => Some(InternalDeckEvent::Position { deck_id, pos: p }),
                    Event::PropertyChange {
                        name: "duration",
                        change: PropertyData::Double(d),
                        ..
                    } => Some(InternalDeckEvent::Duration { deck_id, dur: d }),
                    Event::PropertyChange {
                        name: "pause",
                        change: PropertyData::Flag(p),
                        ..
                    } => {
                        paused = p;
                        None
                    }
                    Event::PropertyChange {
                        name: "idle-active",
                        change: PropertyData::Flag(i),
                        ..
                    } => {
                        idle = i;
                        None
                    }
                    Event::EndFile(reason) => match reason as i32 {
                        EOF => Some(InternalDeckEvent::TrackEnded { deck_id }),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(e) = out {
                    if tx.send(e).is_err() {
                        break;
                    }
                }
                let now = !paused && !idle;
                if now != playing {
                    playing = now;
                    if tx.send(InternalDeckEvent::Playing { deck_id, playing: now }).is_err() {
                        break;
                    }
                }
            }
            Some(Err(e)) => {
                if tx.send(InternalDeckEvent::TrackFailed {
                    deck_id,
                    error: friendly_error(&e),
                }).is_err() {
                    break;
                }
            }
            None => {}
        }
    }
}

fn arbitrate_events(
    active_deck: Arc<AtomicU8>,
    fading_deck: Arc<AtomicI8>,
    primed_deck: Arc<AtomicI8>,
    deck_a: Arc<Deck>,
    deck_b: Arc<Deck>,
    internal_rx: std::sync::mpsc::Receiver<InternalDeckEvent>,
    external_tx: tokio::sync::mpsc::UnboundedSender<PlayerEvent>,
) {
    while let Ok(event) = internal_rx.recv() {
        let active = active_deck.load(Ordering::SeqCst);
        let fading = fading_deck.load(Ordering::SeqCst);
        let primed = primed_deck.load(Ordering::SeqCst);

        match event {
            InternalDeckEvent::Position { deck_id, pos } => {
                if deck_id == active {
                    if external_tx.send(PlayerEvent::Position(pos)).is_err() {
                        break;
                    }
                }
            }
            InternalDeckEvent::Duration { deck_id, dur } => {
                if deck_id == active {
                    if external_tx.send(PlayerEvent::Duration(dur)).is_err() {
                        break;
                    }
                }
            }
            InternalDeckEvent::Playing { deck_id, playing } => {
                if deck_id == active {
                    if external_tx.send(PlayerEvent::Playing(playing)).is_err() {
                        break;
                    }
                }
            }
            InternalDeckEvent::TrackEnded { deck_id } => {
                if deck_id as i8 == fading {
                    // Outgoing track finished its fade-out cleanly! Stop it and reset fading deck.
                    let deck = if deck_id == 0 { &deck_a } else { &deck_b };
                    deck.stop();
                    fading_deck.store(-1, Ordering::SeqCst);
                } else if deck_id == active {
                    // Active track reached EOF (no crossfade happened, or gapless transition)
                    if external_tx.send(PlayerEvent::TrackEnded).is_err() {
                        break;
                    }
                }
            }
            InternalDeckEvent::TrackFailed { deck_id, error } => {
                if deck_id == active {
                    if external_tx.send(PlayerEvent::TrackFailed(error)).is_err() {
                        break;
                    }
                } else if deck_id as i8 == primed {
                    tracing::warn!(deck_id, %error, "primed deck failed to load track");
                    primed_deck.store(-1, Ordering::SeqCst);
                }
            }
        }
    }
}

/// Quote a filename/URL for mpv's command parser.
fn quoted(arg: &str) -> String {
    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Slider percent → mpv `volume` value, over a 60 dB range.
fn perceptual_to_mpv(percent: i64) -> f64 {
    if percent <= 0 {
        return 0.0;
    }
    100.0 * 10f64.powf(-(1.0 - percent.min(100) as f64 / 100.0).powf(1.5))
}

#[cfg(test)]
mod tests {
    use super::{af_chain, perceptual_to_mpv, quoted, AudioFilters};

    #[test]
    fn gain_and_pitch_share_one_chain() {
        let default_af = AudioFilters::default();
        assert_eq!(af_chain(&default_af), "");

        let gain_af = AudioFilters { gain_db: Some(-3.5), ..Default::default() };
        assert_eq!(af_chain(&gain_af), "lavfi=[volume=-3.50dB]");

        let pitch_af = AudioFilters { semitones: 12, ..Default::default() };
        assert_eq!(af_chain(&pitch_af), "rubberband=pitch-scale=2");

        let combined_af =
            AudioFilters { gain_db: Some(-6.0), semitones: -12, ..Default::default() };
        assert_eq!(af_chain(&combined_af), "lavfi=[volume=-6.00dB],rubberband=pitch-scale=0.5");

        let semi_af = AudioFilters { semitones: 1, ..Default::default() };
        assert!(af_chain(&semi_af).ends_with("1.0594630943592953"));
    }

    #[test]
    fn mpv_keeps_the_gain_through_pitch_changes_and_failures() {
        use super::{Error, Player, NO_RUBBERBAND};
        use std::sync::atomic::Ordering;

        let dir = std::env::temp_dir().join("nocturne-af-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = Player::new(dir.to_str().unwrap()).expect("libmpv");
        let dev = p.active_mpv().get_property::<String>("audio-device");
        println!("current audio-device: {:?}", dev);
        let dev_list_str = p.active_mpv().get_property::<String>("audio-device-list");
        println!("audio-device-list string: {:?}", dev_list_str);
        let af = || p.active_mpv().get_property::<String>("af").unwrap();

        // 1. Loudness normalization, then a pitch round trip.
        p.set_gain(Some(-7.7)).unwrap();
        assert!(
            af().contains("volume=-7.70dB") || af().contains("volume=-7.7dB"),
            "gain missing: {}",
            af()
        );
        p.set_pitch(2).unwrap();
        assert!(
            af().contains("volume=-7.70dB") || af().contains("volume=-7.7dB"),
            "pitch dropped the gain: {}",
            af()
        );
        assert!(af().contains("rubberband"), "pitch missing: {}", af());
        p.set_pitch(0).unwrap();
        assert!(
            af().contains("volume=-7.70dB") || af().contains("volume=-7.7dB"),
            "reset dropped the gain: {}",
            af()
        );
        assert!(!af().contains("rubberband"), "pitch 0 left a filter behind: {}", af());

        // 2. Gapless advance
        p.set_pitch(-5).unwrap();
        p.set_gain(Some(-2.5)).unwrap();
        assert!(
            af().contains("volume=-2.50dB") || af().contains("volume=-2.5dB"),
            "retune missed: {}",
            af()
        );
        assert!(af().contains("rubberband"), "retune dropped the pitch: {}", af());
        p.set_pitch(0).unwrap();

        // 3. Rejection rollback
        let before = af();
        NO_RUBBERBAND.store(true, Ordering::Relaxed);
        let err = p.set_pitch(3).unwrap_err();
        NO_RUBBERBAND.store(false, Ordering::Relaxed);
        assert!(matches!(err, Error::NoPitchFilter), "rejection must surface: {err}");
        assert_eq!(err.to_string(), "Pitch shifting isn't available in this build");
        assert_eq!(af(), before, "a rejected pitch changed the live chain");
        assert!(
            af().contains("volume=-2.50dB") || af().contains("volume=-2.5dB"),
            "normalization lost: {}",
            af()
        );
        p.set_gain(Some(-4.0)).unwrap();
        let after = af();
        assert!(
            after.contains("volume=-4.00dB") || after.contains("volume=-4dB"),
            "retune after a rejection failed: {after}"
        );
        assert!(!after.contains("rubberband"), "stored pitch survived the rollback: {after}");
    }

    #[test]
    fn paths_survive_mpvs_command_parser() {
        assert_eq!(quoted("/music/My music/a, b.mp3"), "\"/music/My music/a, b.mp3\"");
        assert_eq!(quoted(r#"/m/say "hi".mp3"#), r#""/m/say \"hi\".mp3""#);
        assert_eq!(quoted(r"C:\Music\x.mp3"), r#""C:\\Music\\x.mp3""#);
        assert_eq!(quoted("https://x/y?a=1&b=2"), "\"https://x/y?a=1&b=2\"");
    }

    #[test]
    fn volume_curve() {
        let db = |s| 60.0 * (perceptual_to_mpv(s) / 100.0).log10();
        assert_eq!(perceptual_to_mpv(0), 0.0);
        assert_eq!(perceptual_to_mpv(100), 100.0);
        assert!((db(50) + 21.21).abs() < 0.01);
        assert!((db(1) + 59.10).abs() < 0.01);
        assert!((1..=100).all(|s| perceptual_to_mpv(s) > perceptual_to_mpv(s - 1)));
        assert!(db(100) - db(99) < db(2) - db(1));
    }

    #[test]
    fn test_two_mpv_instances() {
        let dir1 = std::env::temp_dir().join("nocturne-test-deck-a");
        let dir2 = std::env::temp_dir().join("nocturne-test-deck-b");
        std::fs::create_dir_all(&dir1).unwrap();
        std::fs::create_dir_all(&dir2).unwrap();
        let p = super::Player::new(dir1.to_str().unwrap()).expect("dual deck player");
        assert!(p.is_idle());
        assert!(!p.can_crossfade());
    }
}
