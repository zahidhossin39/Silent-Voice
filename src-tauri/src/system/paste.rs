use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings as EnigoSettings,
};
use std::{thread, time::Duration};

/// Cmd on macOS, Ctrl everywhere else — the platform's clipboard modifier.
pub fn clipboard_modifier() -> Key {
    if cfg!(target_os = "macos") { Key::Meta } else { Key::Control }
}

/// Press the clipboard modifier + `letter` (Cmd/Ctrl+V, Cmd/Ctrl+C).
///
/// On macOS this runs on the main thread: enigo finds the key for a letter
/// through the keyboard-layout API (TSMGetInputSourceProperty), and macOS 14+
/// kills the app with SIGTRAP when that is called from any other thread. That
/// was the v0.1.11 crash after every dictation. Looking the key up (instead of
/// a fixed key code) keeps Cmd+V right on Dvorak and other layouts.
pub fn press_shortcut(letter: char) -> Result<(), String> {
    on_main_thread(move || {
        let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;
        enigo.key(clipboard_modifier(), Press).map_err(|e| e.to_string())?;
        let r = enigo.key(Key::Unicode(letter), Click).map_err(|e| e.to_string());
        // Always let go of Cmd/Ctrl, or it stays stuck down for the user.
        enigo.key(clipboard_modifier(), Release).map_err(|e| e.to_string())?;
        r
    })
}

/// Press Enter (the "press enter" voice command).
pub fn press_enter() -> Result<(), String> {
    on_main_thread(|| {
        let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;
        enigo.key(Key::Return, Click).map_err(|e| e.to_string())
    })
}

#[cfg(target_os = "macos")]
fn on_main_thread<F: FnOnce() -> R, R>(f: F) -> R {
    use std::ffi::c_void;
    extern "C" {
        static _dispatch_main_q: c_void; // what dispatch_get_main_queue() returns
        fn dispatch_sync_f(queue: *const c_void, ctx: *mut c_void, work: extern "C" fn(*mut c_void));
        fn pthread_main_np() -> i32;
    }
    extern "C" fn call<F: FnOnce() -> R, R>(ctx: *mut c_void) {
        let (f, out) = unsafe { &mut *(ctx as *mut (Option<F>, Option<R>)) };
        *out = f.take().map(|f| f());
    }
    // Already on main: dispatching to it synchronously would deadlock.
    if unsafe { pthread_main_np() } != 0 {
        return f();
    }
    let mut ctx = (Some(f), None);
    unsafe {
        dispatch_sync_f(std::ptr::addr_of!(_dispatch_main_q), &mut ctx as *mut _ as *mut c_void, call::<F, R>);
    }
    ctx.1.expect("main-thread task did not run")
}

#[cfg(not(target_os = "macos"))]
fn on_main_thread<R>(f: impl FnOnce() -> R) -> R {
    f()
}

/// Copy `text` to the clipboard, simulate the paste shortcut at the current
/// cursor, then restore the previous clipboard contents.
///
/// Build plan §13 — Paste at Cursor.
pub fn paste_at_cursor(text: &str) -> Result<(), String> {
    // Wayland: the XTest Ctrl+V below can't reach native Wayland windows.
    // `wtype` types through the virtual-keyboard protocol instead (Hyprland,
    // Sway, other wlroots compositors). If it's missing or the compositor
    // refuses it (GNOME, KDE), fall through to the old path, which still
    // reaches XWayland windows — so nothing that worked before breaks.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        match std::process::Command::new("wtype").arg("--").arg(text).status() {
            Ok(s) if s.success() => return Ok(()),
            Ok(_) => crate::logging::log_info("paste", "wtype failed; falling back to Ctrl+V"),
            Err(_) => crate::logging::log_info("paste", "wtype not installed; falling back to Ctrl+V"),
        }
    }

    // Windows: snapshot EVERY format (images, copied files, rich text), not just
    // text, so a dictation never wipes a copied screenshot or file.
    #[cfg(windows)]
    let original = win_clip::save();
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    let original = clipboard.get_text().ok();

    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;

    thread::sleep(Duration::from_millis(50));
    press_shortcut('v')?;

    // Restore the user's original clipboard after the paste lands — but only if
    // the clipboard still holds OUR text. If the user copied something new during
    // the paste window, restoring our saved copy would clobber their fresh one.
    thread::sleep(Duration::from_millis(250));
    let still_ours = clipboard.get_text().ok().as_deref() == Some(text);
    drop(clipboard);
    if still_ours {
        #[cfg(windows)]
        if !original.is_empty() {
            win_clip::restore(&original);
        }
        #[cfg(not(windows))]
        if let Some(original_text) = original {
            let _ = Clipboard::new().and_then(|mut c| c.set_text(original_text));
        }
    }

    Ok(())
}

