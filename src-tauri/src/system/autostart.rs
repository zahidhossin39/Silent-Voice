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

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> Result<(), String> {
    Err("Launch at startup is only implemented on Windows".into())
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}
