use crate::db::Db;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_bytes: u64,
    pub audio_cache_bytes: u64,
    pub cipher_cache_bytes: u64,
    pub covers_cache_bytes: u64,
    pub limit_mb: Option<u64>,
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += dir_size(&entry.path());
                }
            }
        }
    }
    total
}

pub fn is_valid_audio_bytes(bytes: &[u8]) -> bool {
    if bytes.len() < 512 {
        return false;
    }
    let len = bytes.len().min(128);
    let prefix_lower = bytes[..len].to_ascii_lowercase();
    if prefix_lower.starts_with(b"<!doctype")
        || prefix_lower.starts_with(b"<html")
        || prefix_lower.starts_with(b"<?xml")
        || prefix_lower.starts_with(b"{\"error")
        || prefix_lower.starts_with(b"{\"code")
        || prefix_lower.starts_with(b"{\"status")
        || prefix_lower.starts_with(b"error:")
    {
        return false;
    }
    true
}

pub fn detect_audio_extension_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 12 {
        return None;
    }
    if bytes.starts_with(b"fLaC") {
        return Some("flac");
    }
    if bytes.starts_with(b"OggS") {
        return Some("opus");
    }
    if bytes.starts_with(b"ID3") || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0) {
        return Some("mp3");
    }
    if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("webm");
    }
    if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        return Some("m4a");
    }
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WAVE" {
        return Some("wav");
    }
    if bytes[0] == 0xFF && (bytes[1] == 0xF1 || bytes[1] == 0xF9) {
        return Some("aac");
    }
    None
}

pub fn is_valid_audio_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(meta) = fs::metadata(path) else { return false };
    if meta.len() < 4096 {
        return false;
    }
    let Ok(mut f) = fs::File::open(path) else { return false };
    use std::io::Read;
    let mut header = [0u8; 64];
    let Ok(n) = f.read(&mut header) else { return false };
    if n < 12 {
        return false;
    }
    is_valid_audio_bytes(&header[..n])
}

pub fn audio_cache_dir(app: &AppHandle) -> std::path::PathBuf {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir());
    let audio_dir = data_dir.join("audio-cache");
    let _ = fs::create_dir_all(&audio_dir);
    audio_dir
}

pub fn get_cached_audio(app: &AppHandle, video_id: &str) -> Option<std::path::PathBuf> {
    let dir = audio_cache_dir(app);
    let clean_id = video_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>();

    for ext in &["flac", "m4a", "webm", "opus", "mp3", "wav", "aac"] {
        let p = dir.join(format!("{clean_id}.{ext}"));
        if p.is_file() {
            if is_valid_audio_file(&p) {
                return Some(p);
            } else {
                let _ = fs::remove_file(&p);
            }
        }
    }
    None
}

pub fn get_cache_stats(app: &AppHandle, db: &Db) -> CacheStats {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir());
    let audio_dir = data_dir.join("audio-cache");
    let cipher_dir = data_dir.join("cipher_cache");
    let covers_dir = data_dir.join("covers");

    let audio_cache_bytes = dir_size(&audio_dir);
    let cipher_cache_bytes = dir_size(&cipher_dir);
    let covers_cache_bytes = dir_size(&covers_dir);
    let total_bytes = audio_cache_bytes + cipher_cache_bytes + covers_cache_bytes;

    let limit_mb =
        db.get_setting("cache_limit_mb").and_then(|s| s.parse::<u64>().ok()).filter(|&lim| lim > 0);

    CacheStats { total_bytes, audio_cache_bytes, cipher_cache_bytes, covers_cache_bytes, limit_mb }
}

pub fn prune_cache_to_limit(app: &AppHandle, limit_bytes: u64) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir());
    let audio_dir = data_dir.join("audio-cache");
    if !audio_dir.exists() {
        return Ok(());
    }

    let mut current_size = dir_size(&audio_dir);
    if current_size <= limit_bytes {
        return Ok(());
    }

    // Collect all files in audio_dir sorted by modified time (oldest first)
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&audio_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    files.push((entry.path(), meta.len(), mtime));
                }
            }
        }
    }

    files.sort_by_key(|(_, _, mtime)| *mtime);

    for (path, size, _) in files {
        if current_size <= limit_bytes {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            current_size = current_size.saturating_sub(size);
        }
    }

    Ok(())
}

pub fn set_cache_limit(app: &AppHandle, db: &Db, limit_mb: Option<u64>) -> Result<(), String> {
    match limit_mb {
        Some(mb) if mb > 0 => {
            db.set_setting("cache_limit_mb", &mb.to_string());
            let limit_bytes = mb * 1024 * 1024;
            prune_cache_to_limit(app, limit_bytes)?;
        }
        _ => {
            db.set_setting("cache_limit_mb", "0");
        }
    }
    Ok(())
}

pub fn clear_cache(app: &AppHandle, which: Option<&str>) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir());
    let clear_all = which.is_none() || which == Some("all");

    if clear_all || which == Some("audio") {
        let audio_dir = data_dir.join("audio-cache");
        if audio_dir.exists() {
            let _ = fs::remove_dir_all(&audio_dir);
            let _ = fs::create_dir_all(&audio_dir);
        }
    }

    if clear_all || which == Some("cipher") {
        let cipher_dir = data_dir.join("cipher_cache");
        if cipher_dir.exists() {
            let _ = fs::remove_dir_all(&cipher_dir);
            let _ = fs::create_dir_all(&cipher_dir);
        }
    }

    if clear_all || which == Some("covers") {
        let covers_dir = data_dir.join("covers");
        if covers_dir.exists() {
            let _ = fs::remove_dir_all(&covers_dir);
            let _ = fs::create_dir_all(&covers_dir);
        }
    }

    Ok(())
}
