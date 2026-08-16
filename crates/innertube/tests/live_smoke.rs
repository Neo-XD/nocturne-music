//! Live-YouTube extraction smoke test (context/17). NOT in the default run:
//!   cargo test -p innertube --features integration-tests -- --nocapture
#![cfg(feature = "integration-tests")]

use innertube::{
    find_format, AudioQuality, Clients, InnerTube, PlaylistSort, Session, STREAM_FALLBACK_ORDER,
};

const VIDEO_ID: &str = "xl9cFAOKg_Y"; // the id from the user's failing run

/// GET the first KB with the given UA — what mpv effectively does on load.
async fn probe(url: &str, ua: Option<&str>) -> reqwest::StatusCode {
    let client = reqwest::Client::new();
    let mut req = client.get(url).header("Range", "bytes=0-1023");
    if let Some(ua) = ua {
        req = req.header("User-Agent", ua);
    }
    req.send().await.expect("probe request").status()
}

#[tokio::test]
async fn direct_clients_resolve_and_stream() {
    let it = InnerTube::new(Session::default(), None).unwrap();
    let vd = it.fetch_visitor_data().await.ok();
    let it = InnerTube::new(Session { visitor_data: vd, ..Session::default() }, None).unwrap();
    let clients = Clients::bundled();

    let mut any_ok = false;
    for key in STREAM_FALLBACK_ORDER {
        let client = clients.get(key).unwrap();
        let resp = match it.player(client, VIDEO_ID, None, None, None).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{key}: /player failed: {e}");
                continue;
            }
        };
        if !resp.playability_status.is_ok() {
            eprintln!("{key}: status {}", resp.playability_status.status);
            continue;
        }
        let sd = resp.streaming_data.as_ref().expect("streamingData");
        assert!(sd.expires_in_seconds.is_some(), "{key}: expiry must parse");
        let Some(format) = find_format(sd, AudioQuality::High) else {
            eprintln!("{key}: no audio format");
            continue;
        };
        let Some(url) = format.direct_url() else {
            eprintln!("{key}: itag {} cipher-only", format.itag);
            continue;
        };
        let status = probe(url, Some(&client.user_agent)).await;
        eprintln!("{key}: itag {} -> HTTP {status}", format.itag);
        if status.is_success() {
            any_ok = true;
        }
    }
    assert!(any_ok, "no direct client produced a playable (HTTP 2xx) stream URL");
}

/// Live regression for the "load more duplicates tracks" bug: an owned playlist's continuation
/// embeds a nested duplicate renderer per row. Self-skips unless a real session is supplied:
///   LIMUSIC_COOKIE=… LIMUSIC_VISITOR=… cargo test -p innertube --features integration-tests owned_continuation_not_doubled -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn owned_continuation_not_doubled() {
    let Some(cookie) = std::env::var("LIMUSIC_COOKIE").ok().filter(|s| !s.is_empty()) else {
        eprintln!("skipped: set LIMUSIC_COOKIE (+LIMUSIC_VISITOR) to run");
        return;
    };
    let visitor = std::env::var("LIMUSIC_VISITOR").ok().filter(|s| !s.is_empty());
    let it = InnerTube::new(
        Session { cookie: Some(cookie), visitor_data: visitor, ..Session::default() },
        None,
    )
    .unwrap();
    let clients = Clients::bundled();
    let client = clients.get("WEB_REMIX").expect("WEB_REMIX client");

    let libs = it.library_playlists(client).await.expect("library playlists");
    let mut checked = 0;
    for c in &libs {
        let Ok(page) = it.playlist(client, &c.id, None).await else { continue };
        let Some(tok) = page.continuation.clone() else { continue };
        let cont = it.playlist_continuation(client, &tok).await.expect("continuation");
        if cont.items.is_empty() {
            continue; // a suggestions carousel, not more tracks
        }
        checked += 1;
        let mut seen = std::collections::HashSet::new();
        for i in &cont.items {
            assert!(
                seen.insert(i.video_id.clone()),
                "playlist '{}' owned={} continuation doubled video {}",
                page.title.clone().unwrap_or_default(),
                page.owned,
                i.video_id
            );
        }
    }
    assert!(checked > 0, "no playlist with a track continuation found to verify");
    eprintln!("verified {checked} track continuations, no doubling");
}

