// Squiggle overlay + suggestion popup for inline proofreading.
//
// - Squiggles: a pool of tiny click-through layered Win32 windows, one per
//   flagged word (Grammarly's architecture). Deliberately NOT a webview:
//   fullscreen overlays are expensive on integrated GPUs, and transparent
//   WebView2 windows are known-broken on some hardware (CLAUDE.md §8.1).
// - Popup: hovering a flagged word ~250ms shows a small native card with the
//   problem + clickable suggestions. The card is WS_EX_NOACTIVATE and answers
//   WM_MOUSEACTIVATE with MA_NOACTIVATE, so clicking it never steals focus
//   from the app being corrected — essential, because the fix is applied to
//   whatever field still has focus.
//
// Clicking a suggestion sends a FixRequest back to the inline_check watcher
// thread, which owns the UIA objects (COM apartment rules: don't touch UIA
// from this thread).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteDC,
    DeleteObject, FillRect, GetDC, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    TextOutW, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    BLENDFUNCTION, DIB_RGB_COLORS, FW_NORMAL, FW_BOLD, HBITMAP, TRANSPARENT,
    CreatePen, PS_SOLID, RoundRect,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, GetMessageW, WH_MOUSE_LL, WM_MOUSEWHEEL,
    WM_MOUSEHWHEEL,
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, PeekMessageW,
    RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    HWND_TOPMOST, MA_NOACTIVATE, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const SQUIGGLE_H: i32 = 4;
// Each squiggle is one layered window AND a per-poll round of cross-process
// COM (range verify + GetBoundingRectangles), so this trades directly against
// poll responsiveness — raise it further only with a rect-caching pass.
pub(crate) const MAX_SQUIGGLES: usize = 64;
const RED: u32 = 0xFFEF4444; // spelling (premultiplied BGRA as 0xAARRGGBB)
const BLUE: u32 = 0xFF3B82F6; // grammar/style

// Popup metrics/colors (GDI COLORREF is 0x00BBGGRR).
const POPUP_W: i32 = 340;
const PAD: i32 = 18;
const TITLE_H: i32 = 30;
const SUB_H: i32 = 22;
const ROWS_TOP: i32 = PAD + TITLE_H + 4 + SUB_H + 14;
const ROW_H: i32 = 48;
// Popup surface (card background + text). Values come straight from
// design/popup-final.html's TH table — "sv" (dark) and "light" — converted to
// GDI COLORREF (0x00BBGGRR). Translucent design values are pre-blended over
// their own background, since GDI text/fills here are opaque.
#[derive(Clone, Copy)]
struct Surface {
    bg: u32,
    title: u32,
    muted: u32,
    row: u32,
    line: u32,
    footer_bg: u32,
    footer_fg: u32,
    /// bg as plain RGB, for blending the accent-tinted hover highlight.
    bg_rgb: (u8, u8, u8),
}

const SURFACES: [Surface; 2] = [
    // 0 = Dark (#151a26) — the original look.
    Surface {
        bg: 0x00261a15,
        title: 0x00f2ebe8,        // #e8ebf2
        muted: 0x00a7938b,        // #8b93a7
        row: 0x00c4b2aa,          // #aab2c4
        line: 0x003c312c,         // white .10 over bg
        footer_bg: 0x00312621,    // white .05 over bg
        footer_fg: 0x00f2ebe8,
        bg_rgb: (0x15, 0x1a, 0x26),
    },
    // 1 = Light (#ffffff)
    Surface {
        bg: 0x00ffffff,
        title: 0x00281e1a,        // #1a1e28
        muted: 0x0073645c,        // #5c6473
        row: 0x0062514a,          // #4a5162
        line: 0x00e8e8e8,         // black .09 over white
        footer_bg: 0x00f8f8f8,    // black .028 over white
        footer_fg: 0x00281e1a,
        bg_rgb: (0xff, 0xff, 0xff),
    },
];

// Accent palettes for the popup border/highlight. Order + RGB values match
// src/components/dashboard/Theme.tsx PALETTES `dot` colors exactly, so the
// Theme picker preview matches what actually renders here.
pub(crate) const PALETTES: [(u8, u8, u8); 5] = [
    (0xa7, 0x8b, 0xfa), // violet
    (0x2d, 0xd4, 0xbf), // teal
    (0x38, 0x8b, 0xfd), // amber-blue
    (0xf9, 0x73, 0x16), // orange
    (0x8b, 0x93, 0xa7), // brightness
];
static THEME_IDX: AtomicI32 = AtomicI32::new(3); // orange, matches settingsStore default


/// Selects the popup border/highlight accent. Index into PALETTES.
pub fn set_theme(idx: usize) {
    THEME_IDX.store(idx as i32, Ordering::Relaxed);
    NEEDS_REDRAW.store(true, Ordering::Relaxed);
}



