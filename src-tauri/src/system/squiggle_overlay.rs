// Underline renderer for macOS and Linux.
//
// Windows draws these with per-word Win32 layered windows because a transparent
// always-on-top WebView2 window is unreliable there (see the hard rules in
// CLAUDE.md). That constraint is specific to WebView2 on that hardware: WKWebView
// and WebKitGTK both handle transparent always-on-top windows, so one full-screen
// click-through window that draws every underline is a fraction of the code and
// has no z-order fight.
//
// On Linux this needs a compositor for real transparency. Without one the window
// falls back to an opaque rectangle, which is why the overlay is hidden outright
// whenever there is nothing to draw rather than left up and empty.
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
        crate::logging::log_error("squiggle_overlay", &format!("window create failed: {e}"));
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
        crate::logging::log_error("squiggle_overlay", &format!("emit failed: {e}"));
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

pub const POPUP_LABEL: &str = "squiggle-popup";

const POPUP_W: f64 = 300.0;
const POPUP_ROW_H: f64 = 34.0;
const POPUP_CHROME_H: f64 = 46.0;

/// Show the suggestion popup for one flagged word, just below it.
///
/// Unlike the underline overlay this window DOES take clicks — that is the
/// whole point of it — which is why the fix is applied against a retained
/// element rather than whatever has focus once the click lands.
pub fn show_popup(app: &AppHandle, info: &SquiggleInfo) {
    if app.get_webview_window(POPUP_LABEL).is_none() {
        let built = WebviewWindowBuilder::new(
            app,
            POPUP_LABEL,
            WebviewUrl::App("index.html?view=squiggle-popup".into()),
        )
        .title("Silent Voice Suggestions")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build();
        if let Err(e) = built {
            crate::logging::log_error("squiggle_overlay", &format!("popup create failed: {e}"));
            return;
        }
    }
    let Some(win) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };

    let rows = info.suggestions.len().min(4) as f64;
    let height = POPUP_CHROME_H + rows * POPUP_ROW_H;
    let _ = win.set_size(LogicalSize::new(POPUP_W, height));
    // Sit just under the word. ponytail: no screen-edge flipping yet — a word
    // near the bottom of the display will push the popup off-screen.
    let _ = win.set_position(LogicalPosition::new(
        info.x as f64,
        (info.y + info.h) as f64 + 4.0,
    ));

    if let Err(e) = win.emit_to(POPUP_LABEL, "squiggle://popup", info) {
        crate::logging::log_error("squiggle_overlay", &format!("popup emit failed: {e}"));
        return;
    }
    if !win.is_visible().unwrap_or(false) {
        let _ = win.show();
    }
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(POPUP_LABEL) {
        let _ = win.hide();
    }
}

/// Is the pointer over the popup right now? Used to keep it open while the user
/// travels from the underlined word down to a suggestion.
pub fn cursor_over_popup(app: &AppHandle) -> bool {
    let Some(win) = app.get_webview_window(POPUP_LABEL) else {
        return false;
    };
    if !win.is_visible().unwrap_or(false) {
        return false;
    }
    let (Ok(scale), Ok(pos), Ok(size)) = (win.scale_factor(), win.outer_position(), win.outer_size())
    else {
        return false;
    };
    let p = pos.to_logical::<f64>(scale);
    let s = size.to_logical::<f64>(scale);
    // A little slack around the edges so the gap between the word and the popup
    // does not count as leaving.
    match super::ax::cursor_position() {
        Some((cx, cy)) => {
            cx >= p.x - 8.0
                && cx <= p.x + s.width + 8.0
                && cy >= p.y - 10.0
                && cy <= p.y + s.height + 8.0
        }
        None => false,
    }
}
