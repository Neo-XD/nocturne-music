//! BitChord-compatible and Stremio Community Streaming Addons Client & Resolver.
//!
//! Protocol specifications & Compatibility:
//! - BitChord & Stremio manifests:
//!   - `GET /manifest.json` (or `/`): { id, name, version, resources, ... }
//!   - Supports polymorphic resources (strings or objects)
//! - BitChord search & Stremio catalog:
//!   - `GET /search?q={query}&quality={tier}&atmos={atmos}`
//!   - Supports `{ tracks: [...] }`, `{ results: [...] }`, `{ metas: [...] }`, or direct `[...]`
//! - Stream endpoints:
//!   - BitChord stream: `GET /stream/{id}?quality={tier}&atmos={atmos}`
//!   - Stremio stream: `GET /stream/music/{id}.json`, `GET /stream/{id}.json`
//!   - Direct ID / query: `GET /stream/{videoId}?quality={tier}&atmos={atmos}`
//!   - Supports BitChord direct `{ url, format }` and Stremio `{ streams: [...] }`

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::db::Db;
use crate::orchestrator::PlaybackData;

type CacheMap = HashMap<String, (Instant, PlaybackData)>;
static ADDON_STREAM_CACHE: OnceLock<Mutex<CacheMap>> = OnceLock::new();

fn get_addon_cache() -> &'static Mutex<CacheMap> {
    ADDON_STREAM_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_cached_addon_stream(key: &str) -> Option<PlaybackData> {
    let mut cache = get_addon_cache().lock().ok()?;
    if let Some((instant, data)) = cache.get(key) {
        if instant.elapsed() < Duration::from_secs(7200) {
            return Some(data.clone());
        } else {
            cache.remove(key);
        }
    }
    None
}

fn cache_addon_stream(key: String, data: PlaybackData) {
    if let Ok(mut cache) = get_addon_cache().lock() {
        if cache.len() >= 500 {
            cache.retain(|_, (t, _)| t.elapsed() < Duration::from_secs(7200));
        }
        cache.insert(key, (Instant::now(), data));
    }
}

pub const ADDONS_KEY: &str = "streaming_addons";
pub const ADDONS_ENABLED_KEY: &str = "streaming_addons_enabled";
pub const ADDON_TIMEOUT_MS: u64 = 1000;

/// BitChord addon user-agent header. Addons strictly verifying BitChord compatibility
/// expect this user-agent to avoid rejecting third-party hosts.
pub const ADDON_UA: &str = "BitChord/v1.9.0 Nocturne/v1.0.0";
pub const BROWSER_UA: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ",
    "AppleWebKit/537.36 (KHTML, like Gecko) ",
    "Chrome/133.0.0.0 Safari/537.36"
);

pub fn browser_request(
    client: &reqwest::Client,
    url: &str,
    timeout_ms: u64,
) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("User-Agent", ADDON_UA)
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "en-US,en;q=0.9")
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

pub fn is_atmos_text(text: &str) -> bool {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)atmos|dolby|e-?ac-?3|ec-?3|dd\+|ddp|ddplus|spatial|joc").unwrap()
    });
    re.is_match(text)
}

