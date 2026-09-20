use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

    let json: serde_json::Value =
        res.json().await.map_err(|e| format!("Failed to parse Shazam JSON: {e}"))?;

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

        return Ok(RecognizedSong { found: title.is_some(), title, artist, album, cover, query });
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

#[tauri::command]
pub async fn capture_pc_audio(sample_ms: Option<u32>) -> Result<Vec<f32>, String> {
    let dur = sample_ms.unwrap_or(4500);
    #[cfg(target_os = "windows")]
    {
        tokio::task::spawn_blocking(move || capture_wasapi_loopback(dur))
            .await
            .map_err(|e| format!("Capture task error: {e}"))?
    }
    #[cfg(target_os = "linux")]
    {
        tokio::task::spawn_blocking(move || capture_linux_loopback(dur))
            .await
            .map_err(|e| format!("Capture task error: {e}"))?
    }
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(move || capture_macos_loopback(dur))
            .await
            .map_err(|e| format!("Capture task error: {e}"))?
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err("Direct system audio capture is not supported on this platform".to_string())
    }
}

#[cfg(target_os = "windows")]
fn capture_wasapi_loopback(duration_ms: u32) -> Result<Vec<f32>, String> {
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        struct CoUninitGuard;
        impl Drop for CoUninitGuard {
            fn drop(&mut self) {
                unsafe { CoUninitialize() };
            }
        }
        let _guard = CoUninitGuard;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create MMDeviceEnumerator: {e}"))?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("Failed to get default audio endpoint: {e}"))?;

        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Failed to activate IAudioClient: {e}"))?;

        let pwfx = audio_client
            .GetMixFormat()
            .map_err(|e| format!("Failed to get audio mix format: {e}"))?;

        let wfx: &WAVEFORMATEX = &*pwfx;
        let sample_rate = wfx.nSamplesPerSec as usize;
        let channels = wfx.nChannels as usize;
        let bits_per_sample = wfx.wBitsPerSample as usize;

        // Initialize audio client in loopback mode
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                (duration_ms as i64) * 10000,
                0,
                pwfx,
                None,
            )
            .map_err(|e| format!("Failed to initialize audio client: {e}"))?;

        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| format!("Failed to get IAudioCaptureClient: {e}"))?;

        audio_client.Start().map_err(|e| format!("Failed to start audio client: {e}"))?;

        let start_time = std::time::Instant::now();
        let target_duration = std::time::Duration::from_millis(duration_ms as u64);

        let mut raw_mono_samples: Vec<f32> = Vec::new();

        while start_time.elapsed() < target_duration {
            if let Ok(packet_size) = capture_client.GetNextPacketSize() {
                if packet_size == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }

                let mut p_data = std::ptr::null_mut();
                let mut num_frames_read = 0u32;
                let mut flags = 0u32;

                if capture_client
                    .GetBuffer(&mut p_data, &mut num_frames_read, &mut flags, None, None)
                    .is_ok()
                {
                    if num_frames_read > 0 && !p_data.is_null() {
                        let is_silent = (flags & 0x01) != 0; // AUDCLNT_BUFFERFLAGS_SILENT

                        if is_silent {
                            raw_mono_samples
                                .resize(raw_mono_samples.len() + num_frames_read as usize, 0.0);
                        } else if bits_per_sample == 32 {
                            let slice = std::slice::from_raw_parts(
                                p_data as *const f32,
                                (num_frames_read as usize) * channels,
                            );
                            for frame in slice.chunks(channels) {
                                let sum: f32 = frame.iter().copied().sum();
                                raw_mono_samples.push(sum / channels as f32);
                            }
                        } else if bits_per_sample == 16 {
                            let slice = std::slice::from_raw_parts(
                                p_data as *const i16,
                                (num_frames_read as usize) * channels,
                            );
                            for frame in slice.chunks(channels) {
                                let sum: f32 = frame.iter().map(|&s| s as f32 / 32768.0).sum();
                                raw_mono_samples.push(sum / channels as f32);
                            }
                        }
                    }
                    let _ = capture_client.ReleaseBuffer(num_frames_read);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let _ = audio_client.Stop();

        // Resample from `sample_rate` down to 16,000 Hz for Shazam
        let target_rate = 16000.0;
        let source_rate = sample_rate as f64;
        let total_target_samples =
            ((raw_mono_samples.len() as f64) * target_rate / source_rate) as usize;

        let mut resampled = Vec::with_capacity(total_target_samples);
        for i in 0..total_target_samples {
            let src_idx = (i as f64) * source_rate / target_rate;
            let idx0 = src_idx.floor() as usize;
            let frac = (src_idx - idx0 as f64) as f32;
            let s0 = raw_mono_samples.get(idx0).copied().unwrap_or(0.0);
            let s1 = raw_mono_samples.get(idx0 + 1).copied().unwrap_or(s0);
            resampled.push(s0 + frac * (s1 - s0));
        }

        Ok(resampled)
    }
}

