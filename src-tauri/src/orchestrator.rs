//! The brain: videoId → a playable stream. Full context/06 algorithm.
//!
//! Phase 2: WEB_REMIX is the primary client (STS + PoToken + cipher/n-transform), with the
//! direct-URL clients (VISIONOS → ANDROID_VR → IOS) as graceful fallback and rustypipe as the
//! last-ditch net. The context/06 critical behaviors are preserved: metadata from MAIN, the
//! per-videoId WEB_REMIX failure memory, the HIGH two-pass, off-hot-path self-heal, and graceful
//! PoToken/cipher degradation. Every client is HEAD-validated (see the note in `resolve`); for an
//! upload a failed HEAD demotes the URL instead of rejecting it, because there is no anonymous
//! chain behind an upload to fall through to.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use innertube::{
    find_format, find_video_format, rustypipe_fallback, AudioQuality, Clients, Format, InnerTube,
    PlayerResponse, MAIN_CLIENT, STREAM_FALLBACK_ORDER, UPLOAD_FALLBACK_ORDER,
};
use tokio::sync::Mutex;

use crate::cipher::CipherDeobfuscator;
use crate::potoken::PoTokenGenerator;

/// Everything the player + UI + media layer need for one track. context/06 PlaybackData.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaybackData {
    pub video_id: String,
    pub stream_url: String,
    pub itag: i64,
    /// HTTP headers mpv must send (User-Agent; Phase 3 adds Cookie).
    #[serde(skip)]
    pub headers: std::collections::HashMap<String, String>,
    pub expires_in_seconds: i64,
    pub loudness_db: Option<f64>,
    /// Where to register this play in watch history (context/01). `None` when no client that
    /// answered carried the tracking block.
    pub playback_ping: Option<PlaybackPing>,
    pub title: Option<String>,
    pub artists: Option<String>,
    pub duration: Option<String>,
    pub thumbnail: Option<String>,
    /// YouTube's own `musicVideoType` for this videoId: `Some(true)` = a video upload, `Some(false)`
    /// = the generated audio track, `None` = the metadata client never answered. The player view's
    /// music-video mode believes this over the queue row's flag, which several rows arrive without
    /// (a card played from a shelf, a Listen Together mirror, an album row swapped to its audio id).
    pub is_video: Option<bool>,
    /// Which client produced the stream (diagnostics). context/06.
    pub stream_client: String,
    /// Human-readable audio quality label (e.g. "OPUS 160 kbps", "AAC 256 kbps", "FLAC Lossless").
    pub audio_quality: Option<String>,
}

/// The watch-history ping for one play: `playbackTracking.videostatsPlaybackUrl.baseUrl` plus the
/// registry key of the client whose `/player` response carried it (context/01 §registerPlayback).
///
/// The two travel together because the ping's `c=` param, and its headers, have to be the client
/// that was issued the URL. Reading the URL off one client's response and sending it as another's
/// is what YouTube sees as a mismatch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaybackPing {
    pub url: String,
    pub client: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("no client could resolve a playable stream for {0}")]
    AllClientsFailed(String),
    /// One of the user's own uploads that no authenticated client would stream. Distinct from
    /// `AllClientsFailed` because "unavailable" reads as "the song is gone", and the likely cause
    /// here is a session that needs signing in again. Issue #71.
    #[error("this upload could not be played. Try signing in to YouTube Music again ({0})")]
    UploadUnavailable(String),
    /// Every client YouTube answered for this track wanted an account, and there is no session.
    /// Distinct from `AllClientsFailed` because the user can fix this one: some networks and
    /// regions get `LOGIN_REQUIRED` from every anonymous client, and signing in is the whole fix
    /// (issue #292).
    #[error("YouTube would not serve {0} without an account. Sign in from the account menu.")]
    SignInRequired(String),
    /// A local file that was in the library but is no longer on disk (context: local.rs).
    #[error("this file is no longer on your disk: {0}")]
    LocalMissing(String),
    /// Nothing answered at all: no client's `/player` call came back, and neither did the
    /// rustypipe net. That is the network, a dead proxy or a captive portal, and it says nothing
    /// about this particular track, so the queue must not skip past it or drop it.
    #[error("could not reach YouTube. Check your connection and try again ({0})")]
    Unreachable(String),
}

impl ResolveError {
    /// Would every other track in the queue fail this way too? A caller deciding whether to skip
    /// forward, or to delete a row, has to know: skipping is right for a track YouTube refused and
    /// wrong for an outage, where it walks the whole queue and deletes what it passes.
    pub fn affects_every_track(&self) -> bool {
        matches!(self, ResolveError::Unreachable(_) | ResolveError::SignInRequired(_))
    }
}

/// Client keys that need the `n`-transform applied to their stream URLs. context/06.
const NEEDS_N_TRANSFORM: [&str; 5] =
    ["WEB", "WEB_REMIX", "WEB_CREATOR", "TVHTML5", "TVHTML5_SIMPLY"];

// WEB_REMIX is validated with a HEAD like every other client — see `validate_head`.

