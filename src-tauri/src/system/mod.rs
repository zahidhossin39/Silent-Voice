pub mod paths;
pub mod accessibility;
// Inline proofreading off Windows: one watcher (inline_unix) over two
// accessibility backends, drawing into a transparent Tauri window. The Windows
// equivalents are inline_check + squiggle below.
#[cfg(target_os = "linux")]
pub mod atspi;
#[cfg(target_os = "macos")]
pub mod ax;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod inline_unix;
pub mod inline_types;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod squiggle_overlay;
pub mod autostart;
pub mod clipboard_file;
pub mod foreground;
pub mod hardware;
pub mod hotkey;
// Windows-only: UI Automation + Win32 layered windows have no
// cross-platform equivalent. See PORTING.md.
#[cfg(windows)]
pub mod inline_check;
pub mod job;
pub mod overlay;
pub mod paste;
pub mod secure_field;
pub mod sherpa;
pub mod sherpa_stt;
#[cfg(windows)]
pub mod squiggle;
pub mod textfmt;
pub mod tray;
pub mod tts;
