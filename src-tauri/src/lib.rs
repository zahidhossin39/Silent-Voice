mod audio;
mod history;
mod proofread;
mod llm;
mod logging;
mod models;
mod system;
mod transcription;

use audio::capture::{self, Recorder};
use history::HistoryEntry;
use models::{downloader, registry};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use std::time::Instant;
use system::{hardware, hotkey, overlay, paste, tray};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Runtime config mirrored from the frontend so the global-hotkey pipeline
/// (which runs in Rust) knows which model/language/device to use.
pub struct RuntimeConfig {
    pub model_id: String,
    pub language: String,
    pub audio_device: Option<String>,
    pub hotkey: String,
    // When false, whisper.cpp gets `-ng` (force CPU). When true, the sidecar
    // uses its GPU backend if the bundled binary was built with one.
    pub use_gpu: bool,
    // Comma/newline-separated custom words (names, jargon) fed to whisper.cpp
    // as a priming prompt so it recognizes them more reliably.
    pub vocabulary: String,
    // Cloud STT (optional): when stt_source is "cloud", transcription goes to
    // a cloud provider's OpenAI-shaped Whisper endpoint instead of the local
    // whisper.cpp sidecar. See llm::openai::transcribe_audio.
    pub stt_source: String, // "local" | "cloud"
    pub stt_base_url: String,
    pub stt_api_key: String,
    pub stt_cloud_model: String,
    // Spoken-trigger → inserted-text pairs, applied to the final transcript
    // (after AI processing) right before pasting. e.g. ("my email", "a@b.com").
    pub replacements: Vec<(String, String)>,
    // Active AI processing mode (applied after transcription, before paste).
    pub mode_id: String,
    pub mode_source: String, // "none" | "local" (Ollama) | "api" (OpenAI-compatible)
    pub mode_prompt: String,
    pub mode_model: String, // LLM model id / Ollama tag
    pub mode_base_url: String, // for "api": e.g. http://localhost:1234/v1
    pub mode_api_key: String,  // for "api": optional (empty for local servers)
    // Behavior flags (Settings toggles).
    pub toggle_mode: bool, // double-tap the hotkey to lock recording on
    // 0–100 (Discord-style): how loud a sound must be to count as speech.
    // Quieter audio (wind, hum) is trimmed before transcription. See audio/gate.rs.
    pub input_sensitivity: u32,
    // Inline proofreading: squiggles under spelling/grammar errors in ANY
    // app's focused text field (system/inline_check.rs). English-only.
    pub inline_proofread: bool,
    // Read-aloud (TTS): active Piper voice id + the hotkey that reads the
    // current text selection. See system/tts.rs.
    pub tts_voice_id: String,
    pub tts_hotkey: String,
    // Per-app profiles: when the focused app matches, that profile's AI mode
    // overrides the globally active one. Resolved by the frontend (like
    // set_active_mode) so Rust never needs the mode/provider tables.
    pub app_profiles: Vec<AppProfile>,
}

/// One per-app profile rule, fully resolved by the frontend.
#[derive(Deserialize, Clone, Default)]
pub struct AppProfile {
    pub app_match: String, // lowercase substring of the exe name, e.g. "code"
    pub mode_source: String,
    pub mode_prompt: String,
    pub mode_model: String,
    pub mode_base_url: String,
    pub mode_api_key: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            model_id: "base.en".into(),
            language: "auto".into(),
            audio_device: None,
            hotkey: "Ctrl+Shift+Space".into(),
            use_gpu: false,
            vocabulary: String::new(),
            stt_source: "local".into(),
            stt_base_url: String::new(),
            stt_api_key: String::new(),
            stt_cloud_model: String::new(),
            replacements: Vec::new(),
            mode_id: "raw".into(),
            mode_source: "none".into(),
            mode_prompt: String::new(),
            mode_model: String::new(),
            mode_base_url: String::new(),
            mode_api_key: String::new(),
            toggle_mode: true,
            input_sensitivity: 50,
            inline_proofread: true,
            tts_voice_id: String::new(),
            tts_hotkey: "Ctrl+Alt+S".into(),
            app_profiles: Vec::new(),
        }
    }
}

