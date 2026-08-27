// macOS underline renderer.
//
// Windows draws these with per-word Win32 layered windows because a transparent
// always-on-top WebView2 window is unreliable there (see the hard rules in
// CLAUDE.md). None of that applies on macOS: WKWebView handles transparent
// always-on-top windows fine, so one full-screen click-through window that
// draws every underline is a fraction of the code and has no z-order fight.
//
// The window is click-through (`set_ignore_cursor_events`), so it can never
// swallow a click meant for the app underneath. That also means it cannot
// receive hover, which is why the suggestion popup is a separate concern.

use super::inline_types::SquiggleInfo;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub const SQUIGGLE_LABEL: &str = "squiggle";

/// Create the overlay window, hidden, sized to the primary monitor.
fn ensure_window(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(SQUIGGLE_LABEL).is_some() {
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(
        app,
        SQUIGGLE_LABEL,
        WebviewUrl::App("index.html?view=squiggle".into()),
    )
    .title("Silent Voice Squiggles")
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .resizable(false)
    .focused(false)
    .visible(false)
    .build()?;

    // Cover the primary display. ponytail: primary monitor only — a word
    // dragged onto a second screen will not be underlined until this tracks
    // the field's monitor instead.
    if let Ok(Some(monitor)) = win.primary_monitor() {
        let scale = monitor.scale_factor();
        let pos = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        let _ = win.set_position(LogicalPosition::new(pos.x, pos.y));
        let _ = win.set_size(LogicalSize::new(size.width, size.height));
    }
    // Never take a click away from the app the user is typing into.
    let _ = win.set_ignore_cursor_events(true);
    Ok(())
}

/// Push the current underline set to the overlay, showing or hiding it to match.
/// An empty list hides the window outright rather than drawing nothing, so a
/// stuck overlay can never sit invisibly on top of everything.
pub fn draw(app: &AppHandle, infos: &[SquiggleInfo]) {
    if let Err(e) = ensure_window(app) {
        crate::logging::log_error("squiggle_mac", &format!("window create failed: {e}"));
        return;
    }
    let Some(win) = app.get_webview_window(SQUIGGLE_LABEL) else {
        return;
    };

    if infos.is_empty() {
        let _ = win.hide();
        let _ = win.emit_to(SQUIGGLE_LABEL, "squiggle://set", Vec::<SquiggleInfo>::new());
        return;
    }

    // The webview lays out relative to its own top-left, but the rects arrive
    // in screen points, so shift them by the window origin.
    let origin = win
        .outer_position()
        .ok()
        .and_then(|p| win.scale_factor().ok().map(|s| p.to_logical::<f64>(s)))
        .map(|p| (p.x, p.y))
        .unwrap_or((0.0, 0.0));

    let local: Vec<SquiggleInfo> = infos
        .iter()
        .map(|s| SquiggleInfo {
            x: s.x - origin.0 as i32,
            y: s.y - origin.1 as i32,
            ..s.clone()
        })
        .collect();

    if let Err(e) = win.emit_to(SQUIGGLE_LABEL, "squiggle://set", &local) {
        crate::logging::log_error("squiggle_mac", &format!("emit failed: {e}"));
        return;
    }
    if !win.is_visible().unwrap_or(false) {
        let _ = win.show();
    }
}

/// Hide the overlay and forget what was drawn — used when proofreading is
/// switched off or the app is shutting down.
pub fn clear(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(SQUIGGLE_LABEL) {
        let _ = win.emit_to(SQUIGGLE_LABEL, "squiggle://set", Vec::<SquiggleInfo>::new());
        let _ = win.hide();
    }
}
