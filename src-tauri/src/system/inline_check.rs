// Inline proofreading watcher (Grammarly Phase 2): polls the focused UI
// element in ANY app via UI Automation, runs Harper on its text, and drives
// the squiggle overlay (system/squiggle.rs). Rects are re-read every cycle so
// squiggles follow scrolling, typing, and window moves.
//
// Design constraints learned from the standalone prototype (uia-probe/):
// - UIA client threads must be MTA (COINIT_MULTITHREADED).
// - Harper spans are CHAR indices; UIA moves by TextUnit_Character — these
//   align for the English text Harper supports.
// - GetBoundingRectangles returns only the VISIBLE rects (scrolled-away text
//   yields none), so clipping is free.

use crate::proofread;
use crate::AppState;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetUBound, SafeArrayUnaccessData,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_TextPatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

// Squiggling a terminal's scrollback is noise, and password managers are none
// of our business. Everything else is fair game.
const IGNORE_EXES: &[&str] = &[
    "windowsterminal.exe",
    "conhost.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "keepass.exe",
    "1password.exe",
    "bitwarden.exe",
];
const MAX_TEXT: i32 = 6000;
const POLL_MS: u64 = 400;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || watcher(app));
}

fn watcher(app: AppHandle) {
    unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            crate::logging::log_error("inline_check", "CoInitializeEx failed");
            return;
        }
        let automation: IUIAutomation =
            match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                Ok(a) => a,
                Err(e) => {
                    crate::logging::log_error("inline_check", &format!("UIA init failed: {e}"));
                    return;
                }
            };
        let overlay_tx = super::squiggle::spawn();
        let my_pid = GetCurrentProcessId();

        let mut last_text = String::new();
        let mut issues: Vec<proofread::ProofIssue> = Vec::new();
        let mut was_active = false;
        loop {
            std::thread::sleep(Duration::from_millis(POLL_MS));

            let (enabled, vocabulary) = {
                let state = app.state::<AppState>();
                let cfg = match state.config.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                (cfg.inline_proofread, cfg.vocabulary.clone())
            };
            if !enabled {
                if was_active {
                    let _ = overlay_tx.send(Vec::new());
                    was_active = false;
                    last_text.clear();
                    issues.clear();
                }
                std::thread::sleep(Duration::from_millis(600));
                continue;
            }

            let squiggles = poll_once(
                &automation,
                my_pid,
                &vocabulary,
                &mut last_text,
                &mut issues,
            );
            let active = !squiggles.is_empty();
            if active || was_active {
                let _ = overlay_tx.send(squiggles);
            }
            was_active = active;
        }
    }
}

fn poll_once(
    automation: &IUIAutomation,
    my_pid: u32,
    vocabulary: &str,
    last_text: &mut String,
    issues: &mut Vec<proofread::ProofIssue>,
) -> Vec<super::squiggle::Squiggle> {
    unsafe {
        // Never squiggle our own dashboard (its WebView2 child has a different
        // pid, so check the foreground WINDOW's owner, not the element's).
        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut fg_pid));
        if fg_pid == my_pid {
            return Vec::new();
        }

        let el: IUIAutomationElement = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let pid = el.CurrentProcessId().unwrap_or(0) as u32;
        if pid == my_pid {
            return Vec::new();
        }
        if el.CurrentIsPassword().map(|b| b.as_bool()).unwrap_or(false) {
            return Vec::new();
        }
        let exe = process_name(pid);
        if IGNORE_EXES.contains(&exe.as_str()) {
            return Vec::new();
        }
        let pattern: IUIAutomationTextPattern = match el
            .GetCurrentPattern(UIA_TextPatternId)
            .and_then(|unk| unk.cast())
        {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        let doc = match pattern.DocumentRange() {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        let text = match doc.GetText(MAX_TEXT) {
            Ok(t) => t.to_string(),
            Err(_) => return Vec::new(),
        };
        if text.trim().is_empty() {
            last_text.clear();
            issues.clear();
            return Vec::new();
        }
        // Re-lint only when the text actually changed; rects refresh every poll.
        if text != *last_text {
            *issues = proofread::check(&text, vocabulary);
            *last_text = text;
        }
        let mut squiggles = Vec::new();
        for issue in issues.iter() {
            if let Ok(rects) = issue_rects(&doc, issue) {
                for (x, y, w, h) in rects {
                    if w < 2.0 || h < 2.0 {
                        continue;
                    }
                    squiggles.push(super::squiggle::Squiggle {
                        x: x as i32,
                        y: (y + h - 2.0) as i32,
                        w: w as i32,
                        spelling: issue.kind.contains("Spell"),
                    });
                }
            }
        }
        squiggles
    }
}

/// Map one issue's char range to its visible screen rectangles.
fn issue_rects(
    doc: &IUIAutomationTextRange,
    issue: &proofread::ProofIssue,
) -> windows::core::Result<Vec<(f64, f64, f64, f64)>> {
    unsafe {
        let r = doc.Clone()?;
        r.MoveEndpointByRange(TextPatternRangeEndpoint_End, doc, TextPatternRangeEndpoint_Start)?;
        r.MoveEndpointByUnit(
            TextPatternRangeEndpoint_Start,
            TextUnit_Character,
            issue.start as i32,
        )?;
        r.MoveEndpointByUnit(
            TextPatternRangeEndpoint_End,
            TextUnit_Character,
            (issue.end - issue.start) as i32,
        )?;
        Ok(read_rects(r.GetBoundingRectangles()?))
    }
}

fn read_rects(sa: *mut SAFEARRAY) -> Vec<(f64, f64, f64, f64)> {
    let mut out = Vec::new();
    if sa.is_null() {
        return out;
    }
    unsafe {
        if let Ok(ubound) = SafeArrayGetUBound(sa, 1) {
            let count = (ubound + 1) as usize;
            let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
            if SafeArrayAccessData(sa, &mut data).is_ok() {
                let vals = std::slice::from_raw_parts(data as *const f64, count);
                for q in vals.chunks_exact(4) {
                    out.push((q[0], q[1], q[2], q[3]));
                }
                let _ = SafeArrayUnaccessData(sa);
            }
        }
        let _ = SafeArrayDestroy(sa);
    }
    out
}

fn process_name(pid: u32) -> String {
    unsafe {
        let handle: HANDLE = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return String::new(),
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let name = if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
        {
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            full.rsplit(['\\', '/'])
                .next()
                .unwrap_or(&full)
                .to_lowercase()
        } else {
            String::new()
        };
        let _ = CloseHandle(handle);
        name
    }
}