/// Hotkey tap-tracking for double-tap lock mode (see hotkey.rs).
#[derive(Default)]
pub struct TapState {
    /// Recording is locked on (double-tap); next press stops it.
    pub locked: bool,
    /// True while the physical key is held — filters OS key-repeat presses.
    pub key_down: bool,
    /// When the current press started (None when key is up).
    pub press_at: Option<Instant>,
    /// When the last quick tap's release happened (for double-tap detection).
    pub last_tap_at: Option<Instant>,
    /// Bumped on every press; lets the deferred single-tap finalizer detect
    /// that another press superseded it.
    pub press_seq: u64,
    /// Swallow the release that follows a press we already acted on.
    pub ignore_release: bool,
}

/// Shared app state.
#[derive(Default)]
pub struct AppState {
    pub recorder: Mutex<Option<Recorder>>,
    pub config: Mutex<RuntimeConfig>,
    pub llama: Mutex<llm::llama::LlamaServer>,
    /// True only when the user explicitly hid the overlay (menu/tray). The
    /// keep-alive loop respects this and won't force it back.
    pub overlay_hidden: AtomicBool,
    /// Bumped on each overlay resize so an in-flight tween knows it's superseded.
    pub overlay_resize_gen: AtomicU64,
    /// Double-tap hotkey lock state.
    pub tap: Mutex<TapState>,
    /// Exe basename of the app focused when recording started (per-app profiles).
    pub active_app: Mutex<String>,
    /// Read-aloud playback state (see system/tts.rs).
    pub tts: system::tts::TtsState,
}

/// Ensure the bundled llama.cpp server is running for `model_id`, then run a
/// system+user chat through it. Used by the pipeline and the mode test.
pub async fn run_local_llm(
    app: &AppHandle,
    model_id: &str,
    system_prompt: &str,
    text: &str,
) -> Result<String, String> {
    let model_path = registry::llm_model_path(model_id);
    if !model_path.exists() {
        return Err(format!("Local model '{model_id}' is not downloaded"));
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);

    let needs_wait = {
        let state = app.state::<AppState>();
        let mut server = state.llama.lock().map_err(|e| e.to_string())?;
        if server.is_running_model(model_id) {
            false
        } else {
            server.start(&model_path, model_id, threads)?;
            true
        }
    };
    if needs_wait {
        llm::llama::wait_ready(std::time::Duration::from_secs(120)).await?;
    }
    llm::openai::chat(&llm::llama::base_url(), "", model_id, system_prompt, text).await
}

// ---------------- Hardware ----------------

#[tauri::command]
fn get_hardware_info() -> hardware::HardwareInfo {
    hardware::detect()
}

#[tauri::command]
fn list_input_devices() -> Vec<String> {
    capture::list_input_devices()
}

// ---------------- Runtime config ----------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn update_runtime_config(
    state: State<AppState>,
    model_id: String,
    language: String,
    audio_device: Option<String>,
    vocabulary: String,
    stt_source: String,
    stt_base_url: String,
    stt_api_key: String,
    stt_cloud_model: String,
    use_gpu: bool,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.model_id = model_id;
    cfg.language = language;
    cfg.audio_device = audio_device;
    cfg.vocabulary = vocabulary;
    cfg.stt_source = stt_source;
    cfg.stt_base_url = stt_base_url;
    cfg.stt_api_key = stt_api_key;
    cfg.stt_cloud_model = stt_cloud_model;
    cfg.use_gpu = use_gpu;
    Ok(())
}

#[tauri::command]
fn set_text_replacements(
    state: State<AppState>,
    pairs: Vec<(String, String)>,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.replacements = pairs;
    Ok(())
}

