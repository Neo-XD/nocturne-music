use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizedSong {
    pub found: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover: Option<String>,
    pub query: Option<String>,
}

fn random_uuid_v4() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rand::Rng::gen(&mut rng);
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        (bytes[6] & 0x0f) | 0x40, bytes[7],
        (bytes[8] & 0x3f) | 0x80, bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

#[tauri::command]
pub async fn recognize_song_signature(
    _state: tauri::State<'_, Arc<AppState>>,
    signature_uri: String,
    sample_ms: Option<u32>,
) -> Result<RecognizedSong, String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let sample_duration = sample_ms.unwrap_or(4500);

    let payload = serde_json::json!({
        "geolocation": {
            "altitude": 300,
            "latitude": 45,
            "longitude": 2
        },
        "signature": {
            "samplems": sample_duration,
            "timestamp": (now_ms & 0xffffffff) as u32,
            "uri": signature_uri
        },
        "timestamp": (now_ms & 0xffffffff) as u32,
        "timezone": "Europe/Paris"
    });

    let uuid1 = random_uuid_v4().to_uppercase();
    let uuid2 = random_uuid_v4().to_lowercase();

    let url = format!(
        "https://amp.shazam.com/discovery/v5/en/US/android/-/tag/{}/{}\
?sync=true\
&webv3=true\
&sampling=true\
&connected=\
&shazamapiversion=v3\
&sharehub=true\
&video=v3",
        uuid1, uuid2
    );

    let client = crate::http::client();
    let res = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Content-Language", "en_US")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Linux; Android 10; SM-G973F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.210 Mobile Safari/537.36 Shazam/11.23.0-210506",
        )
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network error querying Shazam: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("Shazam server responded with status {}", res.status()));
    }

    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Shazam JSON: {e}"))?;

    if let Some(track) = json.get("track") {
        let title = track["title"].as_str().map(|s| s.to_string());
        let artist = track["subtitle"].as_str().map(|s| s.to_string());
        let cover = track["images"]["coverart"]
            .as_str()
            .or_else(|| track["images"]["coverarthq"].as_str())
            .map(|s| s.to_string());

        let album = track["sections"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["metadata"].as_array())
            .and_then(|meta| {
                meta.iter().find_map(|m| {
                    if m["title"].as_str() == Some("Album") {
                        m["text"].as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            });

        let query = match (&title, &artist) {
            (Some(t), Some(a)) => Some(format!("{t} {a}")),
            (Some(t), None) => Some(t.clone()),
            _ => None,
        };

        return Ok(RecognizedSong {
            found: title.is_some(),
            title,
            artist,
            album,
            cover,
            query,
        });
    }

    Ok(RecognizedSong {
        found: false,
        title: None,
        artist: None,
        album: None,
        cover: None,
        query: None,
    })
}
