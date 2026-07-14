use crate::audio::capture::{self, Recorder};
use crate::history::{self, HistoryEntry};
use crate::llm::openai;
use crate::logging;
use crate::models::registry;
use crate::system::{foreground, overlay, paste, textfmt};
use crate::transcription::whisper;
use crate::AppState;
use serde::Serialize;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::Shortcut;
#[cfg(windows)]
use tauri_plugin_global_shortcut::Code;

/// A press-release shorter than this counts as a "tap" (vs. push-to-talk hold).
const TAP_MS: u64 = 300;
/// A second press within this window after a tap locks recording on.
const DOUBLE_TAP_WINDOW_MS: u64 = 450;

/// Resolve how many CPU threads inference should use.
/// - high_performance OFF → balanced default `max(2, cores/2)` (keeps the
///   system responsive).
/// - high_performance ON  → the user's chosen `performance_threads`, clamped to
///   [default, all cores]; 0 means "auto" = all cores. Shared by STT (whisper)
///   and sherpa TTS so both honor the Performance setting identically.
pub fn resolve_thread_count(high_performance: bool, performance_threads: u32) -> u32 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);
    let default = std::cmp::max(2, cores / 2);
    if !high_performance {
        return default;
    }
    if performance_threads == 0 {
        cores
    } else {
        performance_threads.clamp(default, cores)
    }
}

/// Log + surface a pipeline error in one step so failures always leave a trace
/// in %APPDATA%/SilentVoice/logs even if the UI wasn't watching.
fn report_error(app: &AppHandle, context: &str, msg: &str) {
    logging::log_error(context, msg);
    let _ = app.emit("pipeline://error", msg.to_string());
}

#[derive(Serialize, Clone)]
pub struct PipelineState {
    pub state: String, // "recording" | "processing" | "idle"
}

/// Small models often wrap their answer in a chatty preamble or quotes despite
/// being told not to. Conservatively strip a leading "Here is …:" line and a
/// single pair of surrounding quotes so only the clean result gets pasted.
fn tidy_ai_output(s: &str) -> String {
    let mut text = s.trim().to_string();

    // Drop a short leading preface line that ends with a colon, e.g.
    // "Here is the cleaned text:" or "Sure, here's the rewritten version:".
    if let Some((first, rest)) = text.split_once('\n') {
        let f = first.trim();
        let low = f.to_lowercase();
        let looks_preface = f.ends_with(':')
            && f.chars().count() < 70
            && (low.starts_with("here is")
                || low.starts_with("here's")
                || low.starts_with("sure")
                || low.starts_with("okay")
                || low.starts_with("certainly")
                || low.contains("cleaned")
                || low.contains("rewritten")
                || low.contains("following"));
        if looks_preface {
            text = rest.trim().to_string();
        }
    }

    // Strip one pair of matching surrounding quotes.
    let bytes = text.as_bytes();
    if bytes.len() >= 2 {
        let first = text.chars().next().unwrap();
        let last = text.chars().last().unwrap();
        let pair = matches!((first, last), ('"', '"') | ('\'', '\'') | ('“', '”'));
        if pair {
            let inner: String = text.chars().skip(1).take(text.chars().count() - 2).collect();
            if !inner.contains(first) {
                text = inner.trim().to_string();
            }
        }
    }

    text
}

/// Apply the user's text-replacement snippets to a transcript. Each pair is a
/// spoken trigger and the text to substitute for it (e.g. "my email" →
/// "a@b.com"). Matching is case-insensitive. Longer triggers are applied first
/// so a more specific phrase wins over a shorter one it contains.
fn apply_replacements(text: &str, pairs: &[(String, String)]) -> String {
    let mut ordered: Vec<&(String, String)> = pairs
        .iter()
        .filter(|(t, _)| !t.trim().is_empty())
        .collect();
    ordered.sort_by_key(|(t, _)| std::cmp::Reverse(t.trim().chars().count()));

    let mut out = text.to_string();
    for (trigger, replacement) in ordered {
        out = replace_case_insensitive(&out, trigger.trim(), replacement);
    }
    out
}