#[tauri::command]
fn set_behavior(
    state: State<AppState>,
    toggle_mode: bool,
    input_sensitivity: u32,
    inline_proofread: bool,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.toggle_mode = toggle_mode;
    cfg.input_sensitivity = input_sensitivity.min(100);
    cfg.inline_proofread = inline_proofread;
    Ok(())
}

#[tauri::command]
fn set_app_profiles(state: State<AppState>, profiles: Vec<AppProfile>) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.app_profiles = profiles;
    Ok(())
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<(), String> {
    system::autostart::set_enabled(enabled)
}

#[tauri::command]
fn get_autostart() -> bool {
    system::autostart::is_enabled()
}

#[tauri::command]
fn set_hotkey(app: AppHandle, state: State<AppState>, accelerator: String) -> Result<(), String> {
    // Unregister the previous shortcut, then register the new one.
    let prev = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.hotkey.clone()
    };
    if let Ok(s) = Shortcut::from_str(&prev) {
        let _ = app.global_shortcut().unregister(s);
    }
    let shortcut = Shortcut::from_str(&accelerator)
        .map_err(|_| format!("invalid hotkey: {accelerator}"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| e.to_string())?;
    state.config.lock().map_err(|e| e.to_string())?.hotkey = accelerator;
    Ok(())
}

// ---------------- Read aloud (TTS) ----------------

#[tauri::command]
fn set_tts(app: AppHandle, state: State<AppState>, voice_id: String, hotkey: String) -> Result<(), String> {
    // Store the voice FIRST, unconditionally — a hotkey problem must never
    // leave the voice unset (that bug made every TTS action report
    // "No voice downloaded" even with voices installed).
    let prev = {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.tts_voice_id = voice_id;
        cfg.tts_hotkey.clone()
    };
    if prev != hotkey {
        // Parse the new hotkey BEFORE touching the old registration, so a
        // bad accelerator can't leave read-aloud with no hotkey at all.
        let shortcut = match Shortcut::from_str(&hotkey) {
            Ok(s) => s,
            Err(_) => {
                let msg = format!("Read-aloud hotkey '{hotkey}' is not valid — keeping '{prev}'.");
                crate::logging::log_error("tts", &msg);
                let _ = app.emit("pipeline://error", msg.clone());
                return Err(msg);
            }
        };
        if let Ok(s) = Shortcut::from_str(&prev) {
            let _ = app.global_shortcut().unregister(s);
        }
        if let Err(e) = app.global_shortcut().register(shortcut) {
            // Roll back so the previous hotkey keeps working.
            if let Ok(s) = Shortcut::from_str(&prev) {
                let _ = app.global_shortcut().register(s);
            }
            let msg = format!("Could not register read-aloud hotkey '{hotkey}' ({e}) — keeping '{prev}'.");
            crate::logging::log_error("tts", &msg);
            let _ = app.emit("pipeline://error", msg.clone());
            return Err(msg);
        }
        state.config.lock().map_err(|e| e.to_string())?.tts_hotkey = hotkey;
    }
    Ok(())
}

#[tauri::command]
fn tts_read_selection(app: AppHandle) {
    system::tts::read_selection(&app);
}

#[tauri::command]
fn tts_stop(app: AppHandle) {
    system::tts::stop(&app);
}

#[tauri::command]
fn tts_speak_text(app: AppHandle, text: String) {
    system::tts::speak_text(&app, text);
}

#[tauri::command]
fn list_downloaded_tts() -> Vec<String> {
    registry::list_downloaded_tts()
}

// ---------------- Proofreading (Harper) ----------------

