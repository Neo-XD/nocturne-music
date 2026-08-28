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
struct SpotifySearchResponse {
    tracks: Option<SpotifySearchTracks>,
}

#[derive(Debug, Deserialize)]
struct SpotifySearchTracks {
    items: Option<Vec<SpotifySearchTrackItem>>,
}

#[derive(Debug, Deserialize)]
struct SpotifySearchTrackItem {
    id: Option<String>,
    uri: Option<String>,
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

/// Exchanges `sp_dc` cookie for an authorized web player access token.
async fn get_access_token_from_sp_dc(sp_dc: &str) -> Option<String> {
    let client = crate::http::client();
    let res = client
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

    if !res.status().is_success() {
        return None;
    }

    let token_data: SpotifyAccessToken = res.json().await.ok()?;
    token_data.access_token
}

/// Searches Spotify track ID using the official Web API with access token.
async fn search_track_with_token(title: &str, artists: &str, token: &str) -> Option<String> {
    let client = crate::http::client();
    let query = clean_query(title, artists);
    let search_url = format!(
        "https://api.spotify.com/v1/search?q={}&type=track&limit=1",
        urlencoding::encode(&query)
    );

    let res = client
        .get(&search_url)
        .header("Authorization", format!("Bearer {}", token))
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    let data: SpotifySearchResponse = res.json().await.ok()?;
    let track = data.tracks?.items?.into_iter().next()?;
    track.id.or_else(|| {
        track.uri.and_then(|u| {
            if u.starts_with("spotify:track:") {
                Some(u[14..].to_string())
            } else {
                None
            }
        })
    })
}

/// Fetch canvas URL using Spotify Pathfinder GraphQL API with access token.
async fn fetch_canvas_with_token(track_id: &str, token: &str) -> Option<String> {
    let client = crate::http::client();

    let pathfinder_url = format!(
        "https://api-partner.spotify.com/pathfinder/v1/query?operationName=canvas&variables=%7B%22trackUri%22%3A%22spotify%3Atrack%3A{}%22%7D&extensions=%7B%22persistedQuery%22%3A%7B%22version%22%3A1%2C%22sha256Hash%22%3A%2220eb50013d940562e841804c7c5a0d33e507b587d55eb23b03ad71542f7480a4%22%7D%7D",
        track_id
    );

    let query_res = client
        .get(&pathfinder_url)
        .header("Authorization", format!("Bearer {}", token))
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

/// Fetch canvas via public proxy / mirror endpoints or title+artist queries.
async fn fetch_via_proxy(
    title: &str,
    artists: &str,
    track_id: Option<&str>,
    custom_proxy: Option<&str>,
) -> Option<String> {
    let client = crate::http::client();
    let clean = clean_query(title, artists);
    let mut endpoints = Vec::new();

    if let Some(cp) = custom_proxy {
        if !cp.trim().is_empty() {
            if let Some(tid) = track_id {
                let formatted = if cp.contains("{id}") {
                    cp.replace("{id}", tid)
                } else if cp.contains("?") {
                    format!("{}&track_id={}", cp.trim_end_matches('&'), tid)
                } else {
                    format!("{}?track_id={}", cp.trim_end_matches('/'), tid)
                };
                endpoints.push(formatted);
            }
        }
    }

    if let Some(tid) = track_id {
        endpoints.push(format!("https://canvas-api.boidu.dev/canvas?track_id={}", tid));
        endpoints.push(format!("https://spotify-canvas.vercel.app/api/canvas?trackId={}", tid));
        endpoints.push(format!("https://canvaz-api.fly.dev/canvas/{}", tid));
    }

    endpoints.push(format!(
        "https://canvas-api.boidu.dev/canvas?q={}",
        urlencoding::encode(&clean)
    ));

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
    let mut canvas_url = None;

    // 2. If user provided sp_dc cookie, authenticate and fetch directly
    if let Some(cookie) = sp_dc {
        if !cookie.trim().is_empty() {
            if let Some(token) = get_access_token_from_sp_dc(cookie).await {
                if let Some(track_id) = search_track_with_token(title, artists, &token).await {
                    canvas_url = fetch_canvas_with_token(&track_id, &token).await;
                }
            }
        }
    }

    // 3. Fall back to proxy mirrors if not yet resolved
    if canvas_url.is_none() {
        canvas_url = fetch_via_proxy(title, artists, None, custom_proxy).await;
    }

    // 4. Cache result (even if None, to prevent spamming failed queries)
    {
        let mut cache = cache().lock().unwrap();
        cache.insert(cache_key, canvas_url.clone());
    }

    canvas_url
}
