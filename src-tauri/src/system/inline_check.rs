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

use super::squiggle::{OverlayAction, SquiggleInfo};
use crate::proofread;
use crate::AppState;
use enigo::{Direction::Release, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::collections::HashSet;
use std::sync::mpsc::channel;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{implement, Interface};
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
    IUIAutomationTextRange, IUIAutomationValuePattern, SupportedTextSelection_None,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Character,
    UIA_TextPatternId, UIA_ValuePatternId, TreeScope_Descendants, UIA_HasKeyboardFocusPropertyId,
    IUIAutomationFocusChangedEventHandler, IUIAutomationFocusChangedEventHandler_Impl,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SystemParametersInfoW, SPI_SETSCREENREADER,
    SPIF_SENDCHANGE,
};

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
const POLL_MS: u64 = 250;
// Backoff when nothing has changed for IDLE_AFTER_POLLS cycles (~2s):
// squiggle positions and text are stable, so poll gently until the focused
// window changes or something moves again.
const IDLE_POLL_MS: u64 = 1200;
const IDLE_AFTER_POLLS: u32 = 8;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || watcher(app));
}

/// Undo the system-wide screen-reader flag we set to activate WebView2
/// accessibility (WhatsApp etc.), so other apps don't stay in accessibility
/// mode after Silent Voice exits.
pub fn reset_screen_reader() {
    unsafe {
        let _ = SystemParametersInfoW(SPI_SETSCREENREADER, 0, None, SPIF_SENDCHANGE);
    }
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
        let handler: IUIAutomationFocusChangedEventHandler = FocusHandler.into();

        let (action_tx, action_rx) = channel::<OverlayAction>();
        let overlay_tx = super::squiggle::spawn(action_tx);
        let my_pid = GetCurrentProcessId();

        let mut dismissed_words = HashSet::<String>::new();
        let mut last_text = String::new();
        let mut last_chars: Vec<char> = Vec::new();
        let mut last_rules: Vec<String> = Vec::new();
        let mut issues: Vec<proofread::ProofIssue> = Vec::new();
        // Adaptive backoff: after several polls with nothing changing, slow
        // down to save CPU; snap back to fast on any change or focus switch.
        let mut idle_polls: u32 = 0;
        let mut last_squiggles: Vec<SquiggleInfo> = Vec::new();
        let mut last_fg: isize = 0;
        let mut was_active = false;
        // The screen-reader flag + focus handler make every Chromium/Electron
        // app build accessibility trees (needed for WebView2 apps like
        // WhatsApp, but a system-wide CPU/memory cost) — so they're only
        // engaged while the user's toggle is actually ON.
        let mut a11y_engaged = false;
        loop {
            let timeout = Duration::from_millis(if idle_polls >= IDLE_AFTER_POLLS {
                IDLE_POLL_MS
            } else {
                POLL_MS
            });

            let mut actions = Vec::new();
            match action_rx.recv_timeout(timeout) {
                Ok(action) => {
                    actions.push(action);
                    while let Ok(a) = action_rx.try_recv() {
                        actions.push(a);
                    }
                }
                // Overlay thread gone → recv_timeout returns Disconnected
                // instantly; sleep instead or this loop busy-spins a core.
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(timeout);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }

            // Focus moved to another window → wake up immediately.
            let fg_now = GetForegroundWindow().0 as isize;
            if fg_now != last_fg {
                last_fg = fg_now;
                idle_polls = 0;
            }

            // Process any incoming overlay actions.
            let mut check_needed = false;
            for action in actions {
                match action {
                    OverlayAction::Fix { start, end, expected, replacement } => {
                        crate::logging::log_info(
                            "inline_check",
                            &format!("fix request: {:?} -> {:?}", expected, replacement),
                        );
                        if let Err(e) = apply_fix(&automation, start, end, &expected, &replacement) {
                            crate::logging::log_error("inline_check", &format!("fix failed: {e}"));
                        }
                        check_needed = true;
                    }
                    OverlayAction::Dismiss { word } => {
                        crate::logging::log_info(
                            "inline_check",
                            &format!("dismiss request: {:?}", word),
                        );
                        dismissed_words.insert(word.to_lowercase());
                        check_needed = true;
                    }
                    OverlayAction::AddToVocab { word } => {
                        crate::logging::log_info(
                            "inline_check",
                            &format!("add to vocab request: {:?}", word),
                        );
                        let lower = word.to_lowercase();
                        dismissed_words.insert(lower);
                        if let Err(e) = app.emit("proofread://add-vocab", &word) {
                            crate::logging::log_error("inline_check", &format!("failed to emit add-vocab: {e}"));
                        }
                        check_needed = true;
                    }
                }
            }
            if check_needed {
                last_text.clear();
                issues.clear();
            }

            let (enabled, vocabulary, disabled_rules, ignore_apps) = {
                let state = app.state::<AppState>();
                let cfg = match state.config.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                (
                    cfg.inline_proofread,
                    cfg.vocabulary.clone(),
                    cfg.proofread_disabled_rules.clone(),
                    cfg.proofread_ignore_apps.clone(),
                )
            };
            // Rule toggles changed → force a re-lint of the unchanged text.
            if disabled_rules != last_rules {
                last_rules = disabled_rules.clone();
                last_text.clear();
            }
            if !enabled {
                if was_active {
                    let _ = overlay_tx.send(Vec::new());
                    was_active = false;
                    last_text.clear();
                    issues.clear();
                }
                if a11y_engaged {
                    let _ = automation.RemoveFocusChangedEventHandler(&handler);
                    reset_screen_reader();
                    a11y_engaged = false;
                }
                std::thread::sleep(Duration::from_millis(600));
                continue;
            }
            if !a11y_engaged {
                let _ = SystemParametersInfoW(SPI_SETSCREENREADER, 1, None, SPIF_SENDCHANGE);
                let _ = automation.AddFocusChangedEventHandler(None, &handler);
                a11y_engaged = true;
            }

            // A panic anywhere in the poll (Harper, GECToR, UIA) must not
            // kill this thread — squiggles going permanently dead is worse
            // than one skipped cycle. Log it so the cause stays visible.
            let (squiggles, reason) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                poll_once(
                    &automation,
                    my_pid,
                    &vocabulary,
                    &disabled_rules,
                    &ignore_apps,
                    &dismissed_words,
                    &mut last_text,
                    &mut last_chars,
                    &mut issues,
                )
            })) {
                Ok(r) => r,
                Err(p) => {
                    crate::logging::log_error(
                        "inline_check",
                        &format!("poll panicked: {}", crate::logging::panic_msg(&*p)),
                    );
                    last_text.clear();
                    issues.clear();
                    (Vec::new(), "panic")
                }
            };
            idle_polls = if squiggles == last_squiggles && !check_needed {
                idle_polls.saturating_add(1)
            } else {
                0
            };
            last_squiggles = squiggles.clone();
            let active = !squiggles.is_empty();
            let _ = reason;
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
    disabled_rules: &[String],
    ignore_apps: &[String],
    dismissed_words: &HashSet<String>,
    last_text: &mut String,
    last_chars: &mut Vec<char>,
    issues: &mut Vec<proofread::ProofIssue>,
) -> (Vec<SquiggleInfo>, &'static str) {
    unsafe {
        // Never squiggle our own dashboard (its WebView2 child has a different
        // pid, so check the foreground WINDOW's owner, not the element's).
        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut fg_pid));
        if fg_pid == my_pid {
            return (Vec::new(), "own process foreground");
        }

        let el: IUIAutomationElement = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(_) => return (Vec::new(), "no focused element"),
        };
        let pid = el.CurrentProcessId().unwrap_or(0) as u32;
        if pid == my_pid {
            return (Vec::new(), "own element focused");
        }
        if el.CurrentIsPassword().map(|b| b.as_bool()).unwrap_or(false) {
            return (Vec::new(), "password field");
        }
        let exe = process_name(pid);
        if IGNORE_EXES.contains(&exe.as_str()) {
            return (Vec::new(), "ignored exe");
        }
        // User's own "don't check in these apps" list (lowercase substrings).
        if ignore_apps.iter().any(|a| exe.contains(a.as_str())) {
            return (Vec::new(), "user-ignored exe");
        }
        let el = match resolve_text_element(automation, &el) {
            Some(e) => e,
            None => return (Vec::new(), "no text element"),
        };
        let pattern: IUIAutomationTextPattern = match el
            .GetCurrentPattern(UIA_TextPatternId)
            .and_then(|unk| unk.cast())
        {
            Ok(p) => p,
            Err(_) => return (Vec::new(), "no text pattern"),
        };
        // Editability gate.
        let mut editable = false;
        if let Ok(vp) = el
            .GetCurrentPattern(UIA_ValuePatternId)
            .and_then(|unk| unk.cast::<IUIAutomationValuePattern>())
        {
            // Form control (input/textarea, Notepad, WPF Edit): editable unless read-only.
            if vp.CurrentIsReadOnly().map(|b| b.as_bool()) == Ok(true) {
                return (Vec::new(), "read-only");
            }
            editable = true;
        }
        if !editable {
            // No ValuePattern: contenteditable / rich editor (e.g. ProseMirror,
            // which does not implement TextPattern2). Treat it as editable only
            // if the text supports selection -- static read-only labels report
            // SupportedTextSelection_None. Read-only browser documents are
            // already rejected above via ValuePattern IsReadOnly.
            match pattern.SupportedTextSelection() {
                Ok(sel) if sel != SupportedTextSelection_None => {}
                _ => return (Vec::new(), "not selectable"),
            }
        }
        let doc = match pattern.DocumentRange() {
            Ok(d) => d,
            Err(_) => return (Vec::new(), "no document range"),
        };
        let text = match doc.GetText(MAX_TEXT) {
            Ok(t) => t.to_string(),
            Err(_) => return (Vec::new(), "GetText failed"),
        };
        if text.trim().is_empty() {
            last_text.clear();
            last_chars.clear();
            issues.clear();
            return (Vec::new(), "empty text");
        }
        // Re-lint only when the text actually changed; rects refresh every poll.
        if text != *last_text {
            *issues = proofread::check(&text, vocabulary, disabled_rules);
            *last_chars = text.chars().collect();
            *last_text = text;
        }
        let chars: &Vec<char> = last_chars;
        let mut squiggles = Vec::new();
        for issue in issues.iter() {
            // The overlay draws at most MAX_SQUIGGLES; fetching rects (COM
            // calls) for more would also leave invisible squiggles hoverable.
            if squiggles.len() >= super::squiggle::MAX_SQUIGGLES {
                break;
            }
            let expected: String = chars
                .get(issue.start..issue.end)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            if dismissed_words.contains(&expected.to_lowercase()) {
                continue;
            }
            if let Some(rects) = issue_rects(&doc, chars, issue) {
                for (x, y, w, h) in rects {
                    if squiggles.len() >= super::squiggle::MAX_SQUIGGLES {
                        break;
                    }
                    if w < 2.0 || h < 2.0 {
                        continue;
                    }
                    // Two lints on the same span (e.g. spelling + style) would
                    // stack identical strips — keep the first.
                    if squiggles
                        .iter()
                        .any(|s: &SquiggleInfo| s.x == x as i32 && s.y == y as i32 && s.w == w as i32)
                    {
                        continue;
                    }
                    squiggles.push(SquiggleInfo {
                        x: x as i32,
                        y: y as i32,
                        w: w as i32,
                        h: h as i32,
                        spelling: issue.kind.contains("Spell"),
                        message: issue.message.clone(),
                        suggestions: issue.suggestions.clone(),
                        start: issue.start,
                        end: issue.end,
                        expected: expected.clone(),
                    });
                }
            }
        }
        if squiggles.is_empty() {
            (squiggles, "no visible issue rects")
        } else {
            (squiggles, "active")
        }
    }
}