impl AddonTrack {
    pub fn is_dolby_atmos(&self) -> bool {
        if self.atmos == Some(true) {
            return true;
        }
        if let Some(am) = &self.audio_mode {
            if is_atmos_text(am) {
                return true;
            }
        }
        if self.audio_modes.iter().any(|m| is_atmos_text(m)) {
            return true;
        }
        let text = format!(
            "{} {} {}",
            self.audio_quality.as_deref().unwrap_or_default(),
            self.format.as_deref().unwrap_or_default(),
            self.audio_mode.as_deref().unwrap_or_default(),
        );
        is_atmos_text(&text)
    }

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
        self.artwork_url
            .clone()
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

impl AddonStream {
    pub fn is_dolby_atmos(&self) -> bool {
        if self.atmos == Some(true) {
            return true;
        }
        if let Some(am) = &self.audio_mode {
            if is_atmos_text(am) {
                return true;
            }
        }
        if self.audio_modes.iter().any(|m| is_atmos_text(m)) {
            return true;
        }
        let text = format!(
            "{} {} {} {} {} {} {}",
            self.quality.as_deref().unwrap_or_default(),
            self.stream_quality.as_deref().unwrap_or_default(),
            self.audio_quality.as_deref().unwrap_or_default(),
            self.format.as_deref().unwrap_or_default(),
            self.codec.as_deref().unwrap_or_default(),
            self.file_codec.as_deref().unwrap_or_default(),
            self.audio_mode.as_deref().unwrap_or_default(),
        );
        is_atmos_text(&text)
    }
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
    let target_val = if let Some(m) = val.get("manifest") { m.clone() } else { val };

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
    db.get_setting(ADDONS_ENABLED_KEY).map(|v| v != "false").unwrap_or(true)
}

pub fn set_addons_enabled(db: &Db, enabled: bool) {
    db.set_setting(ADDONS_ENABLED_KEY, if enabled { "true" } else { "false" });
}

// ── BitChord-Grade High-Accuracy Track Matcher ────────────────────────────────

const VERSION_WORDS: &[&str] = &[
    "remix",
    "remixes",
    "rmx",
    "refix",
    "flip",
    "bootleg",
    "mashup",
    "medley",
    "live",
    "concert",
    "unplugged",
    "acoustic",
    "instrumental",
    "karaoke",
    "vocals",
    "vocal",
    "acapella",
    "acappella",
    "backing",
    "stems",
    "stem",
    "cover",
    "demo",
    "reprise",
    "remake",
    "rework",
    "extended",
    "edit",
    "version",
    "mix",
    "dub",
    "vip",
    "session",
    "sessions",
    "sped",
    "slowed",
    "reverb",
    "nightcore",
    "lofi",
    "orchestral",
    "symphonic",
    "part",
    "pt",
    "chapter",
];

const NEUTRAL_SEGMENTS: &[&str] = &[
    "albumversion",
    "originalversion",
    "originalmix",
    "singleversion",
    "radioversion",
    "radioedit",
    "stereoversion",
    "monoversion",
    "studioversion",
    "fullversion",
    "standardversion",
    "explicitversion",
    "deluxeversion",
    "originaltrack",
];

const TRAILING_NOISE: &[&str] = &[
    "song",
    "songs",
    "video",
    "audio",
    "lyrics",
    "lyric",
    "lyrical",
    "official",
    "full",
    "hd",
    "hq",
    "4k",
    "mp3",
    "ost",
    "soundtrack",
];

const TRAILING_TAKES: &[&str] =
    &["instrumental", "karaoke", "acapella", "acappella", "remix", "rmx", "nightcore"];

const NOISE_WORDS: &[&str] = &[
    "official",
    "video",
    "audio",
    "lyrics",
    "lyric",
    "lyrical",
    "visualizer",
    "song",
    "songs",
    "full",
    "music",
    "the",
    "and",
    "from",
    "feat",
    "ft",
    "featuring",
    "with",
    "new",
    "latest",
    "free",
    "download",
    "remaster",
    "remastered",
    "explicit",
    "clean",
    "bonus",
    "track",
    "deluxe",
    "original",
    "album",
    "single",
    "hd",
    "hq",
    "4k",
    "mp3",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleParts {
    pub words: Vec<String>,
    pub core: String,
    pub versions: BTreeSet<String>,
    pub context: BTreeSet<String>,
}

fn normalize_raw(text: &str) -> String {
    let lower = text.to_lowercase().replace('&', " and ");
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        match c {
            '「' | '『' | '【' | '〈' | '《' | '〔' | '［' | '｛' => out.push('('),
            '」' | '』' | '】' | '〉' | '》' | '〕' | '］' | '｝' => out.push(')'),
            _ => out.push(c),
        }
    }
    out
}

fn classify_segment(
    segment: &str,
    versions: &mut BTreeSet<String>,
    context: &mut BTreeSet<String>,
) {
    let re_punct = Regex::new(r"[^\p{L}\p{N}\s]").unwrap();
    let cleaned = re_punct.replace_all(segment, " ");
    let words: Vec<String> =
        cleaned.split_whitespace().map(|w| w.to_lowercase()).filter(|w| !w.is_empty()).collect();

    if words.is_empty() {
        return;
    }
    let combined = words.join("");
    if NEUTRAL_SEGMENTS.contains(&combined.as_str()) {
        return;
    }

    let mut found_version = false;
    for w in &words {
        if VERSION_WORDS.contains(&w.as_str()) {
            versions.insert(w.clone());
            found_version = true;
        }
    }
    if !found_version {
        for w in words {
            if w.len() > 2 && !NOISE_WORDS.contains(&w.as_str()) {
                context.insert(w);
            }
        }
    }
}

pub fn parse_title(raw: &str, artist: &str) -> TitleParts {
    let mut versions = BTreeSet::new();
    let mut context = BTreeSet::new();
    let mut text = normalize_raw(raw);

    // 1. Bracketed asides
    let re_bracket = Regex::new(r"[(\[]([^()\[\]]*)[)\]]").unwrap();
    for _ in 0..3 {
        if !re_bracket.is_match(&text) {
            break;
        }
        text = re_bracket
            .replace_all(&text, |caps: &regex::Captures| {
                if let Some(m) = caps.get(1) {
                    classify_segment(m.as_str(), &mut versions, &mut context);
                }
                " "
            })
            .to_string();
    }

    // 2. Unbalanced brackets
    if let Some(open) = text.find(|c| c == '(' || c == '[') {
        classify_segment(&text[open..], &mut versions, &mut context);
        text.truncate(open);
    }

    // 3. Dash- and pipe-separated tails
    let re_dash = Regex::new(r"\s+[-–—|]+\s+").unwrap();
    for _ in 0..3 {
        if let Some(mat) = re_dash.find(&text) {
            let head = text[..mat.start()].trim().to_string();
            let tail = text[mat.end()..].trim().to_string();
            let norm_artist = normalize_raw(artist);
            if !norm_artist.is_empty() && head.contains(&norm_artist) {
                classify_segment(&head, &mut versions, &mut context);
                text = tail;
            } else {
                classify_segment(&tail, &mut versions, &mut context);
                text = head;
            }
        } else {
            break;
        }
    }

    // 4. Feat tail removal
    let re_feat = Regex::new(r"(?i)\b(feat|ft|featuring|with)\b.*").unwrap();
    text = re_feat.replace_all(&text, " ").to_string();

    // 5. Word tokenization
    let re_punct = Regex::new(r"[^\p{L}\p{N}\s]").unwrap();
    let cleaned = re_punct.replace_all(&text, " ");
    let mut words: Vec<String> = cleaned
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty() && w != "and")
        .collect();

