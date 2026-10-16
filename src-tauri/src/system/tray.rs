use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use crate::AppState;

/// Build the system tray icon and context menu. Build plan §9 — System Tray.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle_record", "Start / Stop Recording", true, None::<&str>)?;
    let dashboard = MenuItem::with_id(app, "open_dashboard", "Open Dashboard", true, None::<&str>)?;
    let overlay_item = MenuItem::with_id(app, "toggle_overlay", "Show / Hide Overlay", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "open_history", "History", true, None::<&str>)?;
    let copy_last = MenuItem::with_id(app, "copy_last", "Copy last dictation", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle, &copy_last, &sep, &overlay_item, &sep, &dashboard, &history,
            &sep, &quit,
        ],
    )?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Silent Voice — idle")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle_record" => {
                let state = app.state::<AppState>();
                let is_recording = {
                    if let Ok(slot) = state.recorder.lock() {
                        slot.is_some()
                    } else {
                        false
                    }
                };
                if is_recording {
                    crate::system::hotkey::stop_capture(app);
                } else {
                    crate::system::hotkey::start_capture(app);
                }
            }
            "copy_last" => crate::system::hotkey::copy_last(),
            "toggle_overlay" => crate::system::overlay::toggle_overlay(app),
            "open_dashboard" | "open_history" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                show_main(tray.app_handle());
            }
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                show_main(tray.app_handle());
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// While a meeting records, the tray icon carries a red dot and says so, so the
/// recording stays visible with the window closed (hard rule 11: close = hide).
pub fn set_meeting_recording(app: &AppHandle, on: bool) {
    let (Some(tray), Some(icon)) = (app.tray_by_id("main-tray"), app.default_window_icon()) else {
        return;
    };
    let icon = if on { with_red_dot(icon) } else { icon.clone() };
    let _ = tray.set_icon(Some(icon));
    let _ = tray.set_tooltip(Some(if on { "Silent Voice — recording a meeting" } else { "Silent Voice — idle" }));
}

/// The app icon with a red dot in its lower-right corner.
fn with_red_dot(icon: &tauri::image::Image<'_>) -> tauri::image::Image<'static> {
    let (w, h) = (icon.width() as i64, icon.height() as i64);
    let mut rgba = icon.rgba().to_vec();
    let r = w.min(h) * 3 / 10;
    let (cx, cy) = (w - r - 1, h - r - 1);
    for y in 0..h {
        for x in 0..w {
            let d = (x - cx).pow(2) + (y - cy).pow(2);
            if d <= r * r {
                let i = ((y * w + x) * 4) as usize;
                // A white ring keeps the dot readable on a red-ish icon or taskbar.
                let px = if d >= (r - r / 4).pow(2) { [255, 255, 255, 255] } else { [239, 68, 68, 255] };
                rgba[i..i + 4].copy_from_slice(&px);
            }
        }
    }
    tauri::image::Image::new_owned(rgba, w as u32, h as u32)
}