/// Replace every case-insensitive occurrence of `needle` in `haystack`.
/// Byte offsets are taken from the ORIGINAL string (not the lowercased copy)
/// so slicing stays valid; for ASCII triggers — the overwhelmingly common
/// case for dictation snippets — lowercasing preserves byte length exactly.
fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let hay_lower = haystack.to_lowercase();
    let need_lower = needle.to_lowercase();
    // If lowercasing shifted byte lengths (non-ASCII), fall back to a plain
    // case-sensitive replace to avoid slicing at an invalid boundary.
    if hay_lower.len() != haystack.len() {
        return haystack.replace(needle, replacement);
    }

    let mut result = String::with_capacity(haystack.len());
    let mut last = 0usize;
    let mut search_from = 0usize;
    while let Some(rel) = hay_lower[search_from..].find(&need_lower) {
        let idx = search_from + rel;
        result.push_str(&haystack[last..idx]);
        result.push_str(replacement);
        let mut after = idx + needle.len();
        // Whisper attaches sentence punctuation to a spoken trigger ("my
        // email" → "My email."). Swallow any punctuation directly following the
        // trigger so the replacement pastes exactly its value — nothing after.
        while let Some(c) = haystack[after..].chars().next() {
            if matches!(c, '.' | ',' | '!' | '?' | ';' | ':') {
                after += c.len_utf8();
            } else {
                break;
            }
        }
        last = after;
        search_from = last;
    }
    result.push_str(&haystack[last..]);
    result
}

#[cfg(test)]
mod replacement_tests {
    use super::apply_replacements;

    fn pairs() -> Vec<(String, String)> {
        vec![("my email".into(), "a@b.com".into())]
    }

    #[test]
    fn strips_trailing_period_whisper_adds() {
        // Whisper capitalizes + adds a period to the standalone utterance.
        assert_eq!(apply_replacements("My email.", &pairs()), "a@b.com");
    }

    #[test]
    fn strips_trailing_comma() {
        assert_eq!(apply_replacements("my email,", &pairs()), "a@b.com");
    }

    #[test]
    fn leaves_following_words_intact() {
        assert_eq!(
            apply_replacements("send my email to John", &pairs()),
            "send a@b.com to John"
        );
    }

    #[test]
    fn no_punctuation_unchanged() {
        assert_eq!(apply_replacements("my email", &pairs()), "a@b.com");
    }
}

#[derive(Serialize, Clone)]
pub struct PipelineResult {
    pub raw_text: String,
    pub processed_text: String,
    pub model_id: String,
    pub duration_ms: i64,
}

fn emit_state(app: &AppHandle, state: &str) {
    let _ = app.emit("pipeline://state", PipelineState { state: state.into() });
}

/// Hotkey pressed. Behavior:
///  - idle → start recording (push-to-talk begins)
///  - recording + this press is the second tap of a double-tap → lock recording on
///  - recording locked → stop & process (single press ends a locked session)
/// OS key-repeat presses (key held down) are filtered via `key_down`.
pub fn on_pressed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let toggle_mode = state
        .config
        .lock()
        .map(|c| c.toggle_mode)
        .unwrap_or(true);

    // Tap bookkeeping — and early exits for repeat/locked cases.
    {
        let mut tap = match state.tap.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        if tap.key_down {
            return; // OS auto-repeat while held — not a new press
        }
        tap.key_down = true;
        tap.press_seq += 1;
        tap.press_at = Some(Instant::now());

        if tap.locked {
            // Single press while locked → stop and process.
            tap.locked = false;
            tap.last_tap_at = None;
            tap.ignore_release = true;
            drop(tap);
            finalize_recording(app.clone());
            return;
        }

        let recording = state
            .recorder
            .lock()
            .map(|s| s.is_some())
            .unwrap_or(false);
        if recording {
            // Recording is still running from a recent quick tap. If this
            // press lands inside the double-tap window → lock recording on.
            if toggle_mode {
                if let Some(last) = tap.last_tap_at {
                    if last.elapsed() < Duration::from_millis(DOUBLE_TAP_WINDOW_MS) {
                        tap.locked = true;
                        tap.last_tap_at = None;
                        tap.ignore_release = true;
                    }
                }
            }
            return;
        }
    }

    start_capture(app);
}