    // 6. Trailing takes and noise
    while words.len() > 1 {
        let last = words.last().unwrap().as_str();
        if TRAILING_NOISE.contains(&last) {
            words.pop();
        } else if TRAILING_TAKES.contains(&last) {
            versions.insert(last.to_string());
            words.pop();
        } else {
            break;
        }
    }

    let core = words.join("");
    TitleParts { words, core, versions, context }
}

pub fn primary_artist(artist: &str) -> String {
    let re_sep = Regex::new(
        r"(?i)\s*(?:[,&/;·|、，×]|\band\b|\bx\b|\bvs\.?\b|\bfeat\.?\b|\bft\.?\b|\bfeaturing\b|\bwith\b)\s*",
    )
    .unwrap();
    let first = re_sep.split(artist).next().unwrap_or(artist).trim();
    let re_punct = Regex::new(r"[^\p{L}\p{N}\s]").unwrap();
    let cleaned = re_punct.replace_all(first, " ");
    let words: Vec<_> = cleaned.split_whitespace().collect();
    words.join(" ").to_lowercase()
}

pub fn artist_names(artist: &str) -> Vec<Vec<String>> {
    let re_sep = Regex::new(
        r"(?i)\s*(?:[,&/;·|、，×]|\band\b|\bx\b|\bvs\.?\b|\bfeat\.?\b|\bft\.?\b|\bfeaturing\b|\bwith\b)\s*",
    )
    .unwrap();
    let re_punct = Regex::new(r"[^\p{L}\p{N}\s]").unwrap();
    re_sep
        .split(artist)
        .filter_map(|name| {
            let cleaned = re_punct.replace_all(name, " ");
            let tokens: Vec<String> = cleaned
                .split_whitespace()
                .map(|t| t.to_lowercase())
                .filter(|t| t.len() > 1)
                .collect();
            if tokens.is_empty() {
                None
            } else {
                Some(tokens)
            }
        })
        .collect()
}

pub fn shares_artist(wanted: &str, got: &str) -> bool {
    let want = artist_names(wanted);
    let have = artist_names(got);
    if want.is_empty() || have.is_empty() {
        return false;
    }
    for w in &want {
        for h in &have {
            // subslice containment
            let w_match = h.windows(w.len()).any(|win| win == w.as_slice());
            let h_match = w.windows(h.len()).any(|win| win == h.as_slice());
            if w_match || h_match {
                return true;
            }
        }
    }
    false
}

pub fn normalize_string(raw: &str) -> String {
    let re_punct = Regex::new(r"[^\p{L}\p{N}\s]").unwrap();
    let stripped = re_punct.replace_all(raw, " ");
    stripped.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn clean_title_for_search(raw: &str) -> String {
    let re = Regex::new(
        r"(?i)\s*[\(\[](official\s*(music\s*)?video|video|audio|official\s*audio|lyric\s*video|lyrics|visualizer|remaster(ed)?|hd|4k)[\)\]]",
    )
    .unwrap();
    re.replace_all(raw, "").trim().to_string()
}

pub fn searchable_title(title: &str, artist: &str) -> String {
    let parts = parse_title(title, artist);
    let mut words = parts.words;
    for v in parts.versions {
        words.push(v);
    }
    words.join(" ")
}

pub fn evaluate_track_match(
    target_title: &str,
    target_artist: &str,
    target_dur: Option<f64>,
    target_album: Option<&str>,
    cand_title: &str,
    cand_artist: &str,
    cand_dur: Option<f64>,
    cand_album: Option<&str>,
    is_video: bool,
) -> Result<i32, &'static str> {
    let wanted = parse_title(target_title, target_artist);
    let got = parse_title(cand_title, cand_artist);

    if wanted.core.is_empty() || got.core.is_empty() {
        return Err("empty core title after normalization");
    }

    // 1. Core Title Match
    let (is_title_match, exact_title) = if wanted.core == got.core {
        (true, true)
    } else {
        // Multi-word wanted title cannot match a single-word candidate
        if wanted.words.len() > 1 && got.words.len() <= 1 {
            return Err("multi-word wanted title cannot match single-word candidate");
        }
        let min_len = wanted.core.len().min(got.core.len());
        let max_len = wanted.core.len().max(got.core.len());
        let ratio = (min_len as f64) / (max_len as f64);

        if ratio >= 0.70 && (wanted.core.contains(&got.core) || got.core.contains(&wanted.core)) {
            if !shares_artist(target_artist, cand_artist) {
                return Err("partial title match rejected: artists do not match");
            }
            (true, false)
        } else {
            return Err("core title mismatch");
        }
    };

    if !is_title_match {
        return Err("title mismatch");
    }

    // 2. Strict Version Match (rejects unwanted remixes, live, acoustic, etc.)
    if wanted.versions != got.versions {
        return Err("version markers mismatch");
    }

    // 3. Artist Match
    let shared_artist = shares_artist(target_artist, cand_artist);
    let artist_score = if shared_artist {
        let norm_w = normalize_string(target_artist);
        let norm_g = normalize_string(cand_artist);
        if !norm_w.is_empty() && norm_w == norm_g {
            30
        } else {
            15
        }
    } else if let (Some(t_d), Some(c_d)) = (target_dur, cand_dur) {
        // BitChord Credit Disagree: exact runtime within 2s permits
        // soundtrack composer / singer discrepancy on non-video tracks.
        if !is_video && (t_d - c_d).abs() <= 2.0 && exact_title {
            -30
        } else {
            return Err("different artist and runtime diff exceeds 2s");
        }
    } else {
        return Err("artist mismatch without runtime verification");
    };

    // 4. Duration Match
    let mut dur_score = 0;
    if let (Some(t_d), Some(c_d)) = (target_dur, cand_dur) {
        let diff = (t_d - c_d).abs();
        if diff > 30.0 {
            return Err("duration difference exceeds 30 seconds");
        } else if diff <= 2.0 {
            dur_score = 40;
        } else if diff <= 5.0 {
            dur_score = 25;
        } else if diff <= 15.0 {
            dur_score = 10;
        }
    }

    // 5. Album Match
    let mut album_score = 0;
    if let (Some(t_a), Some(c_a)) = (target_album, cand_album) {
        let nw = normalize_string(t_a);
        let ng = normalize_string(c_a);
        if !nw.is_empty() && nw == ng {
            album_score = 35;
        }
    }

    let title_bonus = if exact_title { 50 } else { 10 };
    Ok(100 + title_bonus + artist_score + dur_score + album_score)
}

