fn main() {
    // Last.fm API credentials live in a gitignored `lastfm.keys` or `.env` next to this file or in root.
    // Format: `NOCTURNE_LASTFM_API_KEY=…` or `LIMUSIC_LASTFM_API_KEY=…` or `LASTFM_API_KEY=…`
    println!("cargo:rerun-if-changed=lastfm.keys");
    println!("cargo:rerun-if-changed=../lastfm.keys");
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=../.env");

    for path in &["lastfm.keys", "../lastfm.keys", ".env", "../.env"] {
        if let Ok(keys) = std::fs::read_to_string(path) {
            for line in keys.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    let (k, v) = (k.trim(), v.trim());
                    if k == "NOCTURNE_LASTFM_API_KEY"
                        || k == "LIMUSIC_LASTFM_API_KEY"
                        || k == "LASTFM_API_KEY"
                    {
                        println!("cargo:rustc-env=LIMUSIC_LASTFM_API_KEY={v}");
                        println!("cargo:rustc-env=NOCTURNE_LASTFM_API_KEY={v}");
                        println!("cargo:rustc-env=LASTFM_API_KEY={v}");
                    }
                    if k == "NOCTURNE_LASTFM_API_SECRET"
                        || k == "LIMUSIC_LASTFM_API_SECRET"
                        || k == "LASTFM_API_SECRET"
                    {
                        println!("cargo:rustc-env=LIMUSIC_LASTFM_API_SECRET={v}");
                        println!("cargo:rustc-env=NOCTURNE_LASTFM_API_SECRET={v}");
                        println!("cargo:rustc-env=LASTFM_API_SECRET={v}");
                    }
                }
            }
        }
    }
    tauri_build::build()
}
