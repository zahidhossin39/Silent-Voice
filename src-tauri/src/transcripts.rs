// Transcripts of dropped-in files: one JSON file each, next to history.json.
// Saved after every finished piece, so closing the app mid-way keeps the text
// done so far and Resume can carry on from `done_ms`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Line {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transcript {
    pub id: i64,
    /// File name shown in the list ("lecture.mp3").
    pub name: String,
    /// Full path, so Resume can reopen the file.
    pub source: String,
    pub created: i64,
    pub duration_ms: Option<u64>,
    pub model_id: String,
    /// "running" | "done" | "stopped" | "failed"
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    /// Audio handled so far: Resume starts here.
    pub done_ms: u64,
    /// How long the work took, summed over runs.
    #[serde(default)]
    pub work_ms: u64,
    pub lines: Vec<Line>,
}

pub fn dir() -> PathBuf {
    crate::models::registry::history_path()
        .parent()
        .map(|p| p.join("transcripts"))
        .unwrap_or_else(|| PathBuf::from("transcripts"))
}

fn path(id: i64) -> PathBuf {
    dir().join(format!("{id}.json"))
}

/// Write via a temp file + rename, so a crash mid-write never leaves half a file.
pub fn save(t: &Transcript) -> Result<(), String> {
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    let tmp = dir().join(format!("{}.json.tmp", t.id));
    std::fs::write(&tmp, serde_json::to_vec(t).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path(t.id)).map_err(|e| e.to_string())
}

pub fn load(id: i64) -> Result<Transcript, String> {
    let bytes = std::fs::read(path(id)).map_err(|_| "That transcript no longer exists.".to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Newest first. The list view only needs the header, but a few hundred KB of
/// JSON per transcript reads fast enough to keep this one function.
pub fn list() -> Vec<Transcript> {
    let mut out: Vec<Transcript> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

pub fn delete(id: i64) -> Result<(), String> {
    std::fs::remove_file(path(id)).map_err(|e| e.to_string())
}

/// At startup nothing is running, so a "running" transcript means the app was
/// closed mid-way: mark it stopped so it offers Resume.
pub fn mark_interrupted() {
    for mut t in list() {
        if t.status == "running" {
            t.status = "stopped".into();
            let _ = save(&t);
        }
    }
}

use crate::transcribe_job::PARAGRAPH;

fn join(lines: &[Line]) -> String {
    lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn clock(ms: u64, srt: bool) -> String {
    let (h, m, s) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60);
    if srt {
        format!("{h:02}:{m:02}:{s:02},{:03}", ms % 1000)
    } else if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The transcript as .txt (paragraphs), .md (timestamped paragraphs) or .srt.
pub fn render(t: &Transcript, format: &str) -> String {
    match format {
        "srt" => t
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}\n{} --> {}\n{}\n", i + 1, clock(l.start_ms, true), clock(l.end_ms, true), l.text))
            .collect::<Vec<_>>()
            .join("\n"),
        "md" => {
            let mut s = format!("# {}\n\n", t.name);
            for p in t.lines.chunks(PARAGRAPH) {
                s += &format!("**[{}]** {}\n\n", clock(p[0].start_ms, false), join(p));
            }
            s
        }
        _ => t.lines.chunks(PARAGRAPH).map(join).collect::<Vec<_>>().join("\n\n") + "\n",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Transcript {
        Transcript {
            id: 1,
            name: "talk.mp3".into(),
            source: "C:/talk.mp3".into(),
            created: 1,
            duration_ms: Some(3_725_500),
            model_id: "parakeet".into(),
            status: "done".into(),
            error: None,
            done_ms: 3_725_500,
            work_ms: 0,
            lines: vec![
                Line { start_ms: 0, end_ms: 11_900, text: "First part.".into() },
                Line { start_ms: 3_661_005, end_ms: 3_725_500, text: "Last part.".into() },
            ],
        }
    }

    #[test]
    fn srt_has_numbered_cues_with_comma_milliseconds() {
        assert_eq!(
            render(&sample(), "srt"),
            "1\n00:00:00,000 --> 00:00:11,900\nFirst part.\n\n2\n01:01:01,005 --> 01:02:05,500\nLast part.\n"
        );
    }

    #[test]
    fn md_and_txt_group_lines_into_paragraphs() {
        let mut t = sample();
        assert_eq!(render(&t, "md"), "# talk.mp3\n\n**[0:00]** First part. Last part.\n\n");
        // A seventh line starts the second paragraph.
        for i in 0..5 {
            t.lines.push(Line { start_ms: 3_725_500 + i * 1000, end_ms: 3_726_000 + i * 1000, text: format!("L{i}.") });
        }
        assert_eq!(render(&t, "txt"), "First part. Last part. L0. L1. L2. L3.\n\nL4.\n");
        assert!(render(&t, "md").ends_with("**[1:02:09]** L4.\n\n"));
    }
}