/// Replace the flagged char range in the focused field with the clicked
/// suggestion: select the range via UIA, then type the replacement (typed
/// text replaces a selection — preserves the app's undo stack and never
/// touches the clipboard). The popup is WS_EX_NOACTIVATE, so the target
/// field still owns focus and receives the synthetic input.
fn apply_fix(
    automation: &IUIAutomation,
    start: usize,
    end: usize,
    expected: &str,
    replacement: &str,
) -> Result<(), String> {
    unsafe {
        let el: IUIAutomationElement = automation
            .GetFocusedElement()
            .map_err(|e| format!("no focused element: {e}"))?;
        let el = resolve_text_element(automation, &el).ok_or("no text element")?;
        let pattern: IUIAutomationTextPattern = el
            .GetCurrentPattern(UIA_TextPatternId)
            .and_then(|unk| unk.cast())
            .map_err(|e| format!("no text pattern: {e}"))?;
        let doc = pattern.DocumentRange().map_err(|e| e.to_string())?;

        // The text may have changed between the popup opening and the click —
        // verify the range still holds exactly the word we flagged.
        let text = doc
            .GetText(MAX_TEXT)
            .map_err(|e| e.to_string())?
            .to_string();
        let chars: Vec<char> = text.chars().collect();
        let current: String = chars
            .get(start..end)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        if current != expected {
            return Err(format!(
                "text changed under fix (expected {:?}, found {:?})",
                expected, current
            ));
        }

        let r = range_for(&doc, &chars, start, end)
            .ok_or("could not build a verified range for the fix")?;
        r.Select().map_err(|e| format!("select failed: {e}"))?;

        // Let the selection settle before the synthetic keystrokes arrive
        // (Chromium applies UIA selections asynchronously).
        std::thread::sleep(Duration::from_millis(40));
        let mut enigo =
            Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;
        // A physically held Ctrl/Shift/Alt would turn the typed chars into
        // shortcuts — release them before typing.
        for key in [Key::Control, Key::Shift, Key::Alt] {
            let _ = enigo.key(key, Release);
        }
        
        if replacement.is_empty() {
            enigo.key(Key::Delete, enigo::Direction::Click).map_err(|e| e.to_string())?;
        } else {
            // Type char-by-char with a small gap: web apps drop synthetic input
            // that arrives faster than their event loop reconciles.
            for ch in replacement.chars() {
                enigo.text(&ch.to_string()).map_err(|e| e.to_string())?;
                std::thread::sleep(Duration::from_millis(8));
            }
        }
        Ok(())
    }
}

