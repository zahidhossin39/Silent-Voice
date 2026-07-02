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

/// Persist new storage-location preferences.
pub fn save_data_location(loc: &DataLocation) -> Result<(), String> {
    let dir = bootstrap_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(loc).map_err(|e| e.to_string())?;
    std::fs::write(data_location_file(), json).map_err(|e| e.to_string())?;
    Ok(())
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
