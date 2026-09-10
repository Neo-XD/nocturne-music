# Changelog

All notable changes to Nocturne Desktop are documented in this file.

## [v0.8.0] - 2026-09-10

### ✨ New Features
- **Liquid Glass & Translucent Floating Panels**: Translucent acrylic floating panels with rich backdrop blur (`backdrop-blur-xl`) across Top Titlebar, Bottom Player Bar, Left Navigation Sidebar, and Right Sidebars in the Glassy theme.
- **Floating Player Bar**: Dedicated floating bottom player bar option with rounded corners, subtle drop shadows, and uniform spacing matching modern desktop aesthetics.
- **Granular Customization Checkboxes**: Converted icon visibility settings and floating panel toggles to independent, granular checkboxes in Settings (Appearance & Customization).
- **Smooth Right Sidebar Docking & Animation**: Completely overhauled right sidebar switching (Now Playing, Queue, Lyrics, Devices). Panels glide in place with smooth fly-in animation and fade transitions without pushing content or duplicating flex width.
- **Dynamic Width Transitions & Zero-Lag Drag Resizing**: Outer docking slot smoothly animates width when switching between panels of different widths (Now Playing 384px, Queue 320px, Devices 352px), while maintaining instant 1:1 mouse tracking when dragging the Now Playing resize handle.
- **Auto-Resized Height & Consistent Spacing**: Fixed height calculations to ensure exact 8px uniform spacing between floating sidebars, the floating bottom player bar, and window borders.
- **Clean Sidebar Lifecycle**: Completely eliminated unwanted older sidebar popups when closing a panel — closing properly closes the active panel without resurrecting previous states.
- **Collapsed Sidebar Playlist Quick Navigation**: Display playlist buttons with artwork/icons directly on the collapsed left sidebar rail, with auto-collapse in the fullscreen now playing view for distraction-free navigation.
- **Confined Top Bar Scroll Bounds**: Confined home feed scrollbar track and thumb strictly below the floating top bar.

---

## [v0.7.3] - 2026-09-10

### ✨ New Features
- **Smooth Audio Crossfade**: Configurable crossfade (0–12s) in Playback Settings utilizing native mpv `lavfi=[afade=...]` audio filtering.
- **Discord RPC Customization**: Full Discord Rich Presence customization suite including custom application ID, title/artist/album format templates, toggleable buttons with custom labels, pause state display, and elapsed/remaining/hidden time options.
- **Floating-Style Sidebars**: Toggle between docked sidebars and modern floating sidebars with rounded corners, translucent acrylic glass, and subtle drop shadows.
- **Resizable Now Playing Sidebar**: Left-edge draggable handle to resize the Now Playing panel from 280px to 650px with local storage memory persistence.
- **Word-by-Word Lyric Sources**: Integration for YouLyPlus API and Paxsenix Apple Music TTML syllable parser for synchronized karaoke highlighting.
- **Automatic Lyrics Top Scroll**: Automatically resets lyrics scroll position to the top upon track transition.
- **Interface Bar Customization**: Granular toggles to customize visible buttons across the top titlebar and bottom playerbar.
- **Device Icon Harmony**: Updated output device selector icon to match Nocturne Mobile's speaker graphic (`SpeakerIcon.svelte`).
- **Enhanced Mobile Remote Sync**: Bidirectional queue synchronization and sub-second millisecond timestamp tracking.

---

## [v0.7.2] - 2026-09-08

### ✨ New Features
- **Song, Album & Playlist Downloader**: Download individual songs, full albums, or entire playlists directly to local storage (`Music/Nocturne/`).
- **BetterLyrics-Style Lyric Matcher**: Interactive two-pane candidate search and live synchronized preview dialog.
- **Custom User-Defined Lyric Providers**: Configurable custom lyric sources with `{title}`, `{artist}`, `{duration}`, and `{videoId}` endpoint templating.
- **In-App Updater**: Check for and seamlessly install new Nocturne updates with automated background downloading.
- **Storage & Cache Management**: Detailed breakdown of audio buffer, cover cache, and database usage with customizable pruning limits.
- **Native Adaptive Theme & Performance Presets**: WinUI 3 / Mica, KDE Breeze, GNOME Libadwaita, and macOS Cupertino styling alongside Balanced and High Quality performance presets.

---

## [v0.7.1] - 2026-09-06

