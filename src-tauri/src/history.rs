use crate::models::registry;
use serde::{Deserialize, Serialize};

/// A single transcription record. Mirrors the frontend `HistoryEntry` type.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: i64,
    pub raw_text: String,
    pub processed_text: String,
    pub mode_id: String,
    pub model_id: String,
    pub duration_ms: i64,
}

const MAX_ENTRIES: usize = 1000;

/// Load all history entries from the local JSON file. Returns an empty list if
/// the file doesn't exist yet or can't be parsed.
pub fn load() -> Vec<HistoryEntry> {
    let path = registry::history_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Overwrite the history file with the given entries (newest first), capped.
pub fn save(mut entries: Vec<HistoryEntry>) -> Result<(), String> {
    entries.truncate(MAX_ENTRIES);
    registry::ensure_dirs().map_err(|e| e.to_string())?;
    let path = registry::history_path();
    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    // Write atomically via a temp file to avoid corrupting on crash.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Prepend a new entry and persist.
pub fn append(entry: HistoryEntry) -> Result<Vec<HistoryEntry>, String> {
    let mut entries = load();
    entries.insert(0, entry);
    save(entries.clone())?;
    Ok(entries)
}

pub fn clear() -> Result<(), String> {
    save(Vec::new())
}
