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

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DeleteDC, DeleteObject, EndPaint, FillRect, GetDC, InvalidateRect,
    ReleaseDC, SelectObject, SetBkMode, SetTextColor, TextOutW, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, FW_NORMAL, FW_SEMIBOLD,
    HBITMAP, HFONT, PAINTSTRUCT, TRANSPARENT, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, MoveWindow, PeekMessageW,
    RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    HWND_TOPMOST, MA_NOACTIVATE, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const SQUIGGLE_H: i32 = 4;
const MAX_SQUIGGLES: usize = 24;
const RED: u32 = 0xFFEF4444; // spelling (premultiplied BGRA as 0xAARRGGBB)
const BLUE: u32 = 0xFF3B82F6; // grammar/style

// Popup metrics/colors (GDI COLORREF is 0x00BBGGRR).
const POPUP_W: i32 = 260;
const PAD: i32 = 14;
const MSG_H: i32 = 20;
const ROW_H: i32 = 34;
const ROWS_TOP: i32 = PAD + MSG_H + 12;
const BG: u32 = 0x00201b17; // near-black, warm
const ROW_HOVER_BG: u32 = 0x00352a20;
const TXT_MUTED: u32 = 0x009a9a9a;
const TXT_MAIN: u32 = 0x00f0f0f0;
const TXT_ACCENT: u32 = 0x001673f9; // orange #f97316 as BGR

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

/// Overlay → watcher: "replace chars start..end (currently `expected`) with
/// `replacement` in the focused field".
pub struct FixRequest {
    pub start: usize,
    pub end: usize,
    pub expected: String,
    pub replacement: String,
}

pub fn spawn(fix_tx: Sender<FixRequest>) -> Sender<Vec<SquiggleInfo>> {
    let (tx, rx) = channel::<Vec<SquiggleInfo>>();
    std::thread::spawn(move || run(rx, fix_tx));
    tx
}

// ---- popup state shared with the wndproc (single popup, single thread) ----

static POPUP_ROWS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static POPUP_MSG: OnceLock<Mutex<String>> = OnceLock::new();
static HOVER_ROW: AtomicI32 = AtomicI32::new(-1);
static CLICKED_ROW: AtomicI32 = AtomicI32::new(-1);

fn popup_rows() -> &'static Mutex<Vec<String>> {
    POPUP_ROWS.get_or_init(|| Mutex::new(Vec::new()))
}
fn popup_msg() -> &'static Mutex<String> {
    POPUP_MSG.get_or_init(|| Mutex::new(String::new()))
}

fn row_at(y: i32) -> i32 {
    let rel = y - ROWS_TOP;
    if rel < 0 {
        return -1;
    }
    let row = rel / ROW_H;
    let count = popup_rows().lock().map(|r| r.len() as i32).unwrap_or(0);
    if row < count {
        row
    } else {
        -1
    }
}

unsafe extern "system" fn squiggle_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

