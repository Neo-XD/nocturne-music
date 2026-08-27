//! Lyrics fetching. Provider chain (plan `graceful-kindling`):
//!
//! 1. **Better Lyrics** (`api.betterlyrics.org` / `lyrics-api.boidu.dev`) → syllable-by-syllable
//!    and word-level timings in TTML/AAML/JSON/eLRC, giving high-precision karaoke sweeps.
//! 2. **LRCLIB** `/api/get` (exact match) → synced LRC lyrics. Free, no key, best coverage.
//! 3. **YouTube Music timed** — `next(videoId)` → lyrics browseId → mobile-client browse
//!    (`timedLyricsData`). The same real-time lyrics the YTM app shows.
//! 4. **QQ Music / Kugou** → synced LRC. Search hits matched on length (`best_by_duration`).
//! 5. Plain fallbacks: LRCLIB fuzzy search → LRCLIB plain (from step 2's response) → YT plain
//!    (WEB_REMIX browse) → the fuzzy search's plain text.
//!
//! Results are cached in SQLite (`lyrics_cache`): hits forever, "no lyrics" verdicts for 24h.
//! A run where every provider merely *errored* (offline) caches nothing, so lyrics come back
//! when the network does. Everything is best-effort — a lyrics failure is never a user error.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// How long a cached "no lyrics found" verdict suppresses refetching.
const MISS_TTL_SECS: i64 = 24 * 3600;

const LRCLIB_ROOT: &str = "https://lrclib.net/api";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// One display line. `time_ms` present ⇔ the line is synced (a plain-lyrics response has none).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time_ms: Option<u64>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<LyricWord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

impl LyricLine {
    pub fn simple(time_ms: Option<u64>, text: String) -> Self {
        Self { time_ms, end_time_ms: None, text, words: None, translation: None }
    }
}

/// What the UI gets (and what `lyrics_cache` stores as JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    /// Attribution shown in the panel footer ("LRCLIB", "Better Lyrics", "YouTube Music", …).
    pub source: String,
    pub synced: bool,
    #[serde(default)]
    pub instrumental: bool,
    pub lines: Vec<LyricLine>,
}

pub struct LyricsRequest {
    pub video_id: String,
    pub title: String,
    pub artists: String,
    pub album: Option<String>,
    /// Track length in seconds (mpv's), tightens LRCLIB matching. `None`/0 when unknown yet.
    pub duration: Option<f64>,
}

/// `NOCTURNE_LYRICS_ONLY` / `LIMUSIC_LYRICS_ONLY=<betterlyrics|qq|kugou>` pins the chain to that one provider and bypasses
/// the cache both ways.
///
/// Unset (the default) leaves the chain exactly as it ships. Testing aid, not a user setting.
fn forced_provider() -> Option<String> {
    std::env::var("NOCTURNE_LYRICS_ONLY")
        .or_else(|_| std::env::var("LIMUSIC_LYRICS_ONLY"))
        .ok()
        .filter(|s| !s.is_empty())
}

pub const DEFAULT_PROVIDERS: [&str; 5] = ["betterlyrics", "lrclib", "ytm", "qq", "kugou"];

pub fn resolve_providers(db: &crate::db::Db) -> Vec<String> {
    if let Some(s) =
        db.get_setting("lyrics_providers").or_else(|| db.get_setting("lyrics_priority"))
    {
        let list: Vec<String> = if s.trim().starts_with('[') {
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            s.split(',').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect()
        };
        if !list.is_empty() {
            return list;
        }
    }
    DEFAULT_PROVIDERS.iter().map(|s| s.to_string()).collect()
}

/// Cache-through entry point for the `get_lyrics` command.
pub async fn get_lyrics(state: &AppState, req: LyricsRequest) -> Option<Lyrics> {
    let now = now_secs();
    let video_id = req.video_id.clone();
    let forced = forced_provider();
    if forced.is_none() {
        if let Some(cached) = state.db.get_lyrics(&video_id, now, MISS_TTL_SECS) {
            return cached.and_then(|json| serde_json::from_str(&json).ok());
        }
    }
    let (lyrics, cacheable) = fetch(state, req).await;
    if cacheable && forced.is_none() {
        let json = lyrics.as_ref().and_then(|l| serde_json::to_string(l).ok());
        state.db.put_lyrics(&video_id, json.as_deref(), now);
    }
    lyrics
}

