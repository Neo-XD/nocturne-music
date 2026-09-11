# Nocturne Desktop — v0.8.1 Changelog

> Release: Desktop v0.8.1  
> Date: September 11, 2026

---

## 🚀 Key Highlights & Features

### 🎚️ 10-Band Parametric Equalizer & Audio Filtering
- **Studio-Grade Parametric Peaking Filters**: Native FFmpeg `lavfi` biquad peaking equalizer (`equalizer=f=FREQ:t=q:w=Q:g=GAIN`) implemented directly in the libmpv audio processing backend.
- **Preamp Control**: Independent preamp slider (−12 dB to +6 dB) with soft clipping headroom protection.
- **10 Fine-Tuned Frequency Bands**: Standard ISO bands (31 Hz, 62 Hz, 125 Hz, 250 Hz, 500 Hz, 1 kHz, 2 kHz, 4 kHz, 8 kHz, 16 kHz) with ±12 dB gain adjustment and 0.1 dB step precision.
- **Interactive SVG Frequency Response Curve**: Real-time response graph featuring smooth cubic bezier curve interpolation, dynamic color gradient fill matching active theme accent, and zero dB reference line.
- **12 Curated Presets**: Quick-select presets for any genre or listening style:
  - *Flat*, *Bass Boost*, *Bass Reducer*, *Treble Boost*, *Treble Reducer*, *Vocal Boost*, *Electronic*, *Rock*, *Classical*, *Pop*, *Acoustic*, and *Hip Hop*.
- **Quick Reset & Custom Curves**: Double-click any slider thumb to instantly reset that band to 0 dB, or select custom curves seamlessly with auto-saving to SQLite state database.

### 🌊 Glassy Theme & Liquid Warp Polish
- **Enhanced WebGL Domain Warping**: Re-tuned organic simplex noise flow vectors with increased displacement (`0.34 * max(u_intensity, 0.40)`) and liquid chromatic dispersion for deeply visible fluid wave motion.
- **Specular Liquid Caustics**: Added subtle organic caustic highlights that shimmer across fluid contours as the music plays.
- **CORS-Safe Texture Pipeline**: Fixed browser cache collisions by implementing cache-busting and blob/`createImageBitmap` off-thread decoding for WebGL textures, eliminating silent fallback to static images.
- **Graceful Procedural Fallback**: If an image is unavailable or loading, the shader continues running full procedural fluid domain warping in accent palette colors rather than disabling WebGL.
- **Clamped Blur Dynamics**: Background blur radius capped to 24px in the Glassy theme so fluid motion remains crisp, clear, and visible behind translucent UI elements.

### 🪟 Backdrop Blur & Acrylic Translucency Polish
- **MiniPlayer Background Blur**: Replaced opaque solid card background with `bg-card/75 backdrop-blur-2xl` frosted glass and an ambient blurred album art underlay across the miniplayer window.
- **Search Results Popup Blur**: Enhanced both the top search bar typeahead (`TopSearchBar`) and the `/search` preview popup (`SearchSuggest`) with deep `backdrop-blur-2xl` and translucent popover styling (`color-mix(in oklab, var(--popover) 80%, transparent)`).

### 🚫 Artist Blocking
- **Direct Track Menu Blocking**: Block unwanted artists directly from any song context menu (`TrackMenu.svelte`).
- **Mix & Radio Exclusion**: Automatically skips blocked artist tracks and purges them from auto-generated radio queues and dynamic mix sessions.

### 👥 Multi-Account Switcher
- **Multi-Profile Management**: Seamlessly switch between multiple YouTube Music accounts.
- **Per-Account State & Cache**: Dedicated per-account token storage, library caching, and profile avatar badging in the sidebar and user account menu.

### 📱 Remote Sync Device Favorites
- **Pin Preferred Devices**: Star and favorite frequently used playback target devices (desktop / mobile).
- **Persistent Reconnect Prioritization**: Automatically prioritizes favorited synchronization targets upon connection or network restore.

---

## 📋 Version Comparison

| Feature / Area | v0.8.0 | v0.8.1 |
| :--- | :--- | :--- |
| **Equalizer** | N/A | 10-band parametric EQ, 12 presets, SVG curve, preamp control |
| **Glassy Theme Warp** | Standard blur | Enhanced dual-octave warping, caustics, CORS-safe streaming |
| **MiniPlayer Styling** | Solid opaque card | Frosted glass acrylic (`backdrop-blur-2xl`) + ambient art wash |
| **Search Popups** | Opaque popover | Translucent `backdrop-blur-2xl` frosted popup styling |
| **Artist Blocking** | N/A | 1-click artist blocking from track context menu |
| **Account Management**| Single account | Multi-account switcher with profile badging |
| **Remote Sync** | Basic list | Device favorites with priority auto-reconnect |