/// Check text for spelling/grammar issues. Runs on a blocking thread — the
/// curated dictionary load takes a moment on first call. Custom vocabulary
/// words are never flagged (personal dictionary).
#[tauri::command]
async fn proofread_text(state: State<'_, AppState>, text: String) -> Result<Vec<proofread::ProofIssue>, String> {
    let vocabulary = state
        .config
        .lock()
        .map(|c| c.vocabulary.clone())
        .unwrap_or_default();
    tokio::task::spawn_blocking(move || proofread::check(&text, &vocabulary))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_tts_model(
    app: AppHandle,
    voice_id: String,
    url_onnx: String,
    url_json: String,
) -> Result<(), String> {
    downloader::download_tts_model(app, voice_id, url_onnx, url_json).await
}

#[tauri::command]
fn delete_tts_model(voice_id: String) -> Result<(), String> {
    downloader::delete_tts_model(&voice_id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn set_active_mode(
    state: State<AppState>,
    mode_id: String,
    mode_source: String,
    mode_prompt: String,
    mode_model: String,
    mode_base_url: String,
    mode_api_key: String,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.mode_id = mode_id;
    cfg.mode_source = mode_source;
    cfg.mode_prompt = mode_prompt;
    cfg.mode_model = mode_model;
    cfg.mode_base_url = mode_base_url;
    cfg.mode_api_key = mode_api_key;
    Ok(())
}

// ---------------- AI processing (Ollama) ----------------

#[tauri::command]
async fn ollama_status() -> llm::ollama::OllamaStatus {
    llm::ollama::status().await
}

#[tauri::command]
async fn ollama_generate(
    model: String,
    system_prompt: String,
    text: String,
) -> Result<String, String> {
    llm::ollama::generate(&model, &system_prompt, &text).await
}

/// Generic OpenAI-compatible call — works for LM Studio, llama.cpp server,
/// OpenAI, OpenRouter, Groq, etc. Used for mode tests and "Test connection".
#[tauri::command]
async fn api_generate(
    base_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
    text: String,
) -> Result<String, String> {
    llm::openai::chat(&base_url, &api_key, &model, &system_prompt, &text).await
}

#[tauri::command]
async fn api_list_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    llm::openai::list_models(&base_url, &api_key).await
}

/// Test a cloud STT provider end-to-end: sends a short silent clip through
/// the real transcribe_audio path (auth + endpoint shape + model name), the
/// same code the hotkey pipeline uses. A clean response (even empty text, for
/// silence) means the connection actually works.
#[tauri::command]
async fn api_test_stt(base_url: String, api_key: String, model: String) -> Result<String, String> {
    if model.trim().is_empty() {
        return Err("No STT model set for this provider — fill in the STT model field.".into());
    }
    let dir = std::env::temp_dir();
    let path = dir.join("silent-voice-stt-test.wav");
    // 0.5s of silence at 16kHz — enough for providers to accept the request
    // and return a (likely empty) transcript.
    capture::write_wav(&path, &vec![0.0f32; 8_000])?;
    let result = llm::openai::transcribe_audio(&base_url, &api_key, &model, &path, "").await;
    let _ = std::fs::remove_file(&path);
    result.map(|t| {
        if t.is_empty() {
            "Connected — provider accepted the request (silence transcribed as empty text, as expected).".to_string()
        } else {
            format!("Connected — provider replied: \"{t}\"")
        }
    })
}

// ---------------- Storage location ----------------

#[tauri::command]
fn get_data_location() -> models::registry::DataLocation {
    models::registry::load_data_location()
}

#[tauri::command]
fn set_data_location(
    models_root: Option<String>,
    history_root: Option<String>,
) -> Result<(), String> {
    let loc = models::registry::DataLocation {
        models_root,
        history_root,
    };
    models::registry::save_data_location(&loc)?;
    models::registry::ensure_dirs().map_err(|e| e.to_string())
}

/// Open the data folder in the OS file explorer. Opens the parent SilentVoice
/// folder (so models/, llm/, history.json are all visible) and SELECTS a real
/// entry inside it — `explorer /select` forces a fresh, populated view, which
/// avoids the stale/blank-window glitch plain folder-open sometimes shows.
#[tauri::command]
fn open_data_folder(kind: String) -> Result<(), String> {
    let _ = models::registry::ensure_dirs();
    let folder = match kind.as_str() {
        "history" => models::registry::history_path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(models::registry::models_dir),
        _ => models::registry::models_dir()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(models::registry::models_dir),
    };

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Find a real entry inside the folder to select (forces a fresh view).
        let first_entry = std::fs::read_dir(&folder)
            .ok()
            .and_then(|mut it| it.next())
            .and_then(|e| e.ok())
            .map(|e| e.path());

        let mut cmd = std::process::Command::new("explorer");
        match first_entry {
            Some(entry) => {
                cmd.raw_arg(format!("/select,\"{}\"", entry.display()));
            }
            None => {
                cmd.raw_arg(format!("\"{}\"", folder.display()));
            }
        }
        let _ = cmd.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = &folder;
    }
    Ok(())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().to_string())
}

