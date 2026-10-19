//! Rebuilds the crate when the embedded frontend changes. `rust-embed` only
//! tracks the files it saw last time, so new or removed Vite output (hashed
//! file names) would otherwise leave a stale binary.

use std::path::Path;

const DIST: &str = "../../frontend/dist";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Cargo reruns a script on every build while a watched path is missing,
    // which would recompile this crate each time the frontend is unbuilt. An
    // empty (git-ignored) `frontend/dist` serves the "not built" page just
    // the same, and Vite builds into it, which Cargo then sees.
    if !Path::new(DIST).exists() {
        let _ = std::fs::create_dir_all(DIST);
    }
    println!("cargo:rerun-if-changed={DIST}");
}
