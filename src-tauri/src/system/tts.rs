// Read-aloud (text-to-speech) via the bundled Piper engine.
//
// Flow: user selects text anywhere → presses the TTS hotkey → we copy the
// selection (Ctrl+C, clipboard preserved), synthesize a WAV with
// exe_dir/piper/piper.exe, and play it. Pressing the hotkey again while
// speaking stops playback (toggle).
//
// Piper CLI (release 2023.11.14-2): text on stdin,
//   piper.exe --model <voice.onnx> --output_file <out.wav>
// The voice's .onnx.json config must sit next to the .onnx (registry
// guarantees that — both files download together).

use crate::models::registry;
use crate::AppState;
use super::paste::clipboard_modifier;
use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings as EnigoSettings,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub enum TtsCmd {
    Pause,
    Resume,
    Stop,
}

/// Playback bookkeeping stored in AppState.
#[derive(Default)]
pub struct TtsState {
    /// Send a command to the active playback thread (Pause/Resume/Stop).
    pub ctrl: Mutex<Option<Sender<TtsCmd>>>,
    /// True while a playback thread is synthesizing/speaking.
    pub speaking: Arc<AtomicBool>,
    /// True while playback is paused (subset of speaking).
    pub paused: Arc<AtomicBool>,
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default()
}

fn piper_exe() -> PathBuf {
    exe_dir()
        .join("piper")
        .join(if cfg!(windows) { "piper.exe" } else { "piper" })
}


fn report(app: &AppHandle, msg: &str) {
    crate::logging::log_error("tts", msg);
    let _ = app.emit("pipeline://error", msg.to_string());
}

/// Wait (up to ~400ms) until the user physically releases all modifier keys.
///
/// The TTS hotkey (e.g. Ctrl+Alt+S) is still held down when we get here — if
/// we sent our synthetic Ctrl+C immediately, the user's held Alt would merge
/// into it (Ctrl+Alt+C) and nothing would be copied ("Nothing selected" bug).
/// Kept short and runs on a background thread (see `read_selection`) so the
/// pill's "synthesizing" animation is already showing before this returns —
/// it reads as instant instead of a stall.
#[cfg(windows)]
fn wait_modifiers_released() {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // VK_SHIFT, VK_CONTROL, VK_MENU (Alt), VK_LWIN, VK_RWIN
    const VKS: [i32; 5] = [0x10, 0x11, 0x12, 0x5B, 0x5C];
    for _ in 0..20 {
        let held = VKS
            .iter()
            .any(|&vk| (unsafe { GetAsyncKeyState(vk) } as u16) & 0x8000 != 0);
        if !held {
            // Small settle delay so the key-up events finish propagating.
            std::thread::sleep(Duration::from_millis(20));
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(not(windows))]
fn wait_modifiers_released() {}

/// Copy the current selection via Ctrl+C without clobbering the clipboard.
fn copy_selection() -> Result<String, String> {
    wait_modifiers_released();
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let original = clipboard.get_text().ok();
    // Clear so we can tell "nothing was selected" from "old clipboard text".
    let _ = clipboard.clear();

    let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;
    enigo.key(clipboard_modifier(), Press).map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('c'), Click)
        .map_err(|e| e.to_string())?;
    enigo.key(clipboard_modifier(), Release).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(180));

    let selected = clipboard.get_text().unwrap_or_default();

    // Put the user's original clipboard back.
    if let Some(orig) = original {
        let _ = clipboard.set_text(orig);
    }
    Ok(selected.trim().to_string())
}