// ---------------- Local LLM (bundled llama.cpp) ----------------

#[tauri::command]
fn list_downloaded_llm() -> Vec<String> {
    registry::list_downloaded_llm()
}

#[tauri::command]
async fn download_llm_model(
    app: AppHandle,
    model_id: String,
    url: String,
) -> Result<(), String> {
    downloader::download_llm_model(app, model_id, url).await
}

#[tauri::command]
fn delete_llm_model(model_id: String) -> Result<(), String> {
    downloader::delete_llm_model(&model_id)
}

/// Run a downloaded local model through the bundled llama.cpp engine. Used by
/// the mode "Test" button.
#[tauri::command]
async fn local_llm_generate(
    app: AppHandle,
    model_id: String,
    system_prompt: String,
    text: String,
) -> Result<String, String> {
    run_local_llm(&app, &model_id, &system_prompt, &text).await
}

// ---------------- Whisper STT models ----------------

#[tauri::command]
fn list_downloaded_models() -> Vec<String> {
    registry::list_downloaded()
}

#[tauri::command]
async fn download_model(
    app: AppHandle,
    model_id: String,
    url: String,
    file_name: String,
) -> Result<(), String> {
    downloader::download_model(app, model_id, url, file_name).await
}

#[tauri::command]
fn delete_model(model_id: String) -> Result<(), String> {
    downloader::delete_model(&model_id)
}

// ---------------- History (local JSON file) ----------------

#[tauri::command]
fn load_history() -> Vec<HistoryEntry> {
    history::load()
}

#[tauri::command]
fn save_history(entries: Vec<HistoryEntry>) -> Result<(), String> {
    history::save(entries)
}

#[tauri::command]
fn clear_history() -> Result<(), String> {
    history::clear()
}

// ---------------- Manual recording (UI buttons) ----------------

#[tauri::command]
fn start_recording(state: State<AppState>, device: Option<String>) -> Result<(), String> {
    let mut slot = state.recorder.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("already recording".into());
    }
    *slot = Some(Recorder::start(device)?);
    Ok(())
}

#[tauri::command]
async fn stop_and_transcribe(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
    language: String,
) -> Result<String, String> {
    let recorder = {
        let mut slot = state.recorder.lock().map_err(|e| e.to_string())?;
        slot.take().ok_or("not recording")?
    };
    let samples = recorder.stop();
    if samples.is_empty() {
        return Err("no audio captured".into());
    }
    registry::ensure_dirs().map_err(|e| e.to_string())?;
    let wav_path = registry::audio_dir().join("last.wav");
    capture::write_wav(&wav_path, &samples)?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);
    let (vocabulary, stt_source, stt_base_url, stt_api_key, stt_cloud_model, use_gpu) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (
            cfg.vocabulary.clone(),
            cfg.stt_source.clone(),
            cfg.stt_base_url.clone(),
            cfg.stt_api_key.clone(),
            cfg.stt_cloud_model.clone(),
            cfg.use_gpu,
        )
    };
    transcription::whisper::transcribe_dispatch(
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
}

