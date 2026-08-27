mod audio;
mod coedit;
mod gector;
mod history;
mod llm;
mod logging;
mod models;
mod onnx;
mod proofread;
mod system;
mod transcription;

use audio::capture::{self, Recorder};
use history::HistoryEntry;
use models::{downloader, registry, hf};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use system::{hardware, hotkey, overlay, tray};
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
    pub mode_source: String, // "none" | "local" (bundled llama-server) | "api" (OpenAI-compatible)
    pub mode_prompt: String,
    pub mode_model: String, // LLM model id / bundled llama-server model
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
    pub coedit_enabled: bool,
    // Transcribe natural speech chunks in background while hotkey is held.
    pub chunk_on_silence: bool,
    pub high_performance: bool,
    // Thread count when high_performance is on. 0 = auto (all cores). Otherwise
    // the user's chosen count, clamped to [default, all cores] in hotkey.rs.
    pub performance_threads: u32,
    // Harper rule ids the user turned off (Settings → Inline proofreading).
    pub proofread_disabled_rules: Vec<String>,
    pub gector_sensitivity: String,
    // Extra exe-name substrings (lowercase) where squiggles are suppressed.
    pub proofread_ignore_apps: Vec<String>,
    // Read-aloud (TTS): active Piper voice id + the hotkey that reads the
    // current text selection. See system/tts.rs.
    pub tts_voice_id: String,
    pub tts_hotkey: String,
    pub tts_enabled: bool,
    // Per-app profiles: when the focused app matches, that profile's AI mode
    // overrides the globally active one. Resolved by the frontend (like
    // set_active_mode) so Rust never needs the mode/provider tables.
    pub app_profiles: Vec<AppProfile>,
    // When true, the pill hides itself ~5s after returning to idle and
    // reappears on the next hotkey press. When false (default), it stays
    // visible the whole time the app is running, as before.
    pub pill_auto_hide: bool,
    // When true, a single trailing space is appended to the pasted text so the
    // next dictation doesn't butt up against this one. History stores the
    // clean text (no trailing space) — the space is a paste-time nicety only.
    pub append_trailing_space: bool,
    pub save_audio: bool,
    pub audio_clip_limit: usize,
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
            coedit_enabled: true,
            chunk_on_silence: false,
            high_performance: false,
            performance_threads: 0,
            proofread_disabled_rules: Vec::new(),
            gector_sensitivity: "balanced".into(),
            proofread_ignore_apps: Vec::new(),
            tts_voice_id: String::new(),
            tts_hotkey: "Ctrl+Alt+S".into(),
            tts_enabled: true,
            app_profiles: Vec::new(),
            pill_auto_hide: false,
            append_trailing_space: false,
            save_audio: true,
            audio_clip_limit: 20,
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
    pub whisper_server: Mutex<transcription::server::WhisperServer>,
    /// Resident sherpa-onnx STT recognizer (Moonshine or SenseVoice), keyed by
    /// "<model_id>|<threads>" so it loads once and reloads only when the model
    /// or thread count changes.
    pub sherpa_stt: Mutex<Option<(String, Arc<system::sherpa_stt::SherpaSttEngine>)>>,
    pub segmenter: Arc<audio::segmenter::Segmenter>,
    /// True only when the user explicitly hid the overlay (menu/tray). The
    /// keep-alive loop respects this and won't force it back.
    pub overlay_hidden: AtomicBool,
    /// Bumped on each overlay resize so an in-flight tween knows it's superseded.
    pub overlay_resize_gen: AtomicU64,
    /// Bumped whenever the pill has a reason to stay visible (recording
    /// starts, or a fresh idle period begins) so a stale auto-hide timer from
    /// an earlier idle period knows it's been superseded and does nothing.
    pub pill_activity_gen: AtomicU64,
    /// Double-tap hotkey lock state.
    pub tap: Mutex<TapState>,
    /// Exe basename of the app focused when recording started (per-app profiles).
    pub active_app: Mutex<String>,
    /// Raw HWND (as isize) of the window focused when recording started, so the
    /// paste path can refuse to type into a different window if focus was
    /// stolen mid-processing. 0 = unknown.
    pub target_hwnd: std::sync::atomic::AtomicIsize,
    /// Read-aloud playback state (see system/tts.rs).
    pub tts: system::tts::TtsState,
    pub download_cancels: Mutex<std::collections::HashMap<String, Arc<downloader::DownloadStopFlag>>>,
    pub download_locks: Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Serializes ALL speech-to-text inference to one at a time. The background
    /// chunk worker and the final tail must never decode on the shared engine
    /// concurrently — whisper-server and the sherpa recognizer both corrupt
    /// output (garbled/truncated text) under concurrent decode.
    pub stt_gate: tokio::sync::Mutex<()>,
}

