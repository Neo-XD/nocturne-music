use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f32,
    pub status: String, // "downloading", "complete", "error"
    pub path: Option<String>,
}

fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let s: String =
        name.chars().map(|c| if invalid_chars.contains(&c) { '_' } else { c }).collect();
    let trimmed = s.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "track".to_string()
    } else {
        trimmed
    }
}

pub fn get_default_download_dir(app: &AppHandle) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(mut p) = dirs::audio_dir() {
            p.push("Nocturne");
            return p;
        }
    }
    if let Some(mut p) = dirs::download_dir() {
        p.push("Nocturne");
        return p;
    }
    if let Ok(mut p) = app.path().app_data_dir() {
        p.push("downloads");
        return p;
    }
    PathBuf::from("Nocturne_Music")
}

pub async fn download_song_track(
    app: AppHandle,
    state: std::sync::Arc<AppState>,
    video_id: String,
    title: String,
    artist: String,
    custom_dir: Option<String>,
) -> Result<String, String> {
    // 1. Resolve audio stream
    let playback_data = state
        .resolve(&video_id, false)
        .await
        .map_err(|e| format!("Failed to resolve audio stream: {e}"))?;

    let stream_url = playback_data.stream_url;
    if stream_url.is_empty() {
        return Err("Resolved stream URL is empty".into());
    }

    // Determine extension based on itag
    let ext = match playback_data.itag {
        140 => "m4a",
        251 => "webm",
        171 => "webm",
        249 => "webm",
        250 => "webm",
        _ => "m4a",
    };

    let dir = match custom_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => get_default_download_dir(&app),
    };

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create download folder {:?}: {e}", dir))?;

    let filename =
        format!("{} - {}.{}", sanitize_filename(&artist), sanitize_filename(&title), ext);
    let target_path = dir.join(&filename);

    // Initial progress event
    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            video_id: video_id.clone(),
            title: title.clone(),
            artist: artist.clone(),
            downloaded_bytes: 0,
            total_bytes: None,
            percent: 0.0,
            status: "downloading".into(),
            path: None,
        },
    );

    let client = crate::http::client();
    let mut req = client.get(&stream_url);
    for (k, v) in &playback_data.headers {
        req = req.header(k, v);
    }

    let res = req.send().await.map_err(|e| format!("Download connection failed: {e}"))?;

    if !res.status().is_success() {
        let err_msg = format!("Server returned HTTP {}", res.status());
        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                video_id: video_id.clone(),
                title: title.clone(),
                artist: artist.clone(),
                downloaded_bytes: 0,
                total_bytes: None,
                percent: 0.0,
                status: "error".into(),
                path: None,
            },
        );
        return Err(err_msg);
    }

    let total_bytes = res.content_length();
    let mut stream = res.bytes_stream();
    use futures_util::StreamExt;
    use std::io::Write;

    let mut file = std::fs::File::create(&target_path)
        .map_err(|e| format!("Failed to create file {:?}: {e}", target_path))?;

    let mut downloaded_bytes: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Download stream error: {e}"))?;
        file.write_all(&chunk).map_err(|e| format!("Disk write error: {e}"))?;

        downloaded_bytes += chunk.len() as u64;

        if last_emit.elapsed().as_millis() >= 250 {
            let pct = match total_bytes {
                Some(total) if total > 0 => {
                    ((downloaded_bytes as f64 / total as f64) * 100.0) as f32
                }
                _ => 0.0,
            };

            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    video_id: video_id.clone(),
                    title: title.clone(),
                    artist: artist.clone(),
                    downloaded_bytes,
                    total_bytes,
                    percent: pct,
                    status: "downloading".into(),
                    path: None,
                },
            );
            last_emit = std::time::Instant::now();
        }
    }

    file.flush().map_err(|e| format!("Failed to flush file to disk: {e}"))?;

    let saved_path_str = target_path.to_string_lossy().to_string();

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            video_id: video_id.clone(),
            title: title.clone(),
            artist: artist.clone(),
            downloaded_bytes,
            total_bytes: Some(downloaded_bytes),
            percent: 100.0,
            status: "complete".into(),
            path: Some(saved_path_str.clone()),
        },
    );

    Ok(saved_path_str)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistDownloadProgress {
    pub playlist_name: String,
    pub total_tracks: usize,
    pub completed_tracks: usize,
    pub current_track: Option<String>,
    pub status: String, // "starting", "downloading", "complete", "error"
    pub error: Option<String>,
    pub destination_dir: String,
}

pub async fn download_full_playlist(
    app: AppHandle,
    state: std::sync::Arc<AppState>,
    items: Vec<innertube::SongItem>,
    playlist_name: String,
    custom_dir: Option<String>,
) -> Result<String, String> {
    if items.is_empty() {
        return Err("Playlist has no tracks to download".into());
    }

    let base_dir = match custom_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => get_default_download_dir(&app),
    };

    let playlist_dir = base_dir.join(sanitize_filename(&playlist_name));
    std::fs::create_dir_all(&playlist_dir)
        .map_err(|e| format!("Could not create folder {:?}: {e}", playlist_dir))?;

    let playlist_dir_str = playlist_dir.to_string_lossy().to_string();
    let total_tracks = items.len();

    let _ = app.emit(
        "download-playlist-progress",
        PlaylistDownloadProgress {
            playlist_name: playlist_name.clone(),
            total_tracks,
            completed_tracks: 0,
            current_track: None,
            status: "starting".into(),
            error: None,
            destination_dir: playlist_dir_str.clone(),
        },
    );

    let mut completed_tracks = 0;
    for item in items {
        let title = item.title.clone();
        let artist = item.artists.clone();
        let video_id = item.video_id.clone();

        let _ = app.emit(
            "download-playlist-progress",
            PlaylistDownloadProgress {
                playlist_name: playlist_name.clone(),
                total_tracks,
                completed_tracks,
                current_track: Some(format!("{} - {}", artist, title)),
                status: "downloading".into(),
                error: None,
                destination_dir: playlist_dir_str.clone(),
            },
        );

        match download_song_track(
            app.clone(),
            state.clone(),
            video_id,
            title,
            artist,
            Some(playlist_dir_str.clone()),
        )
        .await
        {
            Ok(_) => {
                completed_tracks += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to download track in playlist");
            }
        }
    }

    let _ = app.emit(
        "download-playlist-progress",
        PlaylistDownloadProgress {
            playlist_name: playlist_name.clone(),
            total_tracks,
            completed_tracks,
            current_track: None,
            status: "complete".into(),
            error: None,
            destination_dir: playlist_dir_str.clone(),
        },
    );

    Ok(playlist_dir_str)
}

pub fn show_in_folder(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("explorer");
        if p.is_file() {
            cmd.arg(format!("/select,{}", path));
        } else {
            cmd.arg(path);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if p.is_file() {
            cmd.arg("-R").arg(path);
        } else {
            cmd.arg(path);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        let target = if p.is_file() { p.parent().unwrap_or(p) } else { p };
        std::process::Command::new("xdg-open").arg(target).spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
}
