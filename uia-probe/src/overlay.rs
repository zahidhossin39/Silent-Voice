// Squiggle overlay: a pool of tiny click-through layered windows, one per
// flagged word (Grammarly's own architecture — avoids a fullscreen overlay,
// which is expensive on integrated GPUs and, on this machine, WebView2
// transparent windows are known-broken anyway; these are pure Win32).

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, RegisterClassW, ShowWindow,
    TranslateMessage, UpdateLayeredWindow, MSG, PM_REMOVE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

const SQUIGGLE_H: i32 = 4;
const MAX_SQUIGGLES: usize = 24;
const RED: u32 = 0xFFE5484D; // spelling (premultiplied BGRA as 0xAARRGGBB)
const BLUE: u32 = 0xFF3B82F6; // grammar/style

#[derive(Clone)]
pub struct Squiggle {
    pub x: i32,
    pub y: i32, // top of the strip (caller anchors it to the word's baseline)
    pub w: i32,
    pub spelling: bool,
}

pub fn spawn() -> Sender<Vec<Squiggle>> {
    let (tx, rx) = channel::<Vec<Squiggle>>();
    std::thread::spawn(move || run(rx));
    tx
}

fn run(rx: Receiver<Vec<Squiggle>>) {
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        let class_name = w!("SVSquiggleProbe");
        unsafe extern "system" fn wndproc(
            hwnd: HWND,
            msg: u32,
            wp: windows::Win32::Foundation::WPARAM,
            lp: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::LRESULT {
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let mut pool: Vec<HWND> = Vec::new();
        loop {
            // Pump messages for the windows this thread owns.
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            // Only the newest update matters.
            let mut latest: Option<Vec<Squiggle>> = None;
            while let Ok(v) = rx.try_recv() {
                latest = Some(v);
            }
            if let Some(squiggles) = latest {
                apply(&mut pool, hinst.into(), class_name, &squiggles);
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

unsafe fn apply(
    pool: &mut Vec<HWND>,
    hinst: windows::Win32::Foundation::HINSTANCE,
    class_name: windows::core::PCWSTR,
    squiggles: &[Squiggle],
) {
    let show = squiggles.len().min(MAX_SQUIGGLES);
    while pool.len() < show {
        let hwnd = CreateWindowExW(
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
        )
        .unwrap();
        pool.push(hwnd);
    }
    for (i, s) in squiggles.iter().take(show).enumerate() {
        let w = s.w.clamp(4, 600);
        draw_squiggle(pool[i], s.x, s.y, w, if s.spelling { RED } else { BLUE });
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
    // Classic proofing wave: 4px period, 2px thick.
    const WAVE: [i32; 4] = [2, 1, 0, 1];
    for cx in 0..w {
        let cy = WAVE[(cx % 4) as usize];
        px[(cy * w + cx) as usize] = color;
        if cy + 1 < h {
            px[((cy + 1) * w + cx) as usize] = color;
        }
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
