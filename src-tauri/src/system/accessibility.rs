// macOS gates synthetic keystrokes (the paste) and the global hotkey behind the
// Accessibility permission. Nothing the app does works until it is granted, and
// the OS prompt is easy to miss, so we check and offer to open the pane.

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

#[cfg(target_os = "macos")]
pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

// Everywhere else there is no such gate, so nothing is ever blocked.
#[cfg(not(target_os = "macos"))]
pub fn is_trusted() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub fn open_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_settings() {}
