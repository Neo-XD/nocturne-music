//! Search + next(queue) parsing. context/08.
//!
//! YouTube's response is a deeply-nested "renderer" tree. Rather than port Metrolist's ~40
//! renderer classes, we walk the raw JSON for the two node types we need
//! (`musicResponsiveListItemRenderer` for search, `playlistPanelVideoRenderer` for next) and
//! pull only the handful of fields the playback path uses. Targeted and robust to the tree
//! moving around (fixture reality wins over spec — see plan risks).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A song item — the minimum the playback path (context/06) needs. context/08.
/// Round-trips through the UI (serialized into search results, deserialized back into `play`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SongItem {
    pub video_id: String,
    pub title: String,
    pub artists: String,
    /// The primary artist's channel browseId (`UC…`), when the row links one — lets the UI make
    /// the artist name navigate to its artist page. context/08.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    /// The artist line split into its original runs, each tagged with its own channel id when it
    /// links one — a collab ("Future & Metro Boomin") links each name separately. Empty when the
    /// row links no artist at all; the UI then falls back to plain `artists`. context/08.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artist_runs: Vec<ArtistRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// The album's browseId (`MPRE…`), when the row links one — lets the UI navigate to the album.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// The row's play count as YouTube abbreviates it ("53M"), from an album page's plays column.
    /// Absent on rows that don't carry one (playlists, search, queue).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// `playlistSetVideoId` — the item's id *within a playlist*, needed to remove it (context/01
    /// edit_playlist). Only present when the item came from a playlist page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_video_id: Option<String>,
    /// The signed-in user's rating of this track, from the row's `likeStatus`. `None` when the
    /// response didn't carry one, which the UI treats the same as [`Rating::Indifferent`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<Rating>,
    /// Listen Together: username of the guest who added this queue item (`None` for the user's own
    /// tracks). Never parsed from YouTube — pure queue metadata, carried for attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_by: Option<String>,
    /// "Play next" (or a guest's session add): marks the "up next" block so successive adds stack
    /// FIFO right after the current song. Pure queue metadata, never parsed.
    #[serde(default)]
    pub queued: bool,
    /// "Add to queue": appended at the tail, after everything the user picked. Its own block in the
    /// queue panel — without this it would read as part of the playlist that's playing. Pure queue
    /// metadata, never parsed.
    #[serde(default)]
    pub queued_end: bool,
    /// What either block was added from ("Nightcore Bangers"), when it came from an album/playlist.
    /// The panel heads the block with it instead of the playing playlist's name. `None` for
    /// single-song adds. Pure queue metadata, never parsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_from: Option<String>,
    /// Appended by autoplay radio continuation (vs. chosen by the user). Drives the queue's
    /// "Autoplay" divider + player-bar badge. Pure queue metadata, never parsed.
    #[serde(default)]
    pub autoplay: bool,
    /// This row links a music video, not the audio track ([`is_video_row`]). Drives the
    /// "hide music videos" setting; computed once here, never re-derived downstream.
    #[serde(default)]
    pub is_video: bool,
}

/// How the signed-in user rated a track. One type for both directions: it's what a row's
/// `likeStatus` parses into, and what the write action ([`crate::InnerTube::rate`]) takes. The three
/// values are mutually exclusive on YouTube's side, so rating a liked track "dislike" un-likes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Like,
    Dislike,
    /// No rating: what `like/removelike` leaves behind, and what an unrated row reports.
    Indifferent,
}

/// `MUSIC_VIDEO_TYPE_ATV` — the audio track YouTube Music generates for a release. Anything else
/// (`_OMV`, `_UGC`) is a video upload.
const AUDIO_TRACK_TYPE: &str = "MUSIC_VIDEO_TYPE_ATV";

/// The `musicVideoType` a watch endpoint carries, if any.
fn endpoint_video_type(endpoint: &Value) -> Option<&str> {
    endpoint
        .get("watchEndpoint")
        .or_else(|| endpoint.get("watchPlaylistEndpoint"))?
        .get("watchEndpointMusicSupportedConfigs")?
        .get("watchEndpointMusicConfig")?
        .get("musicVideoType")?
        .as_str()
}

/// True when a watch endpoint points at a music video rather than the audio track.
pub(crate) fn is_video_endpoint(endpoint: &Value) -> bool {
    matches!(endpoint_video_type(endpoint), Some(t) if t != AUDIO_TRACK_TYPE)
}

