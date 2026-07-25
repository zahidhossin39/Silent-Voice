use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Fixed bootstrap location (always C:/Users/.../AppData/Roaming/SilentVoice) ──
// This tiny folder always exists and holds ONLY:
//  • data-location.json  ← where user wants their models/history stored
//  • audio/last.wav      ← temp recording (always C drive, small)
fn bootstrap_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("SilentVoice")
}

fn data_location_file() -> PathBuf {
    bootstrap_dir().join("data-location.json")
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct DataLocation {
    /// Override root for STT + LLM models (e.g. "D:/SilentVoiceData/models").
    pub models_root: Option<String>,
    /// Override root for history.json (e.g. "D:/SilentVoiceData").
    pub history_root: Option<String>,
}

/// Load the user's storage-location preferences (returns defaults if missing).
pub fn load_data_location() -> DataLocation {
    let path = data_location_file();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => DataLocation::default(),
    }
}


// ── Resolved paths (honour overrides, fall back to bootstrap_dir) ──

/// Root data directory (fallback when no override is set).
fn default_data_dir() -> PathBuf {
    bootstrap_dir()
}

/// Where downloaded Whisper GGML models live.
pub fn models_dir() -> PathBuf {
    let loc = load_data_location();
    match loc.models_root {
        Some(p) if !p.is_empty() => PathBuf::from(p).join("stt"),
        _ => default_data_dir().join("models"),
    }
}

/// Where downloaded LLM GGUF models live.
pub fn llm_models_dir() -> PathBuf {
    let loc = load_data_location();
    match loc.models_root {
        Some(p) if !p.is_empty() => PathBuf::from(p).join("llm"),
        _ => default_data_dir().join("llm"),
    }
}

/// Full path for a downloaded LLM model.
pub fn llm_model_path(model_id: &str) -> PathBuf {
    llm_models_dir().join(format!("{model_id}.gguf"))
}

/// Where downloaded TTS (Piper) voices live. Each voice is a pair of files:
/// <id>.onnx (the model) + <id>.onnx.json (its config).
pub fn tts_models_dir() -> PathBuf {
    let loc = load_data_location();
    match loc.models_root {
        Some(p) if !p.is_empty() => PathBuf::from(p).join("tts"),
        _ => default_data_dir().join("tts"),
    }
}

/// Full path for a downloaded TTS voice model.
pub fn tts_model_path(voice_id: &str) -> PathBuf {
    tts_models_dir().join(format!("{voice_id}.onnx"))
}

/// Full path for a TTS voice's config JSON (required by Piper alongside .onnx).
pub fn tts_config_path(voice_id: &str) -> PathBuf {
    tts_models_dir().join(format!("{voice_id}.onnx.json"))
}

/// Directory of a sherpa-onnx voice (extracted archive). Sherpa voices are a
/// whole folder — model .onnx + tokens.txt (+ optional espeak-ng-data) — while
/// Piper voices are a flat .onnx/.onnx.json file pair in tts_models_dir().
pub fn sherpa_voice_dir(voice_id: &str) -> PathBuf {
    tts_models_dir().join(voice_id)
}

/// The .onnx model inside a sherpa voice dir (first *.onnx found), if the
/// voice looks complete (tokens.txt present too).
pub fn sherpa_voice_model(voice_id: &str) -> Option<PathBuf> {
    let dir = sherpa_voice_dir(voice_id);
    if !dir.join("tokens.txt").exists() {
        return None;
    }
    std::fs::read_dir(&dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        (p.extension().and_then(|x| x.to_str()) == Some("onnx")).then_some(p)
    })
}

/// List downloaded TTS voice ids — Piper voices (both files of the pair
/// present) and sherpa voices (directory with tokens.txt + a .onnx).
pub fn list_downloaded_tts() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(tts_models_dir()) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if entry.path().is_dir() {
                    if sherpa_voice_model(name).is_some() {
                        ids.push(name.to_string());
                    }
                } else if let Some(id) = name.strip_suffix(".onnx") {
                    if tts_config_path(id).exists() {
                        ids.push(id.to_string());
                    }
                }
            }
        }
    }
    ids
}

/// Directory for temporary audio captures (always next to the bootstrap dir).
pub fn audio_dir() -> PathBuf {
    bootstrap_dir().join("audio")
}

/// Path to the local history file.
pub fn history_path() -> PathBuf {
    let loc = load_data_location();
    match loc.history_root {
        Some(p) if !p.is_empty() => PathBuf::from(p).join("history.json"),
        _ => default_data_dir().join("history.json"),
    }
}

/// Ensure all resolved app directories exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(bootstrap_dir())?;
    std::fs::create_dir_all(models_dir())?;
    std::fs::create_dir_all(llm_models_dir())?;
    std::fs::create_dir_all(tts_models_dir())?;
    std::fs::create_dir_all(audio_dir())?;
    Ok(())
}

/// Map an STT model id (e.g. "small.en") to its GGML filename.
pub fn model_file_name(model_id: &str) -> String {
    format!("ggml-{model_id}.bin")
}

/// Full path on disk for a downloaded STT model.
pub fn model_path(model_id: &str) -> PathBuf {
    models_dir().join(model_file_name(model_id))
}

/// List downloaded Whisper model ids.
pub fn list_downloaded() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(models_dir()) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(rest) = name.strip_prefix("ggml-") {
                    if let Some(id) = rest.strip_suffix(".bin") {
                        ids.push(id.to_string());
                    }
                }
            }
        }
    }
    ids
}

/// List downloaded LLM model ids.
pub fn list_downloaded_llm() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(llm_models_dir()) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(id) = name.strip_suffix(".gguf") {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids
}

pub fn vad_dir() -> PathBuf {
    models_dir().join("vad")
}

pub fn vad_installed() -> bool {
    vad_dir().join("silero_vad.onnx").exists()
}

pub fn coedit_dir() -> PathBuf {
    models_dir().join("coedit")
}

pub fn coedit_installed() -> bool {
    let d = coedit_dir();
    d.join("encoder_model_int8.onnx").exists()
        && d.join("decoder_model_int8.onnx").exists()
        && d.join("tokenizer.json").exists()
}

pub fn gector_dir() -> PathBuf {
    models_dir().join("gector")
}

// ORT auto-loads the sibling gector.onnx.data external-data file, so only the .onnx path is returned.
pub fn gector_model_path() -> Option<PathBuf> {
    let dir = gector_dir();
    let fp32 = dir.join("gector.onnx");
    if fp32.exists() {
        return Some(fp32);
    }
    let int8 = dir.join("gector-int8.onnx");
    if int8.exists() {
        return Some(int8);
    }
    None
}

pub fn gector_installed() -> bool {
    let dir = gector_dir();
    gector_model_path().is_some()
        && dir.join("tokenizer.json").exists()
        && dir.join("labels.txt").exists()
        && dir.join("verb-form-vocab.txt").exists()
}