#[tauri::command]
fn paste_text(text: String) -> Result<(), String> {
    paste::paste_at_cursor(&text)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn hide_overlay(app: AppHandle) {
    system::overlay::hide_overlay(&app);
}

#[tauri::command]
fn show_overlay(app: AppHandle) {
    system::overlay::show_overlay(&app);
}

#[tauri::command]
fn set_overlay_size(app: AppHandle, width: f64, height: f64) {
    system::overlay::animate_resize(&app, width, height);
}

/// Broadcast the overlay opacity (0-100) to the overlay window.
#[tauri::command]
fn set_overlay_opacity(app: AppHandle, value: f64) {
    use tauri::Emitter;
    let _ = app.emit("overlay://opacity", value);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::log_info("app", &format!("Silent Voice starting (v{})", env!("CARGO_PKG_VERSION")));

    // Stop WebView2 from suspending/blanking occluded background windows — this
    // is what made the always-on-top overlay vanish after a while.
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--disable-features=CalculateNativeWinOcclusion",
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // Route between the dictation hotkey and the read-aloud
                    // (TTS) hotkey — both are registered globally.
                    let tts_hotkey = app
                        .try_state::<AppState>()
                        .and_then(|s| s.config.lock().ok().map(|c| c.tts_hotkey.clone()))
                        .unwrap_or_default();
                    let is_tts = Shortcut::from_str(&tts_hotkey)
                        .map(|s| s == *shortcut)
                        .unwrap_or(false);
                    if is_tts {
                        if let ShortcutState::Pressed = event.state() {
                            system::tts::read_selection(app);
                        }
                    } else {
                        match event.state() {
                            ShortcutState::Pressed => hotkey::on_pressed(app),
                            ShortcutState::Released => hotkey::on_released(app),
                        }
                    }
                })
                .build(),
        )
        .manage(AppState::default())
        // Closing the dashboard hides it to the tray instead of destroying the
        // window — otherwise "Open Dashboard" (tray) has nothing left to show
        // and the app can never surface its UI again without a restart.
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let _ = registry::ensure_dirs();
            tray::build_tray(app.handle())?;
            overlay::create_overlay(app.handle())?;

            // Force the main window to be visible, centered, and focused.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.center();
                let _ = win.show();
                let _ = win.set_focus();
            }

            // Register the default push-to-talk + read-aloud hotkeys.
            let defaults = RuntimeConfig::default();
            if let Ok(shortcut) = Shortcut::from_str(&defaults.hotkey) {
                let _ = app.global_shortcut().register(shortcut);
            }
            if let Ok(shortcut) = Shortcut::from_str(&defaults.tts_hotkey) {
                let _ = app.global_shortcut().register(shortcut);
            }

            // Inline proofreading watcher (squiggles in any app's text field).
            system::inline_check::start(app.handle().clone());

            // Keep-alive: periodically re-assert the overlay as visible +
            // topmost so it never silently disappears (unless the user hid it).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    overlay::ensure_visible(&handle);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_hardware_info,
            list_input_devices,
            update_runtime_config,
            set_text_replacements,
            set_behavior,
            set_app_profiles,
            set_autostart,
            get_autostart,
            set_hotkey,
            set_tts,
            tts_read_selection,
            tts_stop,
            tts_speak_text,
            list_downloaded_tts,
            proofread_text,
            download_tts_model,
            delete_tts_model,
            set_active_mode,
            ollama_status,
            ollama_generate,
            api_generate,
            api_list_models,
            api_test_stt,
            get_data_location,
            set_data_location,
            pick_folder,
            open_data_folder,
            list_downloaded_llm,
            download_llm_model,
            delete_llm_model,
            local_llm_generate,
            list_downloaded_models,
            download_model,
            delete_model,
            load_history,
            save_history,
            clear_history,
            start_recording,
            stop_and_transcribe,
            paste_text,
            quit_app,
            hide_overlay,
            show_overlay,
            set_overlay_size,
            set_overlay_opacity,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Silent Voice")
        .run(|app_handle, event| {
            // Make sure the bundled llama-server is stopped when the app exits.
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Ok(mut server) = state.llama.lock() {
                        server.stop();
                    }
                }
            }
        });
}
