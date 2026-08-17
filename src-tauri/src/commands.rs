//! Tauri commands — the ONLY API the UI calls. context/11 UI contract. No YouTube shapes leak
//! past here; the UI never sees a stream URL.

use std::sync::Arc;

use innertube::{
    AlbumPage, ArtistPage, BrowseItem, HomePage, PlaylistContinuation, PlaylistPage, PlaylistSort,
    Rating, SearchResults, SongItem,
};
use tauri::{Emitter, State};

use crate::state::{AppState, ON_REPEAT_ID, ON_REPEAT_LIMIT, ON_REPEAT_WINDOW_SECS};

type St<'a> = State<'a, Arc<AppState>>;

#[tauri::command]
pub async fn search(state: St<'_>, query: String) -> Result<Vec<SongItem>, String> {
    let client = state.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
    let result = state.it.search_songs(client, &query).await.map_err(|e| e.to_string())?;
    Ok(result.items)
}

/// Unfiltered search → categorized sections for the search page.
#[tauri::command]
pub async fn search_all(state: St<'_>, query: String) -> Result<SearchResults, String> {
    let client = metadata_client(&state)?;
    state.it.search_all(client, &query).await.map_err(|e| e.to_string())
}

/// Filtered "Show more" search for one category (albums / artists / playlists).
#[tauri::command]
pub async fn search_cards(
    state: St<'_>,
    query: String,
    category: String,
) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    state.it.search_cards(client, &query, &category).await.map_err(|e| e.to_string())
}

/// Play a track (from a search result). The UI passes the full item so we can seed the queue
/// with its metadata without another round-trip.
#[tauri::command]
pub async fn play(state: St<'_>, item: SongItem) -> Result<(), String> {
    let state = state.inner().clone();
    state.play_song(item).await;
    Ok(())
}

#[tauri::command]
pub async fn play_index(state: St<'_>, index: usize) -> Result<(), String> {
    let state = state.inner().clone();
    state.play_index(index).await;
    Ok(())
}

/// Remove an upcoming track from the queue (not the one playing). Guests are add-only — blocked
/// inside AppState.
#[tauri::command]
pub async fn remove_from_queue(state: St<'_>, index: usize) -> Result<(), String> {
    state.inner().clone().remove_from_queue(index).await;
    Ok(())
}

/// "Play next" from a ⋯ menu: one track or a whole album/playlist, inserted right after the
/// current song (behind any earlier manual adds). `from` is the album/playlist title, which heads
/// the block in the queue panel.
#[tauri::command]
pub async fn play_next(
    state: St<'_>,
    items: Vec<SongItem>,
    from: Option<String>,
) -> Result<(), String> {
    state.inner().clone().play_next(items, from).await;
    Ok(())
}

/// Drag-to-reorder in the queue panel: move the upcoming track at `from` to `to`. Out-of-range or
/// already-played indices are ignored.
#[tauri::command]
pub async fn move_in_queue(state: St<'_>, from: usize, to: usize) -> Result<(), String> {
    state.inner().clone().move_in_queue(from, to).await;
    Ok(())
}

/// "Add to queue": the tracks go at the back of the "Next in queue" block, so they play after
/// everything already queued by hand and ahead of the playing context (and its radio/filler).
/// `from` heads the block in the queue panel; `continuation` is the source page's next-page token —
/// the rest of a long playlist is walked in in the background.
#[tauri::command]
pub async fn add_to_queue(
    state: St<'_>,
    items: Vec<SongItem>,
    from: Option<String>,
    continuation: Option<String>,
) -> Result<(), String> {
    state.inner().clone().add_to_queue(items, from, continuation).await;
    Ok(())
}

/// Clear every upcoming manually-queued track (the queue panel's "Next in queue" section).
#[tauri::command]
pub async fn clear_queued(state: St<'_>) -> Result<(), String> {
    state.inner().clone().clear_queued().await;
    Ok(())
}

#[tauri::command]
pub async fn next_track(state: St<'_>) -> Result<(), String> {
    state.inner().clone().next_in_queue().await;
    Ok(())
}