/// Run the provider chain in user-configured priority order.
async fn fetch(state: &AppState, mut req: LyricsRequest) -> (Option<Lyrics>, bool) {
    let mut definitive = false;

    // 0. `next()` up front: it carries the lyrics browseId AND — via its seed item — the exact
    //    length of the cut this videoId plays.
    let next = if crate::local::is_local_song(&req.video_id) {
        None
    } else {
        match state
            .it
            .next(state.clients.get(innertube::METADATA_CLIENT).unwrap(), Some(&req.video_id), None)
            .await
        {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::debug!(error = %e, "lyrics: next() failed");
                None
            }
        }
    };
    let browse_id = next.as_ref().and_then(|n| n.lyrics_browse_id.clone());
    if req.duration.is_none() {
        req.duration = next.as_ref().and_then(|n| {
            let item = n.items.iter().find(|i| i.video_id == req.video_id)?;
            duration_str_secs(item.duration.as_deref()?)
        });
    }
    let req = &req;

    // Pinned to one provider: run it alone and report whatever it says, hit or miss.
    if let Some(only) = forced_provider() {
        let hit = match only.as_str() {
            "betterlyrics" | "boidu" => betterlyrics_get(req).await,
            "qq" => qqmusic_get(req).await,
            "kugou" => kugou_get(req).await,
            other => {
                tracing::warn!(provider = other, "NOCTURNE_LYRICS_ONLY: unknown provider");
                Ok(None)
            }
        };
        match &hit {
            Ok(Some(l)) => tracing::info!(provider = only, lines = l.lines.len(), "pinned: hit"),
            Ok(None) => tracing::info!(provider = only, "pinned: no lyrics"),
            Err(e) => tracing::warn!(provider = only, error = %e, "pinned: failed"),
        }
        return (hit.ok().flatten(), false);
    }

    let providers = resolve_providers(&state.db);
    let mut lr: Option<Result<Option<LrclibTrack>, reqwest::Error>> = None;

    for provider in &providers {
        match provider.as_str() {
            "betterlyrics" | "boidu" => {
                if state.db.get_setting("lyrics_boidu").as_deref() != Some("false")
                    && state.db.get_setting("lyrics_betterlyrics").as_deref() != Some("false")
                {
                    if let Ok(Some(l)) = betterlyrics_get(req).await {
                        return (Some(l), req.duration.is_some());
                    }
                }
            }
            "lrclib" => {
                let res = lrclib_get(req).await;
                if let Ok(hit) = &res {
                    definitive = true;
                    if let Some(l) = hit.as_ref().and_then(lrclib_to_lyrics) {
                        if l.synced || l.instrumental {
                            return (Some(l), req.duration.is_some());
                        }
                    }
                }
                lr = Some(res);
            }
            "ytm" | "youtube" | "youtubemusic" => {
                if next.is_some() {
                    definitive = true;
                }
                if let (Some(bid), Some(client)) =
                    (&browse_id, state.clients.get(innertube::LYRICS_TIMED_CLIENT))
                {
                    match state.it.lyrics_timed(client, bid).await {
                        Ok(lines) if !lines.is_empty() => {
                            return (
                                Some(Lyrics {
                                    source: "YouTube Music".into(),
                                    synced: true,
                                    instrumental: false,
                                    lines: lines
                                        .into_iter()
                                        .map(|l| LyricLine::simple(Some(l.time_ms), l.text))
                                        .collect(),
                                }),
                                true,
                            );
                        }
                        Ok(_) => {}
                        Err(e) => tracing::debug!(error = %e, "lyrics: timed browse failed"),
                    }
                }
            }
            "qq" | "qqmusic" => {
                if let Ok(Some(l)) = qqmusic_get(req).await {
                    return (Some(l), req.duration.is_some());
                }
            }
            "kugou" => {
                if let Ok(Some(l)) = kugou_get(req).await {
                    return (Some(l), req.duration.is_some());
                }
            }
            _ => {}
        }
    }

    // --- Fallbacks (fuzzy search & plain text) ---
    let allows_lrclib = providers.iter().any(|p| p == "lrclib");
    let allows_ytm = providers.iter().any(|p| p == "ytm" || p == "youtube" || p == "youtubemusic");

    let mut searched: Option<Result<Option<LrclibTrack>, reqwest::Error>> = None;
    if allows_lrclib {
        let s = lrclib_search(req).await;
        if let Ok(hit) = &s {
            definitive = true;
            if let Some(l) = hit.as_ref().and_then(lrclib_to_lyrics).filter(|l| l.synced) {
                return (Some(l), req.duration.is_some());
            }
        }
        searched = Some(s);
    }

    // Plain from LRCLIB exact match
    if allows_lrclib {
        if let Some(Ok(Some(hit))) = &lr {
            if let Some(l) = plain_from_text(hit.plain_lyrics.as_deref(), "LRCLIB") {
                return (Some(l), req.duration.is_some());
            }
        }
    }

    // Plain from YouTube Music (WEB_REMIX)
    if allows_ytm {
        if let Some(bid) = &browse_id {
            if let Some(client) = state.clients.get(innertube::METADATA_CLIENT) {
                match state.it.lyrics_plain(client, bid).await {
                    Ok(Some(p)) => {
                        let source = p.footer.unwrap_or_else(|| "YouTube Music".into());
                        if let Some(l) = plain_from_text(Some(&p.text), &source) {
                            return (Some(l), true);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::debug!(error = %e, "lyrics: plain browse failed"),
                }
            }
        }
    }

    // Plain from LRCLIB fuzzy search
    if allows_lrclib {
        if let Some(Ok(Some(hit))) = &searched {
            if let Some(l) = lrclib_to_lyrics(hit) {
                return (Some(l), req.duration.is_some());
            }
        }
    }

    (None, definitive)
}

// --- LRCLIB (https://lrclib.net/docs) -------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

/// LRCLIB asks integrations to identify themselves via User-Agent.
const LRCLIB_UA: &str =
    concat!("Nocturne v", env!("CARGO_PKG_VERSION"), " (https://github.com/Neo-XD/nocturne-music)");

/// A GET to LRCLIB, carrying the two things this API wants from us: who we are, and a bound on how
/// long we will wait. Both used to be baked into a client of our own.
fn get(url: String) -> reqwest::RequestBuilder {
    crate::http::client().get(url).header("User-Agent", LRCLIB_UA).timeout(Duration::from_secs(15))
}

/// `/api/get`: exact signature match. `Ok(None)` = definitive "not in LRCLIB" (404);
/// `Err` = transport trouble (don't cache a negative off it).
async fn lrclib_get(req: &LyricsRequest) -> Result<Option<LrclibTrack>, reqwest::Error> {
    let mut q: Vec<(&str, String)> =
        vec![("track_name", req.title.clone()), ("artist_name", req.artists.clone())];
    if let Some(album) = &req.album {
        q.push(("album_name", album.clone()));
    }
    if let Some(d) = req.duration.filter(|d| *d > 0.0) {
        q.push(("duration", format!("{}", d.round() as i64)));
    }
    let resp = get(format!("{LRCLIB_ROOT}/get")).query(&q).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(resp.error_for_status()?.json().await?))
}

