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
    BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DeleteDC, DeleteObject, EndPaint, FillRect, GetDC, InvalidateRect,
    ReleaseDC, SelectObject, SetBkMode, SetTextColor, TextOutW, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, FW_NORMAL, FW_SEMIBOLD,
    HBITMAP, HFONT, PAINTSTRUCT, TRANSPARENT, SetWindowRgn, CreatePen, Ellipse, PS_SOLID,
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
const POPUP_W: i32 = 300;
const PAD: i32 = 14;
const HEADER_H: i32 = 20;
const MSG_H: i32 = 20;
const ROW_H: i32 = 38;
const ACTION_H: i32 = 30;
const ROWS_TOP: i32 = PAD + HEADER_H + 2 + MSG_H + 12;
const BG: u32 = 0x00ffffff; // white
const TXT_MUTED: u32 = 0x00707070;
const TXT_MAIN: u32 = 0x001a1a1a;
const TXT_ACCENT: u32 = 0x001673f9; // orange #f97316 as BGR
const TXT_ACTION: u32 = 0x00909090;

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
        ROWS_TOP + n_rows * ROW_H + 9 + 2 * ACTION_H + PAD
    }
}

fn row_at(y: i32) -> i32 {
    let n = popup_rows().lock().map(|r| r.len() as i32).unwrap_or(0);
    for i in 0..n {
        let top = ROWS_TOP + i * ROW_H;
        if y >= top && y < top + ROW_H {
            return i;
        }
    }
    if PICKER.load(Ordering::Relaxed) {
        return -1;
    }
    let action_start = ROWS_TOP + n * ROW_H + 9;
    if y >= action_start && y < action_start + ACTION_H {
        return n;
    }
    if y >= action_start + ACTION_H && y < action_start + 2 * ACTION_H {
        return n + 1;
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
    let n = rows.len() as i32;
    let is_picker = PICKER.load(Ordering::Relaxed);
    let height = get_popup_height(n, is_picker);

    let bg = CreateSolidBrush(COLORREF(BG));
    FillRect(hdc, &RECT { left: 0, top: 0, right: POPUP_W, bottom: height }, bg);
    let _ = DeleteObject(bg);

    // Draw a 1px border inside the card (color 0x00dcdcdc)
    let border_color = CreateSolidBrush(COLORREF(0x00dcdcdc));
    FillRect(hdc, &RECT { left: 0, top: 0, right: POPUP_W, bottom: 1 }, border_color);
    FillRect(hdc, &RECT { left: 0, top: height - 1, right: POPUP_W, bottom: height }, border_color);
    FillRect(hdc, &RECT { left: 0, top: 0, right: 1, bottom: height }, border_color);
    FillRect(hdc, &RECT { left: POPUP_W - 1, top: 0, right: POPUP_W, bottom: height }, border_color);
    let _ = DeleteObject(border_color);

    SetBkMode(hdc, TRANSPARENT);
    let font_msg: HFONT = CreateFontW(
        -14, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_row: HFONT = CreateFontW(
        -17, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );
    let font_hdr: HFONT = CreateFontW(
        -13, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
    );

    let old = SelectObject(hdc, font_msg);

    // Draw Header
    let (hdr_label, hdr_color) = if is_picker {
        ("Dictionary", TXT_ACCENT)
    } else {
        if POPUP_SPELLING.load(Ordering::Relaxed) {
            ("Spelling", 0x004444ef)
        } else {
            ("Grammar", 0x00f6823b)
        }
    };

    let dot_y_center = PAD + HEADER_H / 2;
    let dot_brush = CreateSolidBrush(COLORREF(hdr_color));
    let dot_pen = CreatePen(PS_SOLID, 1, COLORREF(hdr_color));
    let old_brush = SelectObject(hdc, dot_brush);
    let old_pen = SelectObject(hdc, dot_pen);
    let _ = Ellipse(hdc, PAD, dot_y_center - 4, PAD + 8, dot_y_center + 4);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(dot_brush);
    let _ = DeleteObject(dot_pen);

    SelectObject(hdc, font_hdr);
    SetTextColor(hdc, COLORREF(hdr_color));
    let hdr_utf16: Vec<u16> = hdr_label.encode_utf16().collect();
    let _ = TextOutW(hdc, PAD + 16, PAD + 2, &hdr_utf16);

    // Draw Message
    SelectObject(hdc, font_msg);
    SetTextColor(hdc, COLORREF(TXT_MUTED));
    let msg_utf16: Vec<u16> = message.encode_utf16().collect();
    let _ = TextOutW(hdc, PAD, PAD + HEADER_H + 2, &msg_utf16);

    // Draw Separator 1
    let sep_color = CreateSolidBrush(COLORREF(0x00ececec));
    let sep_y = PAD + HEADER_H + 2 + MSG_H + 6;
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

    // Draw Suggestion rows
    SelectObject(hdc, font_row);
    for (i, row) in rows.iter().enumerate() {
        let top = ROWS_TOP + i as i32 * ROW_H;
        let is_hovered = i as i32 == hover;
        if is_hovered {
            let hb = CreateSolidBrush(COLORREF(TXT_ACCENT)); // Solid orange
            FillRect(
                hdc,
                &RECT { left: 6, top, right: POPUP_W - 6, bottom: top + ROW_H },
                hb,
            );
            let _ = DeleteObject(hb);
            SetTextColor(hdc, COLORREF(0x00ffffff)); // White
        } else {
            SetTextColor(hdc, COLORREF(TXT_MAIN)); // Text main
        }
        let row_utf16: Vec<u16> = row.encode_utf16().collect();
        let text_y = top + (ROW_H - 22) / 2;
        let _ = TextOutW(hdc, PAD, text_y, &row_utf16);
    }

    if !is_picker {
        // Draw second separator
        let sep2_color = CreateSolidBrush(COLORREF(0x00e8e8e8));
        let sep2_y = ROWS_TOP + n * ROW_H + 4;
        FillRect(
            hdc,
            &RECT {
                left: PAD,
                top: sep2_y,
                right: POPUP_W - PAD,
                bottom: sep2_y + 1,
            },
            sep2_color,
        );
        let _ = DeleteObject(sep2_color);

        // Draw Action rows
        SelectObject(hdc, font_msg);
        let action_start = ROWS_TOP + n * ROW_H + 9;

        // Row n: Dismiss
        {
            let top = action_start;
            if hover == n {
                let hb = CreateSolidBrush(COLORREF(0x00f2f2f2));
                FillRect(
                    hdc,
                    &RECT { left: 6, top, right: POPUP_W - 6, bottom: top + ACTION_H },
                    hb,
                );
                let _ = DeleteObject(hb);
                SetTextColor(hdc, COLORREF(0x00333333));
            } else {
                SetTextColor(hdc, COLORREF(TXT_ACTION));
            }
            let txt_utf16: Vec<u16> = "Dismiss".encode_utf16().collect();
            let text_y = top + (ACTION_H - 16) / 2;
            let _ = TextOutW(hdc, PAD, text_y, &txt_utf16);
        }

        // Row n+1: Add to dictionary
        {
            let top = action_start + ACTION_H;
            if hover == n + 1 {
                let hb = CreateSolidBrush(COLORREF(0x00f2f2f2));
                FillRect(
                    hdc,
                    &RECT { left: 6, top, right: POPUP_W - 6, bottom: top + ACTION_H },
                    hb,
                );
                let _ = DeleteObject(hb);
                SetTextColor(hdc, COLORREF(0x00333333));
            } else {
                SetTextColor(hdc, COLORREF(TXT_ACTION));
            }
            let txt_utf16: Vec<u16> = "Add to dictionary".encode_utf16().collect();
            let text_y = top + (ACTION_H - 16) / 2;
            let _ = TextOutW(hdc, PAD, text_y, &txt_utf16);
        }
    }

    SelectObject(hdc, old);
    let _ = DeleteObject(font_msg);
    let _ = DeleteObject(font_row);
    let _ = DeleteObject(font_hdr);
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
                    let is_picker = PICKER.load(Ordering::Relaxed);
                    if is_picker {
                        let rows = popup_rows().lock().map(|r| r.clone()).unwrap_or_default();
                        let n_rows = rows.len() as i32;
                        if clicked < n_rows {
                            if let Some(word) = rows.get(clicked as usize) {
                                let _ = action_tx.send(OverlayAction::AddToVocab {
                                    word: word.clone(),
                                });
                            }
                        }
                        hide_popup(&mut popup);
                        hover_since = None;
                    } else {
                        let rows = popup_rows().lock().map(|r| r.clone()).unwrap_or_default();
                        let n = rows.len() as i32;
                        if clicked < n {
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
                        } else if clicked == n {
                            let _ = action_tx.send(OverlayAction::Dismiss {
                                word: info.expected.clone(),
                            });
                            hide_popup(&mut popup);
                            hover_since = None;
                        } else if clicked == n + 1 {
                            PICKER.store(true, Ordering::Relaxed);
                            let mut picker_rows = vec![info.expected.clone()];
                            picker_rows.extend(info.suggestions.iter().take(3).cloned());
                            let message = "Add which word to the dictionary?".to_string();
                            if let Ok(mut r) = popup_rows().lock() {
                                *r = picker_rows.clone();
                            }
                            if let Ok(mut m) = popup_msg().lock() {
                                *m = message;
                            }
                            HOVER_ROW.store(-1, Ordering::Relaxed);

                            let n_rows = picker_rows.len() as i32;
                            let height = get_popup_height(n_rows, true);
                            let x = info.x;
                            let mut y = info.y - height - 6;
                            if y < 0 {
                                y = info.y + info.h + SQUIGGLE_H + 2;
                            }
                            let _ = MoveWindow(popup.hwnd, x, y, POPUP_W, height, true);
                            let rgn = CreateRoundRectRgn(0, 0, POPUP_W + 1, height + 1, 12, 12);
                            let _ = SetWindowRgn(popup.hwnd, rgn, true);
                            popup.rect = RECT { left: x, top: y, right: x + POPUP_W, bottom: y + height };
                            let _ = InvalidateRect(popup.hwnd, None, true);
                        }
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
    if message.chars().count() > 46 {
        message = message.chars().take(45).collect::<String>() + "…";
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
