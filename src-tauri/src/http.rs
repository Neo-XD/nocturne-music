//! The app's one outbound HTTP client.
//!
//! There used to be six of these inside `src-tauri` (orchestrator, PoToken, cipher fetcher, cipher
//! config, lyrics, and Last.fm twice), each built from its own `Client::builder()`. `reqwest` pools
//! connections and holds its TLS config per client, so that was six rustls configs, six connection
//! pools and six sets of idle sockets to the same handful of Google hosts.
//!
//! Everything the separate clients were configured for (a User-Agent, a timeout) is per-request
//! state, so it moved to the call sites. The User-Agent is deliberately NOT a default here: what
//! YouTube serves for `player.js` depends on it, so it should be visible at the fetch rather than
//! inherited from somewhere else and quietly lost in a later edit.
//!
//! `crates/innertube` keeps its own client on purpose. It is the other side of the transport
//! boundary (context/11) and it configures a proxy per session, which is client-level state.

use std::sync::OnceLock;

/// Desktop Chrome. YouTube serves the web `player.js` and the BotGuard endpoints against this, so
/// the cipher fetcher and the PoToken minter both send it.
pub const WEB_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

static PROXY: OnceLock<String> = OnceLock::new();

/// Point this client at the user's proxy. Called once during setup, before anything fetches:
/// [`client`] reads it when it builds, so a later call changes nothing (the setting already
/// says "restart to apply").
///
/// The `proxy` setting used to reach `crates/innertube` alone, which left player.js, the BotGuard
/// endpoints, lyrics and the music-video proxy going direct (#241).
pub fn set_proxy(proxy: Option<&str>) {
    if let Some(p) = proxy.map(str::trim).filter(|p| !p.is_empty()) {
        let _ = PROXY.set(p.to_owned());
    }
}

/// Whether [`client`] was pointed at a user proxy. mpv gets the same setting as its `http-proxy`
/// (#241), and ffmpeg would send a loopback URL through it, so the audio proxy has to stand down
/// when this is set. See `state::mpv_stream_url`.
pub fn has_proxy() -> bool {
    PROXY.get().is_some()
}

pub fn client() -> &'static reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        // No default User-Agent and no default timeout: both are set per request, by the callers
        // that actually need them.
        let mut b = reqwest::Client::builder();
        if let Some(p) = PROXY.get() {
            match reqwest::Proxy::all(p.as_str()) {
                Ok(proxy) => b = b.proxy(proxy),
                // Scheme only: the URI can carry credentials in its userinfo, and this lands in
                // limusic.log, which is what users attach to bug reports.
                Err(e) => {
                    let scheme = p.split_once("://").map_or("(none)", |(s, _)| s);
                    tracing::warn!(scheme, "unusable proxy setting, going direct: {e}")
                }
            }
        }
        b.build().unwrap_or_default()
    })
}
