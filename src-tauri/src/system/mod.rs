pub mod accessibility;
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
