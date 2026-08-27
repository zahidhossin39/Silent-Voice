// Inline proofreading watcher for macOS and Linux — the counterpart to
// inline_check.rs on Windows.
//
// The loop is identical on both; only the accessibility primitives differ, so
// the platform backend is selected once here and everything below calls the
// same handful of functions. Both backends expose exactly:
//
//   available()            is accessibility usable right now
//   init() / pump()        one-time setup, and per-cycle event draining
//   focused_field()        the focused editable field, or why not
//   Field::{text, rect, chars(), bounds_for_range()}
//   app_name(&Field)       owning app, for the user's ignore list
//   cursor_position()      pointer in the same screen space as the rects
//   remember/forget        hold the field the popup is offering to fix
//   apply_fix()            apply it, from any thread
//   install_fix_channel() / drain_fixes()   marshalling, where a platform needs it
//
// This is a separate file from inline_check.rs on purpose: that one is 950
// lines of working COM lifetime handling, and threading two more platforms
// through it would put the path that already works at risk for no gain.

#[cfg(target_os = "macos")]
use super::ax as backend;
#[cfg(target_os = "linux")]
use super::atspi as backend;

use super::inline_types::{SquiggleInfo, MAX_SQUIGGLES};
use crate::proofread;
use crate::AppState;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Underlines on screen: poll fast so they track typing and scrolling.
const ACTIVE_POLL_MS: u64 = 90;
/// Editable field focused, nothing flagged: gentler.
const IDLE_POLL_MS: u64 = 400;
/// Nothing editable focused. Neither backend wakes us on focus changes, so this
/// is a heartbeat rather than a sleep-until-event.
const DEEP_IDLE_MS: u64 = 1200;
/// Accessibility unavailable — on macOS the user has not granted it yet, on
/// Linux the bus is not running. Nothing works until that changes.
const UNAVAILABLE_MS: u64 = 3000;

/// Apply the fix the popup is offering. Called from the Tauri command thread;
/// each backend handles its own thread-affinity rules.
pub fn apply_pending_fix(start: usize, end: usize, expected: &str, replacement: &str) -> bool {
    backend::apply_fix(start, end, expected, replacement)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || watcher(app));
}

/// Cursor inside a flagged word, with slack below for the underline itself —
/// the strip sits under the baseline, so an exact rect test would fire only on
/// the text and never on the squiggle the user actually aimed at.
fn hovered<'a>(squiggles: &'a [SquiggleInfo], cx: f64, cy: f64) -> Option<&'a SquiggleInfo> {
    squiggles.iter().find(|s| {
        cx >= s.x as f64
            && cx <= (s.x + s.w) as f64
            && cy >= s.y as f64
            && cy <= (s.y + s.h) as f64 + 6.0
    })
}

