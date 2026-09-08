use futures_util::StreamExt;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct UpdateProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f32,
    pub status: String,
}

pub async fn download_and_install_update(
    app: AppHandle,
    version: String,
    direct_url: Option<String>,
) -> Result<(), String> {
    let _ = app.emit(
        "update-download-progress",
        UpdateProgress {
            downloaded_bytes: 0,
            total_bytes: None,
            percent: 0.0,
            status: "fetching_asset".into(),
        },
    );

    let client = reqwest::Client::builder()
        .user_agent("Nocturne-Music-Updater")
        .build()
        .map_err(|e| e.to_string())?;

    let download_url = if let Some(url) = direct_url {
        url
    } else {
        // Look up release assets from GitHub API
        let clean_tag =
            if version.starts_with('v') { version.clone() } else { format!("v{}", version) };
        let api_url = format!(
            "https://api.github.com/repos/Neo-XD/nocturne-music/releases/tags/{}",
            clean_tag
        );

        let res =
            client.get(&api_url).header("Accept", "application/vnd.github.v3+json").send().await;

        let release_json: serde_json::Value = match res {
            Ok(r) if r.status().is_success() => r.json().await.map_err(|e| e.to_string())?,
            _ => {
                // Fallback to latest release endpoint
                let latest_res = client
                    .get("https://api.github.com/repos/Neo-XD/nocturne-music/releases/latest")
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
                    .await
                    .map_err(|e| format!("GitHub API network error: {}", e))?;
                latest_res.json().await.map_err(|e| e.to_string())?
            }
        };

        let assets = release_json["assets"]
            .as_array()
            .ok_or_else(|| "No assets in GitHub release".to_string())?;

        let mut target_url: Option<String> = None;

        #[cfg(target_os = "windows")]
        {
            for a in assets {
                let name = a["name"].as_str().unwrap_or("");
                if (name.ends_with(".exe") || name.ends_with("-setup.exe"))
                    && !name.ends_with(".sig")
                {
                    if let Some(url) = a["browser_download_url"].as_str() {
                        target_url = Some(url.to_string());
                        break;
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            for a in assets {
                let name = a["name"].as_str().unwrap_or("");
                if name.ends_with(".AppImage") && !name.ends_with(".sig") {
                    if let Some(url) = a["browser_download_url"].as_str() {
                        target_url = Some(url.to_string());
                        break;
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            for a in assets {
                let name = a["name"].as_str().unwrap_or("");
                if name.ends_with(".dmg") && !name.ends_with(".sig") {
                    if let Some(url) = a["browser_download_url"].as_str() {
                        target_url = Some(url.to_string());
                        break;
                    }
                }
            }
        }

        target_url.ok_or_else(|| "No matching installer asset found in release".to_string())?
    };

    // Download the installer file
    let temp_dir = std::env::temp_dir();
    let filename = if cfg!(target_os = "windows") {
        format!("nocturne-{}-setup.exe", version)
    } else if cfg!(target_os = "linux") {
        format!("nocturne-{}.AppImage", version)
    } else {
        format!("nocturne-{}.dmg", version)
    };
    let dest_path: PathBuf = temp_dir.join(&filename);

    let res = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Download request error: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Download failed with HTTP status {}", res.status()));
    }

    let total_bytes = res.content_length();
    let mut file = File::create(&dest_path).map_err(|e| e.to_string())?;
    let mut stream = res.bytes_stream();
    let mut downloaded = 0u64;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        let percent = match total_bytes {
            Some(total) if total > 0 => (downloaded as f32 / total as f32) * 100.0,
            _ => 0.0,
        };

        let _ = app.emit(
            "update-download-progress",
            UpdateProgress {
                downloaded_bytes: downloaded,
                total_bytes,
                percent,
                status: "downloading".into(),
            },
        );
    }

    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    let _ = app.emit(
        "update-download-progress",
        UpdateProgress {
            downloaded_bytes: downloaded,
            total_bytes,
            percent: 100.0,
            status: "ready_to_install".into(),
        },
    );

    // Launch installer and exit
    #[cfg(target_os = "windows")]
    {
        // On Windows, launch the installer in passive mode
        let installer_str = dest_path.to_string_lossy().to_string();
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &installer_str, "/passive"])
            .spawn()
            .map_err(|e| format!("Failed to launch installer: {}", e))?;

        // Give process a small moment then exit app
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        app.exit(0);
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&dest_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&dest_path, perms);
        }
        std::process::Command::new(&dest_path)
            .spawn()
            .map_err(|e| format!("Failed to launch AppImage: {}", e))?;
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        app.exit(0);
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dest_path)
            .spawn()
            .map_err(|e| format!("Failed to open DMG: {}", e))?;
    }

    Ok(())
}
