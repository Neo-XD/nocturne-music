fn main() {
    #[cfg(target_os = "windows")]
    {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = std::path::Path::new(&manifest_dir);
            for ancestor in p.ancestors() {
                let libmpv = ancestor.join(".libmpv");
                if libmpv.exists() {
                    println!("cargo:rustc-link-search=native={}", libmpv.display());
                    break;
                }
            }
        }
    }
}
