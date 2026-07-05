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
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle, &sep, &overlay_item, &sep, &dashboard, &history,
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