impl AppState {
    pub async fn register_download(&self, model_id: &str) -> Result<(Arc<downloader::DownloadStopFlag>, tokio::sync::OwnedMutexGuard<()>), String> {
        let lock = {
            let mut locks = self.download_locks.lock().map_err(|e| e.to_string())?;
            locks.entry(model_id.to_string()).or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))).clone()
        };
        let guard = lock.lock_owned().await;
        let mut map = self.download_cancels.lock().map_err(|e| e.to_string())?;
        let flag = Arc::new(downloader::DownloadStopFlag::default());
        map.insert(model_id.to_string(), flag.clone());
        Ok((flag, guard))
    }

    pub fn unregister_download(&self, model_id: &str) {
        if let Ok(mut map) = self.download_cancels.lock() {
            map.remove(model_id);
        }
    }
}

pub struct DownloadGuard<'a>(pub &'a AppState, pub String, pub tokio::sync::OwnedMutexGuard<()>);
impl<'a> Drop for DownloadGuard<'a> {
    fn drop(&mut self) {
        self.0.unregister_download(&self.1);
    }
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
    let (high_performance, performance_threads) = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (cfg.high_performance, cfg.performance_threads)
    };
    let threads = hotkey::resolve_thread_count(high_performance, performance_threads);

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
fn recommend_device_defaults() -> hardware::DeviceRecommendation {
    hardware::recommend(&hardware::detect())
}

/// One pasteable blob of app + system info and the tail of the log, so a
/// non-technical user reporting a problem can share everything at once.
#[tauri::command]
fn copy_diagnostics(state: State<AppState>) -> String {
    let hw = hardware::detect();
    let (model, source, lang, gpu, hp, threads, mode) = state
        .config
        .lock()
        .map(|c| {
            (
                c.model_id.clone(),
                c.stt_source.clone(),
                c.language.clone(),
                c.use_gpu,
                c.high_performance,
                c.performance_threads,
                c.mode_id.clone(),
            )
        })
        .unwrap_or_default();
    format!(
        "Silent Voice diagnostics\n\
         version: {ver}\n\
         os: {os}\n\
         cpu: {cpu} ({phys} cores / {logi} threads)\n\
         ram: {ram_total:.1} GB total, {ram_free:.1} GB free\n\
         gpu: {gpu_name}\n\
         disk free: {disk:.0} GB\n\
         avx2: {avx2}  avx512: {avx512}\n\
         --- settings ---\n\
         stt source: {source}\n\
         speech model: {model}\n\
         language: {lang}\n\
         use gpu: {gpu}\n\
         high performance: {hp} (threads: {threads})\n\
         active mode: {mode}\n\
         --- recent log ---\n{log}\n",
        ver = env!("CARGO_PKG_VERSION"),
        os = hw.os,
        cpu = hw.cpu_brand,
        phys = hw.physical_cores,
        logi = hw.logical_cores,
        ram_total = hw.total_ram_gb,
        ram_free = hw.available_ram_gb,
        gpu_name = hw.gpu_name.clone().unwrap_or_else(|| "none detected".into()),
        disk = hw.free_disk_gb,
        avx2 = hw.has_avx2,
        avx512 = hw.has_avx512,
        source = source,
        model = if model.trim().is_empty() { "(none selected)".to_string() } else { model },
        lang = lang,
        gpu = gpu,
        hp = hp,
        threads = threads,
        mode = if mode.trim().is_empty() { "raw".to_string() } else { mode },
        log = crate::logging::recent(60),
    )
}

#[tauri::command]
fn list_input_devices() -> Vec<String> {
    capture::list_input_devices()
}

static MIC_PROBE: Mutex<Option<std::sync::mpsc::Sender<capture::Control>>> = Mutex::new(None);

