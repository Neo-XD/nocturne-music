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

pub fn get_offline_cache_dir(app: &AppHandle) -> PathBuf {
    if let Ok(mut p) = app.path().app_data_dir() {
        p.push("offline_cache");
        return p;
    }
    PathBuf::from("Nocturne_Offline_Cache")
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
    quality: Option<String>,
) -> Result<String, String> {
    let effective_artist =
        if artist.trim().is_empty() { "Unknown Artist".to_string() } else { artist.clone() };
    let effective_title = if title.trim().is_empty() { "Track".to_string() } else { title.clone() };

    // 0. Check if this track is already downloaded to disk
    if let Some(existing) = get_downloaded_file(&app, &state.db, &video_id) {
        if Path::new(&existing.path).is_file() {
            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    video_id: video_id.clone(),
                    title: existing.title.clone(),
                    artist: existing.artist.clone(),
                    downloaded_bytes: existing.file_size,
                    total_bytes: Some(existing.file_size),
                    percent: 100.0,
                    status: "complete".into(),
                    path: Some(existing.path.clone()),
                },
            );
            return Ok(existing.path);
        }
    }

    let duration_secs = parse_duration_secs(duration.as_deref());
    let dur_f64 = if duration_secs > 0 { Some(duration_secs as f64) } else { None };

    // 1. Resolve audio stream (with fallback retry on cache miss/stale URL)
    let playback_data = match state
        .resolve_for_download(
            &video_id,
            false,
            Some(&title),
            Some(&artist),
            dur_f64,
            quality.as_deref(),
        )
        .await
    {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!(error = ?e, "Initial resolve failed for download, attempting fresh resolve");
            state
                .resolve_for_download(
                    &video_id,
                    true,
                    Some(&title),
                    Some(&artist),
                    dur_f64,
                    quality.as_deref(),
                )
                .await
                .map_err(|err| format!("Failed to resolve audio stream: {err}"))?
        }
    };

    let stream_url = playback_data.stream_url.clone();
    if stream_url.is_empty() {
        return Err("Resolved stream URL is empty".into());
    }

    let dir = match custom_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => get_offline_cache_dir(&app),
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create download folder {:?}: {e}", dir))?;

    let effective_album = album.or_else(|| playback_data.title.clone());
    let effective_duration = duration.or_else(|| playback_data.duration.clone());

    let clean_vid = if video_id.starts_with("SP:") {
        let parts: Vec<&str> = video_id.split(':').collect();
        let track_id = parts.get(1).unwrap_or(&"unknown");
        format!("SP_{}", sanitize_filename(track_id))
    } else {
        sanitize_filename(&video_id)
    };

    // If stream_url is already a local file (e.g. from local library), copy to offline cache or use directly
    if stream_url.starts_with("file://") || Path::new(&stream_url).is_file() {
        let src_path = if let Some(stripped) = stream_url.strip_prefix("file://") {
            PathBuf::from(stripped)
        } else {
            PathBuf::from(&stream_url)
        };
        let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("m4a");
        let filename = format!("{}_{}.{}", clean_vid, sanitize_filename(&effective_title), ext);
        let target_path = dir.join(&filename);

        if src_path != target_path && src_path.is_file() {
            let _ = std::fs::copy(&src_path, &target_path);
        }
        let saved_path_str = target_path.to_string_lossy().to_string();
        let duration_secs = parse_duration_secs(effective_duration.as_deref());
        let file_size = std::fs::metadata(&target_path).map(|m| m.len()).unwrap_or(0);

        let record = crate::db::DownloadedTrack {
            video_id: video_id.clone(),
            title: effective_title.clone(),
            artist: effective_artist.clone(),
            album: effective_album,
            duration: effective_duration,
            duration_secs,
            thumbnail: thumbnail.clone(),
            path: saved_path_str.clone(),
            file_path: Some(saved_path_str.clone()),
            cover_path: thumbnail.clone(),
            file_size,
            downloaded_at: crate::db::now_secs(),
        };
        state.db.save_downloaded_track(&record);

        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                video_id: video_id.clone(),
                title: effective_title,
                artist: effective_artist,
                downloaded_bytes: file_size,
                total_bytes: Some(file_size),
                percent: 100.0,
                status: "complete".into(),
                path: Some(saved_path_str.clone()),
            },
        );
        let _ = app.emit("downloaded-songs-changed", ());
        return Ok(saved_path_str);
    }

    // Determine extension based on itag or addon stream quality/URL
    let mut ext = if playback_data.stream_client.starts_with("addon:") || playback_data.itag == 0 {
        let q_lower = playback_data.audio_quality.as_deref().unwrap_or_default().to_lowercase();
        let u_lower = playback_data.stream_url.to_lowercase();
        if u_lower.contains(".flac") || q_lower.contains("flac") || q_lower.contains("lossless") {
            "flac"
        } else if u_lower.contains(".mp3") || q_lower.contains("mp3") {
            "mp3"
        } else if u_lower.contains(".opus") || q_lower.contains("opus") {
            "opus"
        } else if u_lower.contains(".wav") || q_lower.contains("wav") {
            "wav"
        } else if u_lower.contains(".m4a") || u_lower.contains(".mp4") || q_lower.contains("aac") {
            "m4a"
        } else {
            "flac"
        }
    } else {
        match playback_data.itag {
            140 => "m4a",
            251 => "webm",
            171 => "webm",
            249 => "webm",
            250 => "webm",
            _ => "m4a",
        }
    };

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
    let mut current_playback = playback_data;
    let mut response_opt = None;

    // Try downloading; if server returns 403/410 or connection fails, evict cache and retry with fresh resolve
    for attempt in 0..2 {
        let raw_url = &current_playback.stream_url;
        // Leverage local audioproxy for un-throttled bounded chunk streaming only for googlevideo streams
        let download_url = if raw_url.contains("googlevideo.com") {
            crate::audioproxy::register(raw_url, &current_playback.headers)
                .unwrap_or_else(|| raw_url.clone())
        } else {
            raw_url.clone()
        };

        let mut req = client.get(&download_url);
        if download_url == *raw_url {
            for (k, v) in &current_playback.headers {
                req = req.header(k, v);
            }
        }

        match req.send().await {
            Ok(res) if res.status().is_success() => {
                response_opt = Some(res);
                break;
            }
            Ok(res) => {
                tracing::warn!(status = %res.status(), attempt, "Download request failed");
                if attempt == 0 {
                    state.db.evict_stream(&video_id);
                    if let Ok(fresh) = state
                        .resolve_for_download(
                            &video_id,
                            true,
                            Some(&title),
                            Some(&artist),
                            dur_f64,
                            quality.as_deref(),
                        )
                        .await
                    {
                        current_playback = fresh;
                        continue;
                    }
                }
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
            Err(e) => {
                tracing::warn!(error = %e, attempt, "Download connection error");
                if attempt == 0 {
                    state.db.evict_stream(&video_id);
                    if let Ok(fresh) = state
                        .resolve_for_download(
                            &video_id,
                            true,
                            Some(&title),
                            Some(&artist),
                            dur_f64,
                            quality.as_deref(),
                        )
                        .await
                    {
                        current_playback = fresh;
                        continue;
                    }
                }
                let err_msg = format!("Download connection failed: {e}");
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
        }
    }

    let res = match response_opt {
        Some(r) => r,
        None => return Err("Failed to obtain download response".into()),
    };

    if let Some(ct) = res.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        let ct_lower = ct.to_lowercase();
        if ct_lower.contains("audio/flac") || ct_lower.contains("audio/x-flac") {
            ext = "flac";
        } else if ct_lower.contains("audio/mpeg") || ct_lower.contains("audio/mp3") {
            ext = "mp3";
        } else if ct_lower.contains("audio/ogg") || ct_lower.contains("audio/opus") {
            ext = "opus";
        } else if ct_lower.contains("audio/wav") || ct_lower.contains("audio/x-wav") {
            ext = "wav";
        } else if ct_lower.contains("audio/mp4") || ct_lower.contains("audio/aac") || ct_lower.contains("audio/m4a") {
            ext = "m4a";
        }
    }

    let filename = format!("{}_{}.{}", clean_vid, sanitize_filename(&effective_title), ext);
    let target_path = dir.join(&filename);

    let total_bytes = res.content_length();
    let mut stream = res.bytes_stream();
    use futures_util::StreamExt;
    use std::io::Write;

    let mut file = std::fs::File::create(&target_path)
        .map_err(|e| format!("Failed to create file {:?}: {e}", target_path))?;

    let mut downloaded_bytes: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut first_bytes: Vec<u8> = Vec::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&target_path);
                let err_msg = format!("Download stream error: {e}");
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        video_id: video_id.clone(),
                        title: effective_title.clone(),
                        artist: effective_artist.clone(),
                        downloaded_bytes,
                        total_bytes,
                        percent: 0.0,
                        status: "error".into(),
                        path: None,
                    },
                );
                return Err(err_msg);
            }
        };

        if first_bytes.len() < 512 {
            first_bytes.extend_from_slice(&chunk);
        }

        if let Err(e) = file.write_all(&chunk) {
            drop(file);
            let _ = std::fs::remove_file(&target_path);
            let err_msg = format!("Disk write error: {e}");
            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    video_id: video_id.clone(),
                    title: effective_title.clone(),
                    artist: effective_artist.clone(),
                    downloaded_bytes,
                    total_bytes,
                    percent: 0.0,
                    status: "error".into(),
                    path: None,
                },
            );
            return Err(err_msg);
        }

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
    // Explicitly drop file handle to release Windows sharing locks before Lofty tag inspection
    drop(file);

    if downloaded_bytes < 4096 || !crate::cache::is_valid_audio_bytes(&first_bytes) {
        let _ = std::fs::remove_file(&target_path);
        let err_msg = "Downloaded file is not a valid audio stream (server returned HTML or corrupt data)".to_string();
        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                video_id: video_id.clone(),
                title: effective_title.clone(),
                artist: effective_artist.clone(),
                downloaded_bytes,
                total_bytes,
                percent: 0.0,
                status: "error".into(),
                path: None,
            },
        );
        return Err(err_msg);
    }

    // Align file extension with detected actual audio container bytes
    let mut final_target_path = target_path;
    if let Some(real_ext) = crate::cache::detect_audio_extension_from_bytes(&first_bytes) {
        if real_ext != ext {
            let corrected_name = format!("{}_{}.{}", clean_vid, sanitize_filename(&effective_title), real_ext);
            let corrected_path = dir.join(&corrected_name);
            if std::fs::rename(&final_target_path, &corrected_path).is_ok() {
                final_target_path = corrected_path;
            }
        }
    }

    let saved_path_str = final_target_path.to_string_lossy().to_string();

    // Download and save thumbnail locally for offline use
    let covers_dir = crate::local::covers_dir(&app);
    let _ = std::fs::create_dir_all(&covers_dir);
    let thumb_url = thumbnail.clone().or_else(|| current_playback.thumbnail.clone());
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
    if let Ok(mut tagged_file) = lofty::probe::Probe::open(&final_target_path).and_then(|p| p.read()) {
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
        let _ = tag.save_to_path(&final_target_path, lofty::config::WriteOptions::default());
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
        file_path: Some(saved_path_str.clone()),
        cover_path: local_thumbnail.clone(),
        file_size: downloaded_bytes,
        downloaded_at: crate::db::now_secs(),
    };
    state.db.save_downloaded_track(&record);

    // Register asset scope
    let scope = app.asset_protocol_scope();
    let _ = scope.allow_file(&final_target_path);
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
    quality: Option<String>,
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
            quality.clone(),
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
    app: &AppHandle,
    db: &crate::db::Db,
    video_id: &str,
) -> Option<crate::db::DownloadedTrack> {
    if let Some(track) = db.get_downloaded_track(video_id) {
        let p = Path::new(&track.path);
        if p.is_file() {
            if crate::cache::is_valid_audio_file(p) {
                return Some(track);
            } else {
                let _ = std::fs::remove_file(p);
                db.delete_downloaded_track(video_id);
            }
        }
    }

    // Also check offline_cache on disk by filename prefix
    let cache_dir = get_offline_cache_dir(app);
    let clean_id = if video_id.starts_with("SP:") {
        let parts: Vec<&str> = video_id.split(':').collect();
        let track_id = parts.get(1).unwrap_or(&"unknown");
        format!("SP_{}", sanitize_filename(track_id))
    } else {
        sanitize_filename(video_id)
    };
    let prefix = format!("{}_", clean_id);
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if !crate::cache::is_valid_audio_file(&p) {
                    let _ = std::fs::remove_file(&p);
                    continue;
                }
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with(&prefix)
                        || name == format!("{}.flac", clean_id)
                        || name == format!("{}.m4a", clean_id)
                        || name == format!("{}.webm", clean_id)
                        || name == format!("{}.opus", clean_id)
                        || name == format!("{}.mp3", clean_id)
                        || name == format!("{}.wav", clean_id)
                    {
                        let path_str = p.to_string_lossy().to_string();
                        let file_size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                        let record = crate::db::DownloadedTrack {
                            video_id: video_id.to_string(),
                            title: name.to_string(),
                            artist: "Downloaded Track".to_string(),
                            album: None,
                            duration: None,
                            duration_secs: 0,
                            thumbnail: None,
                            path: path_str.clone(),
                            file_path: Some(path_str.clone()),
                            cover_path: None,
                            file_size,
                            downloaded_at: crate::db::now_secs(),
                        };
                        db.save_downloaded_track(&record);
                        return Some(record);
                    }
                }
            }
        }
    }
    None
}