fn current_accent() -> (u8, u8, u8) {
    let idx = THEME_IDX.load(Ordering::Relaxed) as usize;
    PALETTES.get(idx).copied().unwrap_or(PALETTES[3])
}

static SURFACE_IDX: AtomicI32 = AtomicI32::new(0); // dark by default

/// Selects the popup card surface: 0 = dark, 1 = light.
pub fn set_surface(idx: usize) {
    SURFACE_IDX.store(idx as i32, Ordering::Relaxed);
    NEEDS_REDRAW.store(true, Ordering::Relaxed);
}

fn current_surface() -> Surface {
    let idx = SURFACE_IDX.load(Ordering::Relaxed) as usize;
    SURFACES.get(idx).copied().unwrap_or(SURFACES[0])
}

/// Row hover fill: the chosen accent tinted into the surface, so the highlight
/// reads correctly on both the dark and light card without a second constant.
fn hover_color(s: &Surface, accent: (u8, u8, u8)) -> u32 {
    const A: f32 = 0.22;
    let mix = |c: u8, b: u8| ((c as f32 * A) + (b as f32 * (1.0 - A))).round() as u32;
    let r = mix(accent.0, s.bg_rgb.0);
    let g = mix(accent.1, s.bg_rgb.1);
    let b = mix(accent.2, s.bg_rgb.2);
    (b << 16) | (g << 8) | r // COLORREF is 0x00BBGGRR
}

/// Accent as a GDI COLORREF.
fn accent_colorref(accent: (u8, u8, u8)) -> u32 {
    ((accent.2 as u32) << 16) | ((accent.1 as u32) << 8) | accent.0 as u32
}

/// One flagged word occurrence on screen (word rect in physical pixels).
#[derive(Clone, PartialEq)]
pub struct SquiggleInfo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub spelling: bool,
    pub message: String,
    pub suggestions: Vec<String>,
    /// Char range + exact current text of the flagged span, so the fix can
    /// verify nothing changed before replacing.
    pub start: usize,
    pub end: usize,
    pub expected: String,
}

/// Overlay → watcher: actions representing either fixing a word, dismissing,
/// or adding to the vocabulary.
pub enum OverlayAction {
    Fix { start: usize, end: usize, expected: String, replacement: String },
    Dismiss { word: String },
    AddToVocab { word: String },
}

/// The wheel hook lives on its own thread with nothing but a message pump.
/// A low-level hook is dispatched through the installing thread's message
/// queue, and Windows skips (and may drop) a hook whose thread does not respond
/// within LowLevelHooksTimeout (~300ms) — the overlay thread blocks up to 500ms
/// waiting on its channel, so it cannot host this.
fn spawn_wheel_hook() {
    std::thread::spawn(|| unsafe {
        let hinst = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(e) => {
                crate::logging::log_error("squiggle", &format!("wheel hook GetModuleHandleW: {e}"));
                return;
            }
        };
        if let Err(e) = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HINSTANCE(hinst.0), 0) {
            crate::logging::log_error("squiggle", &format!("SetWindowsHookExW failed: {e}"));
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

pub fn spawn(action_tx: Sender<OverlayAction>) -> Sender<Vec<SquiggleInfo>> {
    let (tx, rx) = channel::<Vec<SquiggleInfo>>();
    spawn_wheel_hook();
    // Restart on panic: a dead overlay thread means squiggles freeze at
    // stale positions forever (and the watcher's channel disconnects).
    std::thread::spawn(move || loop {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&rx, &action_tx)));
        if let Err(p) = r {
            crate::logging::log_error(
                "squiggle",
                &format!("overlay thread panicked: {}", crate::logging::panic_msg(&*p)),
            );
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    tx
}

// ---- popup state shared with the wndproc (single popup, single thread) ----

static POPUP_ROWS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static POPUP_MSG: OnceLock<Mutex<String>> = OnceLock::new();
static HOVER_ROW: AtomicI32 = AtomicI32::new(-1);
static CLICKED_ROW: AtomicI32 = AtomicI32::new(-1);
static POPUP_SPELLING: AtomicBool = AtomicBool::new(false);
static PICKER: AtomicBool = AtomicBool::new(false);
static POPUP_POS_X: AtomicI32 = AtomicI32::new(0);
static POPUP_POS_Y: AtomicI32 = AtomicI32::new(0);
static NEEDS_REDRAW: AtomicBool = AtomicBool::new(false);

fn popup_rows() -> &'static Mutex<Vec<String>> {
    POPUP_ROWS.get_or_init(|| Mutex::new(Vec::new()))
}
fn popup_msg() -> &'static Mutex<String> {
    POPUP_MSG.get_or_init(|| Mutex::new(String::new()))
}