unsafe extern "system" fn popup_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => {
            let y = ((lp.0 as u32) >> 16) as i16 as i32;
            let row = row_at(y);
            if HOVER_ROW.swap(row, Ordering::Relaxed) != row {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = ((lp.0 as u32) >> 16) as i16 as i32;
            let row = row_at(y);
            if row >= 0 {
                CLICKED_ROW.store(row, Ordering::Relaxed);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            paint_popup(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn paint_popup(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let rows = popup_rows().lock().map(|r| r.clone()).unwrap_or_default();
    let message = popup_msg().lock().map(|m| m.clone()).unwrap_or_default();
    let hover = HOVER_ROW.load(Ordering::Relaxed);
    let height = ROWS_TOP + rows.len() as i32 * ROW_H + PAD;

    let bg = CreateSolidBrush(COLORREF(BG));
    FillRect(hdc, &RECT { left: 0, top: 0, right: POPUP_W, bottom: height }, bg);
    let _ = DeleteObject(bg);

    SetBkMode(hdc, TRANSPARENT);
    let font_msg: HFONT = CreateFontW(
        -13, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_row: HFONT = CreateFontW(
        -15, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );

    let old = SelectObject(hdc, font_msg);
    SetTextColor(hdc, COLORREF(TXT_MUTED));
    let msg_utf16: Vec<u16> = message.encode_utf16().collect();
    let _ = TextOutW(hdc, PAD, PAD, &msg_utf16);

    let sep_color = CreateSolidBrush(COLORREF(0x00393028));
    let sep_y = PAD + MSG_H + 6;
    FillRect(
        hdc,
        &RECT {
            left: PAD,
            top: sep_y,
            right: POPUP_W - PAD,
            bottom: sep_y + 1,
        },
        sep_color,
    );
    let _ = DeleteObject(sep_color);

    SelectObject(hdc, font_row);
    for (i, row) in rows.iter().enumerate() {
        let top = ROWS_TOP + i as i32 * ROW_H;
        if i as i32 == hover {
            let hb = CreateSolidBrush(COLORREF(ROW_HOVER_BG));
            FillRect(
                hdc,
                &RECT { left: 6, top, right: POPUP_W - 6, bottom: top + ROW_H },
                hb,
            );
            let _ = DeleteObject(hb);
        }
        SetTextColor(hdc, COLORREF(if i as i32 == hover { TXT_ACCENT } else { TXT_MAIN }));
        let row_utf16: Vec<u16> = row.encode_utf16().collect();
        let text_y = top + (ROW_H - 20) / 2;
        let _ = TextOutW(hdc, PAD, text_y, &row_utf16);
    }

    SelectObject(hdc, old);
    let _ = DeleteObject(font_msg);
    let _ = DeleteObject(font_row);
    let _ = EndPaint(hwnd, &ps);
}

// ---------------------------- overlay thread ----------------------------

struct Popup {
    hwnd: HWND,
    rect: RECT,
    /// Index into the current SquiggleInfo list this popup belongs to.
    info_idx: usize,
    shown: bool,
}

fn run(rx: Receiver<Vec<SquiggleInfo>>, fix_tx: Sender<FixRequest>) {
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
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
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
            if clicked >= 0 && popup.shown {
                if let Some(info) = infos.get(popup.info_idx) {
                    if let Some(replacement) = info.suggestions.get(clicked as usize) {
                        let _ = fix_tx.send(FixRequest {
                            start: info.start,
                            end: info.end,
                            expected: info.expected.clone(),
                            replacement: replacement.clone(),
                        });
                    }
                }
                hide_popup(&mut popup);
                hover_since = None;
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
    let rows: Vec<String> = info.suggestions.iter().take(3).cloned().collect();
    let mut message = info.message.clone();
    if message.chars().count() > 42 {
        message = message.chars().take(41).collect::<String>() + "…";
    }
    if let Ok(mut r) = popup_rows().lock() {
        *r = rows.clone();
    }
    if let Ok(mut m) = popup_msg().lock() {
        *m = message;
    }
    HOVER_ROW.store(-1, Ordering::Relaxed);

    let height = ROWS_TOP + rows.len() as i32 * ROW_H + PAD;
    let x = info.x;
    let mut y = info.y - height - 6;
    if y < 0 {
        y = info.y + info.h + SQUIGGLE_H + 2;
    }
    let _ = MoveWindow(popup.hwnd, x, y, POPUP_W, height, true);
    let rgn = CreateRoundRectRgn(0, 0, POPUP_W + 1, height + 1, 12, 12);
    let _ = SetWindowRgn(popup.hwnd, rgn, true);
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
    let _ = InvalidateRect(popup.hwnd, None, true);
}

unsafe fn hide_popup(popup: &mut Popup) {
    let _ = ShowWindow(popup.hwnd, SW_HIDE);
    popup.shown = false;
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
    // Straight line: 2px thick, drawn on the top 2 rows (rows 0 and 1), full width.
    for cx in 0..w {
        px[cx as usize] = color;
        px[(w + cx) as usize] = color;
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