/// Begin capturing (no tap bookkeeping). Shared by the hotkey path and the
/// tray-menu record toggle.
pub fn start_capture(app: &AppHandle) {
    let state = app.state::<AppState>();

    // Remember which app the user is dictating into (per-app profiles).
    if let Some(exe) = foreground::foreground_app() {
        if let Ok(mut a) = state.active_app.lock() {
            *a = exe;
        }
    }

    let device = state
        .config
        .lock()
        .ok()
        .and_then(|c| c.audio_device.clone());

    let mut slot = match state.recorder.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    if slot.is_some() {
        return;
    }
    match Recorder::start(device) {
        Ok(rec) => {
            *slot = Some(rec);
            overlay::show_overlay(app);
            emit_state(app, "recording");
        }
        Err(e) => {
            report_error(app, "audio", &e);
        }
    }
}

/// Stop the current recording and run the pipeline (no tap bookkeeping).
/// Used by the tray-menu record toggle.
pub fn stop_capture(app: &AppHandle) {
    finalize_recording(app.clone());
}

/// Map a shortcut's MAIN key to its Windows Virtual-Key code, so we can poll
/// its physical state. Returns None for keys we don't map (guard then skipped).
#[cfg(windows)]
fn main_key_vk(shortcut: &Shortcut) -> Option<i32> {
    let vk: i32 = match shortcut.key {
        Code::Space => 0x20,
        Code::Enter => 0x0D,
        Code::Tab => 0x09,
        Code::Backspace => 0x08,
        Code::Escape => 0x1B,
        Code::ArrowLeft => 0x25,
        Code::ArrowUp => 0x26,
        Code::ArrowRight => 0x27,
        Code::ArrowDown => 0x28,
        Code::KeyA => 0x41, Code::KeyB => 0x42, Code::KeyC => 0x43, Code::KeyD => 0x44,
        Code::KeyE => 0x45, Code::KeyF => 0x46, Code::KeyG => 0x47, Code::KeyH => 0x48,
        Code::KeyI => 0x49, Code::KeyJ => 0x4A, Code::KeyK => 0x4B, Code::KeyL => 0x4C,
        Code::KeyM => 0x4D, Code::KeyN => 0x4E, Code::KeyO => 0x4F, Code::KeyP => 0x50,
        Code::KeyQ => 0x51, Code::KeyR => 0x52, Code::KeyS => 0x53, Code::KeyT => 0x54,
        Code::KeyU => 0x55, Code::KeyV => 0x56, Code::KeyW => 0x57, Code::KeyX => 0x58,
        Code::KeyY => 0x59, Code::KeyZ => 0x5A,
        Code::Digit0 => 0x30, Code::Digit1 => 0x31, Code::Digit2 => 0x32, Code::Digit3 => 0x33,
        Code::Digit4 => 0x34, Code::Digit5 => 0x35, Code::Digit6 => 0x36, Code::Digit7 => 0x37,
        Code::Digit8 => 0x38, Code::Digit9 => 0x39,
        Code::F1 => 0x70, Code::F2 => 0x71, Code::F3 => 0x72, Code::F4 => 0x73,
        Code::F5 => 0x74, Code::F6 => 0x75, Code::F7 => 0x76, Code::F8 => 0x77,
        Code::F9 => 0x78, Code::F10 => 0x79, Code::F11 => 0x7A, Code::F12 => 0x7B,
        _ => return None,
    };
    Some(vk)
}

/// True if the shortcut's main key is still physically held. global-hotkey
/// synthesizes release events by polling on Windows, so during a held key with
/// OS auto-repeat it can fire a SPURIOUS Released while the key is still down —
/// we must ignore those or recording stops mid-hold.
#[cfg(windows)]
fn key_still_down(shortcut: &Shortcut) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    match main_key_vk(shortcut) {
        Some(vk) => (unsafe { GetAsyncKeyState(vk) } as u16) & 0x8000 != 0,
        None => false,
    }
}

#[cfg(not(windows))]
fn key_still_down(_shortcut: &Shortcut) -> bool {
    false
}

