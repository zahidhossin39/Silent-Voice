use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings as EnigoSettings,
};
use std::{thread, time::Duration};

/// Copy `text` to the clipboard, simulate Ctrl+V to paste at the current
/// cursor, then restore the previous clipboard contents.
///
/// Build plan §13 — Paste at Cursor (Windows).
pub fn paste_at_cursor(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let original = clipboard.get_text().ok();

    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;

    let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    enigo.key(Key::Control, Press).map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('v'), Click)
        .map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Release).map_err(|e| e.to_string())?;

    // Restore the user's original clipboard after the paste lands.
    if let Some(original_text) = original {
        thread::sleep(Duration::from_millis(200));
        let _ = clipboard.set_text(original_text);
    }

    Ok(())
}