fn get_popup_height(n_rows: i32, is_picker: bool) -> i32 {
    if is_picker {
        ROWS_TOP + n_rows * ROW_H + PAD
    } else {
        let footer_top = ROWS_TOP + n_rows * ROW_H + 10;
        footer_top + 52
    }
}

fn hit_at(x: i32, y: i32) -> i32 {
    let n = popup_rows().lock().map(|r| r.len() as i32).unwrap_or(0);
    for i in 0..n {
        let top = ROWS_TOP + i * ROW_H;
        if y >= top && y < top + ROW_H && x >= 0 && x <= POPUP_W {
            return i;
        }
    }
    if PICKER.load(Ordering::Relaxed) {
        return -1;
    }
    let footer_top = ROWS_TOP + n * ROW_H + 10;
    if y >= footer_top && y < footer_top + 52 {
        if x >= PAD - 4 && x < 190 {
            return 100;
        }
        if x >= 196 && x <= POPUP_W - PAD {
            return 101;
        }
    }
    -1
}

unsafe extern "system" fn squiggle_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

unsafe extern "system" fn popup_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => {
            let x = (lp.0 as u32 & 0xFFFF) as i16 as i32;
            let y = ((lp.0 as u32) >> 16) as i16 as i32;
            let hit = hit_at(x, y);
            if HOVER_ROW.swap(hit, Ordering::Relaxed) != hit {
                NEEDS_REDRAW.store(true, Ordering::Relaxed);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lp.0 as u32 & 0xFFFF) as i16 as i32;
            let y = ((lp.0 as u32) >> 16) as i16 as i32;
            let hit = hit_at(x, y);
            if hit != -1 {
                CLICKED_ROW.store(hit, Ordering::Relaxed);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn render_popup(hwnd: HWND, x: i32, y: i32) {
    let rows = popup_rows().lock().map(|r| r.clone()).unwrap_or_default();
    let message = popup_msg().lock().map(|m| m.clone()).unwrap_or_default();
    let hover = HOVER_ROW.load(Ordering::Relaxed);
    let n = rows.len() as i32;
    let is_picker = PICKER.load(Ordering::Relaxed);
    let height = get_popup_height(n, is_picker);

    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: POPUP_W,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let screen = GetDC(None);
    let memdc = CreateCompatibleDC(screen);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let bmp: HBITMAP = match CreateDIBSection(memdc, &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(b) => b,
        Err(e) => {
            crate::logging::log_error("squiggle", &format!("CreateDIBSection (popup) failed: {e}"));
            let _ = DeleteDC(memdc);
            ReleaseDC(None, screen);
            return;
        }
    };
    let old = SelectObject(memdc, bmp);

    // Initialize bits to 0 (fully transparent)
    let px = std::slice::from_raw_parts_mut(bits as *mut u32, (POPUP_W * height) as usize);
    px.fill(0);

    let surface = current_surface();
    let accent = current_accent();

    // 1. Draw card background
    let bg = CreateSolidBrush(COLORREF(surface.bg));
    FillRect(memdc, &RECT { left: 0, top: 0, right: POPUP_W, bottom: height }, bg);
    let _ = DeleteObject(bg);

    SetBkMode(memdc, TRANSPARENT);

    // 2. Create fonts
    let font_title = CreateFontW(
        -22, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_sub = CreateFontW(
        -15, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_row = CreateFontW(
        -20, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_footer_label = CreateFontW(
        -16, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_footer_glyph = CreateFontW(
        -18, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe MDL2 Assets"),
    );

    // 3. Draw Title
    let title_text = if is_picker {
        "Add to Dictionary"
    } else {
        if POPUP_SPELLING.load(Ordering::Relaxed) {
            "Spelling Insights"
        } else {
            "Grammar Insights"
        }
    };
    let old_font = SelectObject(memdc, font_title);
    SetTextColor(memdc, COLORREF(surface.title));
    let title_utf16: Vec<u16> = title_text.encode_utf16().collect();
    let _ = TextOutW(memdc, PAD, PAD, &title_utf16);

    // 4. Draw Subtitle
    let subtitle_text = if is_picker {
        "Choose the word to add".to_string()
    } else {
        let mut msg = message.clone();
        if msg.chars().count() > 40 {
            msg = msg.chars().take(39).collect::<String>() + "…";
        }
        msg
    };
    SelectObject(memdc, font_sub);
    SetTextColor(memdc, COLORREF(surface.muted));
    let sub_utf16: Vec<u16> = subtitle_text.encode_utf16().collect();
    let _ = TextOutW(memdc, PAD, PAD + TITLE_H + 4, &sub_utf16);

    // 5. Draw Separators between suggestion rows
    let sep_brush = CreateSolidBrush(COLORREF(surface.line));
    for i in 0..(n - 1) {
        let sep_y = ROWS_TOP + (i + 1) * ROW_H;
        FillRect(
            memdc,
            &RECT {
                left: PAD,
                top: sep_y,
                right: POPUP_W - PAD,
                bottom: sep_y + 1,
            },
            sep_brush,
        );
    }
    let _ = DeleteObject(sep_brush);

    // 6. Draw Suggestion rows
    SelectObject(memdc, font_row);
    for (i, row) in rows.iter().enumerate() {
        let top = ROWS_TOP + i as i32 * ROW_H;
        let is_hovered = i as i32 == hover;
        if is_hovered {
            let hb = CreateSolidBrush(COLORREF(hover_color(&surface, accent)));
            let hp = CreatePen(PS_SOLID, 1, COLORREF(hover_color(&surface, accent)));
            let old_brush = SelectObject(memdc, hb);
            let old_pen = SelectObject(memdc, hp);
            let _ = RoundRect(
                memdc,
                8,
                top + 3,
                POPUP_W - 8,
                top + ROW_H - 3,
                24,
                24,
            );
            SelectObject(memdc, old_brush);
            SelectObject(memdc, old_pen);
            let _ = DeleteObject(hb);
            let _ = DeleteObject(hp);
        }
        SetTextColor(memdc, COLORREF(surface.row));
        let row_utf16: Vec<u16> = row.encode_utf16().collect();
        let text_y = top + (ROW_H - 24) / 2;
        let _ = TextOutW(memdc, PAD + 6, text_y, &row_utf16);
    }

    // 7. Draw Footer (normal mode only)
    if !is_picker {
        let footer_top = ROWS_TOP + n * ROW_H + 10;
        let footer_bg = CreateSolidBrush(COLORREF(surface.footer_bg));
        FillRect(
            memdc,
            &RECT {
                left: 0,
                top: footer_top,
                right: POPUP_W,
                bottom: footer_top + 52,
            },
            footer_bg,
        );
        let _ = DeleteObject(footer_bg);

        // Draw 'Add to Dictionary'
        let is_add_hovered = hover == 100;
        let add_color = if is_add_hovered { accent_colorref(accent) } else { surface.footer_fg };

        SelectObject(memdc, font_footer_glyph);
        SetTextColor(memdc, COLORREF(add_color));
        let glyph_add_utf16: Vec<u16> = vec![0xE82D];
        let _ = TextOutW(memdc, PAD, footer_top + 17, &glyph_add_utf16);

        SelectObject(memdc, font_footer_label);
        let label_add_utf16: Vec<u16> = "Add to Dictionary".encode_utf16().collect();
        let _ = TextOutW(memdc, PAD + 26, footer_top + 18, &label_add_utf16);

        // Draw 'Dismiss'
        let is_dismiss_hovered = hover == 101;
        let dismiss_color = if is_dismiss_hovered { accent_colorref(accent) } else { surface.footer_fg };

        SelectObject(memdc, font_footer_glyph);
        SetTextColor(memdc, COLORREF(dismiss_color));
        let glyph_dismiss_utf16: Vec<u16> = vec![0xE711];
        let _ = TextOutW(memdc, 200, footer_top + 17, &glyph_dismiss_utf16);

        SelectObject(memdc, font_footer_label);
        let label_dismiss_utf16: Vec<u16> = "Dismiss".encode_utf16().collect();
        let _ = TextOutW(memdc, 226, footer_top + 18, &label_dismiss_utf16);
    }


    // 9. Clean up fonts
    SelectObject(memdc, old_font);
    let _ = DeleteObject(font_title);
    let _ = DeleteObject(font_sub);
    let _ = DeleteObject(font_row);
    let _ = DeleteObject(font_footer_label);
    let _ = DeleteObject(font_footer_glyph);

    // 10. Post-process every pixel for rounded corners / anti-aliasing
    let center_x = POPUP_W as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let half_width = POPUP_W as f32 / 2.0;
    let half_height = height as f32 / 2.0;
    let radius = 24.0f32;
    let accent = current_accent();

    for py in 0..height {
        let y_f = py as f32 + 0.5;
        let dy = (y_f - center_y).abs();
        let qy = dy - (half_height - radius);
        let my = qy.max(0.0);

        for px_idx in 0..POPUP_W {
            let x_f = px_idx as f32 + 0.5;
            let dx = (x_f - center_x).abs();
            let qx = dx - (half_width - radius);
            let mx = qx.max(0.0);

            let dist = (mx * mx + my * my).sqrt() - radius;
            let c_outer = (0.5f32 - dist).clamp(0.0, 1.0);
            let c_inner = (0.5f32 - (dist + 2.0)).clamp(0.0, 1.0);
            let t = c_outer - c_inner; // 1.0 inside the 2px border ring, AA at both edges
            let idx = (py * POPUP_W + px_idx) as usize;
            if c_outer <= 0.0 {
                px[idx] = 0;
            } else {
                let pixel = px[idx];
                let b = (pixel & 0xFF) as f32;
                let g = ((pixel >> 8) & 0xFF) as f32;
                let r = ((pixel >> 16) & 0xFF) as f32;
                // blend the GDI-drawn content toward the selected accent's border color by t
                let r2 = r * (1.0 - t) + accent.0 as f32 * t;
                let g2 = g * (1.0 - t) + accent.1 as f32 * t;
                let b2 = b * (1.0 - t) + accent.2 as f32 * t;
                let a = (c_outer * 255.0).round() as u32;
                let new_r = (r2 * c_outer).round().min(255.0) as u32;
                let new_g = (g2 * c_outer).round().min(255.0) as u32;
                let new_b = (b2 * c_outer).round().min(255.0) as u32;
                px[idx] = (a << 24) | (new_r << 16) | (new_g << 8) | new_b;
            }
        }
    }

    // 11. Update Layered Window
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
        ..Default::default()
    };
    if let Err(e) = UpdateLayeredWindow(
        hwnd,
        screen,
        Some(&POINT { x, y }),
        Some(&SIZE { cx: POPUP_W, cy: height }),
        memdc,
        Some(&POINT { x: 0, y: 0 }),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    ) {
        crate::logging::log_error("squiggle", &format!("UpdateLayeredWindow (popup) failed: {e}"));
    }

    SelectObject(memdc, old);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(memdc);
    ReleaseDC(None, screen);
}

// ---- scroll intent ----
// A UIA rect read costs 5-25ms of cross-process IPC and Chromium scrolls on its
// compositor thread, so no amount of polling can reposition underlines fast
// enough to stay on their words — they visibly trail the text. A low-level
// mouse hook sees the wheel event before the scroll is even painted, so we hide
// the overlay on the spot and let the watcher bring it back once the view
// settles. Hiding instantly is the only way the user never sees a stale frame.
static SCROLL_CLOCK: OnceLock<Instant> = OnceLock::new();
fn now_ms() -> u64 {
    SCROLL_CLOCK.get_or_init(Instant::now).elapsed().as_millis() as u64
}
static LAST_WHEEL_MS: AtomicU64 = AtomicU64::new(0);
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
/// How long after the last wheel tick the overlay stays suppressed.
const SCROLL_HIDE_MS: u64 = 220;

fn scrolling_now() -> bool {
    let last = LAST_WHEEL_MS.load(Ordering::Relaxed);
    if last == 0 {
        return false;
    }
    now_ms().saturating_sub(last) < SCROLL_HIDE_MS
}

// Runs on the overlay thread (the thread that installed the hook), which is the
// same thread that owns the overlay window — so calling ShowWindow here is safe
// and gives a ~0ms hide.
unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wp.0 as u32;
        if msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
            LAST_WHEEL_MS.store(now_ms(), Ordering::Relaxed);
            let h = OVERLAY_HWND.load(Ordering::Relaxed);
            if h != 0 {
                let _ = ShowWindow(HWND(h as *mut std::ffi::c_void), SW_HIDE);
            }
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

// ---------------------------- overlay thread ----------------------------

struct Overlay {
    hwnd: HWND,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    alpha: i32,
    hiding: bool,
    bmp: Option<HBITMAP>,
    bmp_w: i32,
    bmp_h: i32,
    bits: *mut std::ffi::c_void,
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            if let Some(h) = self.bmp.take() {
                let _ = DeleteObject(h);
                self.bits = std::ptr::null_mut();
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

struct Popup {
    hwnd: HWND,
    rect: RECT,
    /// Index into the current SquiggleInfo list this popup belongs to.
    info_idx: usize,
    shown: bool,
}

fn run(rx: &Receiver<Vec<SquiggleInfo>>, action_tx: &Sender<OverlayAction>) {
    unsafe {
        let hinst = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(e) => {
                crate::logging::log_error("squiggle", &format!("GetModuleHandleW: {e}"));
                return;
            }
        };
        let squiggle_class = w!("SVSquiggle");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(squiggle_proc),
            hInstance: hinst.into(),
            lpszClassName: squiggle_class,
            ..Default::default()
        });
        let popup_class = w!("SVSuggestPopup");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(popup_proc),
            hInstance: hinst.into(),
            lpszClassName: popup_class,
            ..Default::default()
        });
        let popup_hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            popup_class,
            w!(""),
            WS_POPUP,
            0,
            0,
            POPUP_W,
            100,
            None,
            None,
            hinst,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                crate::logging::log_error("squiggle", &format!("popup CreateWindowExW: {e}"));
                return;
            }
        };

        let overlay_hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            squiggle_class,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            hinst,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                crate::logging::log_error("squiggle", &format!("overlay CreateWindowExW: {e}"));
                return;
            }
        };

        OVERLAY_HWND.store(overlay_hwnd.0 as isize, Ordering::Relaxed);

        let mut overlay = Overlay {
            hwnd: overlay_hwnd,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            alpha: 0,
            hiding: false,
            bmp: None,
            bmp_w: 0,
            bmp_h: 0,
            bits: std::ptr::null_mut(),
        };
        let mut infos: Vec<SquiggleInfo> = Vec::new();
        let mut drawn: Vec<SquiggleInfo> = Vec::new();
        let mut popup = Popup { hwnd: popup_hwnd, rect: RECT::default(), info_idx: 0, shown: false };
        let mut hover_since: Option<(usize, Instant)> = None;
        let mut outside_since: Option<Instant> = None;
        // Set when the idle blocking recv below received a list; consumed at
        // the top of the next iteration.
        let mut pending: Option<Vec<SquiggleInfo>> = None;
        let mut was_scrolling = false;

        loop {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // The wheel hook hid the overlay without touching `drawn`, and
            // apply() only runs when the incoming list differs from `drawn` —
            // so clear it on scroll-end, otherwise an unchanged list would
            // never re-apply and the underlines would stay hidden.
            let scrolling = scrolling_now();
            if was_scrolling && !scrolling {
                drawn.clear();
            }
            was_scrolling = scrolling;

            // Newest squiggle list wins.
            let mut latest: Option<Vec<SquiggleInfo>> = pending.take();
            while let Ok(v) = rx.try_recv() {
                latest = Some(v);
            }
            if let Some(new_infos) = latest {
                if new_infos != drawn {
                    apply(&mut overlay, &new_infos, &drawn);
                    drawn = new_infos.clone();
                    // The world moved under the popup — hide it.
                    if popup.shown && !popup_still_valid(&popup, &new_infos, &infos) {
                        hide_popup(&mut popup);
                        hover_since = None;
                    }
                }
                infos = new_infos;
            }

            // Animate fading strips (in AND out).
            if overlay.hiding {
                overlay.alpha = (overlay.alpha - 90).max(0);
                if overlay.alpha == 0 {
                    let _ = ShowWindow(overlay.hwnd, SW_HIDE);
                    overlay.hiding = false;
                } else {
                    push_overlay(&mut overlay);
                }
            } else if overlay.alpha > 0 && overlay.alpha < 255 {
                overlay.alpha = (overlay.alpha + 50).min(255);
                push_overlay(&mut overlay);
            }

            // Suggestion click?
            let clicked = CLICKED_ROW.swap(-1, Ordering::Relaxed);
            if clicked != -1 && popup.shown {
                if let Some(info) = infos.get(popup.info_idx) {
                    let is_picker = PICKER.load(Ordering::Relaxed);
                    if clicked < 100 {
                        let rows = popup_rows().lock().map(|r| r.clone()).unwrap_or_default();
                        if clicked < rows.len() as i32 {
                            if is_picker {
                                if let Some(word) = rows.get(clicked as usize) {
                                    let _ = action_tx.send(OverlayAction::AddToVocab {
                                        word: word.clone(),
                                    });
                                    if word != &info.expected {
                                        let _ = action_tx.send(OverlayAction::Fix {
                                            start: info.start,
                                            end: info.end,
                                            expected: info.expected.clone(),
                                            replacement: word.clone(),
                                        });
                                    }
                                }
                                hide_popup(&mut popup);
                                hover_since = None;
                            } else {
                                if let Some(replacement) = info.suggestions.get(clicked as usize) {
                                    let _ = action_tx.send(OverlayAction::Fix {
                                        start: info.start,
                                        end: info.end,
                                        expected: info.expected.clone(),
                                        replacement: replacement.clone(),
                                    });
                                }
                                hide_popup(&mut popup);
                                hover_since = None;
                            }
                        }
                    } else if clicked == 100 {
                        // Enter picker mode
                        PICKER.store(true, Ordering::Relaxed);
                        let mut picker_rows = vec![info.expected.clone()];
                        picker_rows.extend(info.suggestions.iter().filter(|s| !s.trim().is_empty()).take(3).cloned());
                        if let Ok(mut r) = popup_rows().lock() {
                            *r = picker_rows.clone();
                        }
                        if let Ok(mut m) = popup_msg().lock() {
                            *m = "Choose the word to add".to_string();
                        }
                        HOVER_ROW.store(-1, Ordering::Relaxed);

                        let n_rows = picker_rows.len() as i32;
                        let height = get_popup_height(n_rows, true);
                        let x = info.x;
                        let mut y = info.y - height - 6;
                        if y < 0 {
                            y = info.y + info.h + SQUIGGLE_H + 2;
                        }
                        POPUP_POS_X.store(x, Ordering::Relaxed);
                        POPUP_POS_Y.store(y, Ordering::Relaxed);

                        render_popup(popup.hwnd, x, y);

                        popup.rect = RECT { left: x, top: y, right: x + POPUP_W, bottom: y + height };
                    } else if clicked == 101 {
                        // Dismiss
                        let _ = action_tx.send(OverlayAction::Dismiss {
                            word: info.expected.clone(),
                        });
                        hide_popup(&mut popup);
                        hover_since = None;
                    }
                }
            }

            // Hover tracking.
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            let over_idx = infos.iter().position(|s| {
                cursor.x >= s.x - 2
                    && cursor.x <= s.x + s.w + 2
                    && cursor.y >= s.y - 2
                    && cursor.y <= s.y + s.h + SQUIGGLE_H
            });
            let over_popup = popup.shown
                && cursor.x >= popup.rect.left - 8
                && cursor.x <= popup.rect.right + 8
                && cursor.y >= popup.rect.top - 8
                && cursor.y <= popup.rect.bottom + 8;

            if !popup.shown {
                match (over_idx, hover_since) {
                    (Some(i), Some((j, t))) if i == j => {
                        if t.elapsed() >= Duration::from_millis(250) {
                            show_popup(&mut popup, &infos, i);
                        }
                    }
                    (Some(i), _) => hover_since = Some((i, Instant::now())),
                    (None, _) => hover_since = None,
                }
            } else {
                let inside = over_popup || over_idx == Some(popup.info_idx);
                if inside {
                    outside_since = None;
                } else {
                    match outside_since {
                        Some(t) if t.elapsed() >= Duration::from_millis(350) => {
                            hide_popup(&mut popup);
                            hover_since = None;
                            outside_since = None;
                        }
                        None => outside_since = Some(Instant::now()),
                        _ => {}
                    }
                }
            }

            if NEEDS_REDRAW.swap(false, Ordering::Relaxed) && popup.shown {
                let px = POPUP_POS_X.load(Ordering::Relaxed);
                let py = POPUP_POS_Y.load(Ordering::Relaxed);
                render_popup(popup.hwnd, px, py);
            }

            // Idle (nothing on screen, no popup): block on the channel
            // instead of spinning at 30ms — wakes instantly when the watcher
            // sends squiggles, and only pumps messages every 500ms otherwise.
            // Strips mid-fade keep the loop ticking so the dissolve stays
            // smooth even after the squiggle list itself went empty.
            let animating = overlay.hiding || (overlay.alpha > 0 && overlay.alpha < 255);
            let idle = drawn.is_empty() && !popup.shown && !animating;
            if idle {
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(v) => pending = Some(v),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            } else {
                // Active: don't sleep blind — block on the channel with the
                // same 30ms budget so a "clear" arrives the instant it is
                // sent instead of waiting out the remainder of a sleep.
                match rx.recv_timeout(Duration::from_millis(30)) {
                    Ok(v) => pending = Some(v),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        std::thread::sleep(Duration::from_millis(30));
                    }
                }
            }
        }
    }
}

