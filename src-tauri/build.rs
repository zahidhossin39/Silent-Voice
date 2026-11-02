fn main() {
    // screencapturekit's Swift bridge needs the OS Swift runtime
    // (libswift_Concurrency), but a dependency's link args don't reach our
    // binaries, so the app and its tests must carry the rpath themselves.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