pub fn score_track_match(
    target_title: &str,
    target_artist: &str,
    target_dur: Option<f64>,
    target_album: Option<&str>,
    cand_title: &str,
    cand_artist: &str,
    cand_dur: Option<f64>,
    cand_album: Option<&str>,
    is_video: bool,
) -> Option<i32> {
    evaluate_track_match(
        target_title,
        target_artist,
        target_dur,
        target_album,
        cand_title,
        cand_artist,
        cand_dur,
        cand_album,
        is_video,
    )
    .ok()
}

fn has_version_keyword(s: &str, keyword: &str) -> bool {
    let lower = s.to_lowercase();
    match keyword {
        "remix" => {
            lower.contains("remix")
                || lower.contains("rmx")
                || lower.contains("re-mix")
                || lower.contains("club mix")
                || lower.contains("extended mix")
        }
        "live" => {
            let re = Regex::new(r"(?i)\b(live(\s+(at|from|in|version|audio|session))?)\b").unwrap();
            re.is_match(&lower)
        }
        "acoustic" => {
            lower.contains("acoustic") || lower.contains("unplugged") || lower.contains("stripped")
        }
        "slowed" => lower.contains("slowed") || lower.contains("slow down"),
        "sped_up" => {
            lower.contains("sped up") || lower.contains("speed up") || lower.contains("nightcore")
        }
        "instrumental" => lower.contains("instrumental") || lower.contains("karaoke"),
        "cover" => {
            let re = Regex::new(r"(?i)\b(cover|tribute)\b").unwrap();
            re.is_match(&lower)
        }
        _ => false,
    }
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
        let raw_lower = raw_quality_text.to_lowercase();
        let is_ddp = (raw_lower.contains("plus")
            || raw_lower.contains("dd+")
            || raw_lower.contains("ddp")
            || raw_lower.contains("eac3")
            || raw_lower.contains("e-ac-3"))
            && !raw_lower.contains("atmos");

        let prefix = if is_ddp { "DOLBY DIGITAL PLUS" } else { "DOLBY ATMOS" };

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
            return prefix.to_string();
        } else {
            return format!("{} · {}", prefix, parts.join(" · "));
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
        let spec_str =
            if !specs.is_empty() { format!(" {}", specs.join(" / ")) } else { "".to_string() };
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

fn parse_stream_response(
    val: serde_json::Value,
    target_title: Option<&str>,
) -> Option<(String, AddonStream, HashMap<String, String>)> {
    let target_has_remix = target_title.map_or(false, |t| has_version_keyword(t, "remix"));

    let is_candidate_acceptable = |c: &AddonStreamCandidate| -> bool {
        if c.url.as_ref().map_or(true, |u| u.trim().is_empty()) {
            return false;
        }
        if !target_has_remix {
            let label = format!(
                "{} {}",
                c.title.as_deref().unwrap_or_default(),
                c.quality.as_deref().unwrap_or_default()
            );
            if has_version_keyword(&label, "remix")
                || has_version_keyword(&label, "slowed")
                || has_version_keyword(&label, "sped_up")
            {
                return false;
            }
        }
        true
    };

    if let Ok(candidates) = serde_json::from_value::<Vec<AddonStreamCandidate>>(val.clone()) {
        let chosen = candidates.iter().find(|c| is_candidate_acceptable(c)).or_else(|| {
            candidates.iter().find(|c| c.url.as_ref().map_or(false, |u| !u.trim().is_empty()))
        });
        if let Some(first) = chosen {
            let url = first.url.clone().unwrap();
            let mut headers = HashMap::new();
            if let Some(bh) = &first.behavior_hints {
                if let Some(req_headers) = bh
                    .get("proxyHeaders")
                    .and_then(|p| p.get("request"))
                    .and_then(|r| r.as_object())
                {
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
        let chosen = candidates.iter().find(|c| is_candidate_acceptable(c)).or_else(|| {
            candidates.iter().find(|c| c.url.as_ref().map_or(false, |u| !u.trim().is_empty()))
        });
        if let Some(first) = chosen {
            let url = first.url.clone().unwrap();
            if let Some(bh) = &first.behavior_hints {
                if let Some(req_headers) = bh
                    .get("proxyHeaders")
                    .and_then(|p| p.get("request"))
                    .and_then(|r| r.as_object())
                {
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

/// Query a single addon with strict matching and latency guard.
async fn resolve_candidate_from_addon(
    client: &reqwest::Client,
    addon: &CommunityAddon,
    query_title: &str,
    artist: &str,
    album: Option<&str>,
    duration_secs: Option<f64>,
    quality_pref: &str,
    video_id: Option<&str>,
) -> Option<(i32, i32, PlaybackData)> {
    let addon_start = std::time::Instant::now();
    let base_url = normalize_base_url(&addon.base_url);
    let (tier, maybe_atmos) = match quality_pref {
        "DOLBY_ATMOS" => ("lossless", Some("auto")),
        "HI_RES_LOSSLESS" => ("hires", None),
        "LOSSLESS" | "VERY_HIGH" => ("lossless", None),
        "HIGH" => ("high", None),
        "LOW" => ("low", None),
        _ => ("lossless", None),
    };
    let atmos_query = maybe_atmos.map(|a| format!("&atmos={}", a)).unwrap_or_default();

    let p_artist = primary_artist(artist);
    let s_title = searchable_title(query_title, artist);

    // BitChord query convention: Title FIRST, artist SECOND!
    let primary_query = if !p_artist.is_empty() && !s_title.is_empty() {
        format!("{} {}", s_title, p_artist)
    } else if !s_title.is_empty() {
        s_title.clone()
    } else {
        p_artist.clone()
    };

    let mut queries = vec![primary_query];
    if !s_title.is_empty() && !queries.contains(&s_title) {
        queries.push(s_title);
    }

    let mut found_tracks = Vec::new();

    for q in queries {
        let search_url = format!(
            "{}/search?q={}&quality={}{}",
            base_url,
            urlencoding::encode(&q),
            tier,
            atmos_query
        );

        eprintln!(
            "[addon] [{}] Searching: \"{}\" (tier: {}, atmos: {:?})",
            addon.name, q, tier, maybe_atmos
        );

        let search_start = std::time::Instant::now();
        let req = browser_request(client, &search_url, ADDON_TIMEOUT_MS);
        if let Ok(res) = req.send().await {
            let elapsed = search_start.elapsed().as_millis();
            if res.status().is_success() {
                if let Ok(val) = res.json::<serde_json::Value>().await {
                    let tracks = parse_search_response(val);
                    eprintln!(
                        "[addon] [{}] Search completed in {}ms: {} candidate(s)",
                        addon.name,
                        elapsed,
                        tracks.len()
                    );
                    if !tracks.is_empty() {
                        found_tracks = tracks;
                        break;
                    }
                }
            } else {
                eprintln!(
                    "[addon] [{}] Search failed with HTTP {} in {}ms",
                    addon.name,
                    res.status(),
                    elapsed
                );
            }
        } else {
            eprintln!("[addon] [{}] Search timed out (>{}ms)", addon.name, ADDON_TIMEOUT_MS);
        }
    }

    let mut scored_candidates = Vec::new();

    for track in found_tracks {
        if track.id.is_empty() && track.resolved_stream_url().is_none() {
            continue;
        }
        let cand_raw_title = track.resolved_title();
        let cand_raw_artist = track.resolved_artist();

        match evaluate_track_match(
            query_title,
            artist,
            duration_secs,
            album,
            cand_raw_title,
            &cand_raw_artist,
            track.duration,
            Some(&track.album),
            false,
        ) {
            Ok(score) => {
                eprintln!(
                    "[addon] [{}] Match accepted: \"{}\" by \"{}\" ({:?}) -> SCORE: {}",
                    addon.name, cand_raw_title, cand_raw_artist, track.duration, score
                );
                scored_candidates.push((score, track));
            }
            Err(reason) => {
                eprintln!(
                    "[addon] [{}] Candidate \"{}\" by \"{}\" rejected: {}",
                    addon.name, cand_raw_title, cand_raw_artist, reason
                );
            }
        }
    }

    if scored_candidates.is_empty() {
        eprintln!(
            "[addon] [{}] No candidate passed match criteria for \"{}\"",
            addon.name, query_title
        );
    } else {
        // BitChord rule: if any candidate matches credited artist, retain only credited artist rows
        let has_artist_match =
            scored_candidates.iter().any(|(_, c)| shares_artist(artist, &c.resolved_artist()));
        if has_artist_match {
            scored_candidates.retain(|(_, c)| shares_artist(artist, &c.resolved_artist()));
        }

        if quality_pref == "DOLBY_ATMOS" {
            scored_candidates.sort_by(|a, b| {
                let a_atmos = a.1.is_dolby_atmos();
                let b_atmos = b.1.is_dolby_atmos();
                if a_atmos != b_atmos {
                    b_atmos.cmp(&a_atmos)
                } else {
                    b.0.cmp(&a.0)
                }
            });
        } else {
            scored_candidates.sort_by(|a, b| b.0.cmp(&a.0));
        }
    }

    // Evaluate top candidates for stream (up to 3)
    for (score, candidate) in scored_candidates.into_iter().take(3) {
        let candidate_title = candidate.resolved_title().to_string();
        let candidate_artist = candidate.resolved_artist();
        let candidate_artwork = candidate.resolved_artwork();

        let (stream_url, addon_stream, custom_headers) = if let Some(direct_url) =
            candidate.resolved_stream_url()
        {
            if !direct_url.trim().is_empty() {
                eprintln!("[addon] [{}] Using search result streamURL directly", addon.name);
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
            let stream_ep = format!(
                "{}/stream/{}?quality={}{}",
                base_url,
                urlencoding::encode(&candidate.id),
                tier,
                atmos_query
            );

            eprintln!("[addon] [{}] Requesting stream: id=\"{}\" ...", addon.name, candidate.id);

            let stream_start = std::time::Instant::now();
            let mut stream_res = None;

            let req = browser_request(client, &stream_ep, ADDON_TIMEOUT_MS);
            if let Ok(res) = req.send().await {
                let elapsed = stream_start.elapsed().as_millis();
                if res.status().is_success() {
                    if let Ok(val) = res.json::<serde_json::Value>().await {
                        if let Some(resolved) = parse_stream_response(val, Some(query_title)) {
                            eprintln!(
                                "[addon] [{}] Stream endpoint OK in {}ms",
                                addon.name, elapsed
                            );
                            stream_res = Some(resolved);
                        }
                    }
                } else {
                    eprintln!(
                        "[addon] [{}] Stream endpoint returned HTTP {} in {}ms",
                        addon.name,
                        res.status(),
                        elapsed
                    );
                }
            } else {
                eprintln!(
                    "[addon] [{}] Stream endpoint timed out (>{}ms)",
                    addon.name, ADDON_TIMEOUT_MS
                );
            }

            // Stremio ID fallback
            if stream_res.is_none()
                && (candidate.id.starts_with("tt") || candidate.id.starts_with("yt_id:"))
            {
                let stremio_ep = format!(
                    "{}/stream/music/{}.json",
                    base_url,
                    urlencoding::encode(&candidate.id)
                );
                let req = browser_request(client, &stremio_ep, ADDON_TIMEOUT_MS);
                if let Ok(res) = req.send().await {
                    if res.status().is_success() {
                        if let Ok(val) = res.json::<serde_json::Value>().await {
                            stream_res = parse_stream_response(val, Some(query_title));
                        }
                    }
                }
            }

            match stream_res {
                Some(r) => r,
                None => {
                    eprintln!(
                        "[addon] [{}] Stream unavailable for id=\"{}\"",
                        addon.name, candidate.id
                    );
                    continue;
                }
            }
        };

        let combined_quality_text = format!(
            "{} {} {} {} {} {} {} {}",
            addon_stream.quality.as_deref().unwrap_or_default(),
            addon_stream.stream_quality.as_deref().unwrap_or_default(),
            addon_stream.audio_quality.as_deref().unwrap_or_default(),
            addon_stream.format.as_deref().unwrap_or_default(),
            addon_stream.codec.as_deref().unwrap_or_default(),
            candidate.audio_quality.as_deref().unwrap_or_default(),
            candidate.format.as_deref().unwrap_or_default(),
            addon_stream.audio_mode.as_deref().unwrap_or_default(),
        );

        let is_atmos = addon_stream.is_dolby_atmos() || candidate.is_dolby_atmos();

        // If Hi-Res Lossless or Lossless is requested, do not consider Dolby Atmos at all
        if quality_pref != "DOLBY_ATMOS" && is_atmos {
            continue;
        }

        // If Dolby Atmos is requested, prefer Atmos tracks
        if quality_pref == "DOLBY_ATMOS" && !is_atmos {
            eprintln!(
                "[addon] [{}] Candidate \"{}\" was not Atmos; trying next candidate",
                addon.name, candidate_title
            );
            continue;
        }

        let codec = addon_stream
            .codec
            .as_deref()
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

        eprintln!(
            "[addon] [{}] Quality resolved: {} (codec: {:?}) in {}ms total",
            addon.name,
            quality_label,
            codec,
            addon_start.elapsed().as_millis()
        );

        let mut final_headers = HashMap::new();
        final_headers.insert("User-Agent".to_string(), ADDON_UA.to_string());
        for (k, v) in custom_headers {
            final_headers.insert(k, v);
        }

        let cand_dur_str = candidate.duration.map(|d| {
            let sec = d.round() as u64;
            format!("{}:{:02}", sec / 60, sec % 60)
        });

        let quality_tier_rank = if is_atmos && quality_pref == "DOLBY_ATMOS" {
            3
        } else if quality_label.contains("HI-RES") {
            2
        } else if quality_label.contains("LOSSLESS") {
            1
        } else {
            0
        };

        return Some((
            quality_tier_rank,
            score,
            PlaybackData {
                video_id: if !candidate.id.is_empty() {
                    candidate.id
                } else {
                    video_id.unwrap_or_default().to_string()
                },
                stream_url,
                itag: 0,
                headers: final_headers,
                expires_in_seconds: 7200,
                loudness_db: None,
                playback_ping: None,
                title: if !candidate_title.is_empty() {
                    Some(candidate_title)
                } else {
                    Some(query_title.to_string())
                },
                artists: if !candidate_artist.is_empty() {
                    Some(candidate_artist)
                } else {
                    Some(artist.to_string())
                },
                duration: cand_dur_str,
                thumbnail: candidate_artwork,
                is_video: Some(false),
                stream_client: format!("addon:{}", addon.name),
                audio_quality: Some(quality_label),
            },
        ));
    }

    // Direct stream endpoint for video plugins
    if let Some(vid) = video_id {
        if !vid.is_empty() {
            let ep = format!(
                "{}/stream/{}?quality={}{}",
                base_url,
                urlencoding::encode(vid),
                tier,
                atmos_query
            );
            if let Ok(res) = browser_request(client, &ep, ADDON_TIMEOUT_MS).send().await {
                if res.status().is_success() {
                    if let Ok(val) = res.json::<serde_json::Value>().await {
                        if let Some((stream_url, addon_stream, custom_headers)) =
                            parse_stream_response(val, Some(query_title))
                        {
                            let mut final_headers = HashMap::new();
                            final_headers.insert("User-Agent".to_string(), ADDON_UA.to_string());
                            for (k, v) in custom_headers {
                                final_headers.insert(k, v);
                            }
                            let is_atmos = addon_stream.is_dolby_atmos();
                            if quality_pref != "DOLBY_ATMOS" && is_atmos {
                                return None;
                            }
                            if quality_pref == "DOLBY_ATMOS" && !is_atmos {
                                return None;
                            }
                            let combined_quality_text = format!(
                                "{} {} {} {} {} {}",
                                addon_stream.quality.as_deref().unwrap_or_default(),
                                addon_stream.stream_quality.as_deref().unwrap_or_default(),
                                addon_stream.audio_quality.as_deref().unwrap_or_default(),
                                addon_stream.format.as_deref().unwrap_or_default(),
                                addon_stream.codec.as_deref().unwrap_or_default(),
                                addon_stream.audio_mode.as_deref().unwrap_or_default(),
                            );
                            let codec = addon_stream
                                .codec
                                .as_deref()
                                .or(addon_stream.file_codec.as_deref())
                                .or(addon_stream.format.as_deref());
                            let sample_rate =
                                parse_sample_rate(addon_stream.sample_rate, &combined_quality_text);
                            let bit_depth =
                                parse_bit_depth(addon_stream.bit_depth, &combined_quality_text);
                            let bitrate =
                                parse_bitrate_kbps(addon_stream.bitrate, &combined_quality_text);
                            let quality_label = format_quality_label(
                                codec,
                                sample_rate,
                                bit_depth,
                                bitrate,
                                is_atmos,
                                &combined_quality_text,
                            );
                            let quality_tier_rank = if is_atmos && quality_pref == "DOLBY_ATMOS" {
                                3
                            } else if quality_label.contains("HI-RES") {
                                2
                            } else if quality_label.contains("LOSSLESS") {
                                1
                            } else {
                                0
                            };
                            eprintln!(
                                "[addon] [{}] Direct stream resolved for video_id=\"{}\": {}",
                                addon.name, vid, quality_label
                            );
                            return Some((
                                quality_tier_rank,
                                100,
                                PlaybackData {
                                    video_id: vid.to_string(),
                                    stream_url,
                                    itag: 0,
                                    headers: final_headers,
                                    expires_in_seconds: 7200,
                                    loudness_db: None,
                                    playback_ping: None,
                                    title: Some(query_title.to_string()),
                                    artists: Some(artist.to_string()),
                                    duration: duration_secs.map(|d| {
                                        let sec = d.round() as u64;
                                        format!("{}:{:02}", sec / 60, sec % 60)
                                    }),
                                    thumbnail: None,
                                    is_video: Some(false),
                                    stream_client: format!("addon:{}", addon.name),
                                    audio_quality: Some(quality_label),
                                },
                            ));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Simultaneously queries all enabled addons.
/// If any addon yields a high-accuracy match, the top stream is returned immediately.
/// If no addon yields an acceptable candidate, immediately snaps to None so the player
/// falls back to YouTube Music with zero perceptible delay.
pub async fn resolve_from_addons(
    db: &Db,
    title: &str,
    artist: &str,
    album: Option<&str>,
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
    if query_title.trim().is_empty() && video_id.unwrap_or_default().is_empty() {
        return None;
    }

    let cache_key = format!(
        "{}:{}:{}:{}",
        query_title.to_lowercase(),
        artist.to_lowercase(),
        quality_pref,
        video_id.unwrap_or_default()
    );
    if let Some(cached) = get_cached_addon_stream(&cache_key) {
        eprintln!("[addon] ── In-memory cache hit for \"{}\" [{}] ──", query_title, quality_pref);
        return Some(cached);
    }

    let start_total = std::time::Instant::now();
    eprintln!("[addon] ════════════════════════════════════════════════════════════════");
    eprintln!(
        "[addon] Resolving: \"{}\" by \"{}\" (tier: {}, dur: {:?}s) across {} addon(s)",
        query_title,
        artist,
        quality_pref,
        duration_secs,
        active_addons.len()
    );

    let client = crate::http::client();

    let futures: Vec<_> = active_addons
        .iter()
        .map(|addon| {
            let client = client.clone();
            let addon = addon.clone();
            let query_title = query_title.to_string();
            let artist = artist.to_string();
            let album = album.map(|s| s.to_string());
            let quality_pref = quality_pref.to_string();
            let video_id = video_id.map(|s| s.to_string());
            async move {
                resolve_candidate_from_addon(
                    &client,
                    &addon,
                    &query_title,
                    &artist,
                    album.as_deref(),
                    duration_secs,
                    &quality_pref,
                    video_id.as_deref(),
                )
                .await
            }
        })
        .collect();

    let results = futures_util::future::join_all(futures).await;
    let mut best: Option<(i32, i32, PlaybackData)> = None;

    for candidate in results.into_iter().flatten() {
        let is_better = match &best {
            None => true,
            Some((held_rank, held_score, _)) => {
                if candidate.0 != *held_rank {
                    candidate.0 > *held_rank
                } else {
                    candidate.1 > *held_score
                }
            }
        };
        if is_better {
            best = Some(candidate);
        }
    }

    let total_elapsed = start_total.elapsed().as_millis();
    if let Some((_, _, ref data)) = best {
        eprintln!(
            "[addon] ── OK: Selected \"{}\" from {} in {}ms ──",
            data.audio_quality.as_deref().unwrap_or("Lossless"),
            data.stream_client,
            total_elapsed
        );
        eprintln!("[addon] ════════════════════════════════════════════════════════════════");
        cache_addon_stream(cache_key, data.clone());
    } else {
        eprintln!(
            "[addon] ── No addon matched for \"{}\" in {}ms. Snapping to YouTube Music. ──",
            query_title, total_elapsed
        );
        eprintln!("[addon] ════════════════════════════════════════════════════════════════");
    }

    best.map(|(_, _, data)| data)
}

/// Waterfall HQ resolver: attempts the requested tier (e.g. DOLBY_ATMOS)
/// and cascades down gracefully to HI_RES_LOSSLESS and LOSSLESS.
pub async fn resolve_from_addons_waterfall(
    db: &Db,
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration_secs: Option<f64>,
    target_tier: &str,
    video_id: Option<&str>,
) -> Option<PlaybackData> {
    if !is_addons_enabled(db) {
        return None;
    }

    match target_tier {
        "DOLBY_ATMOS" => {
            // Waterfall tree: 1. Atmos -> 2. Hi-Res -> 3. Lossless -> 4. High
            if let Some(res) = resolve_from_addons(
                db,
                title,
                artist,
                album,
                duration_secs,
                "DOLBY_ATMOS",
                video_id,
            )
            .await
            {
                let q_upper = res.audio_quality.as_deref().unwrap_or_default().to_uppercase();
                if is_atmos_text(&q_upper) {
                    return Some(res);
                }
            }
            if let Some(res) = resolve_from_addons(
                db,
                title,
                artist,
                album,
                duration_secs,
                "HI_RES_LOSSLESS",
                video_id,
            )
            .await
            {
                let q_upper = res.audio_quality.as_deref().unwrap_or_default().to_uppercase();
                if q_upper.contains("HI-RES") {
                    return Some(res);
                }
            }
            resolve_from_addons(db, title, artist, album, duration_secs, "LOSSLESS", video_id).await
        }
        "HI_RES_LOSSLESS" => {
            // If Hi-Res, do not think about Dolby at all: Hi-Res -> Lossless -> High
            if let Some(res) = resolve_from_addons(
                db,
                title,
                artist,
                album,
                duration_secs,
                "HI_RES_LOSSLESS",
                video_id,
            )
            .await
            {
                let q_upper = res.audio_quality.as_deref().unwrap_or_default().to_uppercase();
                if q_upper.contains("HI-RES") {
                    return Some(res);
                }
            }
            resolve_from_addons(db, title, artist, album, duration_secs, "LOSSLESS", video_id).await
        }
        "LOSSLESS" | "VERY_HIGH" => {
            resolve_from_addons(db, title, artist, album, duration_secs, "LOSSLESS", video_id).await
        }
        _ => {
            resolve_from_addons(db, title, artist, album, duration_secs, target_tier, video_id)
                .await
        }
    }
}