#[tauri::command]
pub async fn prev_track(state: St<'_>) -> Result<(), String> {
    state.inner().clone().prev_in_queue().await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_shuffle(state: St<'_>) -> Result<(), String> {
    state.inner().clone().toggle_shuffle().await;
    Ok(())
}

/// `mode` ∈ "off" | "all" | "one".
#[tauri::command]
pub async fn set_repeat(state: St<'_>, mode: String) -> Result<(), String> {
    let mode = match mode.as_str() {
        "off" => crate::state::RepeatMode::Off,
        "all" => crate::state::RepeatMode::All,
        "one" => crate::state::RepeatMode::One,
        other => return Err(format!("unknown repeat mode: {other}")),
    };
    state.inner().clone().set_repeat(mode).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_pause(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    state.resume_or_toggle().await;
    Ok(())
}

#[tauri::command]
pub async fn seek(state: St<'_>, position: f64) -> Result<(), String> {
    // Routed through AppState so a Listen Together host broadcasts the seek and a guest is blocked.
    state.user_seek(position).await
}

#[tauri::command]
pub async fn set_volume(state: St<'_>, volume: i64) -> Result<(), String> {
    state.player.set_volume(volume).map_err(|e| e.to_string())?;
    // There is one volume and there can be two windows (the mini player). Without this the one
    // that didn't move the slider keeps showing the old level and lies about what you're hearing.
    let _ = state.app.emit("volume", volume);
    Ok(())
}

/// Tempo (0.25–2.0) and pitch (−12..=12 semitones), the "Advanced" dialog. Volatile by design:
/// both reset to 1.0 / 0 on restart, so nobody wonders next week why everything sounds wrong.
#[tauri::command]
pub async fn set_playback_params(state: St<'_>, speed: f64, semitones: i32) -> Result<(), String> {
    // Pitch first: it's the one that can fail (no librubberband), and it rolls itself back, so a
    // failure leaves nothing applied and the UI can revert both steppers together.
    state.player.set_pitch(semitones).map_err(|e| e.to_string())?;
    state.player.set_speed(speed).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_queue(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.queue_snapshot().await)
}

/// Settings the UI is allowed to read *and write*. Session/auth material (`session_cookie`,
/// `selected_identity_json`, `data_sync_id`, `account_json`, `account_selection_pending`,
/// `visitor_data`) and internal blobs (`queue_json`, `queue_index`, `queue_position`) never cross
/// into the webview: they'd otherwise ship the login credential to the renderer on every open, and
/// the webview can't overwrite them either.
const UI_SETTINGS: [&str; 13] = [
    "volume",
    "proxy",
    "quality",
    "enable_history",
    "disabled_stream_clients",
    "discord_rpc",
    "close_to_tray",
    "autostart",
    "autoplay",
    "hide_videos",
    "prevent_duplicates",
    "update_banner",
    "lyrics_boidu",
];

#[tauri::command]
pub async fn get_settings(state: St<'_>) -> Result<serde_json::Value, String> {
    let map: serde_json::Map<String, serde_json::Value> = state
        .db
        .all_settings()
        .into_iter()
        .filter(|(k, _)| UI_SETTINGS.contains(&k.as_str()))
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();
    Ok(serde_json::Value::Object(map))
}

#[tauri::command]
pub async fn set_setting(
    app: tauri::AppHandle,
    state: St<'_>,
    key: String,
    value: String,
) -> Result<(), String> {
    if !UI_SETTINGS.contains(&key.as_str()) {
        return Err(format!("unknown setting: {key}"));
    }
    state.db.set_setting(&key, &value);
    // Presence connects/clears the moment it's toggled — the user shouldn't have to skip a track
    // to see it take effect.
    if key == "discord_rpc" {
        state.set_discord_enabled(value == "true");
    }
    // Applies to what's fetched from here on: the live queue keeps whatever is already in it.
    if key == "hide_videos" {
        state.it.set_hide_videos(value == "true");
    }
    // Cached lyrics outlive the setting that produced them, so a track fetched while Boidu was on
    // would keep its word timings (and one fetched while off would never gain them) forever.
    if key == "lyrics_boidu" {
        state.db.clear_lyrics_cache();
    }
    // Registers/removes the login autostart entry on toggle; the OS persists it from there.
    // ponytail: no startup re-sync against the OS state — add reconciliation only if drift is
    // ever reported.
    if key == "autostart" {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let res = if value == "true" {
            al.enable()
        } else if al.is_enabled().unwrap_or(false) {
            al.disable()
        } else {
            Ok(())
        };
        res.map_err(|e| format!("autostart: {e}"))?;
    }
    Ok(())
}

/// The streamable client keys the orchestrator tries, for the "disabled clients" setting. Names
/// come from the innertube crate so the UI stays free of YouTube-shaped identity strings.
#[tauri::command]
pub async fn get_stream_clients() -> Result<Vec<String>, String> {
    let mut v = vec![innertube::MAIN_CLIENT.to_string()];
    v.extend(innertube::STREAM_FALLBACK_ORDER.iter().map(|s| s.to_string()));
    Ok(v)
}

/// Let the webview fetch one font file the user picked in the Themes tab, so a `@font-face` can
/// point at it.
///
/// Same runtime-scope trick as local artwork (`local::allow_covers`): the static asset scope stays
/// empty, and only the exact file gets a URL. The extension check keeps the command from being a
/// general "give the page a URL for any path on this machine" — today only the main window holds a
/// capability to call commands at all, and this stays safe if that ever widens.
#[tauri::command]
pub async fn allow_font_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri::Manager;
    const FONT_EXTS: [&str; 4] = ["ttf", "otf", "woff", "woff2"];
    let p = std::path::Path::new(&path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !FONT_EXTS.contains(&ext.as_str()) {
        return Err(format!("not a font file: {path}"));
    }
    // Scope grants succeed for paths that don't exist, so check here: this failing is how the UI
    // learns a loaded font was deleted or moved, and drops it instead of listing a dead entry.
    if !p.is_file() {
        return Err(format!("font file not found: {path}"));
    }
    let scope = app.asset_protocol_scope();
    scope.allow_file(&path).map_err(|e| e.to_string())?;
    // The scope check canonicalizes what it is asked about, so a font reached through a symlinked
    // folder needs the real path allowed too (see local::allow_covers).
    if let Ok(real) = p.canonicalize() {
        let _ = scope.allow_file(real);
    }
    Ok(())
}

/// Wipe both cache tiers (URL cache + mpv on-disk audio cache). context/14.
#[tauri::command]
pub async fn clear_caches(state: St<'_>) -> Result<(), String> {
    state.clear_caches();
    Ok(())
}

// --- auth (context/15) ---------------------------------------------------------------------

#[tauri::command]
pub async fn get_account(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.account_snapshot())
}

#[tauri::command]
pub async fn get_account_identities(state: St<'_>) -> Result<Vec<serde_json::Value>, String> {
    state.account_identities().await
}

#[tauri::command]
pub async fn switch_account(
    state: St<'_>,
    selection_key: String,
) -> Result<serde_json::Value, String> {
    state.switch_account(&selection_key).await
}

#[tauri::command]
pub async fn sign_out(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    state.sign_out().await;
    Ok(())
}

/// Open the in-app Google sign-in webview (context/15 Path A). Completes asynchronously; the UI
/// hears back via `auth-changed` (success) or `login-error`.
#[tauri::command]
pub async fn login_webview(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    let app = state.app.clone();
    crate::session::open_login(app, state);
    Ok(())
}

/// The current track, play state, position and duration in one shot. Events are the normal
/// channel; this is for a webview that started after them (the mini player, or the main window
/// on a cold start, where the queue is restored before the UI subscribes).
#[tauri::command]
pub async fn get_playback(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.playback_snapshot().await)
}