/// `/api/search`: fuzzy fallback. Prefers a synced candidate whose duration is within ±5s of
/// ours (when known); returns the best or `Ok(None)`.
async fn lrclib_search(req: &LyricsRequest) -> Result<Option<LrclibTrack>, reqwest::Error> {
    let q = [("track_name", req.title.as_str()), ("artist_name", req.artists.as_str())];
    let list: Vec<LrclibTrack> = get(format!("{LRCLIB_ROOT}/search"))
        .query(&q)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ours = req.duration.filter(|d| *d > 0.0);
    // Distance from our track's length; unknown-length candidates rank last but aren't excluded.
    let dist = |t: &LrclibTrack| match (ours, t.duration) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => f64::INFINITY,
    };
    let close = |t: &LrclibTrack| ours.is_none() || dist(t) <= 5.0;
    let synced = |t: &LrclibTrack| t.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty());
    // Prefer the synced candidate whose duration is CLOSEST to ours — LRCLIB carries multiple
    // cuts of popular tracks, and a 4s-different cut plays lyrics 4s off the audio.
    let mut best_synced: Option<(f64, LrclibTrack)> = None;
    let mut best_plain: Option<LrclibTrack> = None;
    for t in list {
        if !close(&t) {
            continue;
        }
        if synced(&t) {
            let d = dist(&t);
            if best_synced.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best_synced = Some((d, t));
            }
        } else if best_plain.is_none() {
            best_plain = Some(t);
        }
    }
    Ok(best_synced.map(|(_, t)| t).or(best_plain))
}

/// Best `Lyrics` an LRCLIB track yields: instrumental > synced > plain > nothing.
fn lrclib_to_lyrics(t: &LrclibTrack) -> Option<Lyrics> {
    if t.instrumental {
        return Some(Lyrics {
            source: "LRCLIB".into(),
            synced: false,
            instrumental: true,
            lines: Vec::new(),
        });
    }
    if let Some(lrc) = t.synced_lyrics.as_deref().filter(|s| !s.trim().is_empty()) {
        let lines = parse_lrc(lrc);
        if !lines.is_empty() {
            return Some(Lyrics {
                source: "LRCLIB".into(),
                synced: true,
                instrumental: false,
                lines,
            });
        }
    }
    plain_from_text(t.plain_lyrics.as_deref(), "LRCLIB")
}

/// Plain text → un-timed lines (blank lines kept as stanza breaks).
fn plain_from_text(text: Option<&str>, source: &str) -> Option<Lyrics> {
    let text = text?.trim();
    if text.is_empty() {
        return None;
    }
    Some(Lyrics {
        source: source.to_owned(),
        synced: false,
        instrumental: false,
        lines: text.lines().map(|l| LyricLine::simple(None, l.trim_end().to_owned())).collect(),
    })
}

/// A provider's parsed lines as a result, or `None` when there was nothing to show.
///
/// `synced` is derived from the lines rather than asserted by the caller. TTML without `begin`
/// attributes, and JSON items carrying text but no time, both parse to real lines with no cue.
/// Declaring those synced puts the UI in its synced view, where no line ever highlights (none has
/// a cue to pass) and clicking one to seek does nothing: lyrics that look broken, rather than
/// lyrics that read as plain text.
fn from_parsed(source: &str, lines: Vec<LyricLine>) -> Option<Lyrics> {
    if lines.is_empty() {
        return None;
    }
    Some(Lyrics {
        source: source.to_owned(),
        // Any cue at all: an LRC with untimed credit or stanza lines is still a synced lyric.
        synced: lines.iter().any(|l| l.time_ms.is_some()),
        instrumental: false,
        lines,
    })
}

// --- LRC parsing ----------------------------------------------------------------------------

/// Parse LRC text (`[mm:ss.xx] line`) into sorted lines. Handles multiple timestamps per line
/// (`[t1][t2]text` — the line repeats at both cues) and skips metadata tags (`[ar:…]`).
/// Timestamped empty lines are kept: they're instrumental gaps the UI can show as such.
fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut out = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            let Some(end) = after.find(']') else { break };
            match parse_lrc_time(&after[..end]) {
                Some(ms) => {
                    times.push(ms);
                    rest = after[end + 1..].trim_start();
                }
                // Not a timestamp: a metadata tag ([ar:…] — no times yet, line skipped) or
                // bracketed lyric text ("[Chorus]" — keep it as the line's text).
                None => break,
            }
        }
        for &ms in &times {
            out.push(LyricLine::simple(Some(ms), rest.to_owned()));
        }
    }
    out.sort_by_key(|l| l.time_ms);
    out
}