/// Raw save/restore of every clipboard format, so non-text content survives.
#[cfg(windows)]
mod win_clip {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

    /// GDI-handle formats aren't memory blocks; Windows re-creates them from
    /// their memory twins (CF_DIB → CF_BITMAP), so skipping them loses nothing.
    const HANDLE_FORMATS: [u32; 8] = [2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E];
    /// Past this the snapshot isn't worth the memory; the clipboard keeps our text.
    const MAX_BYTES: usize = 64 << 20;

    fn open() -> bool {
        // Another app can hold the clipboard for a moment; retry briefly.
        for _ in 0..10 {
            if unsafe { OpenClipboard(HWND::default()) }.is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    pub fn save() -> Vec<(u32, Vec<u8>)> {
        let mut out = Vec::new();
        if !open() {
            return out;
        }
        let mut total = 0usize;
        let mut fmt = 0u32;
        unsafe {
            loop {
                fmt = EnumClipboardFormats(fmt);
                if fmt == 0 {
                    break;
                }
                if HANDLE_FORMATS.contains(&fmt) {
                    continue;
                }
                let Ok(h) = GetClipboardData(fmt) else { continue };
                let g = HGLOBAL(h.0);
                let size = GlobalSize(g);
                let ptr = GlobalLock(g);
                if ptr.is_null() || size == 0 {
                    continue;
                }
                total += size;
                if total > MAX_BYTES {
                    let _ = GlobalUnlock(g);
                    out.clear();
                    break;
                }
                out.push((fmt, std::slice::from_raw_parts(ptr as *const u8, size).to_vec()));
                let _ = GlobalUnlock(g);
            }
            let _ = CloseClipboard();
        }
        out
    }

    /// Touches the real system clipboard, so it's opt-in:
    /// `cargo test --lib clipboard_roundtrip -- --ignored`
    #[test]
    #[ignore]
    fn clipboard_roundtrip() {
        let users = save(); // put the developer's clipboard back afterwards
        // A tiny 1×1 CF_DIB image (format 8) plus an app-private format.
        let mut dib = vec![0u8; 44];
        dib[0] = 40; dib[4] = 1; dib[8] = 1; dib[12] = 1; dib[14] = 24; dib[40] = 0xAB;
        let image = vec![(8u32, dib.clone()), (0xC123, b"private".to_vec())];
        restore(&image);
        let snap = save();
        arboard::Clipboard::new().unwrap().set_text("dictated").unwrap();
        restore(&snap);
        let back = save();
        restore(&users);
        let get = |f| back.iter().find(|(x, _)| *x == f).map(|(_, b)| b.clone());
        assert_eq!(get(8).as_deref().map(|b| &b[..44]), Some(&dib[..]));
        assert_eq!(get(0xC123).as_deref().map(|b| &b[..7]), Some(&b"private"[..]));
    }

    pub fn restore(saved: &[(u32, Vec<u8>)]) {
        if !open() {
            return;
        }
        unsafe {
            let _ = EmptyClipboard();
            for (fmt, bytes) in saved {
                let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else { continue };
                let ptr = GlobalLock(g);
                if ptr.is_null() {
                    let _ = GlobalFree(g);
                    continue;
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
                let _ = GlobalUnlock(g);
                // On success the clipboard owns the memory; free it only on failure.
                if SetClipboardData(*fmt, HANDLE(g.0)).is_err() {
                    let _ = GlobalFree(g);
                }
            }
            let _ = CloseClipboard();
        }
    }
}

/// Put text on the clipboard WITHOUT pasting. Used when a safety guard blocks
/// an automatic paste, so the user can paste it themselves with Ctrl+V.
pub fn set_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())
}
