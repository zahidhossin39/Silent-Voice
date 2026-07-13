// Prototype: system-wide inline proofreading (Grammarly-style squiggles).
// Polls the focused UI element in ANY app via UI Automation, runs Harper on
// its text, and positions tiny squiggle overlay windows under flagged words.
// Rects are re-read every cycle so squiggles follow scrolling and window moves.

mod overlay;

use std::time::Duration;
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
use windows::core::implement;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_TextPatternId, IUIAutomationFocusChangedEventHandler,
    IUIAutomationFocusChangedEventHandler_Impl, TreeScope_Subtree,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_SETSCREENREADER, SPIF_SENDCHANGE,
};

// Apps where squiggling the "document" is nonsense (terminals) or self-referential.
const IGNORE_EXES: &[&str] = &[
    "windowsterminal.exe",
    "conhost.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "claude.exe",
];
const MAX_TEXT: i32 = 6000;

struct Issue {
    start: usize,
    end: usize,
    spelling: bool,
}

fn harper_check(text: &str) -> Vec<Issue> {
    use harper_core::linting::{LintGroup, Linter};
    use harper_core::spell::{FstDictionary, MergedDictionary};
    use harper_core::{Dialect, Document};
    use std::sync::Arc;

    let mut dict = MergedDictionary::new();
    dict.add_dictionary(FstDictionary::curated());
    let dict = Arc::new(dict);
    let doc = Document::new_plain_english(text, &*dict);
    let mut linter = LintGroup::new_curated(dict, Dialect::American);
    linter
        .lint(&doc)
        .into_iter()
        .map(|l| Issue {
            start: l.span.start,
            end: l.span.end,
            spelling: format!("{:?}", l.lint_kind).contains("Spell"),
        })
        .collect()
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

/// Map one issue's char range to visible screen rectangles via UIA.
fn issue_rects(
    doc: &IUIAutomationTextRange,
    issue: &Issue,
) -> windows::core::Result<Vec<(f64, f64, f64, f64)>> {
    unsafe {
        let r = doc.Clone()?;
        // Collapse to document start, then walk endpoints out by char counts.
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

fn poll_once(
    automation: &IUIAutomation,
    my_pid: u32,
    last_text: &mut String,
    issues: &mut Vec<Issue>,
) -> Vec<overlay::Squiggle> {
    unsafe {
        let el: IUIAutomationElement = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let pid = el.CurrentProcessId().unwrap_or(0) as u32;
        if pid == my_pid {
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
            *last_text = text;
            issues.clear();
            return Vec::new();
        }
        // Re-lint only when the text actually changed; rects refresh every poll.
        if text != *last_text {
            *issues = harper_check(&text);
            *last_text = text;
        }
        let mut squiggles = Vec::new();
        for issue in issues.iter() {
            if let Ok(rects) = issue_rects(&doc, issue) {
                for (x, y, w, h) in rects {
                    if w < 2.0 || h < 2.0 {
                        continue;
                    }
                    squiggles.push(overlay::Squiggle {
                        x: x as i32,
                        y: (y + h - 2.0) as i32,
                        w: w as i32,
                        spelling: issue.spelling,
                    });
                }
            }
        }
        squiggles
    }
}

fn main() -> windows::core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--diagnose".to_string()) || args.contains(&"-d".to_string()) {
        return run_diagnose();
    }

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
        let overlay_tx = overlay::spawn();
        let my_pid = GetCurrentProcessId();

        println!("Inline proofreading prototype running (10 min). Type in Notepad or Chrome.");
        let mut last_text = String::new();
        let mut issues: Vec<Issue> = Vec::new();
        for _ in 0..1500 {
            let squiggles = poll_once(&automation, my_pid, &mut last_text, &mut issues);
            let _ = overlay_tx.send(squiggles);
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    Ok(())
}

#[implement(IUIAutomationFocusChangedEventHandler)]
struct FocusHandler;

impl IUIAutomationFocusChangedEventHandler_Impl for FocusHandler_Impl {
    fn HandleFocusChangedEvent(&self, sender: Option<&IUIAutomationElement>) -> windows::core::Result<()> {
        if let Some(el) = sender {
            unsafe {
                let name = el.CurrentName().map(|b| b.to_string()).unwrap_or_default();
                let class_name = el.CurrentClassName().map(|b| b.to_string()).unwrap_or_default();
                println!("[Focus Handler] Focus changed event: Class='{}', Name='{}'", class_name, name);
            }
        }
        Ok(())
    }
}

fn run_diagnose() -> windows::core::Result<()> {
    unsafe {
        println!("============================================================");
        println!("                UIA WEBVIEW2 DIAGNOSTIC MODE                ");
        println!("============================================================");
        
        // 1. Set screen reader flag to TRUE to force Chromium/WebView2 accessibility
        println!("[Diag] Setting SPI_SETSCREENREADER = TRUE...");
        let spi_res = SystemParametersInfoW(
            SPI_SETSCREENREADER,
            1, // TRUE
            None,
            SPIF_SENDCHANGE,
        );
        match spi_res {
            Ok(_) => println!("[Diag] SPI_SETSCREENREADER set successfully."),
            Err(e) => println!("[Diag] Warning: Failed to set SPI_SETSCREENREADER: {:?}", e),
        }

        // Initialize COM UIA
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;

        // 2. Register Focus Changed Event Handler (nudge Chromium activation)
        println!("[Diag] Registering UIA Focus Changed Event Handler...");
        let handler = FocusHandler;
        let handler_interface: IUIAutomationFocusChangedEventHandler = handler.into();
        automation.AddFocusChangedEventHandler(None, &handler_interface)?;
        println!("[Diag] Focus changed handler registered successfully.");

        println!("[Diag] Entering main diagnostic loop (polling focused element every 1.5s).");
        println!("[Diag] Focus on WhatsApp (process WhatsApp.Root) message compose box to test.");
        println!("------------------------------------------------------------");

        loop {
            match automation.GetFocusedElement() {
                Ok(el) => {
                    let name = el.CurrentName().map(|b| b.to_string()).unwrap_or_default();
                    let class_name = el.CurrentClassName().map(|b| b.to_string()).unwrap_or_default();
                    let control_type = el.CurrentControlType().unwrap_or_default();
                    let has_text_pattern = el.GetCurrentPattern(UIA_TextPatternId).is_ok();

                    println!(
                        "[Focused Element] Class: '{}' | Name: '{}' | Type ID: {:?} | HasTextPattern: {}",
                        class_name, name, control_type, has_text_pattern
                    );

                    // Check if it's the WinUI3 WebView2 container/bridge
                    let is_target = class_name.contains("DesktopChildSiteBridge")
                        || class_name.contains("WebView2")
                        || class_name.contains("msedgewebview2")
                        || class_name.contains("Chrome");

                    if is_target && !has_text_pattern {
                        println!("  [Probe] Target element matches WebView2/bridge (no TextPattern).");
                        println!("  [Probe] Walk/Find descendants (Subtree scope) to see if Chromium built the tree...");
                        
                        // We will try polling a few times with a small delay
                        for poll_idx in 1..=5 {
                            std::thread::sleep(Duration::from_millis(300));
                            let condition = automation.CreateTrueCondition()?;
                            
                            match el.FindAll(TreeScope_Subtree, &condition) {
                                Ok(array) => {
                                    let count = array.Length().unwrap_or(0);
                                    println!("    [Poll #{}/5] Found {} descendants", poll_idx, count);
                                    
                                    let mut found_any_text = false;
                                    for i in 0..count {
                                        if let Ok(child) = array.GetElement(i) {
                                            let child_class = child.CurrentClassName().map(|b| b.to_string()).unwrap_or_default();
                                            let child_name = child.CurrentName().map(|b| b.to_string()).unwrap_or_default();
                                            let child_type = child.CurrentControlType().unwrap_or_default();
                                            let child_has_text = child.GetCurrentPattern(UIA_TextPatternId).is_ok();
                                            let child_focus = child.CurrentHasKeyboardFocus().map(|b| b.as_bool()).unwrap_or(false);

                                            // Only the compose box has keyboard focus — this is
                                            // how we isolate it from all the read-only message text.
                                            if child_has_text && child_focus {
                                                println!(
                                                    "    >>> FOCUSED EDITABLE (compose box): Class='{}' Name='{}' Type={:?}",
                                                    child_class, child_name, child_type
                                                );
                                                if let Ok(pat) = child.GetCurrentPattern(UIA_TextPatternId).and_then(|unk| unk.cast::<IUIAutomationTextPattern>()) {
                                                    if let Ok(range) = pat.DocumentRange() {
                                                        if let Ok(text) = range.GetText(200) {
                                                            println!("        Text: {:?}", text.to_string().trim());
                                                        }
                                                    }
                                                }
                                                found_any_text = true;
                                            }
                                        }
                                    }
                                    if found_any_text {
                                        break; // stop polling if we found the active tree elements with text pattern
                                    }
                                }
                                Err(e) => {
                                    println!("    [Poll #{}/5] FindAll failed: {:?}", poll_idx, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[Diag] Failed to get focused element: {:?}", e);
                }
            }
            std::thread::sleep(Duration::from_millis(1500));
        }
    }
}