fn watcher(app: AppHandle) {
    // Both of these must happen on this thread: AT-SPI confines its objects and
    // its event callbacks to whichever thread pumps the main context.
    let started = backend::init();
    backend::install_fix_channel();

    let mut last_text = String::new();
    let mut issues: Vec<proofread::ProofIssue> = Vec::new();
    let mut last_rules: Vec<String> = Vec::new();
    let mut last_sensitivity = String::new();
    let mut active = false;
    let mut warmed = false;
    let mut warned_unavailable = false;
    // Logged only when it changes — logging every poll would write thousands of
    // lines an hour, which is how the Windows watcher learned to do this.
    let mut last_reason = "";
    // The word the popup is currently showing, so it is not re-shown every poll.
    let mut popup_for: Option<SquiggleInfo> = None;

    loop {
        let (enabled, vocabulary, disabled_rules, gector_sensitivity, ignore_apps) = {
            let state = app.state::<AppState>();
            let cfg = match state.config.lock() {
                Ok(c) => c,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(IDLE_POLL_MS));
                    continue;
                }
            };
            (
                cfg.inline_proofread,
                cfg.vocabulary.clone(),
                cfg.proofread_disabled_rules.clone(),
                cfg.gector_sensitivity.clone(),
                cfg.proofread_ignore_apps.clone(),
            )
        };

        if !enabled {
            if active {
                super::squiggle_overlay::clear(&app);
                super::squiggle_overlay::hide_popup(&app);
                backend::forget();
                active = false;
                last_text.clear();
                issues.clear();
            }
            std::thread::sleep(Duration::from_millis(600));
            continue;
        }

        // Without accessibility every call returns nothing, which would
        // otherwise look exactly like "the feature is broken".
        if !started || !backend::available() {
            if !warned_unavailable {
                warned_unavailable = true;
                crate::logging::log_info(
                    "inline_unix",
                    "inline proofreading is on but accessibility is unavailable",
                );
            }
            std::thread::sleep(Duration::from_millis(UNAVAILABLE_MS));
            continue;
        }
        warned_unavailable = false;

        // Deliver focus events and any queued fix before reading anything.
        backend::pump();
        backend::drain_fixes();

        // Rule toggles changed → re-lint the unchanged text.
        if disabled_rules != last_rules || gector_sensitivity != last_sensitivity {
            last_rules = disabled_rules.clone();
            last_sensitivity = gector_sensitivity.clone();
            last_text.clear();
        }

        if !warmed {
            warmed = true;
            let sens = gector_sensitivity.clone();
            std::thread::spawn(move || {
                // Loads and caches Harper + the GECToR session so the user's
                // first real check is not the one that hitches. Result unused.
                let _ = proofread::check(
                    "This is a warmup sentance to preload the models.",
                    "",
                    &[],
                    &sens,
                );
            });
        }

        // A panic in Harper, GECToR or an accessibility call must not kill this
        // thread — underlines going permanently dead is worse than a lost cycle.
        let polled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            poll_once(
                &vocabulary,
                &disabled_rules,
                &gector_sensitivity,
                &ignore_apps,
                &mut last_text,
                &mut issues,
            )
        }));
        let (squiggles, reason) = match polled {
            Ok(v) => v,
            Err(p) => {
                crate::logging::log_error(
                    "inline_unix",
                    &format!("poll panicked: {}", crate::logging::panic_msg(&*p)),
                );
                (Vec::new(), "panic")
            }
        };

        if reason != last_reason {
            last_reason = reason;
            crate::logging::log_info("inline_unix", reason);
        }

        if squiggles.is_empty() {
            if active {
                super::squiggle_overlay::clear(&app);
                super::squiggle_overlay::hide_popup(&app);
                backend::forget();
                popup_for = None;
                active = false;
            }
        } else {
            super::squiggle_overlay::draw(&app, &squiggles);
            active = true;

            // Hover -> popup. The overlay is click-through on both platforms and
            // cannot receive pointer events, so the cursor is polled against the
            // rects instead.
            match backend::cursor_position().and_then(|(cx, cy)| hovered(&squiggles, cx, cy)) {
                Some(hit) if !hit.suggestions.is_empty() => {
                    if popup_for.as_ref() != Some(hit) {
                        popup_for = Some(hit.clone());
                        super::squiggle_overlay::show_popup(&app, hit);
                    }
                }
                _ => {
                    // Leaving the word closes it, but not while the pointer is
                    // over the popup itself — otherwise it would vanish the
                    // instant the user moved toward a suggestion.
                    if popup_for.is_some() && !super::squiggle_overlay::cursor_over_popup(&app) {
                        popup_for = None;
                        super::squiggle_overlay::hide_popup(&app);
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(if active {
            ACTIVE_POLL_MS
        } else if reason == "no issues" {
            IDLE_POLL_MS
        } else {
            DEEP_IDLE_MS
        }));
    }
}

fn poll_once(
    vocabulary: &str,
    disabled_rules: &[String],
    gector_sensitivity: &str,
    ignore_apps: &[String],
    last_text: &mut String,
    issues: &mut Vec<proofread::ProofIssue>,
) -> (Vec<SquiggleInfo>, &'static str) {
    let field = match backend::focused_field() {
        Ok(f) => f,
        Err(reason) => {
            last_text.clear();
            issues.clear();
            return (Vec::new(), reason);
        }
    };

    let name = backend::app_name(&field);
    if !name.is_empty() && ignore_apps.iter().any(|a| name.contains(a.as_str())) {
        return (Vec::new(), "user-ignored app");
    }

    // Re-lint only when the text actually changed; rects are re-read every poll
    // so underlines follow scrolling and window moves.
    if field.text != *last_text {
        *issues = proofread::check(&field.text, vocabulary, disabled_rules, gector_sensitivity);
        *last_text = field.text.clone();
    }

    let chars = field.chars();
    let (fx, fy, fw, fh) = field.rect;
    let mut squiggles: Vec<SquiggleInfo> = Vec::new();
    // Anchor: the first rect read, re-read at the end. Each bounds_for_range is
    // a separate cross-process call — a D-Bus round trip on Linux — so the view
    // can scroll midway and leave one frame describing two scroll positions,
    // which draws underlines through the middle of words. If the anchor moved,
    // the whole frame is discarded.
    let mut anchor: Option<(usize, f64, f64)> = None;

    for (idx, issue) in issues.iter().enumerate() {
        if squiggles.len() >= MAX_SQUIGGLES {
            break;
        }
        let expected: String = chars
            .get(issue.start..issue.end)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        if expected.is_empty() {
            continue;
        }
        let Some((x, y, w, h)) = field.bounds_for_range(issue.start, issue.end) else {
            continue;
        };
        if anchor.is_none() {
            anchor = Some((idx, x, y));
        }
        // Clip to the field so a word scrolled out of view cannot leave a
        // floating underline behind.
        if fw > 0.0 && fh > 0.0 {
            let cy = y + h / 2.0;
            if cy < fy - 1.0 || cy > fy + fh + 1.0 || x + w < fx - 1.0 || x > fx + fw + 1.0 {
                continue;
            }
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
            expected,
        });
    }

    if let Some((idx, ax_, ay)) = anchor {
        let moved = match issues
            .get(idx)
            .and_then(|i| field.bounds_for_range(i.start, i.end))
        {
            Some((nx, ny, _, _)) => (nx - ax_).abs() > 1.0 || (ny - ay).abs() > 1.0,
            None => true,
        };
        if moved {
            return (Vec::new(), "scroll-move");
        }
    }

    if squiggles.is_empty() {
        return (Vec::new(), "no issues");
    }

    // Hold the field so a later popup click can still write to it. Cheap — one
    // retain/release per active poll — and it means the popup never has to
    // re-resolve focus, which by then is the popup itself.
    backend::remember(&field);
    (squiggles, "active")
}
