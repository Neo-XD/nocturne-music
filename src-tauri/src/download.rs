use crate::state::AppState;
use lofty::file::TaggedFileExt;
use lofty::tag::TagExt;
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

fn parse_duration_secs(d: Option<&str>) -> i64 {
    let Some(s) = d else { return 0 };
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        2 => {
            let m: i64 = parts[0].parse().unwrap_or(0);
            let s: i64 = parts[1].parse().unwrap_or(0);
            m * 60 + s
        }
        3 => {
            let h: i64 = parts[0].parse().unwrap_or(0);
            let m: i64 = parts[1].parse().unwrap_or(0);
            let s: i64 = parts[2].parse().unwrap_or(0);
            h * 3600 + m * 60 + s
        }
        _ => parts.first().and_then(|p| p.parse().ok()).unwrap_or(0),
    }
}

pub async fn download_song_track(
    app: AppHandle,
    state: std::sync::Arc<AppState>,
    video_id: String,
    title: String,
    artist: String,
    album: Option<String>,
    duration: Option<String>,
    thumbnail: Option<String>,
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

    let effective_artist = if artist.trim().is_empty() {
        playback_data.artists.clone().unwrap_or_else(|| "Unknown Artist".into())
    } else {
        artist.clone()
    };
    let effective_title = if title.trim().is_empty() {
        playback_data.title.clone().unwrap_or_else(|| "Track".into())
    } else {
        title.clone()
    };
    let effective_album = album.or_else(|| playback_data.title.clone());
    let effective_duration = duration.or_else(|| playback_data.duration.clone());

    let filename = format!(
        "{} - {}.{}",
        sanitize_filename(&effective_artist),
        sanitize_filename(&effective_title),
        ext
    );
    let target_path = dir.join(&filename);

    // Initial progress event
    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            video_id: video_id.clone(),
            title: effective_title.clone(),
            artist: effective_artist.clone(),
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
                title: effective_title.clone(),
                artist: effective_artist.clone(),
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
                    title: effective_title.clone(),
                    artist: effective_artist.clone(),
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

    // Download and save thumbnail locally for offline use
    let covers_dir = crate::local::covers_dir(&app);
    let _ = std::fs::create_dir_all(&covers_dir);
    let thumb_url = thumbnail.clone().or_else(|| playback_data.thumbnail.clone());
    let mut local_thumbnail = None;
    if let Some(ref url) = thumb_url {
        if url.starts_with("http://") || url.starts_with("https://") {
            let cover_filename = format!("download_{}.jpg", sanitize_filename(&video_id));
            let cover_path = covers_dir.join(&cover_filename);
            if let Ok(thumb_res) = client.get(url).send().await {
                if thumb_res.status().is_success() {
                    if let Ok(bytes) = thumb_res.bytes().await {
                        if std::fs::write(&cover_path, &bytes).is_ok() {
                            local_thumbnail = Some(cover_path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        } else {
            local_thumbnail = Some(url.clone());
        }
    }
    if local_thumbnail.is_none() {
        local_thumbnail = thumb_url;
    }

    // Attempt to write tags using lofty
    let duration_secs = parse_duration_secs(effective_duration.as_deref());
    if let Ok(mut tagged_file) = lofty::probe::Probe::open(&target_path).and_then(|p| p.read()) {
        use lofty::tag::Accessor;
        let tag = match tagged_file.primary_tag_mut() {
            Some(t) => t,
            None => {
                if let Some(first_tag) = tagged_file.first_tag_mut() {
                    first_tag
                } else {
                    let tag_type = tagged_file.primary_tag_type();
                    tagged_file.insert_tag(lofty::tag::Tag::new(tag_type));
                    tagged_file.primary_tag_mut().unwrap()
                }
            }
        };
        tag.set_title(effective_title.clone());
        tag.set_artist(effective_artist.clone());
        if let Some(ref alb) = effective_album {
            tag.set_album(alb.clone());
        }
        let _ = tag.save_to_path(&target_path, lofty::config::WriteOptions::default());
    }

    // Save full metadata in SQLite
    let record = crate::db::DownloadedTrack {
        video_id: video_id.clone(),
        title: effective_title.clone(),
        artist: effective_artist.clone(),
        album: effective_album,
        duration: effective_duration,
        duration_secs,
        thumbnail: local_thumbnail.clone(),
        path: saved_path_str.clone(),
        file_size: downloaded_bytes,
        downloaded_at: crate::db::now_secs(),
    };
    state.db.save_downloaded_track(&record);

    // Register asset scope
    let scope = app.asset_protocol_scope();
    let _ = scope.allow_file(&target_path);
    let _ = scope.allow_directory(&dir, true);
    if let Some(ref thumb_path) = local_thumbnail {
        if Path::new(thumb_path).is_file() {
            let _ = scope.allow_file(thumb_path);
        }
    }

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            video_id: video_id.clone(),
            title: effective_title,
            artist: effective_artist,
            downloaded_bytes,
            total_bytes: Some(downloaded_bytes),
            percent: 100.0,
            status: "complete".into(),
            path: Some(saved_path_str.clone()),
        },
    );
    let _ = app.emit("downloaded-songs-changed", ());

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
            item.album.clone(),
            item.duration.clone(),
            item.thumbnail.clone(),
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

pub fn get_downloaded_file(
    db: &crate::db::Db,
    video_id: &str,
) -> Option<crate::db::DownloadedTrack> {
    db.get_downloaded_track(video_id)
}

pub fn scan_downloaded_songs(
    app: &AppHandle,
    db: &crate::db::Db,
) -> Vec<crate::db::DownloadedTrack> {
    // 1. Prune missing files from DB
    let _ = db.prune_missing_downloaded_tracks();

    // 2. Scan download directory for audio files
    let download_dir = get_default_download_dir(app);
    let scope = app.asset_protocol_scope();
    let _ = scope.allow_directory(&download_dir, true);
    let covers_dir = crate::local::covers_dir(app);
    let _ = scope.allow_directory(&covers_dir, true);

    if let Ok(entries) = std::fs::read_dir(&download_dir) {
        let existing = db.get_all_downloaded_tracks();
        let existing_paths: std::collections::HashSet<String> =
            existing.into_iter().map(|t| t.path).collect();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !["mp3", "m4a", "webm", "opus", "flac", "ogg", "wav", "aac"].contains(&ext.as_str())
            {
                continue;
            }

            let path_str = path.to_string_lossy().to_string();
            if existing_paths.contains(&path_str) {
                continue;
            }

            let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let mut title = None;
            let mut artist = None;
            let mut album = None;
            let mut duration_secs = 0;

            if let Ok(tagged_file) = lofty::probe::Probe::open(&path).and_then(|p| p.read()) {
                use lofty::file::AudioFile;
                use lofty::tag::Accessor;
                duration_secs = tagged_file.properties().duration().as_secs() as i64;
                if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
                    title = tag.title().map(|s| s.to_string());
                    artist = tag.artist().map(|s| s.to_string());
                    album = tag.album().map(|s| s.to_string());
                }
            }

            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Track");
            let (f_artist, f_title) = if let Some((a, t)) = file_stem.split_once(" - ") {
                (a.trim().to_string(), t.trim().to_string())
            } else {
                ("Unknown Artist".to_string(), file_stem.to_string())
            };

            let final_title = title.unwrap_or(f_title);
            let final_artist = artist.unwrap_or(f_artist);
            let duration = if duration_secs > 0 {
                let mins = duration_secs / 60;
                let secs = duration_secs % 60;
                Some(format!("{}:{:02}", mins, secs))
            } else {
                None
            };

            let video_id = format!("DL:{}", path_str);

            let record = crate::db::DownloadedTrack {
                video_id,
                title: final_title,
                artist: final_artist,
                album,
                duration,
                duration_secs,
                thumbnail: None,
                path: path_str,
                file_size,
                downloaded_at: crate::db::now_secs(),
            };
            db.save_downloaded_track(&record);
        }
    }

    db.get_all_downloaded_tracks()
}

pub fn delete_downloaded_song(
    app: &AppHandle,
    db: &crate::db::Db,
    video_id: &str,
) -> Result<(), String> {
    if let Some(track) = db.get_downloaded_track(video_id) {
        let p = Path::new(&track.path);
        if p.is_file() {
            let _ = std::fs::remove_file(p);
        }
        if let Some(ref thumb) = track.thumbnail {
            let tp = Path::new(thumb);
            if tp.is_file() && thumb.contains("download_") {
                let _ = std::fs::remove_file(tp);
            }
        }
    }
    db.delete_downloaded_track(video_id);
    let _ = app.emit("downloaded-songs-changed", ());
    Ok(())
}