/// `mm:ss`, `mm:ss.xx`, or `mm:ss.xxx` → milliseconds.
fn parse_lrc_time(tag: &str) -> Option<u64> {
    let (m, rest) = tag.split_once(':')?;
    let m: u64 = m.trim().parse().ok()?;
    let (s, frac) = match rest.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    let s: u64 = s.trim().parse().ok()?;
    let ms = match frac {
        Some(f) => {
            let digits: String = f.chars().filter(char::is_ascii_digit).take(3).collect();
            let val: u64 = digits.parse().ok()?;
            match digits.len() {
                1 => val * 100,
                2 => val * 10,
                _ => val,
            }
        }
        None => 0,
    };
    Some((m * 60 + s) * 1000 + ms)
}

/// `"3:21"` / `"1:02:03"` → seconds.
fn duration_str_secs(s: &str) -> Option<f64> {
    let mut total: u64 = 0;
    for part in s.split(':') {
        total = total * 60 + part.trim().parse::<u64>().ok()?;
    }
    (total > 0).then_some(total as f64)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// --- Additional Providers (Better Lyrics & Asian Catalogues) ------------------------------

/// Better Lyrics provider (Better Lyrics API / Boidu) - syllable-by-syllable & word-level sync
async fn betterlyrics_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let mut q: Vec<(&str, String)> = vec![("s", req.title.clone()), ("a", req.artists.clone())];
    if let Some(album) = &req.album {
        q.push(("al", album.clone()));
    }
    if let Some(d) = req.duration.filter(|d| *d > 0.0) {
        q.push(("d", format!("{}", d.round() as i64)));
    }

    let endpoints =
        ["https://lyrics-api.boidu.dev/getLyrics", "https://api.betterlyrics.org/getLyrics"];

    for url in endpoints {
        tracing::debug!(title = %req.title, artist = %req.artists, %url, "lyrics: querying Better Lyrics provider");
        let resp_opt: Option<serde_json::Value> = match crate::http::client()
            .get(url)
            .query(&q)
            .header("User-Agent", LRCLIB_UA)
            .timeout(Duration::from_secs(8))
            .send()
            .await
        {
            Ok(r) => r.json().await.ok(),
            Err(e) => {
                tracing::debug!(%url, error = %e, "lyrics: Better Lyrics request failed");
                None
            }
        };

        let Some(resp) = resp_opt else {
            continue;
        };

        let lrc_str = resp
            .get("ttml")
            .or_else(|| resp.get("syncedLyrics"))
            .or_else(|| resp.get("lyrics"))
            .or_else(|| resp.get("lrc"))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    if !s.trim().is_empty() {
                        return Some(s.to_string());
                    }
                }
                if v.is_array() {
                    return serde_json::to_string(v).ok();
                }
                None
            });

        if let Some(lrc) = lrc_str {
            let hit = from_parsed("Better Lyrics", parse_lrc_or_ttml(&lrc));
            if let Some(l) = hit {
                tracing::debug!(
                    count = l.lines.len(),
                    synced = l.synced,
                    "lyrics: Better Lyrics hit"
                );
                return Ok(Some(l));
            }
        }
    }

    Ok(None)
}

/// How far a search hit's length may sit from the track we're actually playing. Same tolerance the
/// LRCLIB search above uses, for the same reason.
const MATCH_TOLERANCE_SECS: f64 = 5.0;

/// Pick the search hit closest in length to what we're playing, rejecting anything further off than
/// `MATCH_TOLERANCE_SECS`.
///
/// The providers below rank remixes, live cuts and radio edits right next to the original
/// (Kugou's top hit for "Shape of You" is a 263s edit of a 233s song), so taking whatever came back
/// first plays lyrics seconds out of step with the audio. Closest-match rather than
/// first-within-tolerance matters: the remix is often inside the window too, and only the distance
/// separates it from the real cut.
///
/// With no length on our side there is nothing to check, so the first hit stands. A candidate whose
/// own length is missing ranks last but is not dropped — if a provider renames the field we want
/// degraded matching, not a provider that silently returns nothing.
fn best_by_duration<T>(
    ours: Option<f64>,
    cands: &[T],
    secs: impl Fn(&T) -> Option<f64>,
) -> Option<&T> {
    let Some(ours) = ours.filter(|d| *d > 0.0) else {
        return cands.first();
    };
    cands
        .iter()
        .map(|c| (secs(c).map_or(f64::INFINITY, |d| (d - ours).abs()), c))
        .filter(|(d, _)| *d <= MATCH_TOLERANCE_SECS || d.is_infinite())
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// QQ Music provider
async fn qqmusic_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let query = format!("{} {}", req.title, req.artists);
    let search_url = format!(
        "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?w={}&format=json",
        urlencoding::encode(&query)
    );
    let resp: serde_json::Value = match crate::http::client()
        .get(&search_url)
        .header("User-Agent", LRCLIB_UA)
        .header("Referer", "https://y.qq.com/")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };

    let songs = resp
        .pointer("/data/song/list")
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or_default();
    // QQ reports track length in whole seconds, as `interval`.
    let hit = best_by_duration(req.duration, songs, |s| s.get("interval")?.as_f64());
    let Some(mid) = hit.and_then(|s| s.get("songmid")).and_then(|v| v.as_str()) else {
        return Ok(None);
    };

    let lyric_url = format!(
        "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={mid}&format=json&nobase64=1"
    );
    let l_resp: serde_json::Value = match crate::http::client()
        .get(&lyric_url)
        .header("User-Agent", LRCLIB_UA)
        .header("Referer", "https://y.qq.com/")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };

    let mut lyric_raw = l_resp.get("lyric").and_then(|v| v.as_str()).unwrap_or("");
    let decoded;
    if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, lyric_raw)
    {
        if let Ok(s) = String::from_utf8(bytes) {
            decoded = s;
            lyric_raw = &decoded;
        }
    }
    Ok(from_parsed("QQ Music", parse_lrc_or_ttml(lyric_raw)))
}

