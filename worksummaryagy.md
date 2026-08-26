# Nocturne Music — Work Summary & Roadmap

**Repository:** `Neo-XD/nocturne-music` (Fork of `SimoHypers/limusic`)  
**Current Version:** `v0.6.1`  
**Tech Stack:** Rust (Tauri 2, libmpv, InnerTube API) + SvelteKit / TypeScript / Tailwind CSS

---

## 1. Executive Summary

This document summarizes all engineering accomplishments, bug fixes, architecture improvements, internal rebranding, and future plans implemented in **Nocturne Music**. 

All work adheres to Conventional Commits standards, maintains zero data loss during user migration, and has been fully verified with 100% test pass rates across all crates and frontend checks.

---

## 2. Work Completed

### 🎵 2.1 Immersive Dedicated Fullscreen Player
- **Component Implementation:** Built a dedicated fullscreen overlay component ([`FullscreenPlayer.svelte`](file:///C:/Users/Amritanshu%20Praveen/nocturne-music/ui/src/lib/components/FullscreenPlayer.svelte)) mounted at the application root (`+layout.svelte`).
- **Visual Design & Layout:**
  - Dynamic ambient artwork backdrop (`.art-wash`) using real-time canvas color extraction.
  - Left panel showcasing high-resolution album art, interactive transport controls, seek scrubber, like/favorite button, and playlist context menu.
  - Generous top distance (`pt-20 sm:pt-24 lg:pt-28` header, `pt-48 sm:pt-52 lg:pt-56` content) eliminating clipping with top buttons.
- **Synchronized Lyrics Stream:**
  - Word-by-word karaoke highlighting with 60fps interpolation.
  - Smooth vertical gradient mask with auto-centering on active singing lines.
  - Click-to-seek directly on any lyric line to jump playback.
- **Keyboard Navigation & Shortcuts:** Full keyboard support:
  - `Esc` / `Ctrl+F` / `Cmd+F`: Toggle/exit fullscreen mode.
  - `Space`: Play / Pause toggle.
  - `ArrowLeft` / `ArrowRight`: Seek ±5 seconds.
  - `ArrowUp` / `ArrowDown`: Adjust volume ±5%.

---

### 📜 2.2 Now Playing Sidebar & Lyrics Auto-Scroll Fixes
- **Scroll Math Isolation:** Rewrote lyrics auto-scrolling math in [`NowPlayingSidebar.svelte`](file:///C:/Users/Amritanshu%20Praveen/nocturne-music/ui/src/lib/components/NowPlayingSidebar.svelte) using container-relative bounding client rectangles, preventing unwanted scroll jumping on the parent sidebar.
- **Start-of-Song Reset:** Added explicit scroll reset to the top lyric (`top: 0`) whenever a new track begins, when lyrics load, or when the sidebar is toggled open.
- **Active Lyric Centering:** Restored smooth auto-scrolling centered around the active sung lyric during playback.

---

### 📻 2.3 Last.fm Scrobbling & Authentication Overhaul
- **Backend Allowlist Fix:** Fixed a critical bug in [`src-tauri/src/commands.rs`](file:///C:/Users/Amritanshu%20Praveen/nocturne-music/src-tauri/src/commands.rs#L180-L200) where `UI_SETTINGS` was missing `lastfm_api_key` and `lastfm_api_secret`, causing `set_setting` to reject saved credentials.
- **Direct In-App Credential Inputs:** Added dedicated input fields in [`SettingsDialog.svelte`](file:///C:/Users/Amritanshu%20Praveen/nocturne-music/ui/src/lib/components/SettingsDialog.svelte) under **Settings > General > Last.fm scrobbling**:
  - API Key text field with instant database persistence.
  - Shared Secret password field with interactive **Show / Hide** eye toggle.
  - Direct 1-click link to the Last.fm API key creation page.
- **Seamless Desktop Authentication:** Implemented single-click authentication (`auth.getToken` → browser authorization → 60-try background session polling `auth.getSession`).
- **Titlebar Smart Fallback:** Added automatic Settings dialog opening with toast guidance if a user clicks the titlebar scrobbler button before configuring keys.

---

### 🔄 2.4 Auto-Updater & GitHub Release Routing
- **Repository Redirection:**
  - Pointed `tauri.conf.json` updater endpoint to `https://github.com/Neo-XD/nocturne-music/releases/latest/download/latest.json`.
  - Updated `get_changelog` in [`commands.rs`](file:///C:/Users/Amritanshu%20Praveen/nocturne-music/src-tauri/src/commands.rs#L1250) to query `Neo-XD/nocturne-music/releases`.
- **Fault-Tolerant Check Flow:** Updated [`updater.svelte.ts`](file:///C:/Users/Amritanshu%20Praveen/nocturne-music/ui/src/lib/updater.svelte.ts) to handle 404s/empty initial releases gracefully and report `"You are running the latest version"` instead of throwing unhandled errors.

---

### 🏷️ 2.5 Comprehensive Code Audit & Full Rebranding
Completed an exhaustive 28-point codebase audit across all 5 workspace crates, UI, website, CI/CD, and scripts:
1. **Core Package & Binary Names:** Renamed Cargo package to `nocturne-app`, tray struct to `NocturneTray`, Discord RPC application name, and webview windows to `nocturne-cipher` / `nocturne-potoken`.
2. **Database Migration:** Switched database to `nocturne.sqlite` with automatic zero-loss file copy from legacy `limusic.sqlite`.
3. **MIME & Storage Types:** Updated drag-and-drop MIME types (`application/x-nocturne-*`) and localStorage key (`nocturne:personal`).
4. **Environment Overrides:** Supported `NOCTURNE_*` (`NOCTURNE_LASTFM_API_KEY`, `NOCTURNE_FORCE_GPU`, `NOCTURNE_LYRICS_ONLY`, `NOCTURNE_DISABLED_CLIENTS`) with backward-compatible fallbacks.
5. **Changelog Deduplication:** Refactored release notes deduplication in `commands.rs` to use `HashSet<String>`.
6. **Website & CI Fixes:**
   - Updated `website/src/lib/github.ts` to `REPO = 'Neo-XD/nocturne-music'` for downloads.
   - Updated `scripts/release.sh` and `scripts/fix-appdir-tls.sh`.
   - Updated GitHub Actions workflows (`linux-release.yml`, `macos-release.yml`, `windows-release.yml`) to inject `NOCTURNE_LASTFM_*` keys.
   - Rebranded GitHub issue templates (`bug_report.yml`, `feature_request.yml`, `config.yml`) and `README.md`.
7. **Backwards-Compatibility Anchors:** Safely preserved and documented essential constants (`ON_REPEAT_ID = "LIMUSIC_ON_REPEAT"`, identity hash salt, legacy MIME fallbacks, legacy localStorage fallbacks).

---

### 📦 2.6 Versioning & Git Commits
- Bumped workspace and app version to **`v0.6.1`**.
- Added `v0.6.1` changelog release note entry.
- Pushed commits to `origin/master`:
  - `7617992`: `feat(player): add dedicated fullscreen player, runtime last.fm keys, and internal rebranding`
  - `ee89443`: `fix(lastfm): fix last.fm authentication, in-app credentials input, and bump v0.6.1`
  - `2a8647c`: `refactor(audit): clean up stale upstream references, update workflows, and refactor release notes dedup`

---

## 3. Verification & Quality Metrics

| Suite | Status | Details |
|---|---|---|
| **Svelte Diagnostics (`pnpm check`)** | ✅ Passed | 0 errors, 0 warnings |
| **Frontend Production Build (`pnpm build`)** | ✅ Passed | Built in 7.20s |
| **Website Production Build (`pnpm build`)** | ✅ Passed | Built in 702ms |
| **Cargo Workspace Compilation (`cargo check`)** | ✅ Passed | 0 compiler errors / warnings |
| **Unit Test Suite (`cargo test --all`)** | ✅ Passed | **118 passed**, 0 failed |

---

## 4. Future Roadmap & Planned Work

### 🚀 Phase 1: Performance & Audio Engine Polish
- [ ] **Audio Equalizer (DSP / Libmpv):** Add 10-band parametric equalizer controls in Settings > Playback with customizable presets (Bass Boost, Vocal, Acoustic, Electronic).
- [ ] **Expanded Audio Cache Management:** Provide manual audio cache size limits and 1-click cache purge button in Settings > Storage.
- [ ] **Smart Pre-Buffering:** Optimize next-track buffer priming to make gapless playback transitions even smoother on slower networks.

### 🎨 Phase 2: UI & User Experience Enhancements
- [ ] **Custom Themes & Accent Colors:** Expand theme engine with user-defined OKLCH accent palettes and custom background image uploads.
- [ ] **Fullscreen Player Layout Customizer:** Toggle between left/right lyrics layout, compact view, and album-art-only focus mode.
- [ ] **Enhanced Artist Pages:** Display top tracks by listener count, full discography categorization (Albums, Singles, EPs), and related artist recommendations.

### 🌐 Phase 3: Social & Sync Features
- [ ] **Listen Together UI Polish:** Improve chat/reaction interface during synchronized playback sessions and simplify host connection setup.
- [ ] **Last.fm Extended Stats:** Show current weekly/monthly scrobble count or top track badges directly in the now playing info view.
- [ ] **Local Music Tag Editor:** Enable in-app editing of ID3 tags (title, artist, album, track number, year) for imported local library files.

### 📦 Phase 4: Distribution & CI/CD Packaging
- [ ] **GitHub Releases Automation:** Trigger workflow build for `v0.6.1` release assets (`.AppImage`, `.deb`, `.rpm`, `.exe`, `.msi`, `.dmg`).
- [ ] **AUR Package Update:** Package Nocturne Music for Arch Linux (`nocturne-bin`).
- [ ] **Winget & Flatpak Exploration:** Explore official submissions to Windows Package Manager (`winget`) and Flathub.