/// Hotkey entry point: toggle — stop if speaking, otherwise read the selection.
pub fn read_selection(app: &AppHandle) {
    let state = app.state::<AppState>();

    // Toggle off if currently speaking.
    if state.tts.speaking.load(Ordering::SeqCst) {
        if let Ok(mut slot) = state.tts.ctrl.lock() {
            if let Some(tx) = slot.take() {
                let _ = tx.send(TtsCmd::Stop);
            }
        }
        return;
    }

    let voice_id = state
        .config
        .lock()
        .map(|c| c.tts_voice_id.clone())
        .unwrap_or_default();
    if voice_id.is_empty() {
        report(
            app,
            "No voice selected — download one in Model Store → Text to Speech.",
        );
        return;
    }
    let Some(voice) = resolve_voice(&voice_id) else {
        report(
            app,
            &format!("Voice '{voice_id}' is not downloaded — get it in Model Store → Text to Speech."),
        );
        return;
    };

    // Show feedback immediately — the pill turns blue right away, before the
    // (short) modifier-release wait and clipboard copy below, so nothing
    // feels like a stall. Also unblocks the global-shortcut handler thread.
    let _ = app.emit("tts://state", "synthesizing");
    let app = app.clone();
    std::thread::spawn(move || {
        let text = match copy_selection() {
            Ok(t) => t,
            Err(e) => {
                report(&app, &format!("Could not read the selection: {e}"));
                let _ = app.emit("tts://state", "idle");
                return;
            }
        };
        if text.is_empty() {
            report(&app, "Nothing selected — highlight some text, then press the read-aloud hotkey.");
            let _ = app.emit("tts://state", "idle");
            return;
        }
        spawn_playback(&app, voice, text);
    });
}

/// Speak an explicit string with the active voice (Settings "Test voice").
pub fn speak_text(app: &AppHandle, text: String) {
    let state = app.state::<AppState>();
    if state.tts.speaking.load(Ordering::SeqCst) {
        stop(app);
        return;
    }
    let voice_id = state
        .config
        .lock()
        .map(|c| c.tts_voice_id.clone())
        .unwrap_or_default();
    let voice = if voice_id.is_empty() { None } else { resolve_voice(&voice_id) };
    let Some(voice) = voice else {
        report(app, "No voice downloaded — pick one in Model Store → Text-to-Speech.");
        return;
    };
    spawn_playback(app, voice, text);
}

/// A downloaded voice, resolved to whichever bundled engine can speak it.
enum Voice {
    /// Piper: flat .onnx file (its .onnx.json config sits next to it).
    Piper(PathBuf),
    /// sherpa-onnx VITS voice: extracted archive directory. `model` is the
    /// .onnx inside; tokens.txt (and optional espeak-ng-data) live in `dir`.
    Sherpa { dir: PathBuf, model: PathBuf },
    /// Kokoro sherpa-onnx voice: one archive holds many voices, picked by sid.
    Kokoro { dir: PathBuf, model: PathBuf, sid: i32 },
}

fn resolve_voice(voice_id: &str) -> Option<Voice> {
    let (base_id, sid) = match voice_id.split_once('#') {
        Some((a, b)) => (a, b.parse::<i32>().unwrap_or(0)),
        None => (voice_id, 0),
    };
    let piper = registry::tts_model_path(base_id);
    if piper.exists() {
        return Some(Voice::Piper(piper));
    }
    let model = registry::sherpa_voice_model(base_id)?;
    let dir = registry::sherpa_voice_dir(base_id);
    if dir.join("voices.bin").exists() {
        return Some(Voice::Kokoro { dir, model, sid });
    }
    Some(Voice::Sherpa { dir, model })
}

fn spawn_playback(app: &AppHandle, voice: Voice, text: String) {
    let state = app.state::<AppState>();
    // Register a fresh cancel channel (replacing any stale one).
    let (tx, rx) = std::sync::mpsc::channel::<TtsCmd>();
    if let Ok(mut slot) = state.tts.ctrl.lock() {
        *slot = Some(tx);
    }
    let speaking = state.tts.speaking.clone();
    speaking.store(true, Ordering::SeqCst);
    state.tts.paused.store(false, Ordering::SeqCst);

    let paused = state.tts.paused.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let result = synth_and_play(&app, &voice, &text, &rx);
        speaking.store(false, Ordering::SeqCst);
        paused.store(false, Ordering::SeqCst);
        let _ = app.emit("tts://state", "idle");
        if let Err(e) = result {
            report(&app, &format!("Read aloud failed: {e}"));
        }
    });
}

/// Explicit stop (command / future UI button).
pub fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    let tx = state.tts.ctrl.lock().ok().and_then(|mut slot| slot.take());
    if let Some(tx) = tx {
        let _ = tx.send(TtsCmd::Stop);
    }
}