/// Kugou provider
async fn kugou_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let query = format!("{} {}", req.title, req.artists);
    let search_url = format!(
        "https://songsearch.kugou.com/song_search_v2?keyword={}&page=1&pagesize=5",
        urlencoding::encode(&query)
    );
    let resp: serde_json::Value =
        match crate::http::client().get(&search_url).timeout(Duration::from_secs(8)).send().await {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };

    let songs = resp
        .pointer("/data/lists")
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or_default();
    // Kugou reports track length in whole seconds, as `Duration`.
    let hit = best_by_duration(req.duration, songs, |s| s.get("Duration")?.as_f64());
    let Some(h) = hit.and_then(|s| s.get("FileHash")).and_then(|v| v.as_str()) else {
        return Ok(None);
    };

    // `hash=`, not `h=`: the latter is not a parameter this endpoint knows, so it answered
    // "paramter_error: empty hash and keyword" for every track and the provider never returned
    // anything at all.
    let krc_url = format!("https://krcs.kugou.com/search?ver=1&man=yes&client=mobi&hash={h}");
    let krc_resp: serde_json::Value =
        match crate::http::client().get(&krc_url).timeout(Duration::from_secs(8)).send().await {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };

    let id = krc_resp.pointer("/candidates/0/id").and_then(|v| v.as_str());
    let accesskey = krc_resp.pointer("/candidates/0/accesskey").and_then(|v| v.as_str());
    let (Some(id_str), Some(key_str)) = (id, accesskey) else {
        return Ok(None);
    };

    let dl_url = format!(
        "https://lyrics.kugou.com/download?ver=1&client=pc&id={id_str}&accesskey={key_str}&fmt=lrc"
    );
    let dl_resp: serde_json::Value =
        match crate::http::client().get(&dl_url).timeout(Duration::from_secs(8)).send().await {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };

    let b64_content = dl_resp.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if let Ok(bytes) =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_content)
    {
        if let Ok(lrc_str) = String::from_utf8(bytes) {
            return Ok(from_parsed("Kugou", parse_lrc_or_ttml(&lrc_str)));
        }
    }
    Ok(None)
}

// --- TTML / AAML / eLRC Parsing & LRCMux ----------------------------------------------------

fn parse_time_val(v: &serde_json::Value) -> Option<u64> {
    if let Some(f) = v.as_f64() {
        if f < 500.0 {
            Some((f * 1000.0) as u64)
        } else {
            Some(f as u64)
        }
    } else if let Some(u) = v.as_u64() {
        if u < 500 {
            Some(u * 1000)
        } else {
            Some(u)
        }
    } else if let Some(s) = v.as_str() {
        if let Ok(f) = s.parse::<f64>() {
            if f < 500.0 {
                Some((f * 1000.0) as u64)
            } else {
                Some(f as u64)
            }
        } else {
            None
        }
    } else {
        None
    }
}

