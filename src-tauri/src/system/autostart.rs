// "Launch at startup" — writes/removes a per-user Windows Run-key entry.
// HKCU (not HKLM) so it needs no admin rights and only affects the current
// user, matching how the Settings toggle is scoped.

#[cfg(windows)]
const RUN_KEY_NAME: &str = "SilentVoice";

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| e.to_string())?;

    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_str = exe.to_string_lossy();
        // Quote the path so spaces (e.g. "Program Files") don't break the command line.
        key.set_value(RUN_KEY_NAME, &format!("\"{exe_str}\""))
            .map_err(|e| e.to_string())?;
    } else {
        // Not being present is success, not an error.
        let _ = key.delete_value(RUN_KEY_NAME);
    }
    Ok(())
}

/// Whether the Run-key entry currently exists — the source of truth the
/// Settings toggle hydrates from (localStorage can drift from the registry).
#[cfg(windows)]
pub fn is_enabled() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .and_then(|k| k.get_value::<String, _>(RUN_KEY_NAME))
        .is_ok()
}

#[cfg(target_os = "macos")]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    // Per-user LaunchAgent for macOS autostart.
    let path = dirs::home_dir()
        .ok_or("No home dir")?
        .join("Library/LaunchAgents/app.silentvoice.desktop.plist");

    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
            <plist version=\"1.0\">\n\
            <dict>\n\
                <key>Label</key>\n\
                <string>app.silentvoice.desktop</string>\n\
                <key>ProgramArguments</key>\n\
                <array>\n\
                    <string>{}</string>\n\
                </array>\n\
                <key>RunAtLoad</key>\n\
                <true/>\n\
            </dict>\n\
            </plist>",
            exe.display()
        );
        std::fs::write(&path, plist).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn is_enabled() -> bool {
    dirs::home_dir()
        .map(|d| d.join("Library/LaunchAgents/app.silentvoice.desktop.plist").exists())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    // XDG autostart path for Linux desktops.
    let path = dirs::config_dir()
        .ok_or("No config dir")?
        .join("autostart/silent-voice.desktop");

    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let desktop = format!(
            "[Desktop Entry]\n\
            Type=Application\n\
            Name=Silent Voice\n\
            Exec={}\n\
            X-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(&path, desktop).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn is_enabled() -> bool {
    dirs::config_dir()
        .map(|d| d.join("autostart/silent-voice.desktop").exists())
        .unwrap_or(false)
}
