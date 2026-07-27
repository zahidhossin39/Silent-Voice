use crate::models::registry;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    #[serde(default)]
    pub audio_ms: Option<i64>,
    #[serde(default)]
    pub audio_file: Option<String>,
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
    
    // Delete any clip file whose entry is no longer present
    let surviving_clips: HashSet<String> = entries
        .iter()
        .filter_map(|e| e.audio_file.clone())
        .collect();
        
    // A clip newer than every surviving entry belongs to a dictation the caller
    // did not know about yet (Rust appends the entry, then the frontend saves a
    // list it built moments earlier) — deleting it would destroy the recording
    // that was just made. An empty list is different: it only comes from an
    // explicit "clear all", which really should take every clip with it.
    let clearing_all = entries.is_empty();
    let newest_entry_id = entries.iter().map(|e| e.id).max().unwrap_or(0);

    let clips_dir = registry::audio_clips_dir();
    if let Ok(dir_entries) = std::fs::read_dir(&clips_dir) {
        for entry in dir_entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".wav") && !surviving_clips.contains(name) {
                    if clearing_all {
                        let _ = std::fs::remove_file(entry.path());
                    } else if let Some(id_str) =
                        name.strip_prefix("rec-").and_then(|s| s.strip_suffix(".wav"))
                    {
                        if let Ok(clip_id) = id_str.parse::<i64>() {
                            if clip_id < newest_entry_id {
                                let _ = std::fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }

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

/// Delete all but the `keep` newest files in `audio_clips_dir()`, 
/// newest determined by the numeric id in the filename.
pub fn prune_clips(keep: usize) {
    let dir = registry::audio_clips_dir();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(id_str) = name.strip_prefix("rec-").and_then(|s| s.strip_suffix(".wav")) {
                    if let Ok(id) = id_str.parse::<i64>() {
                        files.push((id, entry.path()));
                    }
                }
            }
        }
    }
    
    // Sort descending by id (newest first)
    files.sort_by_key(|&(id, _)| std::cmp::Reverse(id));
    
    // Delete files after `keep`
    for (_, path) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // History written before audio clips existed has no `audio_file` key. If it
    // ever fails to deserialise, load() swallows the error and returns an empty
    // list — the user would see their whole history silently disappear.
    #[test]
    fn legacy_entries_without_audio_file_still_load() {
        let legacy = r#"[{
            "id": 1753600000000,
            "timestamp": 1753600000000,
            "raw_text": "hello there",
            "processed_text": "Hello there.",
            "mode_id": "raw",
            "model_id": "tiny.en",
            "duration_ms": 1200
        }]"#;

        let entries: Vec<HistoryEntry> = serde_json::from_str(legacy).expect("legacy history must parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_text, "hello there");
        assert!(entries[0].audio_file.is_none());
    }

    #[test]
    fn entries_with_audio_file_round_trip() {
        let entry = HistoryEntry {
            id: 1,
            timestamp: 1,
            raw_text: "a".into(),
            processed_text: "A.".into(),
            mode_id: "raw".into(),
            model_id: "tiny.en".into(),
            duration_ms: 10,
            audio_ms: Some(1500),
            audio_file: Some("rec-1.wav".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.audio_file.as_deref(), Some("rec-1.wav"));
    }
}