fn parse_lrc_or_ttml(text: &str) -> Vec<LyricLine> {
    let trimmed = text.trim();

    // 1. JSON Array / KPOE / LyricsPlus format
    if (trimmed.starts_with('[') || trimmed.starts_with('{'))
        && (trimmed.contains("\"text\"")
            || trimmed.contains("\"time\"")
            || trimmed.contains("\"words\"")
            || trimmed.contains("\"start\"")
            || trimmed.contains("\"startTime\""))
    {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let mut out = Vec::new();
            let arr_opt = val
                .as_array()
                .or_else(|| val.get("lyrics").and_then(|v| v.as_array()))
                .or_else(|| val.get("lines").and_then(|v| v.as_array()))
                .or_else(|| val.get("element").and_then(|v| v.as_array()));
            if let Some(arr) = arr_opt {
                for item in arr {
                    let line_text = item
                        .get("text")
                        .or_else(|| item.get("words"))
                        .or_else(|| item.get("line"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let time_val = item
                        .get("time")
                        .or_else(|| item.get("startTime"))
                        .or_else(|| item.get("start"))
                        .or_else(|| item.get("t"))
                        .and_then(parse_time_val);

                    // Parse inner word array if present
                    let mut words = Vec::new();
                    if let Some(w_arr) = item.get("words").and_then(|v| v.as_array()) {
                        for w in w_arr {
                            let w_text = w
                                .get("text")
                                .or_else(|| w.get("word"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let w_start = w
                                .get("startTime")
                                .or_else(|| w.get("start"))
                                .or_else(|| w.get("time"))
                                .and_then(parse_time_val)
                                .or(time_val);
                            let w_end = w
                                .get("endTime")
                                .or_else(|| w.get("end"))
                                .and_then(parse_time_val)
                                .or_else(|| w_start.map(|s| s + 500));
                            if let (Some(b), Some(e)) = (w_start, w_end) {
                                words.push(LyricWord { text: w_text, start_ms: b, end_ms: e });
                            }
                        }
                    }

                    if !line_text.is_empty() || time_val.is_some() {
                        out.push(LyricLine {
                            time_ms: time_val,
                            end_time_ms: None,
                            text: line_text,
                            words: if !words.is_empty() { Some(words) } else { None },
                            translation: None,
                        });
                    }
                }
                if !out.is_empty() {
                    out.sort_by_key(|l| l.time_ms);
                    return out;
                }
            }
        }
    }

    // 2. TTML / AAML XML
    if trimmed.starts_with('<') || trimmed.contains("<p ") || trimmed.contains("<tt") {
        let ttml_lines = parse_ttml_aaml(trimmed);
        if !ttml_lines.is_empty() {
            return ttml_lines;
        }
    }

    // 3. LRC / eLRC
    parse_elrc(text)
}

/// TTML and Apple Music AAML XML parser
fn parse_ttml_aaml(xml: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let mut pos = 0;
    while let Some(p_start) = xml[pos..].find("<p") {
        let abs_p_start = pos + p_start;
        let Some(p_tag_end) = xml[abs_p_start..].find('>') else {
            break;
        };
        let abs_p_tag_end = abs_p_start + p_tag_end;
        let p_tag_str = &xml[abs_p_start..abs_p_tag_end + 1];

        let Some(p_close) = xml[abs_p_tag_end..].find("</p>") else {
            break;
        };
        let abs_p_close = abs_p_tag_end + p_close;
        let inner_str = &xml[abs_p_tag_end + 1..abs_p_close];

        pos = abs_p_close + 4;

        let line_begin = parse_xml_attr(p_tag_str, "begin").and_then(|s| parse_ttml_time(&s));
        let line_end = parse_xml_attr(p_tag_str, "end").and_then(|s| parse_ttml_time(&s));

        let mut words: Vec<LyricWord> = Vec::new();
        let mut span_pos = 0;
        let mut plain_text_buf = String::new();

        while let Some(s_start) = inner_str[span_pos..].find("<span") {
            let abs_s_start = span_pos + s_start;
            let Some(s_tag_end) = inner_str[abs_s_start..].find('>') else {
                break;
            };
            let abs_s_tag_end = abs_s_start + s_tag_end;
            let s_tag_str = &inner_str[abs_s_start..abs_s_tag_end + 1];

            let before = strip_xml_tags(&inner_str[span_pos..abs_s_start]);
            if !before.is_empty() {
                plain_text_buf.push_str(&before);
                if let Some(last_w) = words.last_mut() {
                    last_w.text.push_str(&before);
                }
            }

            let Some(s_close) = inner_str[abs_s_tag_end..].find("</span>") else {
                break;
            };
            let abs_s_close = abs_s_tag_end + s_close;
            let w_text = strip_xml_tags(&inner_str[abs_s_tag_end + 1..abs_s_close]);

            let w_begin =
                parse_xml_attr(s_tag_str, "begin").and_then(|s| parse_ttml_time(&s)).or(line_begin);
            let w_end =
                parse_xml_attr(s_tag_str, "end").and_then(|s| parse_ttml_time(&s)).or(line_end);

            if let (Some(b), Some(e)) = (w_begin, w_end) {
                if !w_text.is_empty() {
                    words.push(LyricWord { text: w_text.clone(), start_ms: b, end_ms: e });
                }
            }
            plain_text_buf.push_str(&w_text);
            span_pos = abs_s_close + 7;
        }

        if span_pos < inner_str.len() {
            plain_text_buf.push_str(&strip_xml_tags(&inner_str[span_pos..]));
        }

        let words_opt = if !words.is_empty() { Some(words) } else { None };
        let full_text = plain_text_buf.trim().to_string();
        if !full_text.is_empty() || line_begin.is_some() {
            lines.push(LyricLine {
                time_ms: line_begin,
                end_time_ms: line_end,
                text: full_text,
                words: words_opt,
                translation: None,
            });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

/// Enhanced LRC parser (line timestamps + word inline timestamp tags)
fn parse_elrc(lrc: &str) -> Vec<LyricLine> {
    let mut base_lines = parse_lrc(lrc);
    for line in &mut base_lines {
        if line.text.contains('<') || line.text.contains('(') {
            let mut words = Vec::new();
            let mut text_buf = String::new();
            let mut last_ms = line.time_ms.unwrap_or(0);

            let mut pos = 0;
            let text_bytes = line.text.as_bytes();
            while pos < text_bytes.len() {
                if text_bytes[pos] == b'<' {
                    if let Some(end_idx) = line.text[pos..].find('>') {
                        let tag = &line.text[pos + 1..pos + end_idx];
                        if let Some(w_ms) = parse_lrc_time(tag) {
                            pos += end_idx + 1;
                            let next_tag_idx = line.text[pos..]
                                .find('<')
                                .map(|i| pos + i)
                                .unwrap_or(line.text.len());
                            let w_str = &line.text[pos..next_tag_idx];
                            text_buf.push_str(w_str);
                            words.push(LyricWord {
                                text: w_str.to_string(),
                                start_ms: last_ms,
                                end_ms: w_ms,
                            });
                            last_ms = w_ms;
                            pos = next_tag_idx;
                            continue;
                        }
                    }
                }
                // `pos` is a BYTE offset, so step by the character's own width. Indexing it as a
                // char offset silently mangles every non-ASCII line (and the CJK providers below
                // are where word timings mostly come from). Every other jump above lands on an
                // ASCII `<`/`>`, so slicing here is always on a char boundary.
                let ch = line.text[pos..].chars().next().unwrap_or(' ');
                text_buf.push(ch);
                pos += ch.len_utf8();
            }

            if !words.is_empty() {
                line.text = text_buf.trim().to_string();
                line.words = Some(words);
            }
        }
    }
    base_lines
}

/// LRCMux multiplexer: merges line lyrics with word timing or translations
#[cfg(test)]
fn lrc_mux(mut primary: Vec<LyricLine>, word_source: Vec<LyricLine>) -> Vec<LyricLine> {
    if word_source.is_empty() {
        return primary;
    }
    for p in &mut primary {
        let Some(p_time) = p.time_ms else {
            continue;
        };
        let best = word_source.iter().find(|ws| {
            if let Some(ws_time) = ws.time_ms {
                (p_time as i64 - ws_time as i64).abs() <= 800
            } else {
                false
            }
        });
        if let Some(ws) = best {
            if p.words.is_none() && ws.words.is_some() {
                p.words = ws.words.clone();
            }
            if p.translation.is_none() && ws.translation.is_some() {
                p.translation = ws.translation.clone();
            }
            if p.end_time_ms.is_none() && ws.end_time_ms.is_some() {
                p.end_time_ms = ws.end_time_ms;
            }
        }
    }
    primary
}

fn parse_ttml_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(rest) = s.strip_suffix("ms") {
        return rest.parse::<u64>().ok();
    }
    if let Some(rest) = s.strip_suffix('s') {
        let secs: f64 = rest.parse().ok()?;
        return Some((secs * 1000.0) as u64);
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 3 {
            let h: u64 = parts[0].parse().ok()?;
            let m: u64 = parts[1].parse().ok()?;
            let secs: f64 = parts[2].parse().ok()?;
            return Some((h * 3600 + m * 60) * 1000 + (secs * 1000.0) as u64);
        } else if parts.len() == 2 {
            let m: u64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            return Some(m * 60 * 1000 + (secs * 1000.0) as u64);
        }
    }
    let secs: f64 = s.parse().ok()?;
    Some((secs * 1000.0) as u64)
}

fn parse_xml_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    if let Some(idx) = tag.find(&pattern) {
        let start = idx + pattern.len();
        let end = tag[start..].find('"')?;
        return Some(tag[start..start + end].to_string());
    }
    let pattern_single = format!("{attr}='");
    if let Some(idx) = tag.find(&pattern_single) {
        let start = idx + pattern_single.len();
        let end = tag[start..].find('\'')?;
        return Some(tag[start..start + end].to_string());
    }
    None
}

fn strip_xml_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lrc() {
        let lrc = "[ar:Fleetwood Mac]\n[00:27.93] Listen to the wind blow\n[00:31.16] Watch the sun rise\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, Some(27930));
        assert_eq!(lines[0].text, "Listen to the wind blow");
        assert_eq!(lines[1].time_ms, Some(31160));
    }

    #[test]
    fn multi_timestamp_line_repeats() {
        let lines = parse_lrc("[00:10.00][01:10.00]la la la");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, Some(10000));
        assert_eq!(lines[1].time_ms, Some(70000));
        assert!(lines.iter().all(|l| l.text == "la la la"));
    }

    #[test]
    fn keeps_bracketed_lyric_text_and_gap_lines() {
        let lines = parse_lrc("[00:05.5][Chorus] yeah\n[00:20.123]\n[00:30] plain seconds");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time_ms, Some(5500));
        assert_eq!(lines[0].text, "[Chorus] yeah");
        assert_eq!(lines[1].time_ms, Some(20123));
        assert_eq!(lines[1].text, "");
        assert_eq!(lines[2].time_ms, Some(30000));
    }

    #[test]
    fn plain_text_splits_lines() {
        let l = plain_from_text(Some("one\ntwo\n\nthree"), "LRCLIB").unwrap();
        assert!(!l.synced);
        assert_eq!(l.lines.len(), 4);
        assert_eq!(l.lines[2].text, "");
    }

    #[test]
    fn parses_ttml_aaml_word_timestamps() {
        let xml = r#"<tt><p begin="00:10.500" end="00:14.200"><span begin="00:10.500" end="00:11.200">Hello </span><span begin="00:11.200" end="00:12.100">world </span></p></tt>"#;
        let lines = parse_ttml_aaml(xml);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].time_ms, Some(10500));
        assert_eq!(lines[0].end_time_ms, Some(14200));
        assert_eq!(lines[0].text, "Hello world");
        let words = lines[0].words.as_ref().unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello ");
        assert_eq!(words[0].start_ms, 10500);
        assert_eq!(words[0].end_ms, 11200);
    }

    #[test]
    fn parses_elrc_inline_word_timestamps() {
        let lrc = "[00:10.50]<00:10.50>Hello <00:11.20>world";
        let lines = parse_elrc(lrc);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].time_ms, Some(10500));
        assert_eq!(lines[0].text, "Hello world");
        let words = lines[0].words.as_ref().unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello ");
        assert_eq!(words[1].text, "world");
    }

    #[test]
    fn resolve_providers_handles_custom_priority_and_fallbacks() {
        let db = crate::db::Db::open(std::path::Path::new(":memory:")).unwrap();
        // Default when unset
        assert_eq!(resolve_providers(&db), vec!["betterlyrics", "lrclib", "ytm", "qq", "kugou"]);

        // Comma separated list
        db.set_setting("lyrics_providers", "lrclib, betterlyrics, kugou");
        assert_eq!(resolve_providers(&db), vec!["lrclib", "betterlyrics", "kugou"]);

        // JSON list
        db.set_setting("lyrics_providers", "[\"ytm\", \"qq\"]");
        assert_eq!(resolve_providers(&db), vec!["ytm", "qq"]);
    }

    #[test]
    fn elrc_keeps_non_ascii_text_intact() {
        // Text before the first word tag goes through the char-by-char path, which used to walk
        // byte offsets as if they were char offsets and shredded anything multi-byte.
        let lines = parse_elrc("[00:12.00]私は<00:12.50>歌う");
        assert_eq!(lines[0].text, "私は歌う");
        let words = lines[0].words.as_ref().unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "歌う");
        assert_eq!(words[0].end_ms, 12500);
    }

    #[test]
    fn from_parsed_derives_synced_from_the_lines() {
        // TTML with no `begin` parses to real lines carrying no cue. Declaring those synced is
        // what put the UI in a synced view whose highlight could never move.
        let untimed = parse_lrc_or_ttml("<tt><body><div><p>no timing here</p></div></body></tt>");
        assert!(!untimed.is_empty());
        assert!(!from_parsed("X", untimed).unwrap().synced);

        // Same for a JSON payload whose items carry text but no time.
        let json = parse_lrc_or_ttml(r#"[{"text":"one"},{"text":"two"}]"#);
        assert!(!json.is_empty());
        assert!(!from_parsed("X", json).unwrap().synced);

        let timed = parse_lrc_or_ttml("[00:01.00]one\n[00:02.00]two");
        assert!(from_parsed("X", timed).unwrap().synced);

        assert!(from_parsed("X", Vec::new()).is_none());
    }

    /// Real search shapes: the original is not first, and a remix sits inside the
    /// tolerance window, so only closest-match picks the right cut.
    #[test]
    fn best_by_duration_skips_remixes_and_wrong_cuts() {
        let secs = |t: &(f64, &str)| Some(t.0);
        // Kugou's actual top hit for "Shape of You" is a 263s edit of a 233s song.
        let kugou = [(263.0, "wrong cut"), (251.0, "dj edit"), (233.0, "original")];
        assert_eq!(best_by_duration(Some(233.0), &kugou, secs).unwrap().1, "original");
        // Ranked remix second: both inside 5s window, closest distance breaks the tie.
        let remix_cands = [(233.7, "original"), (231.2, "club remix")];
        assert_eq!(best_by_duration(Some(233.0), &remix_cands, secs).unwrap().1, "original");
        // Nothing close enough beats a wrong answer.
        assert!(best_by_duration(Some(233.0), &kugou[..2], secs).is_none());
        // No length on our side: nothing to check, first hit stands.
        assert_eq!(best_by_duration(None, &kugou, secs).unwrap().1, "wrong cut");
        // A hit with no length of its own still gets used rather than silently dropped.
        let unknown = [(0.0, "no duration")];
        let none = |_: &(f64, &str)| None;
        assert_eq!(best_by_duration(Some(233.0), &unknown, none).unwrap().1, "no duration");
    }

    #[test]
    fn lrc_mux_combines_lines_and_word_sources() {
        let primary = vec![LyricLine::simple(Some(10000), "Hello world".into())];
        let word_source = vec![LyricLine {
            time_ms: Some(10100),
            end_time_ms: Some(14000),
            text: "Hello world".into(),
            words: Some(vec![LyricWord { text: "Hello ".into(), start_ms: 10100, end_ms: 12000 }]),
            translation: Some("Halo dunia".into()),
        }];
        let muxed = lrc_mux(primary, word_source);
        assert_eq!(muxed.len(), 1);
        assert!(muxed[0].words.is_some());
        assert_eq!(muxed[0].translation.as_deref(), Some("Halo dunia"));
    }

    /// Are the external providers still alive? Hits all three for real, so it is NOT in the default
    /// run (context/17: network tests are opt-in, or `cargo test` fails offline):
    ///   cargo test -p nocturne-app --lib -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "hits live lyrics APIs"]
    async fn providers_are_alive() {
        let req = LyricsRequest {
            video_id: "test".into(),
            title: "Shape of You".into(),
            artists: "Ed Sheeran".into(),
            album: None,
            duration: Some(233.0),
        };
        let mut alive = 0;
        for (name, hit) in [
            ("Better Lyrics", betterlyrics_get(&req).await),
            ("QQ Music", qqmusic_get(&req).await),
            ("Kugou", kugou_get(&req).await),
        ] {
            match hit {
                Ok(Some(l)) => {
                    println!("{name}: {} lines, synced={}", l.lines.len(), l.synced);
                    assert_eq!(l.source, name);
                    assert!(!l.lines.is_empty());
                    alive += 1;
                }
                // Transport errors never reach here: the providers collapse them into Ok(None),
                // which is exactly why a dead one is invisible in normal use.
                Ok(None) => println!("{name}: NO HIT"),
                Err(e) => println!("{name}: ERROR {e}"),
            }
        }
        assert!(alive > 0, "no lyrics from any provider (offline?)");

        // Better Lyrics carries syllable/per-word timings, which the karaoke sweep renders.
        let bl = betterlyrics_get(&req).await.unwrap().expect("Better Lyrics hit");
        assert!(bl.lines.iter().any(|l| l.words.is_some()));
    }
}
