//! BitChord-compatible and Stremio Community Streaming Addons Client & Resolver.
//!
//! Protocol specifications & Compatibility:
//! - BitChord & Stremio manifests:
//!   - `GET /manifest.json` (or `/`): { id, name, version, resources, ... }
//!   - Supports polymorphic resources (strings e.g. ["stream", "search"] or objects e.g. [{ name: "stream" }])
//! - BitChord search & Stremio catalog:
//!   - `GET /search?q={query}&quality={tier}&atmos={atmos}`
//!   - Supports `{ tracks: [...] }`, `{ results: [...] }`, `{ metas: [...] }`, or direct `[...]`
//! - Stream endpoints:
//!   - BitChord stream: `GET /stream/{id}?quality={tier}&atmos={atmos}`
//!   - Stremio stream: `GET /stream/music/{id}.json`, `GET /stream/track/{id}.json`, `GET /stream/{id}.json`
//!   - Direct ID / query: `GET /stream/{videoId}?quality={tier}&atmos={atmos}`
//!   - Supports BitChord direct `{ url, format, ... }` and Stremio `{ streams: [ { url, title, ... } ] }`

use std::collections::HashMap;
use std::time::Duration;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::orchestrator::PlaybackData;

pub const ADDONS_KEY: &str = "streaming_addons";
pub const ADDONS_ENABLED_KEY: &str = "streaming_addons_enabled";

pub const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";

pub fn browser_request(client: &reqwest::Client, url: &str, timeout_ms: u64) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("User-Agent", BROWSER_UA)
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Sec-Ch-Ua", "\"Not(A:Brand\";v=\"99\", \"Google Chrome\";v=\"133\", \"Chromium\";v=\"133\"")
        .header("Sec-Ch-Ua-Mobile", "?0")
        .header("Sec-Ch-Ua-Platform", "\"Windows\"")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "cross-site")
        .timeout(Duration::from_millis(timeout_ms))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityAddon {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonManifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, deserialize_with = "deserialize_resources")]
    pub resources: Vec<String>,
    #[serde(default)]
    pub types: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub allow_downloads: Option<serde_json::Value>,
    #[serde(default)]
    pub check_valid_lossless: Option<serde_json::Value>,
}

