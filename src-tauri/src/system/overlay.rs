use crate::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const OVERLAY_LABEL: &str = "overlay";
// OPAQUE pill window. Opaque is the reliability choice: a transparent
// always-on-top WebView2 window gets occlusion-blanked (vanishes) on Windows,
// and the documented occlusion-disable browser arg breaks rendering here. The
// window is a FIXED size for idle/recording/processing — WebView2 window
// resizing is inherently janky/flickery on Windows (tauri#4236, #6322), so all
// state transitions are CSS animations INSIDE the window instead. Only the
// right-click menu changes the window size (a discrete popup, snapped
// instantly). Shadow is OFF — the drop shadow inflated size measurements and
// made the pill drift downward each resize. Win11 corners are rounded via DWM
// so it still looks like a pill.
const OVERLAY_W: f64 = 68.0;
const OVERLAY_H: f64 = 22.0;
const BOTTOM_MARGIN: f64 = 72.0;

/// Create the always-visible floating pill window.
pub fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(
        app,
        OVERLAY_LABEL,
        WebviewUrl::App("index.html?view=overlay".into()),
    )
    .title("Silent Voice Overlay")
    .inner_size(OVERLAY_W, OVERLAY_H)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .resizable(false)
    .focused(false)
    .visible(false)
    .build()?;

    position_centered(&win, OVERLAY_W, OVERLAY_H);
    let scale = win.scale_factor().unwrap_or(1.0);
    round_corners(
        &win,
        (OVERLAY_W * scale).round() as i32,
        (OVERLAY_H * scale).round() as i32,
    );
    let _ = win.show();
    Ok(())
}

/// Round the window corners at the given PHYSICAL size. Windows 11 does it
/// via DWM (which keeps rounding through resizes). On Windows 10 that
/// attribute doesn't exist — DWM rejects it and the opaque pill shows square
/// corners — so fall back to a classic rounded window region. The region is
/// fixed to one size, so apply_size re-invokes this after every resize.
#[cfg(windows)]
fn round_corners(win: &tauri::WebviewWindow, w: i32, h: i32) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};

    let Ok(handle) = win.hwnd() else { return };
    let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
    // Attempt DWM every call (cheap, idempotent, and correct even if the
    // window is ever recreated) — only fall back when it's unsupported.
    let pref = DWMWCP_ROUND;
    let dwm_ok = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&pref) as u32,
        )
        .is_ok()
    };
    if dwm_ok {
        return;
    }
    // ~8px logical corner radius, matching Win11's DWM rounding. Region edges
    // are 1-bit (slightly jagged) but far better than square corners.
    let scale = win.scale_factor().unwrap_or(1.0);
    let d = ((16.0 * scale) as i32).min(h);
    unsafe {
        // Exclusive right/bottom bounds: (0,0,w,h) covers pixels 0..w-1 —
        // exactly the window rect.
        let rgn = CreateRoundRectRgn(0, 0, w, h, d, d);
        let _ = SetWindowRgn(hwnd, rgn, true); // SetWindowRgn owns rgn
    }
}

#[cfg(not(windows))]
fn round_corners(_win: &tauri::WebviewWindow, _w: i32, _h: i32) {}

/// Position the window at bottom-center of the primary monitor (bottom edge
/// fixed, so growing for the menu extends upward).
fn position_centered(win: &tauri::WebviewWindow, w: f64, h: f64) {
    if let Ok(Some(monitor)) = win.primary_monitor() {
        let scale = monitor.scale_factor();
        let mw = monitor.size().width as f64 / scale;
        let mh = monitor.size().height as f64 / scale;
        let x = (mw - w) / 2.0;
        let y = mh - h - BOTTOM_MARGIN;
        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
    }
}

/// Current window center in physical pixels (shadow is off, so outer == inner).
fn current_center(win: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
        Some((pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2))
    } else {
        None
    }
}

/// Current size in logical pixels.
fn current_logical_size(win: &tauri::WebviewWindow) -> (f64, f64) {
    let scale = win.scale_factor().unwrap_or(1.0);
    win.inner_size()
        .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
        .unwrap_or((OVERLAY_W, OVERLAY_H))
}

/// Apply a size while keeping the given physical CENTER fixed.
fn apply_size(win: &tauri::WebviewWindow, width: f64, height: f64, center: (i32, i32)) {
    let scale = win.scale_factor().unwrap_or(1.0);
    let new_w = (width * scale).round() as i32;
    let new_h = (height * scale).round() as i32;
    let _ = win.set_size(tauri::LogicalSize::new(width, height));
    let _ = win.set_position(tauri::PhysicalPosition::new(
        center.0 - new_w / 2,
        center.1 - new_h / 2,
    ));
    // Windows 10 rounds via a size-specific window region — refresh it.
    round_corners(win, new_w, new_h);
}

/// Resize the overlay window, center-anchored, in ONE step. Window resizing
/// on Windows/WebView2 repaints so slowly that any multi-step tween reads as
/// flicker (tauri#4236, #6322) — smooth state transitions are done with CSS
/// inside the fixed-size pill instead; this only fires for the right-click
/// menu, where an instant snap looks like a popup opening.
pub fn animate_resize(app: &AppHandle, target_w: f64, target_h: f64) {
    // Bump the generation so any legacy in-flight tween exits immediately.
    if let Some(state) = app.try_state::<AppState>() {
        state.overlay_resize_gen.fetch_add(1, Ordering::SeqCst);
    }
    let Some(win) = app.get_webview_window(OVERLAY_LABEL) else {
        return;
    };
    let Some(center) = current_center(&win) else {
        return;
    };
    let (sw, sh) = current_logical_size(&win);
    if (sw - target_w).abs() < 1.0 && (sh - target_h).abs() < 1.0 {
        return;
    }
    apply_size(&win, target_w, target_h, center);
}

/// Show the overlay and clear the user-hidden flag.
pub fn show_overlay(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.overlay_hidden.store(false, Ordering::Relaxed);
    }
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.show();
        let _ = win.set_always_on_top(true);
    }
}

/// Hide the overlay and set the user-hidden flag (so keep-alive leaves it).
pub fn hide_overlay(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.overlay_hidden.store(true, Ordering::Relaxed);
    }
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.hide();
    }
}

/// Toggle overlay visibility (tray menu).
pub fn toggle_overlay(app: &AppHandle) {
    let hidden = app
        .try_state::<AppState>()
        .map(|s| s.overlay_hidden.load(Ordering::Relaxed))
        .unwrap_or(false);
    if hidden {
        show_overlay(app);
    } else {
        hide_overlay(app);
    }
}

/// Keep-alive: ensure the pill stays shown + topmost (unless the user hid it).
/// With an opaque window this just guards against another window stealing the
/// topmost slot — it can no longer go invisible.
pub fn ensure_visible(app: &AppHandle) {
    let hidden = app
        .try_state::<AppState>()
        .map(|s| s.overlay_hidden.load(Ordering::Relaxed))
        .unwrap_or(false);
    if hidden {
        return;
    }
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        if !win.is_visible().unwrap_or(true) {
            let _ = win.show();
        }
        let _ = win.set_always_on_top(true);
    }
}
