// A build script that cannot do its work should stop the build.
#![allow(clippy::unwrap_used)]

fn main() {
    tauri_build::build()
}
