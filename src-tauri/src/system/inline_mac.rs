// Inline proofreading watcher for macOS — the counterpart to inline_check.rs.
//
// Same pipeline, different platform primitives: the focused field comes from
// the Accessibility API (ax.rs) instead of UI Automation, and the underlines go
// to a transparent Tauri window (squiggle_mac.rs) instead of Win32 layered
// windows. The proofreading itself is the shared `proofread::check`, so both
// platforms flag exactly the same things.
//
// This is a separate file rather than cfg branches inside inline_check.rs on
// purpose: that file is 950 lines of COM lifetime handling that works, and
// threading a second platform through it would put the working path at risk
// for no gain.

use super::inline_types::{SquiggleInfo, MAX_SQUIGGLES};
use crate::proofread;
use crate::AppState;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// The word the popup is currently offering to fix, with its field retained.
///
/// Clicking the popup moves focus, so by the time the fix arrives the focused
/// element is the popup itself. Holding the original element here is what makes
/// the correction land in the text the user was actually looking at.
static PENDING: Mutex<Option<PendingFix>> = Mutex::new(None);

struct PendingFix {
    el: super::ax::ElementRef,
    info: SquiggleInfo,
}

/// Apply the fix the popup is offering. Returns false if the text moved under
/// us, in which case nothing is written.
pub fn apply_pending_fix(start: usize, end: usize, expected: &str, replacement: &str) -> bool {
    let guard = match PENDING.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let Some(p) = guard.as_ref() else {
        return false;
    };
    if p.info.start != start || p.info.end != end || p.info.expected != expected {
        return false;
    }
    p.el.replace_if_matches(start, end, expected, replacement)
}

/// Cursor inside a flagged word, with a little slack below for the underline
/// itself — the strip sits under the baseline, so an exact rect test would make
/// the popup fire only on the text and never on the squiggle the user aimed at.
fn hovered<'a>(squiggles: &'a [SquiggleInfo], cx: f64, cy: f64) -> Option<&'a SquiggleInfo> {
    squiggles.iter().find(|s| {
        cx >= s.x as f64
            && cx <= (s.x + s.w) as f64
            && cy >= s.y as f64
            && cy <= (s.y + s.h) as f64 + 6.0
    })
}

/// Underlines on screen: poll fast so they track typing and scrolling.
const ACTIVE_POLL_MS: u64 = 90;
/// Editable field focused, nothing flagged: gentler.
const IDLE_POLL_MS: u64 = 400;
/// Nothing editable focused: there is no AX focus-change notification wired up
/// here, so this is a plain heartbeat rather than a wake.
const DEEP_IDLE_MS: u64 = 1200;
/// Waiting on the user to grant Accessibility. Nothing works until they do.
const UNTRUSTED_MS: u64 = 3000;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || watcher(app));
}