/// Hotkey released. A long hold releases normally (classic push-to-talk).
/// A quick tap defers the stop briefly: if a second tap arrives in time the
/// recording locks on instead of stopping.
pub fn on_released(app: &AppHandle, shortcut: &Shortcut) {
    // Ignore spurious releases fired while the key is still physically held
    // (global-hotkey polling artifact) — otherwise recording stops mid-hold.
    // Touch no state here so the still-true key_down keeps filtering auto-repeat.
    if key_still_down(shortcut) {
        return;
    }

    let state = app.state::<AppState>();
    let toggle_mode = state
        .config
        .lock()
        .map(|c| c.toggle_mode)
        .unwrap_or(true);

    let (was_tap, seq) = {
        let mut tap = match state.tap.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        tap.key_down = false;
        if tap.ignore_release {
            tap.ignore_release = false;
            return;
        }
        let held = tap
            .press_at
            .map(|t| t.elapsed())
            .unwrap_or(Duration::MAX);
        let was_tap = toggle_mode && held < Duration::from_millis(TAP_MS);
        if was_tap {
            tap.last_tap_at = Some(Instant::now());
        }
        (was_tap, tap.press_seq)
    };

    if !was_tap {
        finalize_recording(app.clone());
        return;
    }

    // Quick tap: wait out the double-tap window. If no second press claimed
    // the recording (and it didn't get locked), finalize what we captured.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(DOUBLE_TAP_WINDOW_MS + 30)).await;
        let state = app.state::<AppState>();
        let superseded = state
            .tap
            .lock()
            .map(|t| t.press_seq != seq || t.locked)
            .unwrap_or(true);
        if !superseded {
            finalize_recording(app.clone());
        }
    });
}

/// Stop the active recording (if any), then run the full pipeline.
fn finalize_recording(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();

        let recorder: Option<Recorder> = {
            let state = app.state::<AppState>();
            if let Ok(mut tap) = state.tap.lock() {
                tap.last_tap_at = None;
            }
            let mut slot = match state.recorder.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            slot.take()
        };

        let Some(rec) = recorder else {
            return;
        };

        emit_state(&app, "processing");
        let samples = rec.stop();
        if samples.is_empty() {
            // pill stays visible; just return to idle state
            emit_state(&app, "idle");
            return;
        }

        process_audio_pipeline(app, samples, started).await;
    });
}

