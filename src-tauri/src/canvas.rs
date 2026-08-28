use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tracing::debug;

static CANVAS_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    CANVAS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Deserialize)]
struct SpotifyAccessToken {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PathfinderCanvasResponse {
    data: Option<PathfinderData>,
}

#[derive(Debug, Deserialize)]
struct PathfinderData {
    #[serde(rename = "trackUnion")]
    track_union: Option<PathfinderTrackUnion>,
    track: Option<PathfinderTrackUnion>,
}

#[derive(Debug, Deserialize)]
struct PathfinderTrackUnion {
    canvas: Option<PathfinderCanvas>,
}

#[derive(Debug, Deserialize)]
struct PathfinderCanvas {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenericCanvasProxyResponse {
    canvas_url: Option<String>,
    url: Option<String>,
    #[serde(rename = "videoUrl")]
    video_url: Option<String>,
}

/// Clean song titles for fuzzy searching across platforms (strip "(Official Audio)", "(feat. ...)", etc.)
fn clean_query(title: &str, artists: &str) -> String {
    let mut cleaned_title = title.to_string();
    if let Some(idx) = cleaned_title.find('(') {
        cleaned_title = cleaned_title[..idx].trim().to_string();
    }
    if let Some(idx) = cleaned_title.find('[') {
        cleaned_title = cleaned_title[..idx].trim().to_string();
    }
    if let Some(idx) = cleaned_title.to_lowercase().find("feat.") {
        cleaned_title = cleaned_title[..idx].trim().to_string();
    }
    if let Some(idx) = cleaned_title.to_lowercase().find("ft.") {
        cleaned_title = cleaned_title[..idx].trim().to_string();
    }

    let first_artist = artists.split(&[',', '&', ';', '/'][..]).next().unwrap_or(artists).trim();
    format!("{} {}", cleaned_title.trim(), first_artist)
}

/// Attempts to find a Spotify track ID by querying Spotify's public embed or search endpoint.
async fn find_spotify_track_id(title: &str, artists: &str) -> Option<String> {
    let client = crate::http::client();
    let query = clean_query(title, artists);
    let search_url = format!(
        "https://open.spotify.com/search/{}",
        urlencoding::encode(&query)
    );

    let res = client
        .get(&search_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
        )
        .header("Accept-Language", "en-US,en;q=0.9")
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    let text = res.text().await.ok()?;

    // Search for Spotify track URIs or paths in the returned markup
    // e.g., "spotify:track:0VjIjW4GlUZAMYd2vXMi3b" or "/track/0VjIjW4GlUZAMYd2vXMi3b"
    if let Some(pos) = text.find("/track/") {
        let after = &text[pos + 7..];
        let id: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect();
        if id.len() == 22 {
            return Some(id);
        }
    }

    if let Some(pos) = text.find("spotify:track:") {
        let after = &text[pos + 14..];
        let id: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect();
        if id.len() == 22 {
            return Some(id);
        }
    }

    None
}

/// Fetch canvas using an official user session token derived from `sp_dc` cookie.
async fn fetch_via_sp_dc(track_id: &str, sp_dc: &str) -> Option<String> {
    let client = crate::http::client();

    // 1. Get access token from web player endpoint
    let token_res = client
        .get("https://open.spotify.com/get_access_token?reason=transport&productType=web_player")
        .header("Cookie", format!("sp_dc={}", sp_dc.trim()))
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
        )
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?;

    let token_data: SpotifyAccessToken = token_res.json().await.ok()?;
    let access_token = token_data.access_token?;

    // 2. Query Pathfinder GraphQL endpoint
    let pathfinder_url = format!(
        "https://api-partner.spotify.com/pathfinder/v1/query?operationName=canvas&variables=%7B%22trackUri%22%3A%22spotify%3Atrack%3A{}%22%7D&extensions=%7B%22persistedQuery%22%3A%7B%22version%22%3A1%2C%22sha256Hash%22%3A%2220eb50013d940562e841804c7c5a0d33e507b587d55eb23b03ad71542f7480a4%22%7D%7D",
        track_id
    );

    let query_res = client
        .get(&pathfinder_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("app-platform", "WebPlayer")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?;

    if !query_res.status().is_success() {
        return None;
    }

    let parsed: PathfinderCanvasResponse = query_res.json().await.ok()?;
    if let Some(data) = parsed.data {
        if let Some(union) = data.track_union.or(data.track) {
            if let Some(canvas) = union.canvas {
                if let Some(url) = canvas.url {
                    if !url.is_empty() {
                        return Some(url);
                    }
                }
            }
        }
    }

    None
}

/// Fetch canvas via public proxy / mirror endpoints.
async fn fetch_via_proxy(track_id: &str, custom_proxy: Option<&str>) -> Option<String> {
    let client = crate::http::client();
    let mut endpoints = Vec::new();

    if let Some(cp) = custom_proxy {
        if !cp.trim().is_empty() {
            let formatted = if cp.contains("{id}") {
                cp.replace("{id}", track_id)
            } else if cp.contains("?") {
                format!("{}&track_id={}", cp.trim_end_matches('&'), track_id)
            } else {
                format!("{}?track_id={}", cp.trim_end_matches('/'), track_id)
            };
            endpoints.push(formatted);
        }
    }

    // Default public mirrors
    endpoints.push(format!("https://canvas-api.boidu.dev/canvas?track_id={}", track_id));
    endpoints.push(format!("https://spotify-canvas.vercel.app/api/canvas?trackId={}", track_id));
    endpoints.push(format!("https://canvaz-api.fly.dev/canvas/{}", track_id));

    for ep in endpoints {
        let res = match client
            .get(&ep)
            .header("User-Agent", "NocturneMusic/0.6.5")
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        if !res.status().is_success() {
            continue;
        }

        if let Ok(data) = res.json::<GenericCanvasProxyResponse>().await {
            if let Some(url) = data.canvas_url.or(data.url).or(data.video_url) {
                if !url.is_empty() && (url.contains(".mp4") || url.contains("canvaz.scdn.co") || url.contains("http")) {
                    return Some(url);
                }
            }
        }
    }

    None
}

/// Resolve the Spotify Canvas URL for a track given its title and artists.
pub async fn resolve_canvas_url(
    title: &str,
    artists: &str,
    sp_dc: Option<&str>,
    custom_proxy: Option<&str>,
) -> Option<String> {
    let cache_key = format!("{}:{}", title.trim().to_lowercase(), artists.trim().to_lowercase());

    // 1. Check in-memory cache
    {
        let cache = cache().lock().unwrap();
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    debug!("Resolving Spotify Canvas for '{}' by '{}'", title, artists);

    // 2. Find Spotify track ID
    let track_id = find_spotify_track_id(title, artists).await;
    let mut canvas_url = None;

    if let Some(ref tid) = track_id {
        // Try sp_dc authenticated resolution first if provided
        if let Some(cookie) = sp_dc {
            if !cookie.trim().is_empty() {
                canvas_url = fetch_via_sp_dc(tid, cookie).await;
            }
        }

        // Fall back to proxies if not resolved
        if canvas_url.is_none() {
            canvas_url = fetch_via_proxy(tid, custom_proxy).await;
        }
    }

    // 3. Cache result (even if None, to prevent spamming failed queries)
    {
        let mut cache = cache().lock().unwrap();
        cache.insert(cache_key, canvas_url.clone());
    }

    canvas_url
}