/// The playlist sort is YouTube's, not ours: the app asks for an order and renders what comes back
/// (`PlaylistSort::params`). Those params are a protobuf literal copied out of YouTube's own menu,
/// so this pins the half that moves under us — that they still sort, and that the menu still says
/// which order the list is in and whether the choice can be written back.
///
/// Read-only on purpose. The write side (`playlist_set_sort`) changes a real playlist for every
/// client on the account, so it is not something a test suite should do behind your back.
///   LIMUSIC_COOKIE=… LIMUSIC_VISITOR=… cargo test -p innertube --features integration-tests playlist_sort -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn playlist_sort_params_still_order_the_server_side_list() {
    let Some(cookie) = std::env::var("LIMUSIC_COOKIE").ok().filter(|s| !s.is_empty()) else {
        eprintln!("skipped: set LIMUSIC_COOKIE (+LIMUSIC_VISITOR) to run");
        return;
    };
    let visitor = std::env::var("LIMUSIC_VISITOR").ok().filter(|s| !s.is_empty());
    let it = InnerTube::new(
        Session { cookie: Some(cookie), visitor_data: visitor, ..Session::default() },
        None,
    )
    .unwrap();
    let clients = Clients::bundled();
    let client = clients.get("WEB_REMIX").expect("WEB_REMIX client");

    // Liked Music is the one list every account has, and YouTube sorts it without being asked to
    // store anything on a playlist.
    let plain = it.playlist(client, "VLLM", None).await.expect("liked music");
    let menu = plain.sort_menu.clone().expect("Liked Music still offers a sort menu");
    assert!(!menu.editable, "Liked Music sorts through browse params, not a playlist edit");
    assert!(plain.items.len() > 1, "need more than one track to tell an order from another");

    let titles = |p: &innertube::PlaylistPage| -> Vec<String> {
        p.items.iter().map(|i| i.title.clone()).collect()
    };
    let asc = it.playlist(client, "VLLM", Some((PlaylistSort::Title, false))).await.expect("A-Z");
    let desc = it.playlist(client, "VLLM", Some((PlaylistSort::Title, true))).await.expect("Z-A");
    assert_eq!(
        asc.sort_menu.as_ref().and_then(|m| m.selected),
        Some(PlaylistSort::Title),
        "the menu has to report back the order we asked for"
    );
    assert_ne!(titles(&asc), titles(&desc), "descending params must not return the same page");
    assert_ne!(titles(&asc), titles(&plain), "a title sort must not return the stored order");

    // Put Liked Music back: asking for an order is what persists it on this one list.
    let restored = it.playlist(client, "VLLM", Some((PlaylistSort::Default, false))).await;
    assert!(restored.is_ok(), "failed to restore Liked Music to its stored order");
    eprintln!("title sort verified against {} tracks, Liked Music restored", plain.items.len());
}

/// Every surface has to hand the queue an artist that is a *name*. That string is the player bar,
/// the OS media widget, Discord, and the Last.fm scrobble, so the two bad shapes are: nothing at all
/// (YouTube ships the per-track artist column empty on single-artist albums, `"text": {}`, because
/// the header names the artist) and a whole display subtitle ("Aqua • 1.7B views" off a song card).
/// The unit tests pin how we parse a fixture; this pins the shape YouTube actually sends, which is
/// the half that moves under us.
///   cargo test -p innertube --features integration-tests every_surface -- --nocapture
#[tokio::test]
async fn every_surface_yields_a_scrobbleable_artist() {
    /// Mirrors `lastfm::scrobbleable` plus the "•" tell: a real artist line separates collabs with
    /// "&" or ",", never a bullet, so a bullet means a display string leaked into the field.
    fn bad(artist: &str) -> Option<&'static str> {
        match artist {
            a if a.trim().is_empty() => Some("no artist (would never scrobble)"),
            a if a.contains('•') => {
                Some("display subtitle, not an artist (would scrobble wrong)")
            }
            _ => None,
        }
    }

    let it = InnerTube::new(Session::default(), None).unwrap();
    let vd = it.fetch_visitor_data().await.ok();
    let it = InnerTube::new(Session { visitor_data: vd, ..Session::default() }, None).unwrap();
    let clients = Clients::bundled();
    let c = clients.get(innertube::METADATA_CLIENT).expect("metadata client");

    let mut problems: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let note = |problems: &mut Vec<String>, surface: &str, title: &str, artist: &str| {
        if let Some(why) = bad(artist) {
            problems.push(format!("{surface}: {title:?} → {artist:?} ({why})"));
        }
    };

    // Single-artist albums: the case that shipped broken. Compilations keep a per-row artist, so
    // these are deliberately all one-artist records.
    for q in ["Rumours Fleetwood Mac", "Midnights Taylor Swift", "IGOR Tyler The Creator"] {
        let cards = it.search_cards(c, q, "albums").await.expect("album search");
        let Some(card) = cards.iter().find(|b| b.kind == "album") else {
            problems.push(format!("album search {q:?} returned no album card"));
            continue;
        };
        let album = it.album(c, &card.id).await.expect("album page");
        assert!(!album.items.is_empty(), "album {q:?} parsed with no tracks");
        for t in &album.items {
            checked += 1;
            note(&mut problems, &format!("album {:?}", card.title), &t.title, &t.artists);
        }
    }

    // Search rows, the up-next queue, and an artist page (top songs + every song card in its
    // carousels, which is where the "• 1.7B views" strings came from).
    let songs = it.search_songs(c, "Barbie Girl Aqua").await.expect("search");
    for t in songs.items.iter().take(5) {
        checked += 1;
        note(&mut problems, "search", &t.title, &t.artists);
    }
    if let Some(first) = songs.items.first() {
        for t in it.next(c, Some(&first.video_id), None).await.expect("next").items.iter().take(5) {
            checked += 1;
            note(&mut problems, "queue", &t.title, &t.artists);
        }
    }
    let found = it.search_all(c, "Aqua").await.expect("search_all");
    if let Some(artist) = found.artists.first() {
        let page = it.artist(c, &artist.id).await.expect("artist page");
        for t in &page.top_songs {
            checked += 1;
            note(&mut problems, "artist top songs", &t.title, &t.artists);
        }
        for carousel in &page.sections {
            for card in carousel.items.iter().filter(|i| i.kind == "song") {
                checked += 1;
                let sub = card.subtitle.clone().unwrap_or_default();
                note(&mut problems, &format!("card {:?}", carousel.title), &card.title, &sub);
            }
        }
    }

    assert!(
        checked > 40,
        "only {checked} tracks reached the check, so the surfaces came back empty"
    );
    assert!(
        problems.is_empty(),
        "{checked} tracks checked, {} unscrobbleable:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
    eprintln!("{checked} tracks across album / search / queue / artist surfaces, all with a usable artist");
}