/// How many complete "\r\n" pairs sit fully before char index `idx`.
/// Harper spans count CRLF as two chars; some UIA providers (WPF, RichEdit)
/// move TextUnit_Character over it as ONE — offsets drift by one per line.
fn crlf_pairs_before(chars: &[char], idx: usize) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i + 1 < idx.min(chars.len()) {
        if chars[i] == '\r' && chars[i + 1] == '\n' {
            n += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    n
}

/// Build a UIA range for chars `start..end`, verified by reading the range's
/// text back and comparing to what we flagged. Tries the CRLF-collapsed
/// offset first (WPF/RichEdit behavior), then the raw char offset.
fn range_for(
    doc: &IUIAutomationTextRange,
    chars: &[char],
    start: usize,
    end: usize,
) -> Option<IUIAutomationTextRange> {
    let expected: String = chars.get(start..end)?.iter().collect();
    let d_start = crlf_pairs_before(chars, start);
    let d_end = crlf_pairs_before(chars, end);
    let mut candidates = vec![(start - d_start, end - d_end)];
    if d_start != 0 || d_end != 0 {
        candidates.push((start, end));
    }
    unsafe {
        for (s, e) in candidates {
            let Ok(r) = doc.Clone() else { continue };
            if r.MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                doc,
                TextPatternRangeEndpoint_Start,
            )
            .is_err()
            {
                continue;
            }
            if r.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, s as i32)
                .is_err()
                || r.MoveEndpointByUnit(
                    TextPatternRangeEndpoint_End,
                    TextUnit_Character,
                    (e - s) as i32,
                )
                .is_err()
            {
                continue;
            }
            if let Ok(got) = r.GetText(256) {
                if got.to_string() == expected {
                    return Some(r);
                }
            }
        }
    }
    None
}