/// A remembered best-but-not-ideal stream, for the HIGH two-pass (context/06 §4).
struct Candidate {
    format: Format,
    url: String,
    expires: i64,
    client: String,
    ping: Option<PlaybackPing>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientStats {
    pub key: String,
    pub latency_ms: f64,
    pub success_count: u32,
    pub failure_count: u32,
    pub penalty: f64,
}

/// Adaptive stream client latency & health tracker
pub struct ClientRanker {
    stats: Arc<Mutex<HashMap<String, ClientStats>>>,
}

impl ClientRanker {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            "VISIONOS".to_string(),
            ClientStats {
                key: "VISIONOS".to_string(),
                latency_ms: 130.0,
                success_count: 1,
                failure_count: 0,
                penalty: 0.0,
            },
        );
        map.insert(
            "TVHTML5_SIMPLY".to_string(),
            ClientStats {
                key: "TVHTML5_SIMPLY".to_string(),
                latency_ms: 180.0,
                success_count: 1,
                failure_count: 0,
                penalty: 0.0,
            },
        );
        Self { stats: Arc::new(Mutex::new(map)) }
    }

    /// Record resolution success with EMA latency update (alpha = 0.3)
    pub async fn record_success(&self, key: &str, latency_ms: f64) {
        let mut map = self.stats.lock().await;
        let entry = map.entry(key.to_string()).or_insert_with(|| ClientStats {
            key: key.to_string(),
            latency_ms,
            success_count: 0,
            failure_count: 0,
            penalty: 0.0,
        });
        entry.success_count += 1;
        entry.latency_ms = (entry.latency_ms * 0.7 + latency_ms * 0.3).max(10.0);
        entry.penalty = (entry.penalty - 50.0).max(0.0);
    }

    /// Record a resolution or 403 failure, adding a temporary latency penalty
    pub async fn record_failure(&self, key: &str) {
        let mut map = self.stats.lock().await;
        let entry = map.entry(key.to_string()).or_insert_with(|| ClientStats {
            key: key.to_string(),
            latency_ms: 300.0,
            success_count: 0,
            failure_count: 0,
            penalty: 0.0,
        });
        entry.failure_count += 1;
        entry.penalty += 500.0;
    }

    /// Get ranked fallback candidate keys sorted by effective score (latency + penalty) or custom priority
    pub async fn get_ranked_stream_clients(
        &self,
        disabled: &HashSet<String>,
        custom_priority: Option<&[String]>,
        auto_rank: bool,
    ) -> Vec<String> {
        let map = self.stats.lock().await;

        let base_order: Vec<String> = if let Some(custom) = custom_priority {
            let mut list: Vec<String> =
                custom.iter().filter(|k| !disabled.contains(*k)).cloned().collect();
            for key in STREAM_FALLBACK_ORDER {
                if !disabled.contains(key) && !list.iter().any(|x| x.as_str() == key) {
                    list.push(key.to_string());
                }
            }
            list
        } else {
            STREAM_FALLBACK_ORDER
                .iter()
                .filter(|k| !disabled.contains(**k))
                .map(|k| k.to_string())
                .collect()
        };

        if !auto_rank {
            return base_order;
        }

        let mut candidates: Vec<(String, f64)> = base_order
            .into_iter()
            .map(|k| {
                let score = map.get(&k).map_or(200.0, |s| s.latency_ms + s.penalty);
                (k, score)
            })
            .collect();

        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.into_iter().map(|(k, _)| k).collect()
    }

    /// Run concurrent prewarm probes against candidate stream clients on startup
    pub async fn benchmark(&self, it: &InnerTube, clients: &Clients, video_id: &str) {
        let candidates = STREAM_FALLBACK_ORDER;
        let mut tasks = Vec::new();

        for &key in &candidates {
            if let Some(client) = clients.get(key) {
                let it = it.clone();
                let client = client.clone();
                let key_str = key.to_string();
                let video_id_str = video_id.to_string();
                tasks.push(tokio::spawn(async move {
                    let start = Instant::now();
                    let res = tokio::time::timeout(
                        Duration::from_millis(3000),
                        it.player(&client, &video_id_str, None, None, None),
                    )
                    .await;
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    match res {
                        Ok(Ok(resp)) if resp.playability_status.is_ok() => {
                            Some((key_str, elapsed, true))
                        }
                        _ => Some((key_str, elapsed, false)),
                    }
                }));
            }
        }

        for task in tasks {
            if let Ok(Some((key, elapsed, ok))) = task.await {
                if ok {
                    tracing::info!(client = %key, latency_ms = elapsed, "stream client benchmark probe ok");
                    self.record_success(&key, elapsed).await;
                } else {
                    tracing::warn!(client = %key, "stream client benchmark probe failed");
                    self.record_failure(&key).await;
                }
            }
        }
    }

    /// Status for UI and diagnostics
    pub async fn get_stats(&self) -> Vec<ClientStats> {
        let map = self.stats.lock().await;
        let mut list: Vec<ClientStats> = map.values().cloned().collect();
        list.sort_by(|a, b| {
            (a.latency_ms + a.penalty)
                .partial_cmp(&(b.latency_ms + b.penalty))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    }
}

pub struct Orchestrator {
    it: InnerTube,
    clients: Clients,
    cipher: Arc<CipherDeobfuscator>,
    potoken: Arc<PoTokenGenerator>,
    pub ranker: Arc<ClientRanker>,
    /// videoId → when its WEB_REMIX stream last 403'd on the real GET, so the next resolve skips
    /// WEB_REMIX for it (context/06 §2). Cleared when the cipher self-heals. `Arc` so the
    /// off-hot-path self-heal task can clear it. Entries expire: the bar only has to survive the
    /// retry that follows the failure, and a permanent one meant a single bad minute cost that
    /// track its best client for the rest of the session.
    web_remix_failed: Arc<Mutex<HashMap<String, Instant>>>,
}

const WEB_REMIX_BLACKLIST_TTL: Duration = Duration::from_secs(30 * 60);

/// Record a failure, dropping expired entries on the way so the map cannot grow.
fn blacklist_insert(map: &mut HashMap<String, Instant>, video_id: &str, now: Instant) {
    map.retain(|_, at| now.duration_since(*at) < WEB_REMIX_BLACKLIST_TTL);
    map.insert(video_id.to_owned(), now);
}

/// Is WEB_REMIX still barred for this id? An entry past the TTL counts as absent.
fn blacklist_blocks(map: &HashMap<String, Instant>, video_id: &str, now: Instant) -> bool {
    map.get(video_id).is_some_and(|at| now.duration_since(*at) < WEB_REMIX_BLACKLIST_TTL)
}

impl Orchestrator {
    pub fn new(
        it: InnerTube,
        clients: Clients,
        cipher: Arc<CipherDeobfuscator>,
        potoken: Arc<PoTokenGenerator>,
    ) -> Self {
        Orchestrator {
            it,
            clients,
            cipher,
            potoken,
            ranker: Arc::new(ClientRanker::new()),
            web_remix_failed: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Background benchmark probe across all stream candidate clients on boot
    pub async fn benchmark_boot(&self) {
        self.ranker.benchmark(&self.it, &self.clients, "dQw4w9WgXcQ").await;
    }

    /// Live client statistics and latencies for UI and diagnostics
    pub async fn get_client_stats(&self) -> Vec<ClientStats> {
        self.ranker.get_stats().await
    }

    /// Record that a WEB_REMIX stream for `video_id` failed on the real GET (called by the player
    /// layer on a playback 403). The next resolve for this id bypasses WEB_REMIX. context/06 §2.
    pub async fn mark_web_remix_failed(&self, video_id: &str) {
        self.ranker.record_failure(MAIN_CLIENT).await;
        blacklist_insert(&mut *self.web_remix_failed.lock().await, video_id, Instant::now());
    }

    /// Resolve a videoId to a playable stream. context/06 full algorithm.
    pub async fn resolve(
        &self,
        video_id: &str,
        is_upload: bool,
        quality: AudioQuality,
        disabled: &HashSet<String>,
        custom_priority: Option<&[String]>,
        auto_rank: bool,
    ) -> Result<PlaybackData, ResolveError> {
        let prefer_high =
            matches!(quality, AudioQuality::VeryHigh | AudioQuality::High | AudioQuality::Auto);
        let logged_in = self.it.is_logged_in();
        let visitor = self.it.visitor_data();
        let ranked_clients: Vec<String> = if is_upload {
            UPLOAD_FALLBACK_ORDER.iter().map(|s| s.to_string()).collect()
        } else {
            self.ranker.get_ranked_stream_clients(disabled, custom_priority, auto_rank).await
        };
        let order: &[String] = &ranked_clients;
        // Without the uploads-playlist context YouTube hands back upload URLs that expire in about
        // 32 seconds (Metrolist PR #3857). Harmless for ordinary tracks, so scoped to uploads.
        let playlist_id = is_upload.then_some("MLPT");

        // 1. Signature timestamp from the deciphering player.js (context/05).
        let sts = self.cipher.signature_timestamp().await;

        // 2. Session PoToken for any client in the chain that needs one (context/04). Cached in
        // Rust with its TTL, so this is usually free; may be None (timeout / broken webview) —
        // degrade gracefully. Computed whenever ANY client in the chain asks for it, not just MAIN.
        let wants_pot = !is_upload
            && (self.clients.get(MAIN_CLIENT).is_some_and(|c| c.use_web_po_tokens)
                || order
                    .iter()
                    .any(|k| self.clients.get(k.as_str()).is_some_and(|c| c.use_web_po_tokens)));
        let session_pot_owned = match &visitor {
            Some(vd) if wants_pot => self.potoken.get_session_po_token(vd).await,
            _ => None,
        };
        let session_pot = session_pot_owned.as_deref();

        // 3. Main request as WEB_REMIX (metadata source even when a fallback wins the stream).
        let mut main_resp = match self.clients.get(MAIN_CLIENT) {
            Some(c) if !disabled.contains(MAIN_CLIENT) => {
                self.it.player(c, video_id, playlist_id, sts, session_pot).await.ok()
            }
            _ => None,
        };

        // Which client `main_resp` actually came from — WEB_CREATOR can replace it just below, and
        // a tracking URL has to be pinged as the client that was issued it.
        let mut main_key = MAIN_CLIENT;

        // Age/login gate on WEB_REMIX → retry with WEB_CREATOR (login-only). context/06 §4, seam #7.
        // ponytail: WEB_CREATOR streams are ciphered, so this depends on the whole web path working
        // (decipher, then a PoToken googlevideo accepts). Both do since 2026-08-25 (KI-1), so an
        // age-gated track now has a real chance here; when the path fails it still falls through to
        // the direct clients / rustypipe exactly as before.
        if logged_in && main_resp.as_ref().is_some_and(|r| r.playability_status.is_age_gated()) {
            if let Some(cc) = self.clients.get("WEB_CREATOR") {
                let cc_pot = if cc.use_web_po_tokens { session_pot } else { None };
                let cc_sts = if cc.use_signature_timestamp { sts } else { None };
                tracing::info!(video_id, "WEB_REMIX age/login-gated → retrying WEB_CREATOR");
                if let Ok(r) = self.it.player(cc, video_id, playlist_id, cc_sts, cc_pot).await {
                    main_resp = Some(r);
                    main_key = "WEB_CREATOR";
                }
            }
        }

        let main_ok = main_resp.as_ref().is_some_and(|r| r.playability_status.is_ok());
        let has_high = main_resp
            .as_ref()
            .and_then(|r| r.streaming_data.as_ref())
            .is_some_and(|s| s.adaptive_formats.iter().any(is_high));
        let mut audio_config_loudness = main_resp.as_ref().and_then(main_loudness);
        // Prefer main's tracking block: a ping sent as WEB_REMIX is what registers the play as a
        // YouTube *Music* one. But `playbackTracking` is only present on an OK response, so when
        // main degraded (no PoToken, age gate, a stale cipher) it isn't there at all and the play
        // would go unregistered even though a fallback client streamed it fine. Take that client's
        // block instead — it carries the same `docid`/`ei`/`of` for this videoId. Issue #83.
        let main_ping = main_resp.as_ref().and_then(|r| playback_ping(r, main_key));

        // 4. Fallback loop. idx == -1 reuses the main response; 0.. are the fallback clients.
        let mut best: Option<Candidate> = None;
        // A login client's upload URL that failed HEAD. Used only if nothing validates.
        let mut upload_fallback: Option<Candidate> = None;
        let mut reached = false;
        let mut login_wanted = false;
        let last_idx = order.len() as isize - 1;

        for idx in -1..=last_idx {
            let client_t0 = Instant::now();
            let (key, resp): (String, PlayerResponse) = if idx == -1 {
                // A WEB_REMIX stream that already died in the player is not retried for this
                // video: it passed HEAD and failed anyway, so validation has nothing left to say.
                // Uploads included. This used to exempt them, on the belief that skipping this
                // slot left the retry with nothing, but the rest of `UPLOAD_FALLBACK_ORDER`
                // (TVHTML5, then WEB_CREATOR) is exactly what the retry is for. Exempting them
                // meant the second attempt re-resolved the same dead WEB_REMIX URL and failed
                // identically, which is the loop issue #71 has been stuck in.
                if !main_ok
                    || disabled.contains(MAIN_CLIENT)
                    || blacklist_blocks(
                        &*self.web_remix_failed.lock().await,
                        video_id,
                        Instant::now(),
                    )
                {
                    continue;
                }
                (MAIN_CLIENT.to_owned(), main_resp.clone().unwrap())
            } else {
                let key = &order[idx as usize];
                if disabled.contains(key) {
                    continue;
                }
                let Some(client) = self.clients.get(key) else { continue };
                if client.login_required && !logged_in {
                    continue;
                }
                let client_pot = if client.use_web_po_tokens { session_pot } else { None };
                if client.use_web_po_tokens && client_pot.is_none() {
                    tracing::debug!(client = key, "no PoToken, skipping");
                    continue;
                }
                let client_sts = if client.use_signature_timestamp { sts } else { None };
                let answered =
                    self.it.player(client, video_id, playlist_id, client_sts, client_pot).await;
                reached |= answered.is_ok();
                match answered {
                    Ok(r) if r.playability_status.is_ok() => (key.clone(), r),
                    Ok(r) => {
                        login_wanted |= r.playability_status.status == "LOGIN_REQUIRED";
                        self.ranker.record_failure(key).await;
                        tracing::debug!(
                            client = key,
                            status = %r.playability_status.status,
                            reason = r.playability_status.reason.as_deref().unwrap_or(""),
                            "not OK"
                        );
                        continue;
                    }
                    Err(e) => {
                        self.ranker.record_failure(key).await;
                        tracing::warn!(client = key, error = %e, "player call failed");
                        continue;
                    }
                }
            };

            let Some(streaming) = resp.streaming_data.as_ref() else { continue };
            let Some(expires) = streaming.expires_in_seconds else { continue };
            let Some(format) = find_format(streaming, quality) else { continue };
            if audio_config_loudness.is_none() {
                audio_config_loudness = main_loudness(&resp);
            }

            // Resolve the URL: direct, else decipher (context/05).
            let Some(mut url) = self.find_url(format, video_id).await else {
                self.ranker.record_failure(&key).await;
                continue;
            };

            // n-transform + &pot= for web clients (context/05, 06).
            let client = self.clients.get(&key);
            let needs_n = client.is_some_and(|c| c.use_web_po_tokens)
                || NEEDS_N_TRANSFORM.contains(&key.as_str());
            if needs_n {
                url = self.cipher.transform_n_param_in_url(&url).await;
                if client.is_some_and(|c| c.use_web_po_tokens) {
                    if let Some(vd) = &visitor {
                        if let Some(pot) = self.potoken.get_streaming_po_token(video_id, vd).await {
                            let sep = if url.contains('?') { '&' } else { '?' };
                            url = format!("{url}{sep}pot={}", urlencoding::encode(&pot));
                        }
                    }
                }
            }

            // HIGH two-pass: remember the best non-HIGH and keep looking if a HIGH exists elsewhere.
            if prefer_high && !is_high(format) && has_high {
                if better(format, best.as_ref().map(|c| &c.format)) {
                    let ping = main_ping.clone().or_else(|| playback_ping(&resp, &key));
                    best =
                        Some(Candidate { format: format.clone(), url, expires, client: key, ping });
                }
                continue;
            }

            let headers =
                stream_headers(client.map(|c| c.user_agent.clone()), self.it.cookie(), is_upload);
            if self.validate_stream(&url, &headers, content_length(format)).await {
                let elapsed_ms = client_t0.elapsed().as_secs_f64() * 1000.0;
                self.ranker.record_success(&key, elapsed_ms).await;
                let ping = main_ping.clone().or_else(|| playback_ping(&resp, &key));
                return Ok(self.build(
                    video_id,
                    format,
                    url,
                    expires,
                    &key,
                    audio_config_loudness,
                    &main_resp,
                    ping,
                    headers,
                ));
            }

            self.ranker.record_failure(&key).await;

            // An upload's failed HEAD is a demotion, never a rejection. Metrolist stopped
            // validating privately-owned tracks outright (PR #3517) because a HEAD against one
            // does not reliably predict its GET, and this app then went further and returned the
            // very first URL unvalidated. That made WEB_REMIX the only client an upload ever
            // used: TVHTML5 and WEB_CREATOR sat behind an unconditional `return` and could never
            // run, so an account whose WEB_REMIX URLs 403 (no accepted PoToken on that machine, a
            // stale cipher) had no second chance and no way to recover. Issue #71.
            //
            // So: keep the first URL as a last resort, let the rest of the chain have its turn,
            // and hand the unvalidated one back only if nothing better turns up. Worst case this
            // is what the old code did, one or two round trips later.
            if is_upload {
                if upload_fallback.is_none() {
                    tracing::info!(video_id, client = %key, "upload stream failed HEAD, trying the next login client");
                    let ping = main_ping.clone().or_else(|| playback_ping(&resp, &key));
                    upload_fallback =
                        Some(Candidate { format: format.clone(), url, expires, client: key, ping });
                }
                continue;
            }

            if needs_n {
                self.self_heal();
            }
        }

        // 6. HIGH wanted but only a non-HIGH found → use the remembered best.
        if let Some(c) = best {
            let headers = self.headers_for(&c.client, is_upload);
            return Ok(self.build(
                video_id,
                &c.format,
                c.url,
                c.expires,
                &c.client,
                audio_config_loudness,
                &main_resp,
                c.ping,
                headers,
            ));
        }

        // 6b. An upload nothing validated: hand back the first URL a login client produced
        // rather than skip the track. See the demotion note in the loop.
        if let Some(c) = upload_fallback {
            tracing::warn!(video_id, client = %c.client, "no upload stream passed HEAD, using the first anyway");
            // Every login client's URL was refused, which is the one upload failure that does say
            // something about the session rather than about the track. Heal off the hot path so a
            // machine stuck on a rejected PoToken or a stale cipher can get itself out; without
            // this an upload-only failure had no route back at all.
            self.self_heal();
            let headers = self.headers_for(&c.client, is_upload);
            return Ok(self.build(
                video_id,
                &c.format,
                c.url,
                c.expires,
                &c.client,
                audio_config_loudness,
                &main_resp,
                c.ping,
                headers,
            ));
        }

        // 7. Net: rustypipe whole-videoId resolution (last-ditch). context/06, seam #11.
        // rustypipe is anonymous, so it can never see a privately-owned track: skip the round trip
        // and say what actually went wrong instead of "unavailable". Issue #71.
        if is_upload {
            tracing::warn!(video_id, "no authenticated client could stream this upload");
            return Err(ResolveError::UploadUnavailable(video_id.to_owned()));
        }
        tracing::info!(video_id, "all InnerTube clients exhausted → rustypipe fallback");
        match rustypipe_fallback::resolve(video_id, prefer_high).await {
            Ok(c) if !self.validate_stream(&c.url, &HashMap::new(), Some(c.size)).await => {
                tracing::warn!(video_id, "rustypipe URL serves only its first MiB, not playable");
                Err(nothing_played(video_id, logged_in, login_wanted, true))
            }
            Ok(c) => {
                self.ranker.record_success("rustypipe", 200.0).await;
                Ok(PlaybackData {
                    video_id: video_id.to_owned(),
                    stream_url: c.url,
                    itag: c.itag as i64,
                    headers: std::collections::HashMap::new(),
                    expires_in_seconds: c.expires_in_seconds as i64,
                    loudness_db: c.loudness_db.map(|f| f as f64),
                    playback_ping: None,
                    title: c.title,
                    artists: None,
                    duration: c.duration_secs.map(|s| s.to_string()),
                    thumbnail: None,
                    // rustypipe answers without a `musicVideoType`, so the queue row's flag stands.
                    is_video: None,
                    stream_client: "rustypipe".to_owned(),
                    audio_quality: Some(format_quality_label(&c.mime, c.bitrate as i64)),
                })
            }
            Err(e) => {
                tracing::error!(video_id, error = %e, "rustypipe fallback failed");
                let reached = reached || e.answered();
                Err(nothing_played(video_id, logged_in, login_wanted, reached))
            }
        }
    }

    /// A video-only stream URL for `video_id`, for the player view's music-video mode (plan 031).
    ///
    /// Deliberately not part of [`resolve`](Self::resolve): this runs only while someone is looking
    /// at the player view with video on, it needs no cipher, no PoToken and no HEAD two-pass, and it
    /// must never be able to make audio slower or less reliable. A `None` here just means the view
    /// keeps the artwork.
    pub async fn resolve_video(&self, video_id: &str, max_height: i32) -> Option<String> {
        for key in ["VISIONOS"] {
            let Some(client) = self.clients.get(key) else { continue };
            let resp = match self.it.player(client, video_id, None, None, None).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(video_id, client = key, error = %e, "video: /player failed");
                    continue;
                }
            };
            if !resp.playability_status.is_ok() {
                continue;
            }
            let Some(sd) = resp.streaming_data.as_ref() else { continue };
            // Only ever a direct URL: these clients don't cipher, and a ciphered video is not worth
            // waking the cipher webview for.
            if let Some(url) = find_video_format(sd, max_height).and_then(|f| f.direct_url()) {
                tracing::debug!(video_id, client = key, "video: resolved");
                return Some(url.to_owned());
            }
        }
        tracing::debug!(video_id, "video: no usable format");
        None
    }

    /// A format's playable URL: direct, else deciphered from its `signatureCipher`. context/05.
    async fn find_url(&self, format: &Format, video_id: &str) -> Option<String> {
        if let Some(u) = format.direct_url() {
            return Some(u.to_owned());
        }
        let cipher = format.cipher_string()?;
        self.cipher.deobfuscate_stream_url(cipher, video_id).await
    }

    /// Will googlevideo serve this URL, all the way to the end? (context/06 §validateStatus.)
    ///
    /// Probe shape, not just probe headers. mpv never opens a googlevideo URL directly any more:
    /// `state::mpv_stream_url` hands it a loopback URL and `audioproxy` fetches bounded ranges
    /// upstream, so a bounded range is the request this has to predict.
    ///
    /// And the range is the last 256 bytes, because that is the one question that separates a
    /// capped URL from a healthy one. Since 2026 googlevideo answers only the first mebibyte of
    /// some URLs (rustypipe's, ANDROID_VR's): measured 2026-09-22, a range ending inside that
    /// window returns 206 and every range ending past it returns 403, on every video tried and at
    /// any chunk size. HEAD and an opening range both pass, and mpv was handed a stream that
    /// delivered no bytes and blamed the audio format (issue #292).
    async fn validate_stream(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        content_length: Option<u64>,
    ) -> bool {
        let req = match content_length.filter(|n| *n > 0).filter(|_| is_youtube_stream(url)) {
            Some(len) => crate::http::client()
                .get(url)
                .header("Range", format!("bytes={}-{}", len.saturating_sub(256), len - 1))
                .header("Accept-Encoding", "identity"),
            None => crate::http::client().head(url),
        };
        let mut req = req.timeout(Duration::from_secs(10));
        for (k, v) in headers {
            req = req.header(k, v);
        }
        matches!(req.send().await, Ok(r) if r.status().is_success())
    }

    /// A cipher client's stream was refused, so its config may be stale. Heal off the hot path so
    /// it never blocks falling through (context/06 §7). If the heal changes the config table,
    /// clear the WEB_REMIX failure memory (context/06 §2).
    fn self_heal(&self) {
        if !claim_heal() {
            tracing::debug!("self-heal ran recently, not repeating it");
            return;
        }
        let cipher = self.cipher.clone();
        let potoken = self.potoken.clone();
        let failed = self.web_remix_failed.clone();
        tauri::async_runtime::spawn(async move {
            // The session PoToken now outlives the process, so a rejected web stream is the only
            // signal left that Google stopped honouring it early. Drop it here rather than replay
            // it for the rest of its nominal 12 hours.
            potoken.invalidate_session_token().await;
            if cipher.on_stream_rejected().await {
                failed.lock().await.clear();
            }
        });
    }

    /// [`stream_headers`] for a client registry key.
    pub(crate) fn headers_for(&self, client: &str, is_upload: bool) -> HashMap<String, String> {
        stream_headers(
            self.clients.get(client).map(|c| c.user_agent.clone()),
            self.it.cookie(),
            is_upload,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        &self,
        video_id: &str,
        format: &Format,
        url: String,
        expires: i64,
        client: &str,
        loudness: Option<f64>,
        main_resp: &Option<PlayerResponse>,
        ping: Option<PlaybackPing>,
        headers: HashMap<String, String>,
    ) -> PlaybackData {
        let vd = main_resp.as_ref().and_then(|r| r.video_details.as_ref());
        tracing::info!(video_id, client, itag = format.itag, "resolved stream");
        PlaybackData {
            video_id: video_id.to_owned(),
            stream_url: url,
            itag: format.itag as i64,
            headers,
            expires_in_seconds: expires,
            loudness_db: format.loudness_db.or(loudness),
            playback_ping: ping,
            title: vd.and_then(|v| v.title.clone()),
            artists: vd.and_then(|v| v.author.clone()),
            duration: vd.and_then(|v| v.length_seconds.clone()),
            thumbnail: main_resp.as_ref().and_then(best_thumbnail),
            is_video: vd.and_then(|v| v.is_music_video()),
            stream_client: client.to_owned(),
            audio_quality: Some(format_quality_label(&format.mime_type, format.bitrate)),
        }
    }
}

/// How long after one self-heal completes before another is permitted.
const HEAL_COOLDOWN: Duration = Duration::from_secs(60);

/// One self-heal at a time, and not more than one per [`HEAL_COOLDOWN`]. Same shape as
/// `session::claim_refresh`, and for the same reason: one bad minute throws off a burst of
/// identical signals and each one used to pay the full price.
/// ponytail: process-global, fine with one `Orchestrator` per process; move it onto `self` if a
/// second one is ever built.
fn claim_heal() -> bool {
    static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
    let Ok(mut last) = LAST.lock() else { return false };
    if last.is_some_and(|t| t.elapsed() < HEAL_COOLDOWN) {
        return false;
    }
    *last = Some(Instant::now());
    true
}

/// Generate a user-friendly quality badge (e.g. "OPUS 160 kbps", "AAC 256 kbps").
pub fn format_quality_label(mime: &str, bitrate: i64) -> String {
    let codec = if mime.contains("opus") {
        "OPUS"
    } else if mime.contains("mp4a") || mime.contains("aac") {
        "AAC"
    } else if mime.contains("flac") {
        "FLAC"
    } else if mime.contains("mp3") {
        "MP3"
    } else {
        "AUDIO"
    };
    if bitrate > 0 {
        let kbps = (bitrate as f64 / 1000.0).round() as i64;
        format!("{codec} {kbps} kbps")
    } else {
        codec.to_string()
    }
}

/// A format's byte length, when it reported one. `"0"` (an RSS-feed enclosure, #294) reads as
/// absent, because a zero-length file has no tail to probe.
fn content_length(f: &Format) -> Option<u64> {
    f.content_length.as_deref()?.parse::<u64>().ok().filter(|n| *n > 0)
}

/// True for a URL served by YouTube's own stream CDN. Everything else (an RSS-feed podcast's
/// enclosure, #294) gets no n-transform, no PoToken and no chunking proxy.
///
/// **Two hosts, not one.** Ordinary tracks come back on `*.googlevideo.com`, but one of the user's
/// own uploads is served from `*.c.youtube.com` (measured on a 0.8.2 report, issue #308). Matching
/// only the first host meant an upload's URL was handed to mpv unsigned: no `n`-transform and no
/// `&pot=`, which googlevideo answers with 403, and mpv opened it directly because the chunking
/// proxy skipped it too. Uploads have no anonymous client behind them, so that was every upload.
pub(crate) fn is_youtube_stream(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| h.ends_with(".googlevideo.com") || h.ends_with(".c.youtube.com"))
        })
        .unwrap_or(false)
}

