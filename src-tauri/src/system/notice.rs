// On-screen notice for problems the user has to see even with the app closed
// to the tray: a muted mic, no speech heard, a paste that didn't land, a
// failed dictation. Same recipe as the pill (overlay.rs): an opaque WebView2
// window (CLAUDE.md hard rule 1), DWM-rounded, always on top, never focused.
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "notice";
const W: f64 = 440.0;
const H: f64 = 116.0;
/// Clear of the taskbar and of the pill sitting above it.
const ABOVE_TASKBAR: f64 = 64.0;

#[derive(serde::Serialize, Clone)]
pub struct Notice {
    pub title: String,
    pub body: String,
    /// What the button does: "mic" / "settings" (open Settings), "logs"
    /// (copy them), "" (no button).
    pub action: &'static str,
}

/// The newest notice. A window created just now hasn't loaded yet and would
/// miss the event, so it asks for this once it has mounted.
static CURRENT: Mutex<Option<Notice>> = Mutex::new(None);

pub fn show(app: &AppHandle, title: &str, body: &str, action: &'static str) {
    let n = Notice { title: title.into(), body: body.into(), action };
    if let Ok(mut c) = CURRENT.lock() {
        *c = Some(n.clone());
    }
    let app2 = app.clone();
    // Window calls must run on the main thread; the pipeline isn't on it.
    let _ = app.run_on_main_thread(move || {
        let win = match app2.get_webview_window(LABEL) {
            Some(w) => w,
            None => match create(&app2) {
                Ok(w) => w,
                Err(e) => {
                    crate::logging::log_error("notice", &format!("could not create window: {e}"));
                    return;
                }
            },
        };
        place(&win);
        let _ = win.emit("notice://show", &n);
        let _ = win.show();
    });
}

pub fn current() -> Option<Notice> {
    CURRENT.lock().ok().and_then(|c| c.clone())
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}

fn create(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=notice".into()))
        .title("Silent Voice")
        .inner_size(W, H)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .visible(false);
    #[cfg(not(windows))]
    let builder = builder.transparent(true);
    let win = builder.build()?;
    let scale = win.scale_factor().unwrap_or(1.0);
    super::overlay::round_corners(&win, (W * scale).round() as i32, (H * scale).round() as i32);
    Ok(win)
}

/// Bottom-center of the primary monitor's work area (above the taskbar).
fn place(win: &tauri::WebviewWindow) {
    if let Ok(Some(m)) = win.primary_monitor() {
        let s = m.scale_factor();
        let a = m.work_area();
        let x = a.position.x as f64 + (a.size.width as f64 - W * s) / 2.0;
        let y = a.position.y as f64 + a.size.height as f64 - (H + ABOVE_TASKBAR) * s;
        let _ = win.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
}
