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

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
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
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, PeekMessageW,
    RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    HWND_TOPMOST, MA_NOACTIVATE, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const SQUIGGLE_H: i32 = 4;
pub(crate) const MAX_SQUIGGLES: usize = 24;
const RED: u32 = 0xFFEF4444; // spelling (premultiplied BGRA as 0xAARRGGBB)
const BLUE: u32 = 0xFF3B82F6; // grammar/style

// Popup metrics/colors (GDI COLORREF is 0x00BBGGRR).
const POPUP_W: i32 = 340;
const PAD: i32 = 18;
const TITLE_H: i32 = 30;
const SUB_H: i32 = 22;
const ROWS_TOP: i32 = PAD + TITLE_H + 4 + SUB_H + 14;
const ROW_H: i32 = 48;
const BG: u32 = 0x00ffffff; // white

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

pub fn spawn(action_tx: Sender<OverlayAction>) -> Sender<Vec<SquiggleInfo>> {
    let (tx, rx) = channel::<Vec<SquiggleInfo>>();
    std::thread::spawn(move || run(rx, action_tx));
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
        Err(_) => {
            let _ = DeleteDC(memdc);
            ReleaseDC(None, screen);
            return;
        }
    };
    let old = SelectObject(memdc, bmp);

    // Initialize bits to 0 (fully transparent)
    let px = std::slice::from_raw_parts_mut(bits as *mut u32, (POPUP_W * height) as usize);
    px.fill(0);

    // 1. Draw white background
    let bg = CreateSolidBrush(COLORREF(BG));
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
    SetTextColor(memdc, COLORREF(0x00404040));
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
    SetTextColor(memdc, COLORREF(0x00585858));
    let sub_utf16: Vec<u16> = subtitle_text.encode_utf16().collect();
    let _ = TextOutW(memdc, PAD, PAD + TITLE_H + 4, &sub_utf16);

    // 5. Draw Separators between suggestion rows
    let sep_brush = CreateSolidBrush(COLORREF(0x00e8e8e8));
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
            let hb = CreateSolidBrush(COLORREF(0x00CEE0FA));
            let hp = CreatePen(PS_SOLID, 1, COLORREF(0x00CEE0FA));
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
        SetTextColor(memdc, COLORREF(0x00202020));
        let row_utf16: Vec<u16> = row.encode_utf16().collect();
        let text_y = top + (ROW_H - 24) / 2;
        let _ = TextOutW(memdc, PAD + 6, text_y, &row_utf16);
    }

    // 7. Draw Footer (normal mode only)
    if !is_picker {
        let footer_top = ROWS_TOP + n * ROW_H + 10;
        let footer_bg = CreateSolidBrush(COLORREF(0x00f1f1f1));
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
        let add_color = if is_add_hovered { 0x00f6823b } else { 0x00303030 };

        SelectObject(memdc, font_footer_glyph);
        SetTextColor(memdc, COLORREF(add_color));
        let glyph_add_utf16: Vec<u16> = vec![0xE82D];
        let _ = TextOutW(memdc, PAD, footer_top + 17, &glyph_add_utf16);

        SelectObject(memdc, font_footer_label);
        let label_add_utf16: Vec<u16> = "Add to Dictionary".encode_utf16().collect();
        let _ = TextOutW(memdc, PAD + 26, footer_top + 18, &label_add_utf16);

        // Draw 'Dismiss'
        let is_dismiss_hovered = hover == 101;
        let dismiss_color = if is_dismiss_hovered { 0x004444ef } else { 0x00303030 };

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
                // blend the GDI-drawn content toward the orange border color by t
                let r2 = r * (1.0 - t) + 249.0 * t;
                let g2 = g * (1.0 - t) + 115.0 * t;
                let b2 = b * (1.0 - t) + 22.0 * t;
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
    let _ = UpdateLayeredWindow(
        hwnd,
        screen,
        Some(&POINT { x, y }),
        Some(&SIZE { cx: POPUP_W, cy: height }),
        memdc,
        Some(&POINT { x: 0, y: 0 }),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(memdc, old);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(memdc);
    ReleaseDC(None, screen);
}

// ---------------------------- overlay thread ----------------------------

struct Popup {
    hwnd: HWND,
    rect: RECT,
    /// Index into the current SquiggleInfo list this popup belongs to.
    info_idx: usize,
    shown: bool,
}

fn run(rx: Receiver<Vec<SquiggleInfo>>, action_tx: Sender<OverlayAction>) {
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

        let mut pool: Vec<HWND> = Vec::new();
        let mut infos: Vec<SquiggleInfo> = Vec::new();
        let mut drawn: Vec<SquiggleInfo> = Vec::new();
        let mut popup = Popup { hwnd: popup_hwnd, rect: RECT::default(), info_idx: 0, shown: false };
        let mut hover_since: Option<(usize, Instant)> = None;
        let mut outside_since: Option<Instant> = None;

        loop {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Newest squiggle list wins.
            let mut latest: Option<Vec<SquiggleInfo>> = None;
            while let Ok(v) = rx.try_recv() {
                latest = Some(v);
            }
            if let Some(new_infos) = latest {
                if new_infos != drawn {
                    apply(&mut pool, hinst.into(), squiggle_class, &new_infos);
                    drawn = new_infos.clone();
                    // The world moved under the popup — hide it.
                    if popup.shown && !popup_still_valid(&popup, &new_infos, &infos) {
                        hide_popup(&mut popup);
                        hover_since = None;
                    }
                }
                infos = new_infos;
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

            std::thread::sleep(Duration::from_millis(30));
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
    pool: &mut Vec<HWND>,
    hinst: windows::Win32::Foundation::HINSTANCE,
    class_name: windows::core::PCWSTR,
    squiggles: &[SquiggleInfo],
) {
    let show = squiggles.len().min(MAX_SQUIGGLES);
    while pool.len() < show {
        match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
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
            Ok(hwnd) => pool.push(hwnd),
            Err(e) => {
                crate::logging::log_error("squiggle", &format!("strip CreateWindowExW: {e}"));
                return;
            }
        }
    }
    for (i, s) in squiggles.iter().take(show).enumerate() {
        let w = s.w.clamp(4, 600);
        let strip_y = s.y + s.h - 2;
        draw_squiggle(pool[i], s.x, strip_y, w, if s.spelling { RED } else { BLUE });
        let _ = ShowWindow(pool[i], SW_SHOWNOACTIVATE);
    }
    for hwnd in pool.iter().skip(show) {
        let _ = ShowWindow(*hwnd, SW_HIDE);
    }
}

/// Render a zigzag wave into a 32bpp premultiplied-alpha DIB and push it to
/// the layered window at screen position (x, y).
unsafe fn draw_squiggle(hwnd: HWND, x: i32, y: i32, w: i32, color: u32) {
    let h = SQUIGGLE_H;
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
    let screen = GetDC(None);
    let memdc = CreateCompatibleDC(screen);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let bmp: HBITMAP = match CreateDIBSection(memdc, &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(b) => b,
        Err(_) => {
            let _ = DeleteDC(memdc);
            ReleaseDC(None, screen);
            return;
        }
    };
    let old = SelectObject(memdc, bmp);

    let px = std::slice::from_raw_parts_mut(bits as *mut u32, (w * h) as usize);
    px.fill(0);
    // Straight line: 3px thick, drawn on the top 3 rows (rows 0, 1 and 2), full width.
    for cx in 0..w {
        px[cx as usize] = color;
        px[(w + cx) as usize] = color;
        px[(2 * w + cx) as usize] = color;
    }

    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
        ..Default::default()
    };
    let _ = UpdateLayeredWindow(
        hwnd,
        screen,
        Some(&POINT { x, y }),
        Some(&SIZE { cx: w, cy: h }),
        memdc,
        Some(&POINT { x: 0, y: 0 }),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(memdc, old);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(memdc);
    ReleaseDC(None, screen);
}