#[tokio::test]
async fn rustypipe_url_is_fetchable() {
    let c =
        innertube::rustypipe_fallback::resolve(VIDEO_ID, true).await.expect("rustypipe resolve");
    let bare = probe(&c.url, None).await;
    eprintln!("rustypipe itag {}: no-UA -> HTTP {bare}", c.itag);
    // mpv sends its own libmpv UA by default; also probe with a browser-ish UA for comparison.
    let browser = probe(&c.url, Some("Mozilla/5.0 (X11; Linux x86_64)")).await;
    eprintln!("rustypipe itag {}: browser-UA -> HTTP {browser}", c.itag);
    assert!(
        bare.is_success() || browser.is_success(),
        "rustypipe URL not fetchable (Raw(-13) root cause)"
    );
}

/// Radio (context/08) is a prefix convention plus one `/next` call, and every part of that is an
/// assumption about YouTube's wire behaviour rather than something a fixture can prove. This
/// checks the three the feature stands on:
///   1. `RDAMVM<videoId>` returns a real queue, not just the seed song.
///   2. `/next` accepts a radio playlist with **no** videoId (how artist radio has to be asked for).
///   3. An artist page still carries a start-radio button to get that playlist id from.
#[tokio::test]
async fn radio_seeds_resolve() {
    let it = InnerTube::new(Session::default(), None).unwrap();
    let vd = it.fetch_visitor_data().await.ok();
    let it = InnerTube::new(Session { visitor_data: vd, ..Session::default() }, None).unwrap();
    let client = Clients::bundled().get(innertube::METADATA_CLIENT).unwrap().clone();

    // 1. Song radio.
    let song = it
        .next(&client, Some(VIDEO_ID), Some(&format!("RDAMVM{VIDEO_ID}")))
        .await
        .expect("song radio /next");
    eprintln!("RDAMVM{VIDEO_ID}: {} tracks", song.items.len());
    assert!(song.items.len() > 1, "song radio came back with only the seed");

    // 2. + 3. Artist radio: the id off the page, then a videoId-less /next with it.
    let artist_id = song.items[0].artist_id.clone().expect("radio rows link their artist");
    let page = it.artist(&client, &artist_id).await.expect("artist page");
    let radio = page.radio_playlist_id.expect("artist header has no start-radio button");
    eprintln!("artist {:?} radio: {radio}", page.name);
    let artist_radio = it.next(&client, None, Some(&radio)).await.expect("artist radio /next");
    eprintln!("{radio}: {} tracks", artist_radio.items.len());
    assert!(artist_radio.items.len() > 1, "playlist-only /next returned nothing");

    // 4. Album radio: `RDAMPL` over the album's *audio* playlist, again with no videoId.
    let album_id =
        song.items.iter().find_map(|i| i.album_id.clone()).expect("a radio row with an album");
    let album = it.album(&client, &album_id).await.expect("album page");
    let pl = album.playlist_id.expect("album has no audio playlist");
    let album_radio =
        it.next(&client, None, Some(&format!("RDAMPL{pl}"))).await.expect("album radio /next");
    eprintln!("RDAMPL{pl}: {} tracks", album_radio.items.len());
    assert!(album_radio.items.len() > 1, "album radio came back empty");
}