/// Core audio post-processing pipeline: writes WAV, transcribes via Whisper,
/// runs active AI mode processing, pastes to active window, and records in history.
pub async fn process_audio_pipeline(app: AppHandle, samples: Vec<f32>, started: std::time::Instant) {
    // Read runtime config (STT model, language, active AI mode).
    let (
        model_id,
        language,
        vocabulary,
        stt_source,
        stt_base_url,
        stt_api_key,
        stt_cloud_model,
        use_gpu,
        high_performance,
        performance_threads,
        input_sensitivity,
        replacements,
        app_profiles,
        mode_id,
        mut mode_source,
        mut mode_prompt,
        mut mode_model,
        mut mode_base_url,
        mut mode_api_key,
    ) = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        (
            cfg.model_id.clone(),
            cfg.language.clone(),
            cfg.vocabulary.clone(),
            cfg.stt_source.clone(),
            cfg.stt_base_url.clone(),
            cfg.stt_api_key.clone(),
            cfg.stt_cloud_model.clone(),
            cfg.use_gpu,
            cfg.high_performance,
            cfg.performance_threads,
            cfg.input_sensitivity,
            cfg.replacements.clone(),
            cfg.app_profiles.clone(),
            cfg.mode_id.clone(),
            cfg.mode_source.clone(),
            cfg.mode_prompt.clone(),
            cfg.mode_model.clone(),
            cfg.mode_base_url.clone(),
            cfg.mode_api_key.clone(),
        )
    };

    // Per-app profile override: if the app that was focused when recording
    // started matches a profile rule, that profile's mode wins.
    {
        let state = app.state::<AppState>();
        let active_app = state
            .active_app
            .lock()
            .map(|a| a.clone())
            .unwrap_or_default();
        if !active_app.is_empty() {
            if let Some(p) = app_profiles.iter().find(|p| {
                let m = p.app_match.trim().to_lowercase();
                !m.is_empty() && active_app.contains(&m)
            }) {
                mode_source = p.mode_source.clone();
                mode_prompt = p.mode_prompt.clone();
                mode_model = p.mode_model.clone();
                mode_base_url = p.mode_base_url.clone();
                mode_api_key = p.mode_api_key.clone();
            }
        }
    }

    if let Err(e) = registry::ensure_dirs() {
        report_error(&app, "storage", &e.to_string());
    }

    // Input-sensitivity gate: trim leading/trailing audio quieter than the
    // user's threshold (wind, hum). A clip with no speech at all is skipped
    // entirely — no transcription time wasted on noise.
    let Some(samples) = crate::audio::gate::trim_silence(&samples, input_sensitivity) else {
        crate::logging::log_info("gate", "clip below sensitivity threshold — skipped");
        emit_state(&app, "idle");
        return;
    };

    let wav_path = registry::audio_dir().join("last.wav");
    if let Err(e) = capture::write_wav(&wav_path, &samples) {
        report_error(&app, "audio", &e);
        // pill stays visible; just return to idle state
        emit_state(&app, "idle");
        return;
    }

    let threads = resolve_thread_count(high_performance, performance_threads);

    let raw_text = match whisper::transcribe_dispatch(
        &app,
        &wav_path,
        &model_id,
        threads,
        &language,
        &vocabulary,
        use_gpu,
        &stt_source,
        &stt_base_url,
        &stt_api_key,
        &stt_cloud_model,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            report_error(&app, "stt", &e);
            // pill stays visible; just return to idle state
            emit_state(&app, "idle");
            return;
        }
    };

    let raw_text = textfmt::collapse_repeated_words(&raw_text);

    // Optional AI processing: run the active mode's prompt through a local
    // LLM (Ollama). On any failure, fall back to the raw transcription so
    // the user never loses their words.
    let processed_text = if !raw_text.is_empty()
        && !mode_prompt.is_empty()
        && (mode_source == "local" || mode_source == "api")
    {
        let result = match mode_source.as_str() {
            "local" => {
                crate::run_local_llm(&app, &mode_model, &mode_prompt, &raw_text).await
            }
            "api" => {
                openai::chat(
                    &mode_base_url,
                    &mode_api_key,
                    &mode_model,
                    &mode_prompt,
                    &raw_text,
                )
                .await
            }
            _ => Ok(raw_text.clone()),
        };
        match result {
            Ok(out) if !out.trim().is_empty() => tidy_ai_output(&out),
            Ok(_) => raw_text.clone(),
            Err(e) => {
                report_error(&app, "ai-mode", &format!("AI mode skipped: {e}"));
                raw_text.clone()
            }
        }
    } else {
        raw_text.clone()
    };

    // Apply user text-replacement snippets (spoken trigger → inserted text)
    // to the final transcript, right before pasting.
    let processed_text = apply_replacements(&processed_text, &replacements);

    // Smart number formatting ("twenty five percent" → "25%") — always on.
    // Runs after replacements so digits inside replacement output are untouched.
    let processed_text = textfmt::format_numbers(&processed_text);

    // Paste the processed (or raw) text at the cursor.
    if !processed_text.is_empty() {
        if let Err(e) = paste::paste_at_cursor(&processed_text) {
            report_error(&app, "paste", &e);
        }
    }

    let elapsed = started.elapsed().as_millis() as i64;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let entry = HistoryEntry {
        id: now,
        timestamp: now,
        raw_text: raw_text.clone(),
        processed_text: processed_text.clone(),
        mode_id: mode_id.clone(),
        model_id: model_id.clone(),
        duration_ms: elapsed,
    };
    let _ = history::append(entry);

    let _ = app.emit(
        "pipeline://result",
        PipelineResult {
            raw_text,
            processed_text,
            model_id,
            duration_ms: elapsed,
        },
    );

    // pill stays visible; just return to idle state
    emit_state(&app, "idle");
}
