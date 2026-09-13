# Nocturne Desktop — v0.8.2 Changelog

> Release: Desktop v0.8.2  
> Date: September 13, 2026

---

## 🚀 Key Highlights & Features

### 🎬 Motion Album Art for Playlists & Albums
- **Looping Video Art from Mobile**: Ported the dynamic motion album art feature from Nocturne Mobile (`nocturne-mobile`) directly to Nocturne Desktop.
- **Dual API Integration**:
  - **Nocturne Canvas Manifest**: Direct `.mp4` and `.m3u8` video artwork streams fetched from Nocturne Canvas API.
  - **Apple Music Motion Catalog**: High-definition square and raw motion video assets (`editorialVideo.motionDetailSquare` and `motionDetailRaw`).
- **Synchronized Dual-Tile & Ambient Header Wash**: Plays synchronized looping video (`<video autoplay loop muted playsinline>`) both within the primary square album/playlist cover tile and across the wide atmospheric background wash.
- **24-Hour Cache Pipeline**: Fast-path in-memory and persistent `localStorage` caching with 24-hour expiration avoids redundant network lookups and preserves instant page navigation.

### 🎤 Mobile Lyric Animation Engine & Syllable Wave Sweep
- **Ported from Nocturne Mobile**: Integrated the complete lyric physics engine from `nocturne-mobile` (`ViviMusicLyrics.kt`) into desktop (`LyricsView.svelte`).
- **180ms Sentence Linger**: On line transitions, previous lines linger in a smooth transition state for 180ms to provide soft reading handoffs without abrupt disappearance.
- **Distance-Based Progressive Blur & Curved Inactive Falloff**:
  - Distance 1: 75% opacity, crisp focus (0px blur).
  - Distance 2: 50% opacity, 0.6px subtle blur.
  - Distance 3: 30% opacity, 1.4px atmospheric blur.
  - Distance ≥ 4: 20% opacity, 2.4px deep blur.
- **Active Line Dynamics**: Scales up to `1.045`, lifts vertically (`-0.125rem`), renders in extra-bold weight, and projects a luminous drop-shadow aura glow.
- **Syllable Wave Sweep with Trailing Feather**: Fluid karaoke sweep across individual words and syllables using a multi-stop trailing feather gradient (`waveFront` to `waveTail`), accurately matching mobile vocal cadence.
- **Active Sentence Shimmer Fallback**: When syllable-level timestamps are not present, lines receive a continuous luminous shimmer sweep across the active sentence.

### 🪟 Fullscreen Now Playing Edge-to-Edge Acrylic Translucency
- **True Edge-to-Edge Backdrop Wash**: Expanded the fullscreen now playing background wash seamlessly under the top bar, under the bottom player bar, and behind the left sidebar.
- **Acrylic Translucent Controls**: Replaced queue and lyrics mode switch buttons with translucent acrylic pills backed by `backdrop-blur-xl`.
- **Unobstructed Lyric Backdrop**: Removed opaque background panels behind fullscreen lyrics, allowing the ambient cover glow and motion video to shine through with crystal clarity.

### 📐 Resizable Left Sidebar & Reorganized Navigation
- **Interactive Drag-to-Resize Handle**: Added an intuitive drag handle on the right edge of the left navigation rail, letting users resize the sidebar width anywhere between 200px and 420px.
- **Persistent Width Preference**: Sidebar width is automatically preserved across restarts in `localStorage`.
- **Responsive Dynamic Viewport**: The fullscreen now playing layout automatically responds to custom sidebar widths without overflow or clipped content.
- **Repositioned Library Action**: Relocated library navigation to the bottom of the sidebar with explicit "See full library" action.
- **Configurable Home Button Placement**: New setting in Appearance & Customization allows users to place the Home button either in the Top Bar or in the Sidebar rail.

### 🧭 Top Bar Redesign, Window Dragging & Quick Settings
- **Restored Window Dragging**: Full `data-tauri-drag-region` support restored across all empty title bar regions for effortless window repositioning.
- **Home Button in Top Bar**: Positioned Home button directly adjacent to the navigation history arrows (Back/Forward) for convenient one-stop navigation.
- **Quick Settings Relocation**: Placed Settings button directly next to the Account profile switcher in the top bar.
- **Top Bar Theme Swapper**: Preserved one-click theme switcher in the top title bar.
- **Docked Titlebar Polish**: Docked top bar matches the floating title bar's blur, acrylic translucency, and frosted glass aesthetic.

---

## 🔧 Improvements & Bug Fixes

### 🔍 Smooth Search Bar & Frosted Glass Popups
- **Full Width Expansion**: Fixed top search bar hover and focus animations to reliably expand to full width without stuttering or stopping halfway.
- **Translucent Frosted Popups**: Applied deep `backdrop-blur-2xl` and translucent popover colors to search suggestion dropdowns (`SearchSuggest` and `TopSearchBar`).

### 🖼️ Playlist & Album Header Gradient Translucency
- **Smooth Radial & Linear Blending**: Replaced heavy vignette styling with dual radial and linear gradient masks, seamlessly blending album and playlist header artwork into transparency and the underlying app background.

### ⌨️ Global Settings Shortcut (Ctrl+;)
- **Keyboard Shortcut**: Press `Ctrl+;` (Ctrl + Semicolon) from anywhere in the application to instantly open and toggle the Settings dialog.

---

## 📋 Version Comparison

| Feature / Area | v0.8.1 | v0.8.2 |
| :--- | :--- | :--- |
| **Motion Album Art** | N/A | Looping video artwork for playlists & albums (Canvas + Apple Music) |
| **Lyric Animations** | Standard line highlight | Mobile engine: 180ms linger, progressive blur, wave sweep |
| **Window Dragging** | Blocked by bar container | Full native window drag regions (`data-tauri-drag-region`) |
| **Left Sidebar** | Fixed width (224px / 64px) | Resizable rail (200px–420px) with persistent width |
| **Fullscreen Player** | Standard bounded wash | Edge-to-edge under top/bottom/sidebar, translucent controls |
| **Header Artwork** | Vignette border cutoff | Multi-directional smooth gradient blend to translucency |
| **Settings Shortcut** | N/A | `Ctrl+;` (Ctrl + Semicolon) global shortcut |
| **Home Button** | Sidebar rail only | Placed in top bar next to arrows (toggleable via settings) |
| **Search Popups** | Translucent popover | `backdrop-blur-2xl` frosted acrylic dropdowns |
| **Top Bar Docking** | Dim opaque bar | Uniform frosted glass blur matching floating titlebar |