/// After a refresh, is the popup's word still on screen at the same spot?
fn popup_still_valid(popup: &Popup, new_infos: &[SquiggleInfo], old_infos: &[SquiggleInfo]) -> bool {
    match (old_infos.get(popup.info_idx), new_infos.get(popup.info_idx)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

unsafe fn show_popup(popup: &mut Popup, infos: &[SquiggleInfo], idx: usize) {
    let Some(info) = infos.get(idx) else { return };
    // No suggestions → still show the message so the user knows what's wrong.
    let rows: Vec<String> = info.suggestions.iter().take(3).map(|s| {
        if s.trim().is_empty() {
            "Remove".to_string()
        } else {
            s.clone()
        }
    }).collect();
    let mut message = info.message.clone();
    if message.chars().count() > 40 {
        message = message.chars().take(39).collect::<String>() + "…";
    }
    if let Ok(mut r) = popup_rows().lock() {
        *r = rows.clone();
    }
    if let Ok(mut m) = popup_msg().lock() {
        *m = message;
    }
    HOVER_ROW.store(-1, Ordering::Relaxed);
    PICKER.store(false, Ordering::Relaxed);
    POPUP_SPELLING.store(info.spelling, Ordering::Relaxed);

    let n = rows.len() as i32;
    let height = get_popup_height(n, false);
    let x = info.x;
    let mut y = info.y - height - 6;
    if y < 0 {
        y = info.y + info.h + SQUIGGLE_H + 2;
    }
    POPUP_POS_X.store(x, Ordering::Relaxed);
    POPUP_POS_Y.store(y, Ordering::Relaxed);

    render_popup(popup.hwnd, x, y);

    popup.rect = RECT { left: x, top: y, right: x + POPUP_W, bottom: y + height };
    popup.info_idx = idx;
    popup.shown = true;
    let _ = ShowWindow(popup.hwnd, SW_SHOWNOACTIVATE);
    // Force to the top of the TOPMOST band — the target app itself may be
    // an always-on-top window sitting above us, which would eat the click.
    let _ = SetWindowPos(
        popup.hwnd,
        HWND_TOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
}

unsafe fn hide_popup(popup: &mut Popup) {
    let _ = ShowWindow(popup.hwnd, SW_HIDE);
    popup.shown = false;
    PICKER.store(false, Ordering::Relaxed);
}

// ------------------------- squiggle strip drawing -------------------------

unsafe fn apply(
    overlay: &mut Overlay,
    squiggles: &[SquiggleInfo],
    drawn: &[SquiggleInfo],
) {
    let show = squiggles.len().min(MAX_SQUIGGLES);
    // A wheel tick just happened — stay hidden until the view settles rather
    // than paint underlines at positions the scroll has already invalidated.
    if scrolling_now() {
        if overlay.alpha != 0 {
            let _ = ShowWindow(overlay.hwnd, SW_HIDE);
            overlay.alpha = 0;
            overlay.hiding = false;
        }
        return;
    }
    if show == 0 {
        if overlay.alpha != 0 {
            let _ = ShowWindow(overlay.hwnd, SW_HIDE);
            overlay.alpha = 0;
            overlay.hiding = false;
        }
        return;
    }

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for s in squiggles.iter().take(show) {
        let w = s.w.clamp(4, 600);
        let strip_y = s.y + s.h - 2;
        min_x = min_x.min(s.x);
        min_y = min_y.min(strip_y);
        max_x = max_x.max(s.x + w);
        max_y = max_y.max(strip_y + SQUIGGLE_H);
    }

    let w = max_x - min_x;
    let h = max_y - min_y;

    // Reuse the existing DIB whenever the bounding box is unchanged - the
    // common case while typing. Recreating a near-fullscreen 32bpp bitmap on
    // every frame was pure allocation churn.
    if overlay.bits.is_null() || overlay.bmp_w != w || overlay.bmp_h != h {
        if let Some(bmp) = overlay.bmp.take() {
            let _ = DeleteObject(bmp);
        }
        overlay.bits = std::ptr::null_mut();

        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(screen);
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        match CreateDIBSection(memdc, &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bmp) => {
                overlay.bmp = Some(bmp);
                overlay.bmp_w = w;
                overlay.bmp_h = h;
                overlay.bits = bits;
            }
            Err(e) => {
                crate::logging::log_error("squiggle", &format!("CreateDIBSection (overlay) failed: {e}"));
                let _ = DeleteDC(memdc);
                ReleaseDC(None, screen);
                return;
            }
        }

        let _ = DeleteDC(memdc);
        ReleaseDC(None, screen);
    }

    let px = std::slice::from_raw_parts_mut(overlay.bits as *mut u32, (w * h) as usize);
    px.fill(0);

    for s in squiggles.iter().take(show) {
        let sw = s.w.clamp(4, 600);
        let strip_y = s.y + s.h - 2;
        let color = if s.spelling { RED } else { BLUE };

        for r in 0..3 {
            for cx in 0..sw {
                let py = strip_y - min_y + r;
                let px_x = s.x - min_x + cx;
                let idx = (py * w + px_x) as usize;
                if idx < px.len() {
                    px[idx] = color;
                }
            }
        }
    }

    let new_alpha = if drawn.is_empty() { 55 } else { 255 };
    overlay.alpha = new_alpha;
    overlay.hiding = false;
    
    overlay.x = min_x;
    overlay.y = min_y;
    overlay.w = w;
    overlay.h = h;
    
    push_overlay(overlay);
    let _ = ShowWindow(overlay.hwnd, SW_SHOWNOACTIVATE);
    // Same reason show_popup() does this: WS_EX_TOPMOST only puts us in the
    // topmost band, and within that band z-order still follows activation. The
    // target app can sit above us, which hides the underlines while the popup
    // (which re-asserts this) stays visible.
    let _ = SetWindowPos(
        overlay.hwnd,
        HWND_TOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
}

unsafe fn push_overlay(overlay: &mut Overlay) {
    let Some(bmp) = overlay.bmp else { return };
    let screen = GetDC(None);
    let memdc = CreateCompatibleDC(screen);
    let old = SelectObject(memdc, bmp);
    
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        SourceConstantAlpha: overlay.alpha as u8,
        AlphaFormat: AC_SRC_ALPHA as u8,
        ..Default::default()
    };
    
    if let Err(e) = UpdateLayeredWindow(
        overlay.hwnd,
        screen,
        Some(&POINT { x: overlay.x, y: overlay.y }),
        Some(&SIZE { cx: overlay.w, cy: overlay.h }),
        memdc,
        Some(&POINT { x: 0, y: 0 }),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    ) {
        crate::logging::log_error("squiggle", &format!("UpdateLayeredWindow (overlay) failed: {e}"));
    }
    
    SelectObject(memdc, old);
    let _ = DeleteDC(memdc);
    ReleaseDC(None, screen);
}