fn watcher(app: AppHandle) {
    let mut last_text = String::new();
    let mut issues: Vec<proofread::ProofIssue> = Vec::new();
    let mut last_rules: Vec<String> = Vec::new();
    let mut last_sensitivity = String::new();
    let mut active = false;
    let mut warmed = false;
    let mut warned_untrusted = false;
    // Logged only when it changes — logging every poll would write thousands of
    // lines an hour, which is how the Windows watcher learned to do this.
    let mut last_reason = "";
    // pid → process name, so the user's ignore list can be honoured without
    // paying for a process lookup on every single poll.
    let mut app_name_cache: (i32, String) = (0, String::new());

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
                super::squiggle_mac::clear(&app);
                active = false;
                last_text.clear();
                issues.clear();
            }
            std::thread::sleep(Duration::from_millis(600));
            continue;
        }

        // Every AX call returns nothing at all without this permission, which
        // would otherwise look exactly like "the feature is broken".
        if !super::accessibility::is_trusted() {
            if !warned_untrusted {
                warned_untrusted = true;
                crate::logging::log_info(
                    "inline_mac",
                    "inline proofreading is on but Accessibility access is not granted",
                );
            }
            std::thread::sleep(Duration::from_millis(UNTRUSTED_MS));
            continue;
        }
        warned_untrusted = false;

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

        // A panic in Harper, GECToR or an AX call must not kill this thread —
        // underlines going permanently dead is worse than one skipped cycle.
        let polled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            poll_once(
                &vocabulary,
                &disabled_rules,
                &gector_sensitivity,
                &ignore_apps,
                &mut app_name_cache,
                &mut last_text,
                &mut issues,
            )
        }));
        let (squiggles, reason, element) = match polled {
            Ok(v) => v,
            Err(p) => {
                crate::logging::log_error(
                    "inline_mac",
                    &format!("poll panicked: {}", crate::logging::panic_msg(&*p)),
                );
                (Vec::new(), "panic", None)
            }
        };

        if reason != last_reason {
            last_reason = reason;
            crate::logging::log_info("inline_mac", reason);
        }

        if squiggles.is_empty() {
            if active {
                super::squiggle_mac::clear(&app);
                super::squiggle_mac::hide_popup(&app);
                if let Ok(mut g) = PENDING.lock() {
                    *g = None;
                }
                active = false;
            }
        } else {
            super::squiggle_mac::draw(&app, &squiggles);
            active = true;

            // Hover → popup. The overlay is click-through and cannot receive
            // pointer events, so the cursor is polled instead.
            match super::ax::cursor_position().and_then(|(cx, cy)| hovered(&squiggles, cx, cy)) {
                Some(hit) => {
                    let changed = PENDING
                        .lock()
                        .map(|g| {
                            g.as_ref().map(|p| p.info != *hit).unwrap_or(true)
                        })
                        .unwrap_or(true);
                    if changed && !hit.suggestions.is_empty() {
                        if let Some(el) = element {
                            if let Ok(mut g) = PENDING.lock() {
                                *g = Some(PendingFix { el, info: hit.clone() });
                            }
                            super::squiggle_mac::show_popup(&app, hit);
                        }
                    }
                }
                None => {
                    // Leaving the word closes it, but only if the pointer is not
                    // over the popup itself — otherwise it would vanish the
                    // instant the user moved toward a suggestion.
                    let over_popup = super::squiggle_mac::cursor_over_popup(&app);
                    if !over_popup {
                        super::squiggle_mac::hide_popup(&app);
                        if let Ok(mut g) = PENDING.lock() {
                            *g = None;
                        }
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

/// Lowercase name of the app owning the focused field, cached against its pid.
///
/// Reuses the existing frontmost-app lookup rather than resolving the pid
/// directly: that lookup shells out to osascript and costs real milliseconds,
/// which is only acceptable because the cache means it runs once per focus
/// change rather than once per poll.
fn app_name(pid: i32, cache: &mut (i32, String)) -> String {
    if cache.0 == pid {
        return cache.1.clone();
    }
    let name = super::foreground::foreground_app().unwrap_or_default();
    *cache = (pid, name.clone());
    name
}

#[allow(clippy::too_many_arguments)]
fn poll_once(
    vocabulary: &str,
    disabled_rules: &[String],
    gector_sensitivity: &str,
    ignore_apps: &[String],
    app_name_cache: &mut (i32, String),
    last_text: &mut String,
    issues: &mut Vec<proofread::ProofIssue>,
) -> (Vec<SquiggleInfo>, &'static str, Option<super::ax::ElementRef>) {
    let field = match super::ax::focused_field() {
        Ok(f) => f,
        Err(reason) => {
            last_text.clear();
            issues.clear();
            return (Vec::new(), reason, None);
        }
    };

    let name = app_name(field.pid, app_name_cache);
    if !name.is_empty() && ignore_apps.iter().any(|a| name.contains(a.as_str())) {
        return (Vec::new(), "user-ignored app", None);
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
    // Anchor: the first rect we read, re-read at the end. Each bounds_for_range
    // is a separate cross-process call, so the view can scroll midway and leave
    // one frame describing two scroll positions — which draws underlines
    // through the middle of words. If the anchor moved, drop the whole frame.
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
        // Clip to the field, so a word scrolled out of view cannot leave a
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
            Some((nx, ny)) => (nx - ax_).abs() > 1.0 || (ny - ay).abs() > 1.0,
            None => true,
        };
        if moved {
            return (Vec::new(), "scroll-move", None);
        }
    }

    if squiggles.is_empty() {
        (Vec::new(), "no issues", None)
    } else {
        let retained = field.retain();
        (squiggles, "active", Some(retained))
    }
}