pub(crate) fn is_googlevideo(url: &str) -> bool {
    is_youtube_stream(url)
}

/// The headers mpv (and the validating HEAD) must send for one stream.
///
/// A privately-owned track's stream URL (`c.youtube.com`, #308) is only served to the session
/// that owns it, so an upload's GET has to carry the cookie. Uploads only: this is the hot path
/// and there is no evidence an ordinary stream wants one. Issue #71.
///
/// mpv's header properties are global (crates/player: `http-header-fields`), so a track appended
/// for gapless playback inherits whatever the current one set. Same host either way, so it is
/// harmless, but it means the cookie can outlive the upload that needed it.
fn stream_headers(
    ua: Option<String>,
    cookie: Option<String>,
    is_upload: bool,
) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    if let Some(ua) = ua {
        headers.insert("User-Agent".to_owned(), ua);
    }
    if is_upload {
        if let Some(cookie) = cookie {
            headers.insert("Cookie".to_owned(), cookie);
        }
    }
    headers
}

/// The error for a track nothing could stream. Signing in is a real fix when YouTube asked for an
/// account and there is no session, and useless noise otherwise (issue #292).
/// When no response came back at all (`reached` false), we never learned anything about this video.
fn nothing_played(
    video_id: &str,
    logged_in: bool,
    login_wanted: bool,
    reached: bool,
) -> ResolveError {
    if !reached {
        return ResolveError::Unreachable(video_id.to_owned());
    }
    if login_wanted && !logged_in {
        ResolveError::SignInRequired(video_id.to_owned())
    } else {
        ResolveError::AllClientsFailed(video_id.to_owned())
    }
}

