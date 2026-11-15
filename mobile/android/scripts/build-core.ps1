# Rebuilds the shared Rust crate (crates/sv-core) for Android and regenerates
# the Kotlin bindings. Run after changing anything in crates/sv-core.
$ErrorActionPreference = 'Stop'
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
$env:ANDROID_NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.2.12479018"
$android = Split-Path -Parent $PSScriptRoot
$core = Join-Path (Split-Path -Parent (Split-Path -Parent $android)) 'crates\sv-core'
Push-Location $core
try {
    cargo test
    cargo ndk -t arm64-v8a -t x86_64 -o "$android\app\src\main\jniLibs" build --release
    # Bindings come from an unstripped host build: the release .so has its
    # UniFFI metadata stripped, so the generator finds nothing in it.
    cargo build
    cargo run --bin uniffi-bindgen -- generate --library target\debug\sv_core.dll --language kotlin --no-format --out-dir "$android\app\src\main\java"
} finally { Pop-Location }
