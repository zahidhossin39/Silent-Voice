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
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let original = clipboard.get_text().ok();

    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;

    thread::sleep(Duration::from_millis(50));
    press_shortcut('v')?;

    // Restore the user's original clipboard after the paste lands — but only if
    // the clipboard still holds OUR text. If the user copied something new during
    // the paste window, restoring our saved copy would clobber their fresh one.
    if let Some(original_text) = original {
        thread::sleep(Duration::from_millis(250));
        let still_ours = clipboard.get_text().ok().as_deref() == Some(text);
        if still_ours {
            let _ = clipboard.set_text(original_text);
        }
    }

    Ok(())
}

/// Put text on the clipboard WITHOUT pasting. Used when a safety guard blocks
/// an automatic paste, so the user can paste it themselves with Ctrl+V.
pub fn set_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())
}
