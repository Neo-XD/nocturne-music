//! "Now playing" notifications on a track change, behind the `track_notifications`
//! setting. Off by default.
//!
//! Only while no Nocturne window has focus: a song change the user is looking at needs no toast.
//! Best-effort like `media.rs`: a missing notification daemon is a `debug!` line.

#[cfg(all(unix, not(target_os = "macos")))]
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Manager};

/// The daemon's id for the last one we showed. Passed back as `replaces_id`, so each song replaces
/// the previous song's notification instead of leaving one per track in the history.
#[cfg(all(unix, not(target_os = "macos")))]
static LAST_ID: AtomicU32 = AtomicU32::new(0);

pub fn track_changed(app: &AppHandle, title: &str, artists: &str) {
    if app.webview_windows().values().any(|w| w.is_focused().unwrap_or(false)) {
        return;
    }
    let mut n = notify_rust::Notification::new();
    n.summary(title).body(artists).appname("Nocturne");
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        n.auto_icon()
            .hint(notify_rust::Hint::SuppressSound(true))
            .id(LAST_ID.load(Ordering::Relaxed));
    }
    #[cfg(target_os = "windows")]
    if !tauri::is_dev() {
        n.app_id(&app.config().identifier);
    }
    #[cfg(target_os = "macos")]
    let _ = notify_rust::set_application(if tauri::is_dev() {
        "com.apple.Terminal"
    } else {
        &app.config().identifier
    });
    std::thread::spawn(move || match n.show() {
        #[cfg(all(unix, not(target_os = "macos")))]
        Ok(h) => LAST_ID.store(h.id(), Ordering::Relaxed),
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        Ok(_) => {}
        Err(e) => tracing::debug!("notification: {e}"),
    });
}
