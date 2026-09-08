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