/// Pause the current read-aloud (no-op if not speaking).
pub fn pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.tts.speaking.load(Ordering::SeqCst) { return; }
    if let Ok(slot) = state.tts.ctrl.lock() {
        if let Some(tx) = slot.as_ref() { let _ = tx.send(TtsCmd::Pause); }
    }
    state.tts.paused.store(true, Ordering::SeqCst);
    let _ = app.emit("tts://state", "paused");
}

/// Resume a paused read-aloud.
pub fn resume(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.tts.speaking.load(Ordering::SeqCst) { return; }
    if let Ok(slot) = state.tts.ctrl.lock() {
        if let Some(tx) = slot.as_ref() { let _ = tx.send(TtsCmd::Resume); }
    }
    state.tts.paused.store(false, Ordering::SeqCst);
    let _ = app.emit("tts://state", "speaking");
}

fn synthesize(voice: &Voice, text: &str, wav: &std::path::Path, num_threads: i32) -> Result<(), String> {
    // Both engines get the text flattened to one line so the whole selection
    // is spoken as one passage.
    let one_line = text.replace(['\r', '\n'], " ");

    // sherpa voices go through the in-process C API (see sherpa.rs) — the
    // CLI mangles non-Latin text on Windows (narrow argv → ANSI codepage).
    let model = match voice {
        Voice::Sherpa { dir, model } => return super::sherpa::synthesize(dir, model, &one_line, wav, num_threads, 0),
        Voice::Kokoro { dir, model, sid } => return super::sherpa::synthesize(dir, model, &one_line, wav, num_threads, *sid),
        Voice::Piper(model) => model,
    };

    let exe = piper_exe();
    if !exe.exists() {
        return Err(format!(
            "Piper engine not found at {} — reinstall the app.",
            exe.display()
        ));
    }
    let mut cmd = Command::new(&exe);
    cmd.arg("--model")
        .arg(model)
        .arg("--output_file")
        .arg(wav)
        .stdin(Stdio::piped()) // Piper reads the text from stdin (UTF-8-safe)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| format!("could not start TTS engine: {e}"))?;
    if let Some(stdin) = child.stdin.take() {
        let mut stdin = stdin;
        let _ = stdin.write_all(one_line.as_bytes());
        // stdin drops here, closing the pipe so Piper finishes.
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("TTS engine exited with an error (voice files may be corrupted — re-download the voice).".into());
    }
    Ok(())
}

fn synth_and_play(
    app: &AppHandle,
    voice: &Voice,
    text: &str,
    rx: &Receiver<TtsCmd>,
) -> Result<(), String> {
    let (high_performance, performance_threads) = app
        .state::<AppState>()
        .config
        .lock()
        .map(|c| (c.high_performance, c.performance_threads))
        .unwrap_or((false, 0));
    let threads = super::hotkey::resolve_thread_count(high_performance, performance_threads) as i32;

    let wav = registry::audio_dir().join("tts.wav");

    let _ = app.emit("tts://state", "synthesizing");

    synthesize(voice, text, &wav, threads)?;

    // Cancelled while synthesizing?
    if matches!(rx.try_recv(), Ok(TtsCmd::Stop) | Err(TryRecvError::Disconnected)) {
        return Ok(());
    }

    let _ = app.emit("tts://state", "speaking");

    let (_stream, handle) =
        rodio::OutputStream::try_default().map_err(|e| format!("no audio output: {e}"))?;
    let sink = rodio::Sink::try_new(&handle).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(&wav).map_err(|e| e.to_string())?;
    let source =
        rodio::Decoder::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    sink.append(source);

    loop {
        match rx.try_recv() {
            Ok(TtsCmd::Stop) | Err(TryRecvError::Disconnected) => {
                sink.stop();
                break;
            }
            Ok(TtsCmd::Pause) => sink.pause(),
            Ok(TtsCmd::Resume) => sink.play(),
            Err(TryRecvError::Empty) => {}
        }
        if sink.empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(60));
    }
    Ok(())
}