/// True when a renderer row (list row, two-row card, or queue panel row) links a music video.
///
/// The authoritative tag sits on the thumbnail overlay's play button (`overlay` on list rows,
/// `thumbnailOverlay` on cards); queue-panel rows have no overlay, so the row's own navigation
/// endpoint is the fallback. Absent ⇒ we can't tell ⇒ audio, so a parse that misses the tag
/// degrades to "keep everything" rather than hiding the library.
pub(crate) fn is_video_row(node: &Value) -> bool {
    let overlay = node
        .get("overlay")
        .or_else(|| node.get("thumbnailOverlay"))
        .and_then(|o| o.get("musicItemThumbnailOverlayRenderer"))
        .and_then(|o| o.get("content"))
        .and_then(|c| c.get("musicPlayButtonRenderer"))
        .and_then(|p| p.get("playNavigationEndpoint"));
    match overlay {
        Some(ep) if endpoint_video_type(ep).is_some() => is_video_endpoint(ep),
        _ => node.get("navigationEndpoint").is_some_and(is_video_endpoint),
    }
}

/// One run of an artist line: the literal text plus its channel browseId when it links one
/// (separators like " & " carry no id).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtistRun {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub items: Vec<SongItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NextResult {
    pub items: Vec<SongItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
    /// The lyrics tab's browseId (`MPLYt…`) — feed it to a lyrics `browse` (models::lyrics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics_browse_id: Option<String>,
    /// The mix the panel says it continues into (`automixPreviewVideoRenderer`, context/08): a
    /// radio playlist id to re-request `next` with. Present on a bare `next(videoId)`, which is
    /// otherwise just the seed song — that's how a dead `RDAMVM` radio finds a live one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automix_playlist_id: Option<String>,
}

/// Logged-in account summary from `account/account_menu`. context/01, context/04A, context/15.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountInfo {
    pub name: Option<String>,
    /// Channel handle or email (whichever the header carries).
    pub handle: Option<String>,
    pub thumbnail: Option<String>,
    /// `onBehalfOfUser` id, `||`-split (context/04A). None when absent / single-account.
    pub data_sync_id: Option<String>,
    /// A login-bound visitorData, if the response carried one (context/15).
    pub visitor_data: Option<String>,
}

/// Parse a `search` response into song items. context/08.
pub fn parse_search(root: &Value) -> SearchResult {
    let mut items = Vec::new();
    for node in find_all(root, "musicResponsiveListItemRenderer") {
        if let Some(item) = parse_list_item(node) {
            items.push(item);
        }
    }
    SearchResult { items }
}

/// Parse a `next` response into the up-next queue + continuation token. context/08.
pub fn parse_next(root: &Value) -> NextResult {
    let mut items = Vec::new();
    for node in find_all(root, "playlistPanelVideoRenderer") {
        if let Some(item) = parse_panel_video(node) {
            items.push(item);
        }
    }
    // The automix/radio continuation (context/08): the panel ends with a continuation token
    // used to fetch the endless mix. Take the first continuation token we find.
    let continuation = find_first_str(root, "continuation");
    let automix_playlist_id = find_all(root, "automixPreviewVideoRenderer")
        .into_iter()
        .find_map(|n| find_first_str(n, "playlistId"));
    NextResult {
        items,
        continuation,
        lyrics_browse_id: lyrics_browse_id(root),
        automix_playlist_id,
    }
}

/// The lyrics tab's browseId from a `next` response: the browseEndpoint whose pageType is
/// `MUSIC_PAGE_TYPE_TRACK_LYRICS`. context/08 §lyrics.
fn lyrics_browse_id(root: &Value) -> Option<String> {
    find_all(root, "browseEndpoint").into_iter().find_map(|be| {
        (find_first_str(be, "pageType").as_deref() == Some("MUSIC_PAGE_TYPE_TRACK_LYRICS"))
            .then(|| be.get("browseId").and_then(Value::as_str).map(str::to_owned))
            .flatten()
    })
}