#[cfg(target_os = "linux")]
fn capture_linux_loopback(duration_ms: u32) -> Result<Vec<f32>, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let target_samples = (16000 * duration_ms as usize) / 1000;
    let target_bytes = target_samples * 2;

    // Detect default sink to monitor via pactl
    let monitor_source =
        Command::new("pactl").arg("get-default-sink").output().ok().and_then(|out| {
            if out.status.success() {
                let sink = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !sink.is_empty() {
                    return Some(format!("{sink}.monitor"));
                }
            }
            None
        });

    // Try parec with specific monitor device first, then default monitor, then pw-record
    let mut child = if let Some(ref dev) = monitor_source {
        Command::new("parec")
            .args(["--format=s16le", "--rate=16000", "--channels=1", "-d", dev])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    } else {
        Command::new("parec")
            .args(["--format=s16le", "--rate=16000", "--channels=1", "-d", "@DEFAULT_MONITOR@"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    };

    if child.is_err() {
        child = Command::new("parec")
            .args(["--format=s16le", "--rate=16000", "--channels=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
    }

    if child.is_err() {
        child = Command::new("pw-record")
            .args(["--rate", "16000", "--channels", "1", "--format", "s16", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
    }

    let mut child = child.map_err(|e| {
        format!(
            "Failed to start Linux loopback audio capture (parec / pw-record not available): {e}"
        )
    })?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout of Linux audio recorder".to_string())?;

    let mut raw_bytes = Vec::with_capacity(target_bytes);
    let mut buf = [0u8; 4096];
    let start = std::time::Instant::now();
    let max_dur = std::time::Duration::from_millis(duration_ms as u64 + 1000);

    while raw_bytes.len() < target_bytes && start.elapsed() < max_dur {
        match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let needed = target_bytes - raw_bytes.len();
                let take = n.min(needed);
                raw_bytes.extend_from_slice(&buf[..take]);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Error reading Linux audio stream: {e}"));
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    if raw_bytes.is_empty() {
        return Err("No audio captured from Linux system output".to_string());
    }

    let mut samples = Vec::with_capacity(raw_bytes.len() / 2);
    for chunk in raw_bytes.chunks_exact(2) {
        let val = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(val as f32 / 32768.0);
    }

    Ok(samples)
}

#[cfg(target_os = "macos")]
fn capture_macos_loopback(duration_ms: u32) -> Result<Vec<f32>, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let target_samples = (16000 * duration_ms as usize) / 1000;
    let target_bytes = target_samples * 2;
    let dur_sec = format!("{:.2}", (duration_ms as f64) / 1000.0);

    let candidates =
        ["BlackHole 2ch", "BlackHole 16ch", "Soundflower (2ch)", "Background Music", "Loopback"];

    let mut child = None;

    // Try ffmpeg with candidate virtual devices
    for dev in candidates {
        if let Ok(c) = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-f",
                "avfoundation",
                "-i",
                &format!(":{dev}"),
                "-ar",
                "16000",
                "-ac",
                "1",
                "-f",
                "s16le",
                "-t",
                &dur_sec,
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            child = Some(c);
            break;
        }
    }

    // Try sox with candidate devices if ffmpeg was not used
    if child.is_none() {
        for dev in candidates {
            if let Ok(c) = Command::new("sox")
                .args([
                    "-t",
                    "coreaudio",
                    dev,
                    "-r",
                    "16000",
                    "-c",
                    "1",
                    "-b",
                    "16",
                    "-e",
                    "signed-integer",
                    "-t",
                    "raw",
                    "-",
                    "trim",
                    "0",
                    &dur_sec,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                child = Some(c);
                break;
            }
        }
    }

    // If still none, try rec with default input
    if child.is_none() {
        if let Ok(c) = Command::new("rec")
            .args([
                "-r",
                "16000",
                "-c",
                "1",
                "-b",
                "16",
                "-e",
                "signed-integer",
                "-t",
                "raw",
                "-",
                "trim",
                "0",
                &dur_sec,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            child = Some(c);
        }
    }

    let mut child = child.ok_or_else(|| {
        "Direct system audio capture on macOS requires a virtual loopback device (such as BlackHole or Soundflower) with ffmpeg/sox. Falling back to microphone.".to_string()
    })?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout of macOS audio recorder".to_string())?;

    let mut raw_bytes = Vec::with_capacity(target_bytes);
    let mut buf = [0u8; 4096];
    let start = std::time::Instant::now();
    let max_dur = std::time::Duration::from_millis(duration_ms as u64 + 1000);

    while raw_bytes.len() < target_bytes && start.elapsed() < max_dur {
        match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let needed = target_bytes - raw_bytes.len();
                let take = n.min(needed);
                raw_bytes.extend_from_slice(&buf[..take]);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Error reading macOS audio stream: {e}"));
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    if raw_bytes.is_empty() {
        return Err("No audio captured from macOS system output".to_string());
    }

    let mut samples = Vec::with_capacity(raw_bytes.len() / 2);
    for chunk in raw_bytes.chunks_exact(2) {
        let val = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(val as f32 / 32768.0);
    }

    Ok(samples)
}
