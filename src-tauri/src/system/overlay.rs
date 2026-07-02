use crate::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const OVERLAY_LABEL: &str = "overlay";
// OPAQUE pill window. Opaque is the reliability choice: a transparent
// always-on-top WebView2 window gets occlusion-blanked (vanishes) on Windows,
// and the documented occlusion-disable browser arg breaks rendering here. The
// window is sized to the pill and resized between idle/recording. Shadow is OFF
// — the drop shadow inflated size measurements and made the pill drift
// downward each resize. Win11 corners are rounded via DWM so it still looks
// like a pill.
const OVERLAY_W: f64 = 54.0;
const OVERLAY_H: f64 = 20.0;
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
    round_corners(&win);
    let _ = win.show();
    Ok(())
}

/// Ask DWM to round the window corners (Windows 11). Harmless elsewhere.
#[cfg(windows)]
fn round_corners(win: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    if let Ok(handle) = win.hwnd() {
        let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
        let pref = DWMWCP_ROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

#[cfg(not(windows))]
fn round_corners(_win: &tauri::WebviewWindow) {}

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
}

/// Smoothly animate the overlay to a target size, center-anchored, with an
/// ease-out curve — so idle↔recording expands/contracts in place instead of
/// snapping (which looked like a glitch). A generation counter supersedes an
/// in-flight tween if a newer resize arrives.
pub fn animate_resize(app: &AppHandle, target_w: f64, target_h: f64) {
    let gen = app
        .try_state::<AppState>()
        .map(|s| s.overlay_resize_gen.fetch_add(1, Ordering::SeqCst) + 1)
        .unwrap_or(0);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
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
        const STEPS: u32 = 10;
        for i in 1..=STEPS {
            if app
                .try_state::<AppState>()
                .map(|s| s.overlay_resize_gen.load(Ordering::SeqCst) != gen)
                .unwrap_or(false)
            {
                return;
            }
            let t = i as f64 / STEPS as f64;
            let e = 1.0 - (1.0 - t).powi(3); // ease-out cubic
            apply_size(&win, sw + (target_w - sw) * e, sh + (target_h - sh) * e, center);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        apply_size(&win, target_w, target_h, center);
    });
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