/// Parse an `account/account_menu` response into an account summary. context/01, context/15.
pub fn parse_account_menu(root: &Value) -> AccountInfo {
    let header = find_all(root, "activeAccountHeaderRenderer").into_iter().next();
    let name = header.and_then(|h| runs_text(h.get("accountName")));
    // YTM labels the second line `channelHandle` on newer accounts, `email` on older ones.
    let handle = header
        .and_then(|h| runs_text(h.get("channelHandle")).or_else(|| runs_text(h.get("email"))));
    let thumbnail = header.and_then(last_thumbnail);

    let rc = root.get("responseContext");
    // dataSyncId lives in the response context, not the menu header. context/04A.
    let data_sync_id = rc
        .and_then(|r| r.get("mainAppWebResponseContext"))
        .and_then(|m| m.get("datasyncId"))
        .and_then(Value::as_str)
        .map(split_datasync_id);
    let visitor_data = rc
        .and_then(|r| r.get("visitorData"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    AccountInfo { name, handle, thumbnail, data_sync_id, visitor_data }
}

/// Split a `dataSyncId` (`"<id>||<other>"`): prefer the part before `||`, else after. context/04A.
fn split_datasync_id(raw: &str) -> String {
    match raw.split_once("||") {
        Some((before, _)) if !before.is_empty() => before.to_owned(),
        Some((_, after)) => after.to_owned(),
        None => raw.to_owned(),
    }
}

// --- node parsers -------------------------------------------------------------------------

pub(crate) fn parse_list_item(node: &Value) -> Option<SongItem> {
    let video_id = list_item_video_id(node)?;
    let flex = node.get("flexColumns").and_then(Value::as_array);
    let title = flex.and_then(|c| c.first()).and_then(flex_text).unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    // Second flex column holds subtitle runs: "Artist • Album • duration" (• separated).
    let subtitle_runs = flex_runs(node, 1);
    let (artists, album, duration) = split_subtitle(subtitle_runs);
    // Playlist/album rows keep the length in a fixed column instead of the subtitle. context/08.
    let duration = duration.or_else(|| fixed_column_text(node));
    let artist_id = subtitle_runs.and_then(|r| first_artist_id(r));
    let set_video_id = node
        .get("playlistItemData")
        .and_then(|d| d.get("playlistSetVideoId"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(SongItem {
        video_id,
        title,
        artists,
        artist_id,
        artist_runs: subtitle_runs.map(|r| artist_runs(r)).unwrap_or_default(),
        album,
        album_id: album_id(node),
        duration,
        play_count: play_count(node),
        thumbnail: last_thumbnail(node),
        set_video_id,
        rating: like_status(node),
        queued_by: None,
        queued: false,
        queued_end: false,
        queued_from: None,
        autoplay: false,
        is_video: is_video_row(node),
    })
}

/// The play count from an album row's third flex column ("53M plays" → "53M"). Playlist rows put
/// the album name in that column instead, so the trailing "plays" is the discriminator — the
/// locale is pinned to en (models::context), so it's always that word. Live-verified 2026-08.
fn play_count(node: &Value) -> Option<String> {
    let text = flex_column_text(node, 2)?;
    let (count, unit) = text.trim().rsplit_once(' ')?;
    unit.eq_ignore_ascii_case("plays").then(|| count.to_owned())
}

/// The album's browseId (`MPRE…`): either the linked album run or the row menu's "Go to album"
/// entry — whichever the renderer carries. Tolerant: first `MPRE…` browseId in the node. context/08.
fn album_id(node: &Value) -> Option<String> {
    find_all(node, "browseId")
        .into_iter()
        .filter_map(Value::as_str)
        .find(|id| id.starts_with("MPRE"))
        .map(str::to_owned)
}

/// The artist field of a run list, kept run by run so each linked artist of a collab navigates to
/// its own page. Empty when nothing links a channel. Cut at the "•" separators and dropping a
/// leading type label exactly like `split_subtitle`, so these runs describe the same field as the
/// `artists` string beside them: a search row reads "Song • Delara • 3:02", and taking everything
/// before the first "•" would hand back the unlinked word "Song". context/08.
pub(crate) fn artist_runs(runs: &[Value]) -> Vec<ArtistRun> {
    let mut fields: Vec<Vec<ArtistRun>> = vec![Vec::new()];
    for run in runs {
        let text = run.get("text").and_then(Value::as_str).unwrap_or_default();
        if text.trim() == "•" {
            fields.push(Vec::new());
        } else {
            fields.last_mut().expect("never empty").push(ArtistRun {
                text: text.to_owned(),
                id: first_artist_id(std::slice::from_ref(run)),
            });
        }
    }
    let linked = |f: &Vec<ArtistRun>| f.iter().any(|r| r.id.is_some());
    if fields.len() > 1 && !linked(&fields[0]) {
        let label: String = fields[0].iter().map(|r| r.text.as_str()).collect();
        if is_type_label(label.trim()) || fields[1..].iter().any(linked) {
            fields.remove(0);
        }
    }
    let out = fields.swap_remove(0);
    if !linked(&out) {
        return Vec::new();
    }
    out
}

/// First run that links an artist channel (`browseEndpoint.browseId` starting with `UC`). context/08.
pub(crate) fn first_artist_id(runs: &[Value]) -> Option<String> {
    runs.iter().find_map(|r| {
        let id = r.get("navigationEndpoint")?.get("browseEndpoint")?.get("browseId")?.as_str()?;
        id.starts_with("UC").then(|| id.to_owned())
    })
}

/// The track's rating from its menu's `likeStatus` (`LIKE` / `INDIFFERENT` / `DISLIKE`).
/// Tolerant: grabs the first `likeStatus` anywhere in the node, and reads anything it doesn't
/// recognise as unrated rather than dropping the row. context/08.
fn like_status(node: &Value) -> Option<Rating> {
    find_first_str(node, "likeStatus").map(|s| match s.as_str() {
        "LIKE" => Rating::Like,
        "DISLIKE" => Rating::Dislike,
        _ => Rating::Indifferent,
    })
}

fn parse_panel_video(node: &Value) -> Option<SongItem> {
    let video_id = node.get("videoId").and_then(Value::as_str)?.to_owned();
    let title = runs_text(node.get("title"))?;
    let byline = node.get("longBylineText").or_else(|| node.get("shortBylineText"));
    let byline_runs = byline.and_then(|b| b.get("runs")).and_then(Value::as_array);
    // The byline is a full descriptor ("Delara • Sjelen • 2026"), not a name: take its artist
    // field only, or the queue (and the scrobbler behind it) gets the whole string as the artist.
    let artists = artists_from_runs(byline_runs).unwrap_or_default();
    let artist_id = byline_runs.and_then(|r| first_artist_id(r));
    let duration = node.get("lengthText").and_then(runs_text_opt);
    Some(SongItem {
        video_id,
        title,
        artists,
        artist_id,
        artist_runs: byline_runs.map(|r| artist_runs(r)).unwrap_or_default(),
        album: None,
        album_id: album_id(node),
        duration,
        play_count: None,
        thumbnail: last_thumbnail(node),
        set_video_id: None,
        rating: like_status(node),
        queued_by: None,
        queued: false,
        queued_end: false,
        queued_from: None,
        autoplay: false,
        is_video: is_video_row(node),
    })
}

/// Joined text of a `musicResponsiveListItemRenderer` flex column (0 = title, 1 = subtitle). Used
/// by the search-section parser to build cards from list rows. context/08.
/// The raw runs of a list row's `i`th flex column (the text with its per-run links intact).
pub(crate) fn flex_runs(node: &Value, i: usize) -> Option<&Vec<Value>> {
    node.get("flexColumns")
        .and_then(Value::as_array)
        .and_then(|c| c.get(i))
        .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|r| r.get("text"))
        .and_then(|t| t.get("runs"))
        .and_then(Value::as_array)
}

pub(crate) fn flex_column_text(node: &Value, i: usize) -> Option<String> {
    node.get("flexColumns").and_then(Value::as_array).and_then(|c| c.get(i)).and_then(flex_text)
}

/// videoId from any of the three known locations. context/08 / AlbumPage.kt.
pub(crate) fn list_item_video_id(node: &Value) -> Option<String> {
    let direct = node
        .get("playlistItemData")
        .and_then(|d| d.get("videoId"))
        .and_then(Value::as_str)
        .or_else(|| {
            node.get("navigationEndpoint")
                .and_then(|n| n.get("watchEndpoint"))
                .and_then(|w| w.get("videoId"))
                .and_then(Value::as_str)
        });
    match direct {
        Some(id) => Some(id.to_owned()),
        // Last resort: the play-button overlay's watchEndpoint videoId.
        None => node.get("overlay").and_then(|o| find_first_str(o, "videoId")),
    }
}

// --- small helpers ------------------------------------------------------------------------

/// The row's `fixedColumns` duration ("3:47"). Playlist and album track rows carry it here.
fn fixed_column_text(node: &Value) -> Option<String> {
    node.get("fixedColumns")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|c| {
            c.get("musicResponsiveListItemFixedColumnRenderer")
                .and_then(|r| r.get("text"))
                .and_then(runs_text_opt)
        })
        .filter(|s| s.contains(':'))
}

fn flex_text(col: &Value) -> Option<String> {
    col.get("musicResponsiveListItemFlexColumnRenderer")
        .and_then(|r| r.get("text"))
        .and_then(runs_text_opt)
}

pub(crate) fn runs_text(v: Option<&Value>) -> Option<String> {
    v.and_then(runs_text_opt)
}

/// Join all `runs[].text` in a `{ runs: [...] }` object.
pub(crate) fn runs_text_opt(v: &Value) -> Option<String> {
    let runs = v.get("runs").and_then(Value::as_array)?;
    let s: String = runs.iter().filter_map(|r| r.get("text").and_then(Value::as_str)).collect();
    (!s.is_empty()).then_some(s)
}

/// One "•"-separated field of a subtitle, plus whether it links an artist channel (`UC…`).
struct Group {
    text: String,
    artist_link: bool,
}

/// Cut a subtitle run list at its "•" separators, keeping each field's artist link.
fn subtitle_groups(runs: &[Value]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut cur = Group { text: String::new(), artist_link: false };
    for run in runs {
        let t = run.get("text").and_then(Value::as_str).unwrap_or("");
        if t.trim() == "•" {
            groups.push(std::mem::replace(
                &mut cur,
                Group { text: String::new(), artist_link: false },
            ));
        } else {
            cur.text.push_str(t);
            cur.artist_link |= first_artist_id(std::slice::from_ref(run)).is_some();
        }
    }
    groups.push(cur);
    for g in &mut groups {
        g.text = g.text.trim().to_string();
    }
    groups
}

/// Result rows on an unfiltered search lead with the result type: "Song • Delara • 3:02". Nothing
/// downstream wants that word, and taken as an artist it lands in the user's Last.fm scrobbles.
fn is_type_label(s: &str) -> bool {
    matches!(
        s,
        "Song"
            | "Video"
            | "Album"
            | "Single"
            | "EP"
            | "Playlist"
            | "Artist"
            | "Episode"
            | "Podcast"
    )
}

/// Split a "• "-separated subtitle run list into (artists, album, duration). context/08.
fn split_subtitle(runs: Option<&Vec<Value>>) -> (String, Option<String>, Option<String>) {
    let Some(runs) = runs else { return (String::new(), None, None) };
    let mut groups = subtitle_groups(runs);
    // Drop a leading type label so artist/album don't both shift one field to the right. A later
    // field linking an artist channel proves the first one isn't the artist; the word list covers
    // the rows where nothing is linked at all.
    if groups.len() > 1
        && !groups[0].artist_link
        && (is_type_label(&groups[0].text) || groups[1..].iter().any(|g| g.artist_link))
    {
        groups.remove(0);
    }
    let groups: Vec<String> = groups.into_iter().map(|g| g.text).collect();
    let artists = groups.first().cloned().unwrap_or_default();
    // Last group that looks like a duration (contains ':') is the duration; the middle is album.
    let duration = groups.iter().rev().find(|g| g.contains(':')).cloned();
    let album = groups.get(1).filter(|g| Some(*g) != duration.as_ref()).cloned();
    (artists, album, duration)
}

/// Just the artist field of a subtitle run list, for the surfaces that keep a flat string.
pub(crate) fn artists_from_runs(runs: Option<&Vec<Value>>) -> Option<String> {
    Some(split_subtitle(runs).0).filter(|s| !s.is_empty())
}

/// Deepest/last thumbnail URL under this node (highest resolution).
pub(crate) fn last_thumbnail(node: &Value) -> Option<String> {
    // Find any `thumbnails: [ { url }, ... ]` array and take the last url.
    fn walk(v: &Value) -> Option<String> {
        match v {
            Value::Object(map) => {
                if let Some(arr) = map.get("thumbnails").and_then(Value::as_array) {
                    if let Some(url) = arr.last().and_then(|t| t.get("url")).and_then(Value::as_str)
                    {
                        return Some(url.to_owned());
                    }
                }
                map.values().find_map(walk)
            }
            Value::Array(arr) => arr.iter().find_map(walk),
            _ => None,
        }
    }
    walk(node)
}

/// Recursively collect every object that is the value of a key named `key`.
pub(crate) fn find_all<'a>(root: &'a Value, key: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    fn walk<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
        match v {
            Value::Object(map) => {
                for (k, val) in map {
                    if k == key {
                        out.push(val);
                    }
                    walk(val, key, out);
                }
            }
            Value::Array(arr) => arr.iter().for_each(|e| walk(e, key, out)),
            _ => {}
        }
    }
    walk(root, key, &mut out);
    out
}