fn is_high(f: &Format) -> bool {
    f.audio_quality.as_deref() == Some("AUDIO_QUALITY_HIGH")
}

/// Better-than comparison for the HIGH two-pass (context/06 §isBetter): quality rank, then audio
/// channels, then codec (opus > mp4a), then bitrate.
fn better(a: &Format, b: Option<&Format>) -> bool {
    let Some(b) = b else { return true };
    let rank = |f: &Format| match f.audio_quality.as_deref() {
        Some("AUDIO_QUALITY_HIGH") => 3,
        Some("AUDIO_QUALITY_MEDIUM") => 2,
        Some("AUDIO_QUALITY_LOW") => 1,
        _ => 0u8,
    };
    let codec = |f: &Format| {
        if f.mime_type.contains("opus") {
            2
        } else if f.mime_type.contains("mp4a") {
            1
        } else {
            0u8
        }
    };
    (rank(a), a.audio_channels.unwrap_or(2), codec(a), a.bitrate)
        > (rank(b), b.audio_channels.unwrap_or(2), codec(b), b.bitrate)
}

fn main_loudness(resp: &PlayerResponse) -> Option<f64> {
    resp.player_config.as_ref().and_then(|c| c.audio_config.as_ref()).and_then(|a| a.loudness_db)
}

