// Foreground-application detection for per-app profiles (Windows only).
// Returns the focused window's executable basename, lowercased — e.g.
// "code.exe", "chrome.exe". Captured when recording STARTS, since that's the
// window the user is dictating into.

#[cfg(windows)]
pub fn foreground_app() -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        ok.ok()?;

        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let base = full.rsplit(['\\', '/']).next().unwrap_or(&full);
        Some(base.to_lowercase())
    }
}

#[cfg(target_os = "macos")]
pub fn foreground_app() -> Option<String> {
    run_cmd(
        "osascript",
        &[
            "-e",
            "tell application \"System Events\" to get name of first process whose frontmost is true",
        ],
    )
    .filter(|s| !s.is_empty())
    .map(|s| s.to_lowercase())
}

#[cfg(target_os = "linux")]
pub fn foreground_app() -> Option<String> {
    let active_win = run_cmd("xprop", &["-root", "_NET_ACTIVE_WINDOW"])?;
    let win_id = active_win.split_whitespace().last()?;
    let wm_class = run_cmd("xprop", &["-id", win_id, "WM_CLASS"])?;
    let trimmed = wm_class.trim_end().trim_end_matches('"');
    let start = trimmed.rfind('"')?;
    Some(trimmed[start + 1..].to_lowercase())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Raw handle (as isize) of the current foreground window, 0 if none. Captured
/// when recording starts and compared at paste time so text is never typed into
/// a window that stole focus mid-processing.
#[cfg(windows)]
pub fn foreground_hwnd() -> isize {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow().0 as isize }
}

#[cfg(target_os = "macos")]
pub fn foreground_hwnd() -> isize {
    // Provides app-level, not window-level, granularity on macOS.
    use std::hash::{Hash, Hasher};
    foreground_app()
        .map(|app| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            app.hash(&mut hasher);
            hasher.finish() as isize
        })
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
pub fn foreground_hwnd() -> isize {
    run_cmd("xprop", &["-root", "_NET_ACTIVE_WINDOW"])
        .and_then(|out| {
            let id = out.split_whitespace().last()?;
            i64::from_str_radix(id.trim_start_matches("0x"), 16).ok()
        })
        .unwrap_or(0) as isize
}
