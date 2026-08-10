//! `PlayerJsFetcher` (context/05 §1) — fetch and disk-cache YouTube's `player.js`.
//!
//! `iframe_api` → player hash → `base.js`, cached under `cipher_cache/` with a 6-hour TTL. The
//! hash pins the exact player we derive the STS from and decipher with (context/05 §3).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use regex::Regex;

// Desktop web UA: YouTube serves the web `player.js` to this (context/05). Sent per request rather
// than baked into a client, so it stays visible at the fetch whose answer it decides.
use crate::http::WEB_UA;

const IFRAME_API: &str = "https://www.youtube.com/iframe_api";
const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not find player hash in iframe_api")]
    NoHash,
}

pub struct PlayerJs {
    pub js: String,
    pub hash: String,
}

/// What the last webview discovery found on one exact `player.js`: a usable signature function, a
/// usable `n`-transform, or (KI-1's steady state) neither.
///
/// Cached on disk next to the player it describes, because finding out costs a whole hidden web
/// process and a 2.9 MB script injection, and the answer cannot change until YouTube rotates the
/// player. Dropped by [`PlayerJsFetcher::invalidate`], so a self-heal always re-probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discovery {
    pub sig: bool,
    pub n: bool,
}

impl Discovery {
    /// Nothing on this player is usable, so a webview built for it can do no work at all.
    pub fn is_dead(&self) -> bool {
        !self.sig && !self.n
    }
}

pub struct PlayerJsFetcher {
    cache_dir: PathBuf,
}

impl PlayerJsFetcher {
    pub fn new(app_data_dir: &Path) -> Self {
        let cache_dir = app_data_dir.join("cipher_cache");
        let _ = std::fs::create_dir_all(&cache_dir);
        PlayerJsFetcher { cache_dir }
    }

    /// Return the current `player.js` (cached if fresh, else fetched fresh and cached).
    pub async fn fetch(&self) -> Result<PlayerJs, Error> {
        let hash = self.current_hash().await?;
        let cached = self.cache_dir.join(format!("player_{hash}.js"));
        if let Some(js) = read_if_fresh(&cached) {
            tracing::debug!(hash, "player.js cache hit");
            return Ok(PlayerJs { js, hash });
        }
        let url =
            format!("https://www.youtube.com/s/player/{hash}/player_ias.vflset/en_GB/base.js");
        let js = get(&url).await?.error_for_status()?.text().await?;
        std::fs::write(&cached, &js)?;
        std::fs::write(
            self.cache_dir.join("current_hash.txt"),
            format!("{hash}\n{}", now_secs()),
        )?;
        tracing::info!(hash, bytes = js.len(), "fetched fresh player.js");
        Ok(PlayerJs { js, hash })
    }

    /// The stored [`Discovery`] verdict for `hash`, if this player has been probed before.
    pub fn discovery(&self, hash: &str) -> Option<Discovery> {
        parse_discovery(&std::fs::read_to_string(self.discovery_path(hash)).ok()?)
    }

    /// Record what discovery found, so the next launch on the same player needn't build a webview
    /// to learn it again. Best-effort: a failed write only costs the probe next time.
    pub fn set_discovery(&self, hash: &str, d: Discovery) {
        let _ = std::fs::write(self.discovery_path(hash), format!("{},{}", d.sig as u8, d.n as u8));
    }

    fn discovery_path(&self, hash: &str) -> PathBuf {
        self.cache_dir.join(format!("discovery_{hash}.txt"))
    }

    /// Delete cached `player_*.js` + `current_hash.txt` + the discovery verdicts (NOT the shared
    /// config files). context/05. Forces the next `fetch` to re-download and the next
    /// `ensure_analyzed` to re-probe — the self-heal path after a stale-signature 403.
    pub fn invalidate(&self) {
        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for e in entries.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("player_") && name.ends_with(".js")
                    || name.starts_with("discovery_")
                    || name == "current_hash.txt"
                {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }

    async fn current_hash(&self) -> Result<String, Error> {
        let body = get(IFRAME_API).await?.error_for_status()?.text().await?;
        extract_hash(&body).ok_or(Error::NoHash)
    }
}

/// GET as the desktop web player. Both of this module's requests decide what YouTube hands
/// back based on the User-Agent, so neither may go out without it.
async fn get(url: &str) -> reqwest::Result<reqwest::Response> {
    crate::http::client().get(url).header("User-Agent", WEB_UA).send().await
}

/// Extract the player hash from `iframe_api`. The URL there has escaped slashes
/// (`...\/s\/player\/<hash>\/www-widgetapi...`), so the separators are optional-backslash.
fn extract_hash(iframe_api_js: &str) -> Option<String> {
    let re = Regex::new(r"player\\?/([0-9A-Za-z_-]+)\\?/").ok()?;
    re.captures(iframe_api_js)?.get(1).map(|m| m.as_str().to_owned())
}

fn parse_discovery(raw: &str) -> Option<Discovery> {
    let (sig, n) = raw.trim().split_once(',')?;
    Some(Discovery { sig: sig == "1", n: n == "1" })
}

fn read_if_fresh(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let age = meta.modified().ok()?.elapsed().unwrap_or(CACHE_TTL);
    if age < CACHE_TTL {
        std::fs::read_to_string(path).ok()
    } else {
        None
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_from_escaped_iframe_api() {
        let body = r#"var scriptUrl='https:\/\/www.youtube.com\/s\/player\/4918c89a\/www-widgetapi.vflset\/www-widgetapi.js';"#;
        assert_eq!(extract_hash(body).as_deref(), Some("4918c89a"));
    }

    #[test]
    fn hash_from_plain_url() {
        assert_eq!(
            extract_hash("https://www.youtube.com/s/player/abcd1234/base.js").as_deref(),
            Some("abcd1234")
        );
    }

    #[test]
    fn discovery_round_trips() {
        for d in [
            Discovery { sig: false, n: false },
            Discovery { sig: true, n: false },
            Discovery { sig: false, n: true },
            Discovery { sig: true, n: true },
        ] {
            let written = format!("{},{}", d.sig as u8, d.n as u8);
            assert_eq!(parse_discovery(&written), Some(d), "round trip of {d:?}");
        }
        // Only the all-false verdict is allowed to skip the webview build.
        assert!(Discovery { sig: false, n: false }.is_dead());
        assert!(!Discovery { sig: false, n: true }.is_dead());
        assert!(!Discovery { sig: true, n: false }.is_dead());
        // A truncated or empty file must read as "never probed", not as a dead player.
        assert_eq!(parse_discovery(""), None);
        assert_eq!(parse_discovery("1"), None);
    }
}
