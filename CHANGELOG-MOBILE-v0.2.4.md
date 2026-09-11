# Nocturne Mobile — v0.2.4-m Changelog

> Release: Mobile v0.2.4-m  
> Date: September 11, 2026  
> Version Code: 6

---

## 🚀 Key Highlights & Features

### 🚫 Artist Blocking
- **Direct Blocking from Track Overflow Menus**: Added "Block Artist" option in song and track action menus.
- **Dynamic Mix Exclusion**: Automatically filters out and skips songs by blocked artists during dynamic mix, radio, and autoplay queues.
- **Settings Management**: View and unblock blocked artists directly within the Settings menu.

### 👥 Multi-Account Switcher
- **Multi-Profile Support**: Connect and switch between multiple YouTube Music accounts on a single mobile device.
- **Isolated Account Storage**: Separate session credentials, history, and cached playlists per account.
- **Account Badging**: Displays active profile information and avatar badging in the navigation drawer and user profile sheet.

### 📱 Remote Sync Auto-Reconnect & Device Favorites
- **Priority Reconnection**: Improved direct connection reliability with Nocturne Desktop via prioritized device reconnects.
- **Device Favoriting**: Star favorite desktop devices for instant 1-tap connection.
- **Sub-Second Latency**: Optimized websocket heartbeat pinging and millisecond playback position reconciliation.

### 🛠️ Android CI & Build Stability Fixes
- **ExoPlayer & Media3 State Flow**: Fixed `StateFlowValueCalledInComposition` lint error by switching `playerConnection.mediaMetadata.value` to reactive `collectAsState()`.
- **Vector Drawable & Lint Quality Gate**: Cleaned up vector resource definitions (`block.xml`) and updated `app/lint.xml` to pass `lintUniversalFossDebug` in GitHub Actions CI without false positives.