fn playback_ping(resp: &PlayerResponse, client: &str) -> Option<PlaybackPing> {
    let url = resp
        .playback_tracking
        .as_ref()
        .and_then(|t| t.videostats_playback_url.as_ref())
        .and_then(|b| b.base_url.clone())?;
    Some(PlaybackPing { url, client: client.to_owned() })
}

fn best_thumbnail(resp: &PlayerResponse) -> Option<String> {
    resp.video_details
        .as_ref()
        .and_then(|v| v.thumbnail.as_ref())
        .and_then(|t| t.thumbnails.last())
        .map(|t| t.url.clone())
}

#[cfg(test)]
mod tests {
    use super::{
        blacklist_blocks, blacklist_insert, claim_heal, content_length, is_youtube_stream,
        nothing_played, stream_headers, ResolveError, WEB_REMIX_BLACKLIST_TTL,
    };
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    // An RSS-feed podcast streams from the feed's own host (#294), which gets no PoToken. One of
    // the user's own uploads streams from `c.youtube.com` (#308), which needs everything a
    // googlevideo URL needs: miss it and the URL reaches mpv unsigned and 403s.
    #[test]
    fn both_of_youtubes_stream_hosts_count_and_nothing_else_does() {
        assert!(is_youtube_stream("https://rr5---sn-abc.googlevideo.com/videoplayback?n=x"));
        assert!(is_youtube_stream("https://rr2---sn-2onja5-5i.c.youtube.com/videoplayback?n=x"));
        assert!(!is_youtube_stream("https://www.podtrac.com/pts/redirect.mp3/x.mp3"));
        assert!(!is_youtube_stream("https://evil.com/googlevideo.com/videoplayback"));
        assert!(!is_youtube_stream("https://evil.com/rr2---sn-x.c.youtube.com/videoplayback"));
        assert!(!is_youtube_stream("https://www.youtube.com/watch?v=x"));
        assert!(!is_youtube_stream("/home/me/song.flac"));
    }