pub fn scan_downloaded_songs(
    app: &AppHandle,
    db: &crate::db::Db,
) -> Vec<crate::db::DownloadedTrack> {
    // 1. Prune missing files from DB
    let _ = db.prune_missing_downloaded_tracks();

    let scope = app.asset_protocol_scope();
    let covers_dir = crate::local::covers_dir(app);
    let _ = scope.allow_directory(&covers_dir, true);

    let dirs_to_scan = vec![get_offline_cache_dir(app), get_default_download_dir(app)];

    for scan_dir in dirs_to_scan {
        let _ = scope.allow_directory(&scan_dir, true);
        if let Ok(entries) = std::fs::read_dir(&scan_dir) {
            let existing = db.get_all_downloaded_tracks();
            let existing_paths: std::collections::HashSet<String> =
                existing.into_iter().map(|t| t.path).collect();

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() || !crate::cache::is_valid_audio_file(&path) {
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if !["mp3", "m4a", "webm", "opus", "flac", "ogg", "wav", "aac"]
                    .contains(&ext.as_str())
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
                    if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())
                    {
                        title = tag.title().map(|s| s.to_string());
                        artist = tag.artist().map(|s| s.to_string());
                        album = tag.album().map(|s| s.to_string());
                    }
                }

                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Track");
                // Check if file stem starts with video_id (pattern: "<video_id>_<title>")
                let (vid, f_artist, f_title) =
                    if let Some((id_part, rest)) = file_stem.split_once('_') {
                        if id_part.len() == 11 || id_part.starts_with("SP_") {
                            let restored_id = if id_part.starts_with("SP_") {
                                id_part.replacen("SP_", "SP:", 1)
                            } else {
                                id_part.to_string()
                            };
                            (restored_id, "Unknown Artist".to_string(), rest.to_string())
                        } else if let Some((a, t)) = file_stem.split_once(" - ") {
                            (format!("DL:{}", path_str), a.trim().to_string(), t.trim().to_string())
                        } else {
                            (
                                format!("DL:{}", path_str),
                                "Unknown Artist".to_string(),
                                file_stem.to_string(),
                            )
                        }
                    } else if let Some((a, t)) = file_stem.split_once(" - ") {
                        (format!("DL:{}", path_str), a.trim().to_string(), t.trim().to_string())
                    } else {
                        (
                            format!("DL:{}", path_str),
                            "Unknown Artist".to_string(),
                            file_stem.to_string(),
                        )
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

                let record = crate::db::DownloadedTrack {
                    video_id: vid,
                    title: final_title,
                    artist: final_artist,
                    album,
                    duration,
                    duration_secs,
                    thumbnail: None,
                    path: path_str.clone(),
                    file_path: Some(path_str),
                    cover_path: None,
                    file_size,
                    downloaded_at: crate::db::now_secs(),
                };
                db.save_downloaded_track(&record);
            }
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
