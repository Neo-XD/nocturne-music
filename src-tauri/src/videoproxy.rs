//! The `limusicvideo://` custom scheme: the player view's `<video>` fetches its bytes from here,
//! never from googlevideo (context/11: no YouTube shapes past the command boundary).
//!
//! The UI is handed `limusicvideo://localhost/<videoId>`; `video_stream` has already put the real
//! URL in [`AppState`] under that id. This handler is a thin range-proxy on top of it: parse the
//! `Range` the media element asked for, ask googlevideo for the same bytes, hand back a 206.

use std::sync::Arc;

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, UriSchemeContext, UriSchemeResponder, Wry};

use crate::state::AppState;

/// Ceiling on one served slice. `UriSchemeResponder::respond` takes a fully buffered `Vec<u8>`
/// (tauri 2.11.5, `app.rs`), so an open-ended `bytes=0-` on a 45 MB video would allocate 45 MB
/// before a single frame drew. The media element just asks for the next slice.
const MAX_SLICE: u64 = 2 * 1024 * 1024;

/// The URL the UI puts in `<video src>`. Windows resolves custom schemes through a hostname, every
/// other platform through the scheme itself (tauri `app.rs`).
pub fn url_for(video_id: &str) -> String {
    #[cfg(windows)]
    {
        format!("http://limusicvideo.localhost/{video_id}")
    }
    #[cfg(not(windows))]
    {
        format!("limusicvideo://localhost/{video_id}")
    }
}

/// The inclusive `(start, end)` byte range to fetch, clamped to [`MAX_SLICE`]. `None` for a `Range`
/// header we can't read, which the caller answers 416.
///
/// No total-size clamp: upstream knows the length and simply returns fewer bytes than we asked for,
/// and its own `Content-Range` is what we echo back.
fn slice(range: Option<&str>) -> Option<(u64, u64)> {
    // No Range at all: the element wants the whole file, we start it at the first slice.
    let Some(raw) = range else { return Some((0, MAX_SLICE - 1)) };
    let spec = raw.trim().strip_prefix("bytes=")?;
    // Single-range form only; WebKit's media stack never sends a multi-range for <video>.
    let (from, to) = spec.split_once('-')?;
    let start: u64 = from.trim().parse().ok()?;
    let end = match to.trim() {
        "" => start + MAX_SLICE - 1,
        n => n.parse::<u64>().ok()?.min(start + MAX_SLICE - 1),
    };
    (end >= start).then_some((start, end))
}

fn fail(responder: UriSchemeResponder, status: u16) {
    if let Ok(r) = Response::builder().status(status).body(Vec::new()) {
        responder.respond(r);
    }
}

/// Registered on the builder as `register_asynchronous_uri_scheme_protocol("limusicvideo", ..)`.
/// Returns immediately; the fetch runs on the tokio runtime.
pub fn handle(
    ctx: UriSchemeContext<'_, Wry>,
    req: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let video_id = req.uri().path().trim_start_matches('/').to_owned();
    let range = req.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).map(str::to_owned);

    let Some(state) = ctx.app_handle().try_state::<Arc<AppState>>() else {
        return fail(responder, 500);
    };
    // Nothing resolved this id: a stale element, or a `video_stream` that returned None.
    let Some(upstream) = state.video_url(&video_id) else {
        return fail(responder, 404);
    };

    tauri::async_runtime::spawn(async move {
        let Some((start, end)) = slice(range.as_deref()) else {
            return fail(responder, 416);
        };
        let res = crate::http::client()
            .get(&upstream)
            .header(header::RANGE.as_str(), format!("bytes={start}-{end}"))
            .send()
            .await;
        let upstream_resp = match res {
            Ok(r) if r.status().is_success() => r,
            // Expired URL (googlevideo links last ~6h) or a network failure. The element errors and
            // the view falls back to artwork; see plan 031's maintenance notes.
            Ok(r) => {
                tracing::debug!(video_id, status = %r.status(), "video proxy: upstream refused");
                return fail(responder, 502);
            }
            Err(e) => {
                tracing::debug!(video_id, error = %e, "video proxy: upstream failed");
                return fail(responder, 502);
            }
        };

        let partial = upstream_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let pick = |name: &str| {
            upstream_resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
        };
        let content_type = pick("content-type").unwrap_or_else(|| "video/webm".to_owned());
        let content_range = pick("content-range");

        let body = match upstream_resp.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                tracing::debug!(video_id, error = %e, "video proxy: body failed");
                return fail(responder, 502);
            }
        };

        let mut builder = Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, body.len().to_string());
        builder = match (partial, content_range) {
            // Echo upstream's own Content-Range: it carries the total size the element needs to
            // know the duration, and it already describes exactly the bytes in `body`.
            (true, Some(cr)) => {
                builder.status(StatusCode::PARTIAL_CONTENT).header(header::CONTENT_RANGE, cr)
            }
            (true, None) => builder.status(StatusCode::PARTIAL_CONTENT).header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{}/*", start + body.len().saturating_sub(1) as u64),
            ),
            // Upstream ignored the Range → pass the 200 through, so the element stops asking for
            // slices it will not get.
            (false, _) => builder.status(StatusCode::OK),
        };

        match builder.body(body) {
            Ok(r) => responder.respond(r),
            Err(_) => fail(responder, 500),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_clamps_to_max() {
        assert_eq!(slice(None), Some((0, MAX_SLICE - 1)));
        assert_eq!(slice(Some("bytes=0-")), Some((0, MAX_SLICE - 1)));
        assert_eq!(slice(Some("bytes=5000000-")), Some((5_000_000, 5_000_000 + MAX_SLICE - 1)));
        assert_eq!(slice(Some("bytes=0-99999999")), Some((0, MAX_SLICE - 1)));
    }

    /// A small explicit range is honoured, not padded out to MAX_SLICE: WebKit probes the header
    /// with a short read first, and answering that with 2 MiB wastes the probe.
    #[test]
    fn slice_honours_small_explicit_range() {
        assert_eq!(slice(Some("bytes=0-1023")), Some((0, 1023)));
        assert_eq!(slice(Some(" bytes=100-200 ")), Some((100, 200)));
    }

    #[test]
    fn slice_rejects_garbage() {
        assert_eq!(slice(Some("items=0-1")), None);
        assert_eq!(slice(Some("bytes=abc-")), None);
        assert_eq!(slice(Some("bytes=500-100")), None);
        assert_eq!(slice(Some("bytes=0")), None);
    }
}