### ✨ New Features
- **Dynamic Glow Theme**: Real-time ambient backdrop glowing matching album art dominant palette.
- **Remote Sync Architecture**: Initial implementation of Nocturne Direct Connect pairing with mobile clients.
- **Full Library Search & Caching**: High-performance local library index with instant multi-criteria filtering.

---

## [v0.7d] - 2026-09-02

### ✨ Highlights & Improvements
- **Instant Cold Playback & Latency Optimization**: Stream resolver prioritizes high-speed Android VR audio clients (~130ms resolution) and bounds BotGuard V8 VM solver retries to eliminate cold playback freezes.
- **Hardware Acceleration Setting**: Dedicated toggle in Settings ▸ Performance for GPU compositing vs CPU software rendering with automatic NVIDIA Wayland explicit sync stability workarounds.
- **Streamlined Last.fm OAuth Web Login**: Removed manual API Key and Secret input forms in favor of 1-click web OAuth authorization using CI/CD environment secrets.
- **Listen Together Polish**: Typable `CODE@host` links and automatic cleanup of pending joiners on participant disconnect.

---

## [v0.6.7] - 2026-08-30

### ✨ New Features
- **Zero-PIN Remote Device Sync**: Automated UDP discovery and WebSocket pairing between mobile and desktop Nocturne clients.
- **Dedicated Output Devices Sidebar**: Spotify-Connect-style Output Devices sidebar opened from the speaker icon in the bottom player bar.
- **Bidirectional Playback Handoff**: Hand off songs, active queue, and seek position between PC and mobile on the fly.

---

## [v0.6.6] - 2026-08-29

### ✨ New Features
- **Embedded Remote Playback Server**: Built-in local WebSocket server with secure pairing for mobile control.
- **Save to Library from Context Menus**: "Save to library" and "Remove from library" on context menus across songs, albums, artists, and playlists.
- **Frosted Glass Context Menus**: Context menus and popovers render with real-time backdrop blur in the Glassy theme.

---

## [v0.6.5] - 2026-08-28

### ✨ New Features
- **Glassy Ambient Theme**: Fluid GPU domain warping with customizable warping intensity, brightness, blur radius, and saturation.
- **Persistent Floating Tab Bars**: Unified modern floating pill tab bars on Home and Library.
- **Performance Settings**: Dedicated Performance tab in Settings with transparency and motion toggles and optimization presets.

---

## [v0.6.4] - 2026-08-28

### ✨ New Features
- **Nested Playlist Folders**: Recursive subfolders, collapsible folder trees, and folder count badges.
- **Library Drag & Drop**: Drop playlists onto each other to make folders, nest playlists into folder cards, or drag out to unfile.
- **Customizable Shortcuts Editor**: Keybind manager in Settings (General) with Ctrl+Space search shortcut.
- **Explicit Content Filter & Badges**: Explicit content filter toggle in Playback Settings and explicit badges across all views.

---

## [v0.6.3] - 2026-08-27

### ✨ Improvements
- **Instant Window Startup**: Resolved launch permission delay for immediate window display on launch.
- **PoToken Background Minting**: Upstream BotGuard PoToken background minting worker and YouTube uploads tab in Library.

---

## [v0.6.2] - 2026-08-26

### ✨ Improvements
- **Fullscreen Player & Spacing**: Refined fullscreen music player spacing, compact controls, and synchronized lyrics.
- **Music Videos**: Resilient music video playback with stream caching and background handling.

---

## [v0.6.1] - 2026-08-25

### ✨ Improvements
- **Last.fm Authentication**: Restored direct in-app API Key and Shared Secret configuration.
- **Now Playing Auto-Scroll**: Isolated lyrics scrolling with start-of-track reset in the info sidebar.

---

## [v0.6.0] - 2026-08-25

### ✨ Launch
- **Rebrand to Nocturne Music**: Full brand identity overhaul with modern assets, updated tray, window controls, and Discord RPC.
- **Docked Now Playing Sidebar**: Spotify-style right sidebar displaying artwork, video playback, artist profiles, listener stats, and next track preview.
- **Live Synced Lyrics**: Real-time synchronized lyrics with word-by-word karaoke highlighting and interactive click-to-seek.
- **Clean Monochrome Theme**: Sleek, distraction-free neutral monochrome color scheme.

> *Note: Nocturne Music builds on Limusic. All official releases prior to v0.6.0 are available on the upstream repository.*