/// Like [`find_all`], but does not descend into a node once it matches `key`. Use when collecting
/// "top-level" renderers (e.g. playlist track rows): an *editable* playlist item embeds a nested
/// copy of its own `musicResponsiveListItemRenderer` inside an add-suggestion edit command, so a
/// deep search counts every track twice. Stopping at the first match avoids that double-count.
pub(crate) fn find_all_shallow<'a>(root: &'a Value, key: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    fn walk<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
        match v {
            Value::Object(map) => {
                for (k, val) in map {
                    if k == key {
                        out.push(val); // matched — do NOT recurse into it
                    } else {
                        walk(val, key, out);
                    }
                }
            }
            Value::Array(arr) => arr.iter().for_each(|e| walk(e, key, out)),
            _ => {}
        }
    }
    walk(root, key, &mut out);
    out
}

/// First string value under any key named `key`.
pub(crate) fn find_first_str(root: &Value, key: &str) -> Option<String> {
    match root {
        Value::Object(map) => {
            for (k, v) in map {
                if k == key {
                    if let Some(s) = v.as_str() {
                        return Some(s.to_owned());
                    }
                }
                if let Some(s) = find_first_str(v, key) {
                    return Some(s);
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(|e| find_first_str(e, key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // The whole "hide music videos" feature is this predicate, and the way it fails is silent: a
    // wrong JSON path reads `None` everywhere, the filter quietly becomes a no-op, and the setting
    // just looks broken. So: every shape a row arrives in, plus the fail-open case.
    #[test]
    fn video_rows_are_recognised_in_every_renderer_shape() {
        let cfg = |t: &str| {
            json!({ "watchEndpoint": { "videoId": "v",
                "watchEndpointMusicSupportedConfigs": {
                    "watchEndpointMusicConfig": { "musicVideoType": t } } } })
        };
        let overlay = |t: &str| {
            json!({ "musicItemThumbnailOverlayRenderer": { "content": {
                "musicPlayButtonRenderer": { "playNavigationEndpoint": cfg(t) } } } })
        };

        // Queue-panel row (`/next`): no overlay, the tag sits on the row's own endpoint.
        assert!(is_video_row(&json!({ "navigationEndpoint": cfg("MUSIC_VIDEO_TYPE_OMV") })));
        assert!(is_video_row(&json!({ "navigationEndpoint": cfg("MUSIC_VIDEO_TYPE_UGC") })));
        assert!(!is_video_row(&json!({ "navigationEndpoint": cfg("MUSIC_VIDEO_TYPE_ATV") })));

        // List row: `overlay` wins over the row endpoint. Card: same, under `thumbnailOverlay`.
        assert!(is_video_row(&json!({
            "overlay": overlay("MUSIC_VIDEO_TYPE_OMV"),
            "navigationEndpoint": cfg("MUSIC_VIDEO_TYPE_ATV"),
        })));
        assert!(!is_video_row(&json!({
            "thumbnailOverlay": overlay("MUSIC_VIDEO_TYPE_ATV"),
            "navigationEndpoint": cfg("MUSIC_VIDEO_TYPE_OMV"),
        })));

        // Fail open: no tag (or an overlay carrying none) means audio, never hide.
        assert!(!is_video_row(
            &json!({ "navigationEndpoint": { "watchEndpoint": { "videoId": "v" } } })
        ));
        assert!(!is_video_row(&json!({})));
    }

    // The rating drives both thumbs on every row, and the third state is new: `DISLIKE` used to
    // collapse into "not liked". An unknown value must read as unrated rather than as a dislike.
    #[test]
    fn reads_all_three_rating_states() {
        let row = |status: &str| {
            json!({ "menu": { "menuRenderer": { "topLevelButtons": [
                { "likeButtonRenderer": { "likeStatus": status } }
            ] } } })
        };
        assert_eq!(like_status(&row("LIKE")), Some(Rating::Like));
        assert_eq!(like_status(&row("DISLIKE")), Some(Rating::Dislike));
        assert_eq!(like_status(&row("INDIFFERENT")), Some(Rating::Indifferent));
        assert_eq!(like_status(&row("SOMETHING_NEW")), Some(Rating::Indifferent));
        assert_eq!(like_status(&json!({})), None);
    }

    // A dead `RDAMVM` radio answers with the seed song plus this marker, which names the mix the
    // song really belongs to. It's the escalation the "start radio did nothing" case runs on.
    #[test]
    fn parses_the_automix_the_panel_continues_into() {
        let root = json!({
            "contents": { "playlistPanelRenderer": { "contents": [
                { "playlistPanelVideoRenderer": {
                    "videoId": "seed1",
                    "title": { "runs": [{ "text": "Only Song" }] },
                    "longBylineText": { "runs": [{ "text": "An Artist" }] },
                    "lengthText": { "runs": [{ "text": "3:00" }] },
                    "thumbnail": { "thumbnails": [{ "url": "https://t/1" }] }
                } },
                { "automixPreviewVideoRenderer": { "content": { "automixPlaylistVideoRenderer": {
                    "navigationEndpoint": { "watchPlaylistEndpoint": { "playlistId": "RDAMVMseed1" } }
                } } } }
            ] } }
        });
        let out = parse_next(&root);
        assert_eq!(out.items.len(), 1);
        assert_eq!(out.automix_playlist_id.as_deref(), Some("RDAMVMseed1"));
    }

    // A real radio page has no automix marker — nothing to escalate to, and nothing to mistake a
    // regular playlist id for.
    #[test]
    fn no_automix_on_a_live_radio_page() {
        let root = json!({
            "contents": { "playlistPanelRenderer": {
                "playlistId": "RDAMVMseed1",
                "contents": []
            } }
        });
        assert_eq!(parse_next(&root).automix_playlist_id, None);
    }

    #[test]
    fn parses_search_item() {
        let root = json!({
            "a": { "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abc123" },
                "flexColumns": [
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Song Title" }] } } },
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                        { "text": "The Artist", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCartist1" } } },
                        { "text": " & " },
                        { "text": "Guest", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCartist2" } } },
                        { "text": " • " },
                        { "text": "The Album", "navigationEndpoint": { "browseEndpoint": { "browseId": "MPREalbum1" } } },
                        { "text": " • " }, { "text": "3:21" }
                    ] } } }
                ],
                "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                    { "url": "small.jpg" }, { "url": "big.jpg" }
                ] } } }
            }}
        });
        let r = parse_search(&root);
        assert_eq!(r.items.len(), 1);
        let s = &r.items[0];
        assert_eq!(s.video_id, "abc123");
        assert_eq!(s.title, "Song Title");
        assert_eq!(s.artists, "The Artist & Guest");
        assert_eq!(s.artist_id.as_deref(), Some("UCartist1"));
        // Each artist keeps its own link; the run list stops at the first "•" (album/duration).
        assert_eq!(
            s.artist_runs.iter().map(|r| (r.text.as_str(), r.id.as_deref())).collect::<Vec<_>>(),
            vec![("The Artist", Some("UCartist1")), (" & ", None), ("Guest", Some("UCartist2"))]
        );
        assert_eq!(s.album.as_deref(), Some("The Album"));
        assert_eq!(s.album_id.as_deref(), Some("MPREalbum1"));
        assert_eq!(s.duration.as_deref(), Some("3:21"));
        assert_eq!(s.thumbnail.as_deref(), Some("big.jpg"));
    }

    // Album rows carry "53M plays" in the third flex column; playlist rows put the album name
    // there. Confusing the two would print an album title where the play count goes.
    #[test]
    fn play_count_comes_only_from_a_plays_column() {
        let row = |third: Value| {
            json!({ "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abc123" },
                "flexColumns": [
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Song Title" }] } } },
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "The Artist" }] } } },
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": third }] } } }
                ]
            }})
        };
        let plays = |third: Value| parse_list_item(&row(third)["musicResponsiveListItemRenderer"]);
        assert_eq!(plays(json!("53M plays")).unwrap().play_count.as_deref(), Some("53M"));
        assert_eq!(plays(json!("1,234 plays")).unwrap().play_count.as_deref(), Some("1,234"));
        assert_eq!(plays(json!("The Album")).unwrap().play_count, None);
        assert_eq!(plays(json!("")).unwrap().play_count, None);
    }

    #[test]
    fn parses_next_panel_video() {
        let root = json!({
            "contents": { "playlistPanelRenderer": { "contents": [
                { "playlistPanelVideoRenderer": {
                    "videoId": "vid9",
                    "title": { "runs": [{ "text": "Next Song" }] },
                    "longBylineText": { "runs": [{ "text": "Artist A" }, { "text": " & " }, { "text": "Artist B" }] },
                    "lengthText": { "runs": [{ "text": "4:05" }] },
                    "thumbnail": { "thumbnails": [{ "url": "t.jpg" }] }
                }}
            ], "continuations": [{ "nextContinuationData": { "continuation": "CONT_TOKEN" } }] } },
            "tabs": [{ "tabRenderer": { "title": "Lyrics", "endpoint": { "browseEndpoint": {
                "browseId": "MPLYt_abc123",
                "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig": {
                    "pageType": "MUSIC_PAGE_TYPE_TRACK_LYRICS" } }
            } } } }]
        });
        let r = parse_next(&root);
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].video_id, "vid9");
        assert_eq!(r.items[0].title, "Next Song");
        assert_eq!(r.items[0].artists, "Artist A & Artist B");
        assert_eq!(r.items[0].duration.as_deref(), Some("4:05"));
        assert_eq!(r.continuation.as_deref(), Some("CONT_TOKEN"));
        assert_eq!(r.lyrics_browse_id.as_deref(), Some("MPLYt_abc123"));
    }

    /// An unfiltered search row leads with the result type ("Song • Delara • 3:02"). It must not
    /// end up in `artists` — that string is what gets scrobbled.
    #[test]
    fn drops_the_result_type_from_a_search_row() {
        let root = json!({
            "a": { "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abc123" },
                "flexColumns": [
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Hele uka" }] } } },
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                        { "text": "Song" }, { "text": " • " },
                        { "text": "Delara", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCdelara" } } },
                        { "text": " • " }, { "text": "3:02" }
                    ] } } }
                ]
            }}
        });
        let s = &parse_search(&root).items[0];
        assert_eq!(s.artists, "Delara");
        assert_eq!(s.album, None);
        assert_eq!(s.duration.as_deref(), Some("3:02"));
        // The links describe the same field as `artists` — never the "Song" label in front of it.
        assert_eq!(
            s.artist_runs.iter().map(|r| (r.text.as_str(), r.id.as_deref())).collect::<Vec<_>>(),
            [("Delara", Some("UCdelara"))]
        );
    }

    /// A queue row's byline is a whole descriptor; only its artist field is the artist.
    #[test]
    fn panel_byline_keeps_only_the_artist() {
        let root = json!({
            "contents": { "playlistPanelRenderer": { "contents": [
                { "playlistPanelVideoRenderer": {
                    "videoId": "vid9",
                    "title": { "runs": [{ "text": "Hele uka" }] },
                    "longBylineText": { "runs": [
                        { "text": "Delara", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCdelara" } } },
                        { "text": " • " }, { "text": "Sjelen" }, { "text": " • " }, { "text": "2026" }
                    ] }
                }}
            ] } }
        });
        assert_eq!(parse_next(&root).items[0].artists, "Delara");
    }

    #[test]
    fn splits_datasync_id() {
        assert_eq!(split_datasync_id("realid||other"), "realid");
        assert_eq!(split_datasync_id("||fallback"), "fallback");
        assert_eq!(split_datasync_id("plain"), "plain");
    }

    #[test]
    fn parses_account_menu() {
        let root = json!({
            "responseContext": {
                "visitorData": "CgtNEWVISITOR",
                "mainAppWebResponseContext": { "datasyncId": "1234||5678" }
            },
            "actions": [{ "openPopupAction": { "popup": { "multiPageMenuRenderer": { "sections": [{
                "activeAccountHeaderRenderer": {
                    "accountName": { "runs": [{ "text": "Jane Doe" }] },
                    "channelHandle": { "runs": [{ "text": "@janedoe" }] },
                    "accountPhoto": { "thumbnails": [{ "url": "small.jpg" }, { "url": "big.jpg" }] }
                }
            }] } } } }]
        });
        let a = parse_account_menu(&root);
        assert_eq!(a.name.as_deref(), Some("Jane Doe"));
        assert_eq!(a.handle.as_deref(), Some("@janedoe"));
        assert_eq!(a.thumbnail.as_deref(), Some("big.jpg"));
        assert_eq!(a.data_sync_id.as_deref(), Some("1234"));
        assert_eq!(a.visitor_data.as_deref(), Some("CgtNEWVISITOR"));
    }
}