fn deserialize_resources<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    let mut list = Vec::new();
    match val {
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(s) = item.as_str() {
                    list.push(s.to_string());
                } else if let Some(obj) = item.as_object() {
                    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                        list.push(name.to_string());
                    }
                }
            }
        }
        serde_json::Value::String(s) => {
            list.push(s);
        }
        _ => {}
    }
    Ok(list)
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddonTrack {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub artists: Option<serde_json::Value>,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default, rename = "artworkURL")]
    pub artwork_url: Option<String>,
    #[serde(default, rename = "albumArtworkURL")]
    pub album_artwork_url: Option<String>,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub audio_quality: Option<String>,
    #[serde(default)]
    pub audio_mode: Option<String>,
    #[serde(default)]
    pub audio_modes: Vec<String>,
    #[serde(default)]
    pub atmos: Option<bool>,
    #[serde(default, rename = "streamURL")]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl AddonTrack {
    pub fn resolved_title(&self) -> &str {
        if !self.title.trim().is_empty() {
            &self.title
        } else if let Some(n) = &self.name {
            n.as_str()
        } else {
            ""
        }
    }

    pub fn resolved_artist(&self) -> String {
        if !self.artist.trim().is_empty() {
            return self.artist.clone();
        }
        if let Some(artists_val) = &self.artists {
            if let Some(s) = artists_val.as_str() {
                return s.to_string();
            } else if let Some(arr) = artists_val.as_array() {
                let names: Vec<String> = arr
                    .iter()
                    .filter_map(|v| {
                        if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else if let Some(obj) = v.as_object() {
                            obj.get("name").and_then(|n| n.as_str()).map(|n| n.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !names.is_empty() {
                    return names.join(", ");
                }
            }
        }
        String::new()
    }

    pub fn resolved_stream_url(&self) -> Option<String> {
        self.stream_url.clone().or_else(|| self.url.clone())
    }

    pub fn resolved_artwork(&self) -> Option<String> {
        self.artwork_url.clone()
            .or_else(|| self.album_artwork_url.clone())
            .or_else(|| self.poster.clone())
            .or_else(|| self.thumbnail.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddonStreamCandidate {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub stream_quality: Option<String>,
    #[serde(default)]
    pub audio_quality: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub behavior_hints: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddonStream {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub streams: Option<Vec<AddonStreamCandidate>>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub stream_quality: Option<String>,
    #[serde(default)]
    pub audio_quality: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub file_codec: Option<String>,
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<f64>,
    #[serde(default)]
    pub bit_depth: Option<f64>,
    #[serde(default)]
    pub bitrate: Option<f64>,
    #[serde(default)]
    pub audio_mode: Option<String>,
    #[serde(default)]
    pub audio_modes: Vec<String>,
    #[serde(default)]
    pub atmos: Option<bool>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub error: Option<String>,
}

pub fn normalize_base_url(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.ends_with("/manifest.json") {
        s.truncate(s.len() - "/manifest.json".len());
    }
    while s.ends_with('/') {
        s.pop();
    }
    s
}

fn parse_manifest_json(val: serde_json::Value, base_url: &str) -> Result<AddonManifest, String> {
    let target_val = if let Some(m) = val.get("manifest") {
        m.clone()
    } else {
        val
    };

    let mut manifest: AddonManifest = serde_json::from_value(target_val)
        .map_err(|e| format!("Failed to parse manifest JSON: {}", e))?;

    if manifest.id.trim().is_empty() {
        if !manifest.name.trim().is_empty() {
            manifest.id = manifest
                .name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string();
        } else {
            let host = base_url
                .strip_prefix("https://")
                .or_else(|| base_url.strip_prefix("http://"))
                .unwrap_or(base_url)
                .split('/')
                .next()
                .unwrap_or("addon")
                .replace(['.', ':', '-'], "_");
            manifest.id = format!("addon_{}", host);
        }
    }

    if manifest.name.trim().is_empty() {
        manifest.name = manifest.id.clone();
    }

    if manifest.resources.is_empty() {
        manifest.resources = vec!["stream".to_string(), "search".to_string()];
    }

    Ok(manifest)
}

pub async fn test_addon_manifest(url: &str) -> Result<AddonManifest, String> {
    let raw = url.trim();
    if raw.is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    let base = normalize_base_url(raw);
    if base.is_empty() {
        return Err("Invalid addon URL".to_string());
    }

    let mut urls_to_try = Vec::new();
    let trimmed_no_slash = raw.trim_end_matches('/');
    if trimmed_no_slash.ends_with("/manifest.json") {
        urls_to_try.push(trimmed_no_slash.to_string());
        urls_to_try.push(format!("{}/manifest.json", base));
        urls_to_try.push(base.clone());
        urls_to_try.push(format!("{}/", base));
    } else {
        urls_to_try.push(format!("{}/manifest.json", base));
        urls_to_try.push(format!("{}/", base));
        urls_to_try.push(base.clone());
    }
    urls_to_try.dedup();

    let client = crate::http::client();
    let mut last_err = String::new();

    for candidate_url in &urls_to_try {
        let req = browser_request(&client, candidate_url, 10000);
        let res = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("Failed to reach {}: {}", candidate_url, e);
                continue;
            }
        };

        if !res.status().is_success() {
            last_err = format!("Endpoint {} returned HTTP status {}", candidate_url, res.status());
            continue;
        }

        let body_text = match res.text().await {
            Ok(t) => t,
            Err(e) => {
                last_err = format!("Failed to read response from {}: {}", candidate_url, e);
                continue;
            }
        };

        let trimmed_body = body_text.trim();
        if trimmed_body.starts_with('<') {
            last_err = format!(
                "Server at {} returned HTML instead of JSON manifest (possible Cloudflare challenge)",
                candidate_url
            );
            continue;
        }

        let json_val: serde_json::Value = match serde_json::from_str(trimmed_body) {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("Invalid JSON from {}: {}", candidate_url, e);
                continue;
            }
        };

        match parse_manifest_json(json_val, &base) {
            Ok(manifest) => return Ok(manifest),
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }

    Err(if last_err.is_empty() {
        "Failed to discover addon manifest".to_string()
    } else {
        last_err
    })
}

pub fn get_addons(db: &Db) -> Vec<CommunityAddon> {
    db.get_setting(ADDONS_KEY)
        .and_then(|s| serde_json::from_str::<Vec<CommunityAddon>>(&s).ok())
        .unwrap_or_default()
}

pub fn save_addons(db: &Db, addons: &[CommunityAddon]) {
    if let Ok(json) = serde_json::to_string(addons) {
        db.set_setting(ADDONS_KEY, &json);
    }
}

pub fn is_addons_enabled(db: &Db) -> bool {
    db.get_setting(ADDONS_ENABLED_KEY)
        .map(|v| v != "false")
        .unwrap_or(true)
}

pub fn set_addons_enabled(db: &Db, enabled: bool) {
    db.set_setting(ADDONS_ENABLED_KEY, if enabled { "true" } else { "false" });
}

fn clean_title_for_search(raw: &str) -> String {
    let re = Regex::new(r"(?i)\s*[\(\[](official\s*(music\s*)?video|video|audio|official\s*audio|lyric\s*video|lyrics|visualizer|remaster(ed)?|hd|4k)[\)\]]").unwrap();
    let cleaned = re.replace_all(raw, "");
    cleaned.trim().to_string()
}

fn normalize_string(raw: &str) -> String {
    // Strip only promotional/video wrappers like (Official Video), [Official Audio], (Lyric Video), etc.
    // Preserve musical version descriptions like (Travis Scott Remix), (Acoustic), (Live), (Feat. X)
    let re_boilerplate = Regex::new(r"(?i)\s*[\(\[](official\s*(music\s*)?video|video|audio|official\s*audio|lyric\s*video|lyrics|visualizer|remaster(ed)?(\s*\d{4})?|hd|4k|explicit|clean)[\)\]]").unwrap();
    let cleaned = re_boilerplate.replace_all(raw, " ");
    let re_punct = Regex::new(r"[^a-zA-Z0-9\s]").unwrap();
    let stripped = re_punct.replace_all(&cleaned, " ");
    stripped.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn has_version_keyword(s: &str, keyword: &str) -> bool {
    let lower = s.to_lowercase();
    match keyword {
        "remix" => lower.contains("remix") || lower.contains("rmx") || lower.contains("re-mix") || lower.contains("club mix") || lower.contains("extended mix"),
        "live" => {
            let re = Regex::new(r"(?i)\b(live(\s+(at|from|in|version|audio|session))?)\b").unwrap();
            re.is_match(&lower)
        }
        "acoustic" => lower.contains("acoustic") || lower.contains("unplugged") || lower.contains("stripped"),
        "slowed" => lower.contains("slowed") || lower.contains("slow down"),
        "sped_up" => lower.contains("sped up") || lower.contains("speed up") || lower.contains("nightcore"),
        "instrumental" => lower.contains("instrumental") || lower.contains("karaoke"),
        "cover" => {
            let re = Regex::new(r"(?i)\b(cover|tribute)\b").unwrap();
            re.is_match(&lower)
        }
        _ => false,
    }
}

fn is_atmos_text(text: &str) -> bool {
    let re = Regex::new(r"(?i)atmos|dolby|eac3[_-]?joc|ec-?3").unwrap();
    re.is_match(text)
}

fn is_hires_text(text: &str) -> bool {
    let re = Regex::new(r"(?i)hi[-_ ]?res|24[-_ ]?bit|96\s*khz|192\s*khz").unwrap();
    re.is_match(text)
}

fn parse_sample_rate(val: Option<f64>, text: &str) -> Option<u32> {
    if let Some(r) = val.filter(|&v| v > 0.0) {
        let hz = if r < 1000.0 { (r * 1000.0) as u32 } else { r as u32 };
        return Some(hz);
    }
    let re = Regex::new(r"(?i)([\d.]+)\s*kHz").unwrap();
    if let Some(caps) = re.captures(text) {
        if let Some(m) = caps.get(1) {
            if let Ok(num) = m.as_str().parse::<f64>() {
                if num > 0.0 {
                    return Some((num * 1000.0) as u32);
                }
            }
        }
    }
    None
}

fn parse_bit_depth(val: Option<f64>, text: &str) -> Option<u32> {
    if let Some(b) = val.filter(|&v| v >= 8.0 && v <= 32.0) {
        return Some(b as u32);
    }
    let re = Regex::new(r"(?i)(\d{1,2})\s*-?\s*bit").unwrap();
    if let Some(caps) = re.captures(text) {
        if let Some(m) = caps.get(1) {
            if let Ok(b) = m.as_str().parse::<u32>() {
                if (8..=32).contains(&b) {
                    return Some(b);
                }
            }
        }
    }
    None
}

fn parse_bitrate_kbps(val: Option<f64>, text: &str) -> Option<u32> {
    if let Some(b) = val.filter(|&v| v > 0.0) {
        let kbps = if b > 3000.0 { (b / 1000.0) as u32 } else { b as u32 };
        return Some(kbps);
    }
    let re = Regex::new(r"(?i)(\d{2,4})\s*kbps").unwrap();
    if let Some(caps) = re.captures(text) {
        if let Some(m) = caps.get(1) {
            if let Ok(kbps) = m.as_str().parse::<u32>() {
                return Some(kbps);
            }
        }
    }
    None
}

fn format_quality_label(
    codec_hint: Option<&str>,
    sample_rate: Option<u32>,
    bit_depth: Option<u32>,
    bitrate: Option<u32>,
    is_atmos: bool,
    raw_quality_text: &str,
) -> String {
    if is_atmos {
        let mut parts = Vec::new();
        if let Some(sr) = sample_rate {
            if sr % 1000 == 0 {
                parts.push(format!("{} kHz", sr / 1000));
            } else {
                parts.push(format!("{:.1} kHz", sr as f64 / 1000.0));
            }
        }
        if let Some(kbps) = bitrate {
            parts.push(format!("{} kbps", kbps));
        }
        if parts.is_empty() {
            return "DOLBY ATMOS".to_string();
        } else {
            return format!("DOLBY ATMOS · {}", parts.join(" · "));
        }
    }

    let is_hires = sample_rate.map_or(false, |sr| sr > 44100)
        || bit_depth.map_or(false, |b| b > 16)
        || is_hires_text(raw_quality_text);

    let is_lossless = is_hires
        || codec_hint.map_or(false, |c| {
            let cl = c.to_lowercase();
            cl.contains("flac") || cl.contains("alac") || cl.contains("wav")
        })
        || raw_quality_text.to_lowercase().contains("lossless")
        || raw_quality_text.to_lowercase().contains("flac");

    if is_hires {
        let mut specs = Vec::new();
        if let Some(bits) = bit_depth {
            specs.push(format!("{}-bit", bits));
        }
        if let Some(sr) = sample_rate {
            if sr % 1000 == 0 {
                specs.push(format!("{} kHz", sr / 1000));
            } else {
                specs.push(format!("{:.1} kHz", sr as f64 / 1000.0));
            }
        }
        let spec_str = if !specs.is_empty() {
            format!(" {}", specs.join(" / "))
        } else {
            "".to_string()
        };
        let br_str = bitrate.map_or("".to_string(), |br| format!(" · {} kbps", br));
        return format!("HI-RES LOSSLESS{}{}", spec_str, br_str);
    }

    if is_lossless {
        let bits = bit_depth.unwrap_or(16);
        let sr_str = match sample_rate {
            Some(sr) if sr % 1000 == 0 => format!("{} kHz", sr / 1000),
            Some(sr) => format!("{:.1} kHz", sr as f64 / 1000.0),
            None => "44.1 kHz".to_string(),
        };
        let br_str = bitrate.map_or("".to_string(), |br| format!(" · {} kbps", br));
        return format!("LOSSLESS {}-bit / {}{}", bits, sr_str, br_str);
    }

    // Lossy audio format
    let codec_name = codec_hint.map(str::to_uppercase).unwrap_or_else(|| {
        if raw_quality_text.to_lowercase().contains("aac") {
            "AAC".to_string()
        } else if raw_quality_text.to_lowercase().contains("opus") {
            "OPUS".to_string()
        } else {
            "MP3".to_string()
        }
    });
    let br_str = bitrate.map_or("".to_string(), |br| format!(" {} kbps", br));
    format!("{}{}", codec_name, br_str).trim().to_string()
}

fn parse_search_response(val: serde_json::Value) -> Vec<AddonTrack> {
    if let Ok(tracks) = serde_json::from_value::<Vec<AddonTrack>>(val.clone()) {
        return tracks;
    }
    if let Some(obj) = val.as_object() {
        for key in &["tracks", "results", "metas", "data", "items"] {
            if let Some(arr) = obj.get(*key) {
                if let Ok(tracks) = serde_json::from_value::<Vec<AddonTrack>>(arr.clone()) {
                    if !tracks.is_empty() {
                        return tracks;
                    }
                }
            }
        }
    }
    Vec::new()
}

fn parse_stream_response(val: serde_json::Value, target_title: Option<&str>) -> Option<(String, AddonStream, HashMap<String, String>)> {
    let target_has_remix = target_title.map_or(false, |t| has_version_keyword(t, "remix"));

    let is_candidate_acceptable = |c: &AddonStreamCandidate| -> bool {
        if c.url.as_ref().map_or(true, |u| u.trim().is_empty()) {
            return false;
        }
        if !target_has_remix {
            let label = format!("{} {}", c.title.as_deref().unwrap_or_default(), c.quality.as_deref().unwrap_or_default());
            if has_version_keyword(&label, "remix") || has_version_keyword(&label, "slowed") || has_version_keyword(&label, "sped_up") {
                return false;
            }
        }
        true
    };

    if let Ok(candidates) = serde_json::from_value::<Vec<AddonStreamCandidate>>(val.clone()) {
        let chosen = candidates.iter().find(|c| is_candidate_acceptable(c))
            .or_else(|| candidates.iter().find(|c| c.url.as_ref().map_or(false, |u| !u.trim().is_empty())));
        if let Some(first) = chosen {
            let url = first.url.clone().unwrap();
            let mut headers = HashMap::new();
            if let Some(bh) = &first.behavior_hints {
                if let Some(req_headers) = bh.get("proxyHeaders").and_then(|p| p.get("request")).and_then(|r| r.as_object()) {
                    for (k, v) in req_headers {
                        if let Some(vs) = v.as_str() {
                            headers.insert(k.clone(), vs.to_string());
                        }
                    }
                }
            }
            let stream = AddonStream {
                url: Some(url.clone()),
                format: first.format.clone(),
                quality: first.quality.clone().or(first.title.clone()),
                stream_quality: first.stream_quality.clone(),
                audio_quality: first.audio_quality.clone(),
                codec: first.codec.clone(),
                ..Default::default()
            };
            return Some((url, stream, headers));
        }
    }

    let mut stream_obj: AddonStream = serde_json::from_value(val).ok()?;
    if let Some(err) = &stream_obj.error {
        tracing::debug!(error = %err, "addon reported stream error");
        return None;
    }

    let mut custom_headers = stream_obj.headers.clone().unwrap_or_default();

    if let Some(ref direct_url) = stream_obj.url {
        if !direct_url.trim().is_empty() {
            return Some((direct_url.clone(), stream_obj, custom_headers));
        }
    }

    if let Some(candidates) = &stream_obj.streams {
        let chosen = candidates.iter().find(|c| is_candidate_acceptable(c))
            .or_else(|| candidates.iter().find(|c| c.url.as_ref().map_or(false, |u| !u.trim().is_empty())));
        if let Some(first) = chosen {
            let url = first.url.clone().unwrap();
            if let Some(bh) = &first.behavior_hints {
                if let Some(req_headers) = bh.get("proxyHeaders").and_then(|p| p.get("request")).and_then(|r| r.as_object()) {
                    for (k, v) in req_headers {
                        if let Some(vs) = v.as_str() {
                            custom_headers.insert(k.clone(), vs.to_string());
                        }
                    }
                }
            }
            if stream_obj.quality.is_none() {
                stream_obj.quality = first.quality.clone().or_else(|| first.title.clone());
            }
            if stream_obj.format.is_none() {
                stream_obj.format = first.format.clone();
            }
            if stream_obj.codec.is_none() {
                stream_obj.codec = first.codec.clone();
            }
            stream_obj.url = Some(url.clone());
            return Some((url, stream_obj, custom_headers));
        }
    }

    None
}

pub async fn resolve_from_addons(
    db: &Db,
    title: &str,
    artist: &str,
    duration_secs: Option<f64>,
    quality_pref: &str,
    video_id: Option<&str>,
) -> Option<PlaybackData> {
    if !is_addons_enabled(db) {
        return None;
    }

    let addons = get_addons(db);
    let active_addons: Vec<_> = addons.into_iter().filter(|a| a.enabled).collect();
    if active_addons.is_empty() {
        return None;
    }

    let clean_title = clean_title_for_search(title);
    let query_title = if !clean_title.is_empty() { &clean_title } else { title };
    let query = if !artist.trim().is_empty() && !query_title.trim().is_empty() {
        format!("{} {}", artist.trim(), query_title)
    } else if !query_title.trim().is_empty() {
        query_title.to_string()
    } else {
        artist.trim().to_string()
    };

    let tier = match quality_pref {
        "DOLBY_ATMOS" => "lossless",
        "HI_RES_LOSSLESS" => "hires",
        "LOSSLESS" | "VERY_HIGH" => "lossless",
        "HIGH" => "high",
        "LOW" => "low",
        _ => "lossless",
    };
    let atmos_param = if quality_pref == "DOLBY_ATMOS" || quality_pref == "HI_RES_LOSSLESS" || quality_pref == "LOSSLESS" {
        "auto"
    } else {
        "none"
    };

    let client = crate::http::client();
    let norm_target_title = normalize_string(query_title);
    let norm_target_artist = normalize_string(artist);

    for addon in active_addons {
        let base_url = normalize_base_url(&addon.base_url);

        // 1. Try search catalog if query is available
        let mut candidates_to_check = Vec::new();

        if !query.trim().is_empty() {
            let search_url = format!(
                "{}/search?q={}&quality={}&atmos={}",
                base_url,
                urlencoding::encode(&query),
                tier,
                atmos_param
            );

            tracing::info!(addon = %addon.name, query = %query, "searching community addon");
            let search_res = browser_request(&client, &search_url, 5000).send().await;

            if let Ok(res) = search_res {
                if res.status().is_success() {
                    if let Ok(val) = res.json::<serde_json::Value>().await {
                        let tracks = parse_search_response(val);
                        for track in tracks {
                            if track.id.is_empty() && track.resolved_stream_url().is_none() {
                                continue;
                            }
                            let cand_raw_title = track.resolved_title();
                            let mut score = 0i32;

                            // Version tag mismatch checks: NEVER replace an original track with an unwanted remix/live/acoustic/etc.
                            let version_tags = ["remix", "live", "acoustic", "slowed", "sped_up", "instrumental", "cover"];
                            let mut unwanted_version = false;
                            for tag in &version_tags {
                                let target_has = has_version_keyword(query_title, tag);
                                let cand_has = has_version_keyword(cand_raw_title, tag);
                                if !target_has && cand_has {
                                    unwanted_version = true;
                                    break;
                                } else if target_has && cand_has {
                                    score += 35; // Bonus for matching desired version (e.g. user specifically requested a remix)
                                } else if target_has && !cand_has {
                                    score -= 60; // Requested a specific version but candidate is plain
                                }
                            }
                            if unwanted_version {
                                tracing::debug!(
                                    target = %query_title,
                                    candidate = %cand_raw_title,
                                    "skipping candidate due to unwanted version/remix mismatch"
                                );
                                continue;
                            }

                            // If target does not mention featured artists, but candidate introduces a featured artist
                            let target_lower = format!("{} {}", query_title, artist).to_lowercase();
                            let cand_title_lower = cand_raw_title.to_lowercase();
                            if !target_lower.contains("feat") && !target_lower.contains("ft.") && !target_lower.contains("with") {
                                if cand_title_lower.contains("feat.") || cand_title_lower.contains("feat ") || cand_title_lower.contains("ft.") || cand_title_lower.contains("with ") {
                                    score -= 80;
                                }
                            }

                            // Duration comparison (±6 seconds tolerance)
                            if let (Some(target_dur), Some(cand_dur)) = (duration_secs, track.duration) {
                                let diff = (target_dur - cand_dur).abs();
                                if diff <= 3.0 {
                                    score += 50;
                                } else if diff <= 6.0 {
                                    score += 30;
                                } else if diff > 15.0 {
                                    score -= 80;
                                }
                            }

                            // Title matching
                            let norm_cand_title = normalize_string(cand_raw_title);
                            if !norm_target_title.is_empty() && !norm_cand_title.is_empty() {
                                if norm_target_title == norm_cand_title {
                                    score += 70;
                                } else if norm_cand_title.starts_with(&norm_target_title) || norm_target_title.starts_with(&norm_cand_title) {
                                    score += 40;
                                } else if norm_cand_title.contains(&norm_target_title) || norm_target_title.contains(&norm_cand_title) {
                                    score += 25;
                                } else {
                                    let target_words: Vec<&str> = norm_target_title.split_whitespace().collect();
                                    let cand_words: Vec<&str> = norm_cand_title.split_whitespace().collect();
                                    let shared = target_words.iter().filter(|w| cand_words.contains(w)).count();
                                    if !target_words.is_empty() {
                                        score += ((shared as f64 / target_words.len() as f64) * 20.0) as i32;
                                    }
                                }
                            }

                            // Artist matching
                            let norm_cand_artist = normalize_string(&track.resolved_artist());
                            if !norm_target_artist.is_empty() && !norm_cand_artist.is_empty() {
                                if norm_target_artist == norm_cand_artist {
                                    score += 50;
                                } else if norm_cand_artist.starts_with(&norm_target_artist) || norm_target_artist.starts_with(&norm_cand_artist) {
                                    score += 30;
                                } else if norm_cand_artist.contains(&norm_target_artist) || norm_target_artist.contains(&norm_cand_artist) {
                                    score += 15;
                                }
                            }

                            let is_cand_atmos = track.atmos == Some(true)
                                || track.audio_modes.iter().any(|m| is_atmos_text(m))
                                || track.audio_mode.as_deref().map_or(false, is_atmos_text)
                                || track.audio_quality.as_deref().map_or(false, is_atmos_text)
                                || track.format.as_deref().map_or(false, is_atmos_text);

                            if quality_pref == "DOLBY_ATMOS" && is_cand_atmos {
                                score += 25;
                            }

                            let is_cand_hires = track.audio_quality.as_deref().map_or(false, is_hires_text)
                                || track.format.as_deref().map_or(false, is_hires_text);
                            if quality_pref == "HI_RES_LOSSLESS" && is_cand_hires {
                                score += 20;
                            }

                            if score > 20 {
                                candidates_to_check.push((score, track));
                            }
                        }
                    }
                }
            }
        }

        candidates_to_check.sort_by(|a, b| b.0.cmp(&a.0));

        // Try scored candidates from search
        for (_, candidate) in candidates_to_check.into_iter().take(3) {
            let candidate_title = candidate.resolved_title().to_string();
            let candidate_artist = candidate.resolved_artist();
            let candidate_artwork = candidate.resolved_artwork();

            let (stream_url, addon_stream, custom_headers) = if let Some(direct_url) = candidate.resolved_stream_url() {
                if !direct_url.trim().is_empty() {
                    (
                        direct_url.clone(),
                        AddonStream {
                            url: Some(direct_url),
                            format: candidate.format.clone(),
                            audio_quality: candidate.audio_quality.clone(),
                            audio_mode: candidate.audio_mode.clone(),
                            audio_modes: candidate.audio_modes.clone(),
                            atmos: candidate.atmos,
                            ..Default::default()
                        },
                        HashMap::new(),
                    )
                } else {
                    continue;
                }
            } else {
                let mut stream_result = None;
                let stream_endpoints = [
                    format!("{}/stream/{}?quality={}&atmos={}", base_url, urlencoding::encode(&candidate.id), tier, atmos_param),
                    format!("{}/stream/music/{}.json", base_url, urlencoding::encode(&candidate.id)),
                    format!("{}/stream/track/{}.json", base_url, urlencoding::encode(&candidate.id)),
                    format!("{}/stream/{}.json", base_url, urlencoding::encode(&candidate.id)),
                ];

                for ep in &stream_endpoints {
                    let stream_res = browser_request(&client, ep, 5000).send().await;
                    if let Ok(res) = stream_res {
                        if res.status().is_success() {
                            if let Ok(val) = res.json::<serde_json::Value>().await {
                                if let Some(resolved) = parse_stream_response(val, Some(query_title)) {
                                    stream_result = Some(resolved);
                                    break;
                                }
                            }
                        }
                    }
                }

                match stream_result {
                    Some(r) => r,
                    None => continue,
                }
            };

            let combined_quality_text = format!(
                "{} {} {} {} {}",
                addon_stream.quality.as_deref().unwrap_or_default(),
                addon_stream.stream_quality.as_deref().unwrap_or_default(),
                addon_stream.audio_quality.as_deref().unwrap_or_default(),
                addon_stream.format.as_deref().unwrap_or_default(),
                candidate.audio_quality.as_deref().unwrap_or_default()
            );

            let is_atmos = addon_stream.atmos == Some(true)
                || candidate.atmos == Some(true)
                || addon_stream.audio_modes.iter().any(|m| is_atmos_text(m))
                || candidate.audio_modes.iter().any(|m| is_atmos_text(m))
                || is_atmos_text(&combined_quality_text);

            let codec = addon_stream.codec.as_deref()
                .or(addon_stream.file_codec.as_deref())
                .or(addon_stream.format.as_deref())
                .or(candidate.format.as_deref());

            let sample_rate = parse_sample_rate(addon_stream.sample_rate, &combined_quality_text);
            let bit_depth = parse_bit_depth(addon_stream.bit_depth, &combined_quality_text);
            let bitrate = parse_bitrate_kbps(addon_stream.bitrate, &combined_quality_text);

            let quality_label = format_quality_label(
                codec,
                sample_rate,
                bit_depth,
                bitrate,
                is_atmos,
                &combined_quality_text,
            );

            let mut final_headers = HashMap::new();
            final_headers.insert("User-Agent".to_string(), BROWSER_UA.to_string());
            for (k, v) in custom_headers {
                final_headers.insert(k, v);
            }

            let cand_dur_str = candidate.duration.map(|d| {
                let sec = d.round() as u64;
                format!("{}:{:02}", sec / 60, sec % 60)
            });

            tracing::info!(
                addon = %addon.name,
                title = %candidate_title,
                quality = %quality_label,
                "successfully resolved lossless stream from community addon"
            );

            return Some(PlaybackData {
                video_id: if !candidate.id.is_empty() { candidate.id } else { video_id.unwrap_or_default().to_string() },
                stream_url,
                itag: 0,
                headers: final_headers,
                expires_in_seconds: 7200,
                loudness_db: None,
                playback_ping: None,
                title: if !candidate_title.is_empty() { Some(candidate_title) } else { Some(title.to_string()) },
                artists: if !candidate_artist.is_empty() { Some(candidate_artist) } else { Some(artist.to_string()) },
                duration: cand_dur_str,
                thumbnail: candidate_artwork,
                is_video: Some(false),
                stream_client: format!("addon:{}", addon.name),
                audio_quality: Some(quality_label),
            });
        }

        // 2. Fallback: Direct stream queries (for stream-only addons like GGLOS / BitChord stream plugins)
        let mut direct_endpoints = Vec::new();
        if let Some(vid) = video_id {
            if !vid.is_empty() {
                direct_endpoints.push(format!("{}/stream/music/{}.json", base_url, urlencoding::encode(vid)));
                direct_endpoints.push(format!("{}/stream/track/{}.json", base_url, urlencoding::encode(vid)));
                direct_endpoints.push(format!("{}/stream/{}.json", base_url, urlencoding::encode(vid)));
                direct_endpoints.push(format!("{}/stream/{}?quality={}&atmos={}", base_url, urlencoding::encode(vid), tier, atmos_param));
                direct_endpoints.push(format!("{}/stream/music/{}.json", base_url, urlencoding::encode(&format!("yt:{}", vid))));
                direct_endpoints.push(format!("{}/stream/{}.json", base_url, urlencoding::encode(&format!("yt:{}", vid))));
            }
        }
        if !query.trim().is_empty() {
            direct_endpoints.push(format!("{}/stream?q={}&quality={}&atmos={}", base_url, urlencoding::encode(&query), tier, atmos_param));
            direct_endpoints.push(format!("{}/stream?title={}&artist={}&quality={}&atmos={}", base_url, urlencoding::encode(query_title), urlencoding::encode(artist), tier, atmos_param));
        }

        for ep in direct_endpoints {
            let res = browser_request(&client, &ep, 5000).send().await;
            if let Ok(r) = res {
                if r.status().is_success() {
                    if let Ok(val) = r.json::<serde_json::Value>().await {
                        if let Some((stream_url, addon_stream, custom_headers)) = parse_stream_response(val, Some(query_title)) {
                            let combined_quality_text = format!(
                                "{} {} {} {}",
                                addon_stream.quality.as_deref().unwrap_or_default(),
                                addon_stream.stream_quality.as_deref().unwrap_or_default(),
                                addon_stream.audio_quality.as_deref().unwrap_or_default(),
                                addon_stream.format.as_deref().unwrap_or_default()
                            );

                            let is_atmos = addon_stream.atmos == Some(true)
                                || addon_stream.audio_modes.iter().any(|m| is_atmos_text(m))
                                || is_atmos_text(&combined_quality_text);

                            let codec = addon_stream.codec.as_deref()
                                .or(addon_stream.file_codec.as_deref())
                                .or(addon_stream.format.as_deref());

                            let sample_rate = parse_sample_rate(addon_stream.sample_rate, &combined_quality_text);
                            let bit_depth = parse_bit_depth(addon_stream.bit_depth, &combined_quality_text);
                            let bitrate = parse_bitrate_kbps(addon_stream.bitrate, &combined_quality_text);

                            let quality_label = format_quality_label(
                                codec,
                                sample_rate,
                                bit_depth,
                                bitrate,
                                is_atmos,
                                &combined_quality_text,
                            );

                            let mut final_headers = HashMap::new();
                            final_headers.insert("User-Agent".to_string(), BROWSER_UA.to_string());
                            for (k, v) in custom_headers {
                                final_headers.insert(k, v);
                            }

                            tracing::info!(
                                addon = %addon.name,
                                endpoint = %ep,
                                quality = %quality_label,
                                "successfully resolved stream via direct addon query"
                            );

                            return Some(PlaybackData {
                                video_id: video_id.unwrap_or_default().to_string(),
                                stream_url,
                                itag: 0,
                                headers: final_headers,
                                expires_in_seconds: 7200,
                                loudness_db: None,
                                playback_ping: None,
                                title: Some(title.to_string()),
                                artists: Some(artist.to_string()),
                                duration: duration_secs.map(|d| {
                                    let sec = d.round() as u64;
                                    format!("{}:{:02}", sec / 60, sec % 60)
                                }),
                                thumbnail: None,
                                is_video: Some(false),
                                stream_client: format!("addon:{}", addon.name),
                                audio_quality: Some(quality_label),
                            });
                        }
                    }
                }
            }
        }
    }

    None
}