/// Map one issue's char range to its visible screen rectangles.
fn issue_rects(
    doc: &IUIAutomationTextRange,
    chars: &[char],
    issue: &proofread::ProofIssue,
) -> Option<Vec<(f64, f64, f64, f64)>> {
    let r = range_for(doc, chars, issue.start, issue.end)?;
    unsafe { r.GetBoundingRectangles().ok().map(read_rects) }
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

#[implement(IUIAutomationFocusChangedEventHandler)]
struct FocusHandler;

impl IUIAutomationFocusChangedEventHandler_Impl for FocusHandler_Impl {
    fn HandleFocusChangedEvent(&self, _sender: Option<&IUIAutomationElement>) -> windows::core::Result<()> {
        Ok(())
    }
}

// When the focused element has no TextPattern (e.g. a WebView2/WinUI3 host
// like WhatsApp), the real editable field is a descendant with keyboard
// focus. Resolve to it; otherwise return the element itself.
unsafe fn resolve_text_element(automation: &IUIAutomation, el: &IUIAutomationElement) -> Option<IUIAutomationElement> {
    if el.GetCurrentPattern(UIA_TextPatternId).and_then(|u| u.cast::<IUIAutomationTextPattern>()).is_ok() {
        return Some(el.clone());
    }
    // find the descendant with keyboard focus (the compose box)
    let cond = automation.CreatePropertyCondition(UIA_HasKeyboardFocusPropertyId, &windows::core::VARIANT::from(true)).ok()?;
    let found = el.FindFirst(TreeScope_Descendants, &cond).ok()?;
    // must expose a TextPattern to be usable
    if found.GetCurrentPattern(UIA_TextPatternId).and_then(|u| u.cast::<IUIAutomationTextPattern>()).is_ok() {
        Some(found)
    } else { None }
}