    #[test]
    fn the_web_remix_bar_expires_and_stays_bounded() {
        let now = Instant::now();
        let mut map = HashMap::new();

        blacklist_insert(&mut map, "fresh", now);
        assert!(blacklist_blocks(&map, "fresh", now), "a fresh failure bars WEB_REMIX");
        assert!(!blacklist_blocks(&map, "never-failed", now));

        // Past the TTL the entry reads as absent, so the track gets its best client back.
        let later = now + WEB_REMIX_BLACKLIST_TTL + Duration::from_secs(1);
        assert!(!blacklist_blocks(&map, "fresh", later));

        // And inserting at that point drops it, so the map cannot grow across a long session.
        blacklist_insert(&mut map, "other", later);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("other"));
    }

    /// The HEAD probe and mpv share this, so what it returns has to be identical for both callers
    /// (that mismatch is issue #71): the cookie rides along for an upload and for nothing else.
    #[test]
    fn only_an_upload_carries_the_cookie() {
        let ua = || Some("UA/1".to_owned());
        let cookie = || Some("SAPISID=secret".to_owned());

        let up = stream_headers(ua(), cookie(), true);
        assert_eq!(up.get("User-Agent").map(String::as_str), Some("UA/1"));
        assert_eq!(up.get("Cookie").map(String::as_str), Some("SAPISID=secret"));

        let ordinary = stream_headers(ua(), cookie(), false);
        assert_eq!(ordinary.get("User-Agent").map(String::as_str), Some("UA/1"));
        assert!(!ordinary.contains_key("Cookie"), "an ordinary stream must not send the cookie");

        // Signed out: an upload cannot play at all, but it must not produce a bogus header.
        assert!(!stream_headers(ua(), None, true).contains_key("Cookie"));
    }

    /// Silence outranks a half-heard verdict: with nothing reached, even a `LOGIN_REQUIRED` seen
    /// earlier must not turn an outage into "sign in".
    #[test]
    fn nothing_played_prefers_unreachable_over_a_verdict() {
        for (logged_in, login_wanted) in
            [(false, true), (false, false), (true, true), (true, false)]
        {
            assert!(
                matches!(
                    nothing_played("v", logged_in, login_wanted, false),
                    ResolveError::Unreachable(_)
                ),
                "an outage must never read as a verdict on the track"
            );
        }
    }

    /// The regression guard for issue #292's fix, once YouTube did answer.
    #[test]
    fn nothing_played_keeps_its_old_answers_when_youtube_answered() {
        assert!(matches!(nothing_played("v", false, true, true), ResolveError::SignInRequired(_)));
        for (logged_in, login_wanted) in [(false, false), (true, true), (true, false)] {
            assert!(matches!(
                nothing_played("v", logged_in, login_wanted, true),
                ResolveError::AllClientsFailed(_)
            ));
        }
    }

    #[test]
    fn only_the_systemic_errors_affect_every_track() {
        let v = || "v".to_owned();
        assert!(
            ResolveError::Unreachable(v()).affects_every_track(),
            "an outage would walk the queue and delete rows"
        );
        assert!(
            ResolveError::SignInRequired(v()).affects_every_track(),
            "every anonymous track fails the same way until the user signs in"
        );
        assert!(
            !ResolveError::AllClientsFailed(v()).affects_every_track(),
            "an unavailable video must still be skipped, or the queue stalls on it"
        );
        assert!(
            !ResolveError::UploadUnavailable(v()).affects_every_track(),
            "a mixed queue must skip past a failed upload to the ordinary tracks"
        );
        assert!(
            !ResolveError::LocalMissing(v()).affects_every_track(),
            "a deleted local file must keep leaving the queue"
        );
    }

    #[test]
    fn content_length_reads_only_a_real_length() {
        let fmt = |len: &str| -> super::Format {
            let mut v = serde_json::json!({ "itag": 251, "mimeType": "audio/webm" });
            if !len.is_empty() {
                v["contentLength"] = len.into();
            }
            serde_json::from_value(v).unwrap()
        };
        assert_eq!(content_length(&fmt("4194304")), Some(4194304));
        assert_eq!(
            content_length(&fmt("0")),
            None,
            "a zero-length enclosure has no tail to probe, so it must fall back to the HEAD"
        );
        assert_eq!(content_length(&fmt("")), None);
        assert_eq!(content_length(&fmt("not a number")), None);
    }

    #[test]
    fn claim_heal_allows_one_and_then_holds_the_door() {
        assert!(claim_heal());
        assert!(
            !claim_heal(),
            "a burst of identical probe failures must cost one heal, not one heal per track"
        );
    }
}