#[tauri::command]
fn start_mic_probe(app: AppHandle, device: Option<String>) -> Result<(), String> {
    stop_mic_probe()?;
    let tx = capture::start_level_probe(device, move |level| {
        let _ = app.emit("mic://level", level);
    });
    *MIC_PROBE.lock().map_err(|e| e.to_string())? = Some(tx);
    Ok(())
}

#[tauri::command]
fn stop_mic_probe() -> Result<(), String> {
    if let Some(tx) = MIC_PROBE.lock().map_err(|e| e.to_string())?.take() {
        let _ = tx.send(capture::Control::Stop);
    }
    Ok(())
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
    high_performance: bool,
    performance_threads: u32,
    proofread_disabled_rules: Vec<String>,
    gector_sensitivity: String,
    proofread_ignore_apps: Vec<String>,
    pill_auto_hide: bool,
    append_trailing_space: bool,
    coedit_enabled: bool,
    chunk_on_silence: bool,
    save_audio: bool,
    audio_clip_limit: usize,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.toggle_mode = toggle_mode;
    cfg.input_sensitivity = input_sensitivity.min(100);
    cfg.inline_proofread = inline_proofread;
    cfg.high_performance = high_performance;
    cfg.performance_threads = performance_threads;
    cfg.proofread_disabled_rules = proofread_disabled_rules;
    cfg.gector_sensitivity = gector_sensitivity;
    cfg.proofread_ignore_apps = proofread_ignore_apps
        .into_iter()
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
        .collect();
    cfg.pill_auto_hide = pill_auto_hide;
    cfg.append_trailing_space = append_trailing_space;
    let coedit_was_enabled = cfg.coedit_enabled;
    cfg.coedit_enabled = coedit_enabled;
    cfg.chunk_on_silence = chunk_on_silence;
    cfg.save_audio = save_audio;
    cfg.audio_clip_limit = audio_clip_limit;
    if coedit_enabled && !coedit_was_enabled {
        std::thread::spawn(coedit::prewarm);
    }
    Ok(())
}

/// Active accent palette for the inline-proofread suggestion popup. Index maps
/// to squiggle::PALETTES order (violet, teal, amber-blue, orange, brightness).
/// Theme is a popup-only visual choice, so it lives here rather than in
/// RuntimeConfig — the frontend persists it and re-pushes on startup.
#[tauri::command]
fn set_popup_theme(theme: String) -> Result<(), String> {
    let idx = match theme.as_str() {
        "violet" => 0,
        "teal" => 1,
        "amber-blue" => 2,
        "orange" => 3,
        "brightness" => 4,
        _ => 0,
    };
    #[cfg(windows)]
    system::squiggle::set_theme(idx);
    #[cfg(not(windows))]
    let _ = idx;
    Ok(())
}