// --- mini player (mini.rs) ------------------------------------------------------------------

/// Swap the app for the floating widget: the main window hides to the tray behind it.
#[tauri::command]
pub async fn open_mini(app: tauri::AppHandle) -> Result<(), String> {
    // GTK wants window creation on the main thread, so hop and post the result back rather than
    // logging a failure the user would only see as a click that did nothing.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::mini::open(&handle));
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "the mini player never answered".to_string())?
}

/// Swap back. Same path as the tray, so the widget and the tray can't disagree about what
/// "show Limusic" means.
#[tauri::command]
pub async fn close_mini(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::show_main(&app);
    Ok(())
}

// --- browse / library (context/08) ---------------------------------------------------------

fn metadata_client(state: &Arc<AppState>) -> Result<&innertube::YouTubeClient, String> {
    state.clients.get(innertube::METADATA_CLIENT).ok_or_else(|| "metadata client missing".into())
}

#[tauri::command]
pub async fn get_home(state: St<'_>, params: Option<String>) -> Result<HomePage, String> {
    let client = metadata_client(&state)?;
    state.it.home(client, params.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_home_more(state: St<'_>, token: String) -> Result<HomePage, String> {
    let client = metadata_client(&state)?;
    state.it.home_continuation(client, &token).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library(state: St<'_>) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    // Signed out there is no YouTube library to ask for (the browse would come back as a sign-in
    // shell), but On Repeat is built from this machine's play history and is still real.
    let mut items = if state.it.is_logged_in() {
        state.it.library_playlists(client).await.map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    // On Repeat leads the library once there's anything in it. Hidden while empty rather than
    // shown as a dead tile on a fresh install.
    let songs = on_repeat_songs(&state);
    if !songs.is_empty() {
        items.insert(
            0,
            BrowseItem {
                kind: "playlist",
                id: ON_REPEAT_ID.into(),
                title: "On Repeat".into(),
                subtitle: Some(format!("{} songs", songs.len())),
                thumbnail: None, // the UI draws an icon cover for this one
                duration: None,
                artist_runs: Vec::new(),
                is_video: false,
                explicit: false,
            },
        );
    }
    // A card has nowhere to put two images, so a custom cover simply is the artwork here.
    for item in &mut items {
        if let Some(cover) = custom_cover(&state, &item.id) {
            item.thumbnail = Some(cover);
        }
    }
    Ok(items)
}

/// Empty rather than an error when signed out: the Library page merges the user's local saves into
/// these grids, so "nothing of yours on YouTube" is an answer, not a failure.
#[tauri::command]
pub async fn get_library_albums(state: St<'_>) -> Result<Vec<BrowseItem>, String> {
    if !state.it.is_logged_in() {
        return Ok(Vec::new());
    }
    let client = metadata_client(&state)?;
    state.it.library_albums(client).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_artists(state: St<'_>) -> Result<Vec<BrowseItem>, String> {
    if !state.it.is_logged_in() {
        return Ok(Vec::new());
    }
    let client = metadata_client(&state)?;
    state.it.library_artists(client).await.map_err(|e| e.to_string())
}

/// A playlist or album page. `id` is the browseId (`VL…` / `MPRE…`); Liked Songs is `VLLM`, and
/// `LIMUSIC_ON_REPEAT` is the local auto-playlist rather than anything YouTube knows about.
///
/// `sort` asks YouTube for the tracks in a given order; `None` gets whatever order the account
/// already has the list in, which is what a fresh visit wants (it matches YouTube Music).
#[tauri::command]
pub async fn get_playlist(
    state: St<'_>,
    id: String,
    sort: Option<PlaylistSort>,
    desc: Option<bool>,
) -> Result<PlaylistPage, String> {
    if id == ON_REPEAT_ID {
        let items = on_repeat_songs(&state);
        return Ok(PlaylistPage {
            title: Some("On Repeat".into()),
            subtitle: Some(format!("{} songs you've played most this month", items.len())),
            thumbnail: None,
            description: None,
            privacy: None,
            cover: None,
            items,
            continuation: None,
            owned: false, // nothing to rename or delete; it rebuilds itself from what you play
            sort_menu: None, // built from local history, so YouTube has no order to give
        });
    }
    let client = metadata_client(&state)?;
    let sort = sort.map(|s| (s, desc.unwrap_or(false)));
    let mut page = state.it.playlist(client, &id, sort).await.map_err(|e| e.to_string())?;
    // Alongside YouTube's own thumbnail, not over it: the dialog offers to drop the custom one.
    page.cover = custom_cover(&state, &id);
    Ok(page)
}

/// Store a sort order on a playlist, so YouTube Music and every other client show it the same way.
///
/// Only for a playlist whose `sortMenu.editable` said the options are writes. Everywhere else the
/// order is view-only and this would 400.
#[tauri::command]
pub async fn set_playlist_sort(
    state: St<'_>,
    playlist_id: String,
    sort: PlaylistSort,
) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state.it.playlist_set_sort(client, &playlist_id, sort).await.map_err(|e| e.to_string())
}

/// videoId → how many times it was played, over the same trailing window On Repeat is built from
/// (the history table is pruned to it, so there is no older data to offer). Feeds the playlist
/// page's "Most played" sort; a track the map doesn't mention has not been played this month.
#[tauri::command]
pub fn play_counts(state: St<'_>) -> std::collections::HashMap<String, i64> {
    state.db.play_counts(now_secs() - ON_REPEAT_WINDOW_SECS).into_iter().collect()
}

/// The On Repeat track list: most-played first, over the trailing window. Rows whose stored JSON
/// no longer parses (a `SongItem` shape change) are dropped rather than failing the whole page.
fn on_repeat_songs(state: &Arc<AppState>) -> Vec<SongItem> {
    let since = now_secs() - ON_REPEAT_WINDOW_SECS;
    state
        .db
        .top_plays(since, ON_REPEAT_LIMIT)
        .into_iter()
        .filter_map(|(json, _plays)| serde_json::from_str(&json).ok())
        .map(shed_queue_context)
        .collect()
}

/// A play record is the whole `SongItem` as it sat in the queue, so it carries that slot's queue
/// metadata: `queued`/`queued_by` when the track was "added to queue" (in a Listen Together session,
/// stamped with who added it), `autoplay` when radio appended it, `set_video_id` from whatever
/// playlist it was played from. None of that describes the song, so On Repeat sheds it: otherwise
/// the row wears a session member's name forever, and playing On Repeat drops it into "Next in
/// queue" instead of the playlist. Strips on read so rows already stored this way are fixed too.
fn shed_queue_context(s: SongItem) -> SongItem {
    SongItem {
        queued: false,
        queued_end: false,
        queued_from: None,
        queued_by: None,
        autoplay: false,
        set_video_id: None,
        ..s
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn get_playlist_more(
    state: St<'_>,
    token: String,
) -> Result<PlaylistContinuation, String> {
    let client = metadata_client(&state)?;
    state.it.playlist_continuation(client, &token).await.map_err(|e| e.to_string())
}

/// An album page. `id` is the album browseId (`MPRE…`).
#[tauri::command]
pub async fn get_album(state: St<'_>, id: String) -> Result<AlbumPage, String> {
    // A local album is built from SQLite, so it opens the same page while offline (local.rs).
    if let Some(key) = id.strip_prefix(crate::local::ALBUM_PREFIX) {
        return Ok(crate::local::album_page(&state.db, key));
    }
    // A local artist rides this route too: same page shape, and none of the artist route's
    // YouTube furniture applies to files on disk (see `local::artist_page`).
    if let Some(name) = id.strip_prefix(crate::local::ARTIST_PREFIX) {
        return Ok(crate::local::artist_page(&state.db, name));
    }
    let client = metadata_client(&state)?;
    state.it.album(client, &id).await.map_err(|e| e.to_string())
}

/// An artist page. `id` is the channel browseId (`UC…`).
#[tauri::command]
pub async fn get_artist(state: St<'_>, id: String) -> Result<ArtistPage, String> {
    let client = metadata_client(&state)?;
    state.it.artist(client, &id).await.map_err(|e| e.to_string())
}

/// A card grid reached from a carousel's "More" button (e.g. an artist's full albums list).
#[tauri::command]
pub async fn get_browse_grid(
    state: St<'_>,
    id: String,
    params: Option<String>,
) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    state.it.browse_grid(client, &id, params.as_deref()).await.map_err(|e| e.to_string())
}

/// Play a playlist/album: the given items become the queue (no radio). `start` is the clicked
/// track index; `None`/omitted means "just play it" (random opener when shuffle is on).
/// `source_id` (the page's playlist/album playlist id) makes autoplay continue with that
/// context's radio when the queue runs out. `source_name` (the page title) feeds the queue
/// panel's "Next from" header; `shuffle: true` (page Shuffle buttons) turns shuffle on for
/// this queue — pass the items in their real order, the backend shuffles. `continuation` is the
/// page's next-page token when it has one: pass the tracks that are loaded and the backend walks
/// the rest into the queue in the background, so playback starts on page 1.
#[tauri::command]
pub async fn play_playlist(
    state: St<'_>,
    items: Vec<SongItem>,
    start: Option<usize>,
    source_id: Option<String>,
    source_name: Option<String>,
    shuffle: Option<bool>,
    continuation: Option<String>,
) -> Result<(), String> {
    let state = state.inner().clone();
    state
        .play_tracks(items, start, source_id, source_name, shuffle.unwrap_or(false), continuation)
        .await;
    Ok(())
}

/// Start a radio seeded on a song, artist, album or playlist (context/08). `kind` is
/// `song` | `artist` | `album` | `playlist`; `id` is the videoId (song) or browseId/playlistId
/// (everything else) — the backend resolves it to a radio playlist. `name` titles the queue.
///
/// Starting a song radio on the track that's already playing keeps it playing and replaces only
/// what comes after it; every other case replaces the queue.
#[tauri::command]
pub async fn start_radio(
    state: St<'_>,
    kind: String,
    id: String,
    name: Option<String>,
) -> Result<(), String> {
    let state = state.inner().clone();
    state.start_radio(&kind, &id, name).await
}

// --- write actions (context/01 ✎, context/15) ----------------------------------------------

fn require_login(state: &Arc<AppState>) -> Result<&innertube::YouTubeClient, String> {
    if !state.it.is_logged_in() {
        return Err("Sign in first to use this.".into());
    }
    metadata_client(state)
}

/// Like, dislike, or clear a track's rating. One command for all three: YouTube's states are
/// mutually exclusive, so a dislike un-likes in the same call and the UI never has to send two.
#[tauri::command]
pub async fn rate(state: St<'_>, video_id: String, rating: Rating) -> Result<(), String> {
    let client = require_login(&state)?;
    state.it.rate(client, &video_id, rating).await.map_err(|e| e.to_string())
}

/// Save an album to the library, or remove it. `playlist_id` is the album's `OLAK5uy_…`
/// (`AlbumPage.playlistId`).
#[tauri::command]
pub async fn set_album_saved(
    state: St<'_>,
    playlist_id: String,
    saved: bool,
) -> Result<(), String> {
    let client = require_login(&state)?;
    state.it.like_playlist(client, &playlist_id, saved).await.map_err(|e| e.to_string())
}

/// Login, plus the guard every playlist edit needs: On Repeat has no YouTube playlist behind it, so
/// its synthetic id must never reach `edit_playlist`, which answers 400 for an id it doesn't know.
fn editable_playlist<'a>(
    state: &'a Arc<AppState>,
    playlist_id: &str,
) -> Result<&'a innertube::YouTubeClient, String> {
    if playlist_id == ON_REPEAT_ID {
        return Err("On Repeat builds itself from what you play.".into());
    }
    require_login(state)
}

/// `false` means the playlist already had the track and YouTube added nothing — not an error, but
/// the UI must not draw an optimistic row for it (there is no real row to remove later).
#[tauri::command]
pub async fn add_to_playlist(
    state: St<'_>,
    playlist_id: String,
    video_id: String,
) -> Result<bool, String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state.it.playlist_add(client, &playlist_id, &video_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_playlist(
    state: St<'_>,
    playlist_id: String,
    video_id: String,
    set_video_id: String,
) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state
        .it
        .playlist_remove(client, &playlist_id, &video_id, &set_video_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_playlist(state: St<'_>, title: String) -> Result<String, String> {
    let client = require_login(&state)?;
    state.it.create_playlist(client, &title).await.map_err(|e| e.to_string())
}

/// Edit a playlist you own, from the "Edit playlist" dialog: name, description, visibility.
///
/// Each field is `None` when the user left it alone, and only what changed is sent: an edit of
/// the name must not blank a description we failed to read back off the page.
#[tauri::command]
pub async fn edit_playlist_details(
    state: St<'_>,
    playlist_id: String,
    name: Option<String>,
    description: Option<String>,
    public: Option<bool>,
) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    // The switch is two-state; YouTube's third value (UNLISTED) is only ever left as it was.
    let privacy = public.map(|p| if p { "PUBLIC" } else { "PRIVATE" });
    state
        .it
        .playlist_edit_details(
            client,
            &playlist_id,
            name.as_deref(),
            description.as_deref(),
            privacy,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Custom playlist artwork, kept on this machine: YouTube has no API to set a playlist thumbnail
/// (it builds one from the tracks), so the picked image is copied in beside the local-music covers
/// and swapped in wherever this playlist is drawn. `path: None` drops it, and YouTube's own
/// artwork comes back. Answers the stored path, for the UI to render at once.
#[tauri::command]
pub async fn set_playlist_cover(
    app: tauri::AppHandle,
    state: St<'_>,
    playlist_id: String,
    path: Option<String>,
) -> Result<Option<String>, String> {
    use tauri::Manager;
    const IMAGE_EXTS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];

    let key = cover_key(&playlist_id);
    // Whatever was there is ours and is about to be replaced or dropped.
    if let Some(old) = state.db.get_setting(&key) {
        let _ = std::fs::remove_file(old);
    }
    let Some(src) = path else {
        state.db.delete_setting(&key);
        return Ok(None);
    };
    let src = std::path::Path::new(&src);
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err("Pick a JPEG, PNG, WebP or GIF image.".into());
    }
    let dir = crate::local::covers_dir(&app).join("playlists");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Timestamped, so replacing a cover can't be served out of the webview's cache under the name
    // it already has. The id is filtered to filename characters rather than trusted: it arrives
    // from the UI, and a `..` in it would write outside this directory.
    let stem: String = playlist_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let dest = dir.join(format!("{stem}-{}.{ext}", crate::db::now_secs()));
    std::fs::copy(src, &dest).map_err(|e| e.to_string())?;
    let dest = dest.to_string_lossy().to_string();
    // The covers directory is allowed recursively at startup, but the first cover on a fresh
    // install is written after that ran, so name this file explicitly too.
    let _ = app.asset_protocol_scope().allow_file(&dest);
    state.db.set_setting(&key, &dest);
    Ok(Some(dest))
}

fn cover_key(playlist_id: &str) -> String {
    // Browse ids arrive `VL`-prefixed and playlist ids don't; one playlist, one key either way.
    format!("playlist_cover:{}", playlist_id.strip_prefix("VL").unwrap_or(playlist_id))
}

/// The custom artwork stored for a playlist, if the file is still there. The user owns that
/// directory and can empty it, and a dead path renders as a broken image.
fn custom_cover(state: &Arc<AppState>, playlist_id: &str) -> Option<String> {
    let path = state.db.get_setting(&cover_key(playlist_id))?;
    std::path::Path::new(&path).is_file().then_some(path)
}

#[tauri::command]
pub async fn delete_playlist(state: St<'_>, playlist_id: String) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state.it.delete_playlist(client, &playlist_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn subscribe(state: St<'_>, channel_id: String, subscribed: bool) -> Result<(), String> {
    let client = require_login(&state)?;
    state.it.subscribe(client, &channel_id, subscribed).await.map_err(|e| e.to_string())
}

// --- local music (local.rs) ------------------------------------------------------------------

/// Rescan the watched folders and return the library. The scan is the deletion check too: its
/// `removed` list is every id that was on screen but is gone from disk, so the UI can drop those
/// tiles without waiting for anyone to click a dead one.
#[tauri::command]
pub async fn get_local_library(state: St<'_>) -> Result<crate::local::LocalLibrary, String> {
    scan_local(&state).await
}

#[tauri::command]
pub async fn add_local_folder(
    state: St<'_>,
    path: String,
) -> Result<crate::local::LocalLibrary, String> {
    crate::local::add_folder(&state.db, path);
    scan_local(&state).await
}

/// Stop watching a folder. Its tracks disappear from the library on the rescan that follows (they
/// come back untouched if the folder is added again — nothing on disk is modified).
#[tauri::command]
pub async fn remove_local_folder(
    state: St<'_>,
    path: String,
) -> Result<crate::local::LocalLibrary, String> {
    crate::local::remove_folder(&state.db, &path);
    scan_local(&state).await
}

/// Disk IO + tag parsing off the async runtime's worker threads.
async fn scan_local(state: &Arc<AppState>) -> Result<crate::local::LocalLibrary, String> {
    let app = state.app.clone();
    let state = state.clone();
    let covers = crate::local::covers_dir(&state.app);
    let lib = tauri::async_runtime::spawn_blocking(move || crate::local::scan(&state.db, &covers))
        .await
        .map_err(|e| e.to_string())?;
    // Artwork reaches the page over the asset protocol, which starts out allowing nothing.
    crate::local::allow_covers(&app, &lib.songs);
    Ok(lib)
}

// --- Listen Together (context/19) ----------------------------------------------------------

/// Current client-side LT state (status, role, room, participants, pending joins, suggestions).
#[tauri::command]
pub async fn lt_get_state(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.lt.snapshot().await)
}

/// Set + persist the sync server URL (e.g. the Tailscale Funnel `wss://…` address).
#[tauri::command]
pub async fn lt_set_server_url(state: St<'_>, url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    state.db.set_setting("lt_server_url", &url);
    state.lt.set_server_url(url).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_create_room(state: St<'_>, username: String) -> Result<(), String> {
    state.lt.create_room(username).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_join_room(state: St<'_>, code: String, username: String) -> Result<(), String> {
    state.lt.join_room(code, username).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_leave(state: St<'_>) -> Result<(), String> {
    state.lt.leave().await;
    Ok(())
}

#[tauri::command]
pub async fn lt_approve_join(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.approve_join(user_id).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_reject_join(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.reject_join(user_id).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_kick(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.kick(user_id).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_transfer_host(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.transfer_host(user_id).await;
    Ok(())
}

/// Guest: send a track to the session queue (auto-approved by the host client, which stamps
/// who added it).
#[tauri::command]
pub async fn lt_suggest(state: St<'_>, item: SongItem) -> Result<(), String> {
    state.lt.suggest(crate::state::song_to_track(&item)).await;
    Ok(())
}

/// Host: approve a suggestion — add it to the real queue and notify the suggester. (Unused since
/// guest adds auto-approve, kept for a future "require approval" setting.)
#[tauri::command]
pub async fn lt_approve_suggestion(state: St<'_>, id: String) -> Result<(), String> {
    if let Some(track) = state.lt.approve_suggestion(id).await {
        state.inner().clone().lt_enqueue_track(track).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn lt_reject_suggestion(state: St<'_>, id: String) -> Result<(), String> {
    state.lt.reject_suggestion(id).await;
    Ok(())
}

/// Guest: force a re-sync with the room (drift correction).
#[tauri::command]
pub async fn lt_request_sync(state: St<'_>) -> Result<(), String> {
    state.lt.request_sync().await;
    Ok(())
}

// --- lyrics ---------------------------------------------------------------------------------

/// Lyrics for a track (cached). The UI passes the metadata it already has from `now-playing`;
/// `duration` is mpv's length in seconds. `None` = no lyrics found anywhere.
#[tauri::command]
pub async fn get_lyrics(
    state: St<'_>,
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    duration: Option<f64>,
) -> Result<Option<crate::lyrics::Lyrics>, String> {
    Ok(crate::lyrics::get_lyrics(
        state.inner(),
        crate::lyrics::LyricsRequest { video_id, title, artists, album, duration },
    )
    .await)
}

// --- Changelog ------------------------------------------------------------------------------

#[derive(Clone, serde::Serialize)]
pub struct ReleaseNote {
    version: String,
    /// `YYYY-MM-DD`, or empty for an unpublished tag.
    date: String,
    /// The release description, verbatim markdown. The About tab renders it.
    body: String,
}

/// What's new, read straight from the GitHub releases API so the release description is the only
/// place the changelog is written. Cached for the process: the list only changes when a release
/// is cut, and unauthenticated GitHub allows 60 requests an hour.
#[tauri::command]
pub async fn release_notes() -> Result<Vec<ReleaseNote>, String> {
    static CACHE: std::sync::OnceLock<Vec<ReleaseNote>> = std::sync::OnceLock::new();
    if let Some(cached) = CACHE.get() {
        return Ok(cached.clone());
    }
    #[derive(serde::Deserialize)]
    struct GhRelease {
        tag_name: String,
        published_at: Option<String>,
        body: Option<String>,
        draft: bool,
        prerelease: bool,
    }
    let releases: Vec<GhRelease> = crate::http::client()
        .get("https://api.github.com/repos/SimoHypers/limusic/releases?per_page=20")
        .header("User-Agent", concat!("Limusic/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let notes: Vec<ReleaseNote> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .map(|r| ReleaseNote {
            version: r.tag_name.trim_start_matches('v').to_string(),
            date: r
                .published_at
                .and_then(|d| d.split('T').next().map(str::to_string))
                .unwrap_or_default(),
            body: r.body.unwrap_or_default(),
        })
        .collect();
    Ok(CACHE.get_or_init(|| notes).clone())
}

/// Open a link from the UI in the real browser. An `<a href>` inside the webview would navigate
/// the app itself off the SPA, with no way back.
#[tauri::command]
pub async fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("only http(s) links".into());
    }
    crate::lastfm::open_browser(&url)
}

// --- Last.fm scrobbling ---------------------------------------------------------------------

/// Start the browser auth flow. Returns once the authorize page is open; the outcome (session
/// stored, or an error) arrives via the `lastfm-state` event.
#[tauri::command]
pub async fn lastfm_connect(state: St<'_>) -> Result<(), String> {
    crate::lastfm::connect(state.inner().clone()).await
}

#[tauri::command]
pub async fn lastfm_disconnect(state: St<'_>) -> Result<(), String> {
    crate::lastfm::disconnect(&state);
    Ok(())
}

/// `{ connected, username }` from the persisted session — seeds the titlebar button on mount.
#[tauri::command]
pub async fn lastfm_status(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(crate::lastfm::status(&state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_repeat_rows_shed_the_queue_slot_they_were_played_from() {
        let played = SongItem {
            video_id: "abc".into(),
            title: "Grace".into(),
            queued: true,
            queued_by: Some("simohypers".into()),
            autoplay: true,
            set_video_id: Some("SVI".into()),
            ..Default::default()
        };
        let row = shed_queue_context(played.clone());
        assert_eq!(
            row,
            SongItem { video_id: "abc".into(), title: "Grace".into(), ..Default::default() }
        );
        assert_eq!(row.title, played.title, "the song itself survives");
    }
}