/// Card surface for the inline-proofread popup: "dark" (#151a26, the original
/// look) or "light" (#ffffff). Popup-only like the theme/style above, so the
/// frontend persists it and re-pushes on startup.
#[tauri::command]
fn set_popup_surface(surface: String) -> Result<(), String> {
    let idx = match surface.as_str() {
        "light" => 1,
        _ => 0,
    };
    #[cfg(windows)]
    system::squiggle::set_surface(idx);
    #[cfg(not(windows))]
    let _ = idx;
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
fn accessibility_granted() -> bool {
    system::accessibility::is_trusted()
}

#[tauri::command]
fn open_accessibility_settings() {
    system::accessibility::open_settings()
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
fn set_tts(app: AppHandle, state: State<AppState>, voice_id: String, hotkey: String, enabled: bool) -> Result<(), String> {
    // Store the voice FIRST, unconditionally — a hotkey problem must never
    // leave the voice unset (that bug made every TTS action report
    // "No voice downloaded" even with voices installed).
    let prev = {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.tts_voice_id = voice_id;
        cfg.tts_hotkey.clone()
    };
    let prev_enabled = { let c = state.config.lock().map_err(|e| e.to_string())?; c.tts_enabled };
    let was_registered = prev_enabled;              // prev hotkey was registered iff it was enabled (and valid)
    let want_registered = enabled;
    if prev != hotkey || prev_enabled != enabled {
        // Validate the new hotkey up-front only if we intend to register it.
        let new_shortcut = if want_registered {
            match Shortcut::from_str(&hotkey) {
                Ok(s) => Some(s),
                Err(_) => {
                    let msg = format!("Read-aloud hotkey '{hotkey}' is not valid — keeping '{prev}'.");
                    crate::logging::log_error("tts", &msg);
                    let _ = app.emit("pipeline://error", msg.clone());
                    return Err(msg);
                }
            }
        } else { None };
        // Unregister the previously-registered hotkey.
        if was_registered {
            if let Ok(s) = Shortcut::from_str(&prev) { let _ = app.global_shortcut().unregister(s); }
        }
        // Register the new one if enabled.
        if let Some(s) = new_shortcut {
            if let Err(e) = app.global_shortcut().register(s) {
                // Roll back to the previous registration.
                if was_registered { if let Ok(ps) = Shortcut::from_str(&prev) { let _ = app.global_shortcut().register(ps); } }
                let msg = format!("Could not register read-aloud hotkey '{hotkey}' ({e}) — keeping '{prev}'.");
                crate::logging::log_error("tts", &msg);
                let _ = app.emit("pipeline://error", msg.clone());
                return Err(msg);
            }
        }
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.tts_hotkey = hotkey;
        cfg.tts_enabled = enabled;
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
fn tts_pause(app: AppHandle) {
    system::tts::pause(&app);
}

#[tauri::command]
fn tts_resume(app: AppHandle) {
    system::tts::resume(&app);
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
    let (vocabulary, disabled_rules, gector_sensitivity) = state
        .config
        .lock()
        .map(|c| (c.vocabulary.clone(), c.proofread_disabled_rules.clone(), c.gector_sensitivity.clone()))
        .unwrap_or_default();
    tokio::task::spawn_blocking(move || proofread::check(&text, &vocabulary, &disabled_rules, &gector_sensitivity))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn pause_download(state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    let map = state.download_cancels.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = map.get(&model_id) {
        flag.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
fn cancel_download(app: AppHandle, state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    let mut active = false;
    {
        let map = state.download_cancels.lock().map_err(|e| e.to_string())?;
        if let Some(flag) = map.get(&model_id) {
            flag.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            flag.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            active = true;
        }
    }
    if !active {
        downloader::clean_part_and_emit_cancelled(&app, &model_id);
    }
    Ok(())
}

#[tauri::command]
async fn download_tts_model(
    app: AppHandle,
    state: State<'_, AppState>,
    voice_id: String,
    url_onnx: String,
    url_json: String,
) -> Result<bool, String> {
    let (flag, lock_guard) = state.register_download(&voice_id).await?;
    let _guard = DownloadGuard(&state, voice_id.clone(), lock_guard);
    downloader::download_tts_model(app, voice_id, url_onnx, url_json, Some(&flag)).await
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


// ---------------- Local LLM (bundled llama.cpp) ----------------

#[tauri::command]
fn list_downloaded_llm() -> Vec<String> {
    registry::list_downloaded_llm()
}

#[tauri::command]
async fn download_llm_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    let (flag, lock_guard) = state.register_download(&model_id).await?;
    let _guard = DownloadGuard(&state, model_id.clone(), lock_guard);
    downloader::download_llm_model(app, model_id, url, Some(&flag)).await
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
    state: State<'_, AppState>,
    model_id: String,
    url: String,
    file_name: String,
) -> Result<bool, String> {
    let (flag, lock_guard) = state.register_download(&model_id).await?;
    let _guard = DownloadGuard(&state, model_id.clone(), lock_guard);
    downloader::download_model(app, model_id, url, file_name, Some(&flag)).await
}

/// Download a sherpa STT model archive (Moonshine) and extract it. Separate
/// from download_model because it's a multi-file .tar.bz2, not a single .bin.
#[tauri::command]
async fn download_stt_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    let (flag, lock_guard) = state.register_download(&model_id).await?;
    let _guard = DownloadGuard(&state, model_id.clone(), lock_guard);
    downloader::download_stt_archive(app, model_id, url, Some(&flag)).await
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

#[derive(serde::Serialize)]
struct RetranscribeResult {
    text: String,
    model_id: String,
}

/// Re-run transcription on a saved history clip using the CURRENTLY active STT
/// model + settings, then apply the same deterministic text cleanup a fresh
/// dictation gets (repeat-collapse, replacements, number formatting). Lets the
/// user re-transcribe an old clip with a model they've since switched to.
#[tauri::command]
async fn retranscribe_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    file_name: String,
) -> Result<RetranscribeResult, String> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
        return Err("invalid clip name".into());
    }
    let wav = registry::audio_clips_dir().join(&file_name);
    if !wav.exists() {
        return Err("this recording's audio was not saved, so it can't be re-transcribed".into());
    }

    let (
        model_id,
        language,
        vocabulary,
        use_gpu,
        high_performance,
        performance_threads,
        stt_source,
        stt_base_url,
        stt_api_key,
        stt_cloud_model,
        replacements,
    ) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (
            cfg.model_id.clone(),
            cfg.language.clone(),
            cfg.vocabulary.clone(),
            cfg.use_gpu,
            cfg.high_performance,
            cfg.performance_threads,
            cfg.stt_source.clone(),
            cfg.stt_base_url.clone(),
            cfg.stt_api_key.clone(),
            cfg.stt_cloud_model.clone(),
            cfg.replacements.clone(),
        )
    };
    if model_id.is_empty() {
        return Err("no speech model is selected — pick one in the Model Store first".into());
    }

    let threads = hotkey::resolve_thread_count(high_performance, performance_threads);
    let raw = transcription::whisper::transcribe_dispatch(
        &app,
        &wav,
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
    .await?;

    let text = system::textfmt::strip_fillers(&raw);
    let text = system::textfmt::collapse_repeated_words(&text);
    let text = hotkey::apply_replacements(&text, &replacements);
    let text = system::textfmt::format_numbers(&text);
    Ok(RetranscribeResult { text, model_id })
}

/// Returns the clip as a RAW binary IPC response, not a serialised Vec<u8>.
/// A plain Vec<u8> crosses the bridge as a JSON array of numbers, which the
/// frontend cannot hand to Blob() — it stringifies to "82,73,70,70,..." and
/// produces an undecodable text blob. It is also several times larger on the
/// wire than the audio it carries.
#[tauri::command]
fn read_audio_clip(file_name: String) -> Result<tauri::ipc::Response, String> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
        return Err("Invalid file name".into());
    }
    let path = registry::audio_clips_dir().join(&file_name);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
fn copy_audio_file(file_name: String) -> Result<(), String> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
        return Err("Invalid file name".into());
    }
    let path = registry::audio_clips_dir().join(&file_name);
    system::clipboard_file::copy_audio_file(&path)
}

#[tauri::command]
fn prune_audio_clips(keep: usize) -> Result<(), String> {
    history::prune_clips(keep);
    Ok(())
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
    state.segmenter.reset();
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
    let (vocabulary, stt_source, stt_base_url, stt_api_key, stt_cloud_model, use_gpu, threads) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let threads = hotkey::resolve_thread_count(cfg.high_performance, cfg.performance_threads);
        (
            cfg.vocabulary.clone(),
            cfg.stt_source.clone(),
            cfg.stt_base_url.clone(),
            cfg.stt_api_key.clone(),
            cfg.stt_cloud_model.clone(),
            cfg.use_gpu,
            threads,
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

// The webview's navigator.clipboard.writeText() needs a clipboard permission
// WebView2 has no prompt UI for in a desktop app, so it can silently fail.
// Going through arboard (already a dependency for paste) writes to the OS
// clipboard directly and always works.
#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text))
        .map_err(|e| e.to_string())
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

/// Tell WebView2 to shed renderer memory while the dashboard is hidden in the
/// tray, and go back to normal when it's shown (perf roadmap item 10).
fn set_webview_memory_low(app: &AppHandle, low: bool) {
    #[cfg(windows)]
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.with_webview(move |webview| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
            };
            use windows_core_061::Interface;
            if let Ok(core) = webview.controller().CoreWebView2() {
                if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
                    let level = if low {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                    } else {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                    };
                    let _ = wv19.SetMemoryUsageTargetLevel(level);
                }
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (app, low);
}

#[tauri::command]
async fn download_coedit_model(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let model_id = "coedit".to_string();
    let (flag, lock_guard) = state.register_download(&model_id).await?;
    let _guard = DownloadGuard(&state, model_id, lock_guard);
    models::downloader::download_coedit_model(app, Some(&flag)).await
}
#[tauri::command]
fn coedit_installed() -> bool { models::registry::coedit_installed() }
#[tauri::command]
fn delete_coedit_model() -> Result<(), String> { models::downloader::delete_coedit_model() }

#[tauri::command]
async fn download_gector_model(app: AppHandle, state: State<'_, AppState>, variant: String) -> Result<bool, String> {
    let model_id = "gector".to_string();
    let (flag, lock_guard) = state.register_download(&model_id).await?;
    let _guard = DownloadGuard(&state, model_id, lock_guard);
    models::downloader::download_gector_model(app, variant, Some(&flag)).await
}
#[tauri::command]
fn gector_installed() -> bool { models::registry::gector_installed() }
#[tauri::command]
fn delete_gector_model() -> Result<(), String> { models::downloader::delete_gector_model() }

#[tauri::command]
async fn download_vad_model(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let model_id = "vad".to_string();
    let (flag, lock_guard) = state.register_download(&model_id).await?;
    let _guard = DownloadGuard(&state, model_id, lock_guard);
    models::downloader::download_vad_model(app, Some(&flag)).await
}
#[tauri::command]
fn vad_installed() -> bool { models::registry::vad_installed() }
#[tauri::command]
fn delete_vad_model() -> Result<(), String> { models::downloader::delete_vad_model() }


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
                            ShortcutState::Released => hotkey::on_released(app, shortcut),
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
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                        set_webview_memory_low(window.app_handle(), true);
                    }
                    // Any focus regain means the window is visible again.
                    tauri::WindowEvent::Focused(true) => {
                        set_webview_memory_low(window.app_handle(), false);
                    }
                    _ => {}
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
            if defaults.tts_enabled {
                if let Ok(shortcut) = Shortcut::from_str(&defaults.tts_hotkey) {
                    let _ = app.global_shortcut().register(shortcut);
                }
            }

            // Inline proofreading watcher (squiggles in any app's text field).
            #[cfg(windows)]
            system::inline_check::start(app.handle().clone());
            #[cfg(target_os = "macos")]
            system::inline_mac::start(app.handle().clone());

            system::job::reap_orphans();

            // Keep-alive: periodically re-assert the overlay as visible +
            // topmost so it never silently disappears (unless the user hid it).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    overlay::ensure_visible(&handle);
                }
            });

            // Idle-unload sweep (perf roadmap item 7, GECToR half only):
            // whisper-server is intentionally NOT auto-stopped here — killing
            // it means the next dictation pays a multi-second model reload
            // mid-recording, which clips the start of what gets said. GECToR
            // reloads in well under a second (no audio involved), so it's
            // safe to free after 10 min idle.
            tauri::async_runtime::spawn(async move {
                let idle = std::time::Duration::from_secs(600);
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    gector::unload_if_idle(idle);
                    coedit::unload_if_idle(std::time::Duration::from_secs(180));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_hardware_info,
            recommend_device_defaults,
            copy_diagnostics,
            list_input_devices,
            start_mic_probe,
            stop_mic_probe,
            update_runtime_config,
            set_text_replacements,
            set_behavior,
            set_popup_theme,

            set_popup_surface,
            set_app_profiles,
            set_autostart,
            get_autostart,
            accessibility_granted,
            open_accessibility_settings,
            set_hotkey,
            set_tts,
            tts_read_selection,
            tts_stop,
            tts_pause,
            tts_resume,
            tts_speak_text,
            list_downloaded_tts,
            proofread_text,
            download_tts_model,
            delete_tts_model,
            pause_download,
            cancel_download,
            set_active_mode,
            api_generate,
            api_list_models,
            api_test_stt,

            list_downloaded_llm,
            download_llm_model,
            delete_llm_model,
            local_llm_generate,
            list_downloaded_models,
            download_model,
            download_stt_archive,
            delete_model,
            retranscribe_clip,
            load_history,
            save_history,
            clear_history,
            read_audio_clip,
            copy_audio_file,
            prune_audio_clips,
            start_recording,
            stop_and_transcribe,
            copy_to_clipboard,
            quit_app,
            hide_overlay,
            show_overlay,
            set_overlay_size,
            hf::hf_search_models,
            hf::hf_model_details,
            hf::hf_piper_voices,
            download_coedit_model,
            coedit_installed,
            delete_coedit_model,
            download_gector_model,
            gector_installed,
            delete_gector_model,
            download_vad_model,
            vad_installed,
            delete_vad_model,
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
                    if let Ok(mut server) = state.whisper_server.lock() {
                        server.stop();
                    }
                }
                #[cfg(windows)]
                system::inline_check::reset_screen_reader();
            }
        });
}
