// Transcripts of dropped-in files: one JSON file each, next to history.json.
// Saved after every finished piece, so closing the app mid-way keeps the text
// done so far and Resume can carry on from `done_ms`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Lines per paragraph in the reader and in .txt/.md (~1 min); also how many
/// pieces the job repunctuates together. The page mirrors it as TRANSCRIPT_PARAGRAPH.
pub const PARAGRAPH: usize = 6;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Line {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    /// Who said it, in a recorded meeting; absent for files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<Speaker>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    /// The microphone.
    You,
    /// The computer's sound: everyone else on the call.
    Others,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    File,
    Meeting,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transcript {
    pub id: i64,
    #[serde(default)]
    pub kind: Kind,
    /// File name shown in the list ("lecture.mp3").
    pub name: String,
    /// Full path, so Resume can reopen the file.
    pub source: String,
    pub created: i64,
    pub duration_ms: Option<u64>,
    pub model_id: String,
    pub status: Status,
    #[serde(default)]
    pub error: Option<String>,
    /// Audio handled so far: Resume starts here.
    pub done_ms: u64,
    /// How long the work took, summed over runs.
    #[serde(default)]
    pub work_ms: u64,
    pub lines: Vec<Line>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Done,
    /// Cancelled, or the app closed mid-way: offers Resume.
    Stopped,
    Failed,
}

pub fn dir() -> PathBuf {
    crate::models::registry::history_path().parent().expect("history.json sits in the data folder").join("transcripts")
}

fn path_in(dir: &Path, id: i64) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// An id for a new transcript: its start time in ms, bumped past any taken one.
pub fn new_id() -> Result<i64, String> {
    let mut id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64;
    while path_in(&dir(), id).exists() {
        id += 1;
    }
    Ok(id)
}

pub fn save(t: &Transcript) -> Result<(), String> {
    save_in(&dir(), t)
}

/// Write via a temp file + rename, so a crash mid-write never leaves half a file.
fn save_in(dir: &Path, t: &Transcript) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{}.json.tmp", t.id));
    std::fs::write(&tmp, serde_json::to_vec(t).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path_in(dir, t.id)).map_err(|e| e.to_string())
}

pub fn load(id: i64) -> Result<Transcript, String> {
    let bytes = std::fs::read(path_in(&dir(), id)).map_err(|_| "That transcript no longer exists.".to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Every transcript in full, newest first.
fn all_in(dir: &Path) -> Vec<Transcript> {
    let mut out: Vec<Transcript> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

/// Newest first, headers only (`lines` empty): the page loads the one it opens.
// ponytail: still parses every file; keep a small index file if the list gets slow.
pub fn list() -> Vec<Transcript> {
    let mut out = all_in(&dir());
    out.iter_mut().for_each(|t| t.lines = Vec::new());
    out
}

pub fn delete(id: i64) -> Result<(), String> {
    for s in [Speaker::You, Speaker::Others] {
        let _ = std::fs::remove_file(audio_path(id, s));
    }
    std::fs::remove_file(path_in(&dir(), id)).map_err(|e| e.to_string())
}

/// A recorded meeting's audio, one 16 kHz WAV per track.
pub fn audio_path(id: i64, speaker: Speaker) -> PathBuf {
    dir().join(format!("{id}-{}.wav", if speaker == Speaker::You { "you" } else { "others" }))
}

/// At startup nothing is running, so a "running" transcript means the app was
/// closed mid-way: mark it stopped so it offers Resume.
pub fn mark_interrupted() {
    mark_interrupted_in(&dir());
}

fn mark_interrupted_in(dir: &Path) {
    for mut t in all_in(dir) {
        if t.status == Status::Running {
            t.status = Status::Stopped;
            let _ = save_in(dir, &t);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Txt,
    Md,
    Srt,
}

impl Format {
    /// From a file name: only .txt, .md and .srt are ever written.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "txt" => Some(Self::Txt),
            "md" => Some(Self::Md),
            "srt" => Some(Self::Srt),
            _ => None,
        }
    }
}

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

/// Runs of the same speaker, at most PARAGRAPH lines each: one paragraph in
/// the reader and the exports, and one punctuation repair for meetings.
pub fn paragraphs(lines: &[Line]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    for i in 1..=lines.len() {
        if i == lines.len() || i - from == PARAGRAPH || lines[i].speaker != lines[from].speaker {
            out.push(from..i);
            from = i;
        }
    }
    out
}

fn label(l: &Line) -> &'static str {
    match l.speaker {
        Some(Speaker::You) => "You: ",
        Some(Speaker::Others) => "Others: ",
        None => "",
    }
}

/// The transcript as .txt (paragraphs), .md (timestamped paragraphs) or .srt.
pub fn render(t: &Transcript, format: Format) -> String {
    match format {
        Format::Srt => t
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}\n{} --> {}\n{}{}\n", i + 1, clock(l.start_ms, true), clock(l.end_ms, true), label(l), l.text))
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Md => {
            let mut s = format!("# {}\n\n", t.name);
            for r in paragraphs(&t.lines) {
                let p = &t.lines[r];
                s += &format!("**[{}]** {}{}\n\n", clock(p[0].start_ms, false), label(&p[0]), join(p));
            }
            s
        }
        Format::Txt => {
            let paras: Vec<String> = paragraphs(&t.lines).into_iter().map(|r| format!("{}{}", label(&t.lines[r.start]), join(&t.lines[r]))).collect();
            paras.join("\n\n") + "\n"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Transcript {
        Transcript {
            kind: Kind::File,
            id: 1,
            name: "talk.mp3".into(),
            source: "C:/talk.mp3".into(),
            created: 1,
            duration_ms: Some(3_725_500),
            model_id: "parakeet".into(),
            status: Status::Done,
            error: None,
            done_ms: 3_725_500,
            work_ms: 0,
            lines: vec![
                Line { start_ms: 0, end_ms: 11_900, text: "First part.".into(), speaker: None },
                Line { start_ms: 3_661_005, end_ms: 3_725_500, text: "Last part.".into(), speaker: None },
            ],
        }
    }

    #[test]
    fn srt_has_numbered_cues_with_comma_milliseconds() {
        assert_eq!(
            render(&sample(), Format::Srt),
            "1\n00:00:00,000 --> 00:00:11,900\nFirst part.\n\n2\n01:01:01,005 --> 01:02:05,500\nLast part.\n"
        );
    }

    #[test]
    fn md_and_txt_group_lines_into_paragraphs() {
        let mut t = sample();
        assert_eq!(render(&t, Format::Md), "# talk.mp3\n\n**[0:00]** First part. Last part.\n\n");
        // A seventh line starts the second paragraph.
        for i in 0..5 {
            t.lines.push(Line { start_ms: 3_725_500 + i * 1000, end_ms: 3_726_000 + i * 1000, text: format!("L{i}."), speaker: None });
        }
        assert_eq!(render(&t, Format::Txt), "First part. Last part. L0. L1. L2. L3.\n\nL4.\n");
        assert!(render(&t, Format::Md).ends_with("**[1:02:09]** L4.\n\n"));
    }

    #[test]
    fn only_txt_md_and_srt_files_are_written() {
        assert_eq!(Format::from_path(Path::new("C:/a/talk.SRT")), Some(Format::Srt));
        assert_eq!(Format::from_path(Path::new("talk.md")), Some(Format::Md));
        assert_eq!(Format::from_path(Path::new("talk.txt")), Some(Format::Txt));
        assert_eq!(Format::from_path(Path::new("talk.exe")), None);
        assert_eq!(Format::from_path(Path::new("talk")), None);
    }

    #[test]
    fn startup_marks_only_running_transcripts_as_stopped() {
        let dir = std::env::temp_dir().join(format!("sv-transcripts-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (id, status) in [(1, Status::Running), (2, Status::Done), (3, Status::Failed)] {
            save_in(&dir, &Transcript { id, status, ..sample() }).unwrap();
        }
        mark_interrupted_in(&dir);
        let after: Vec<(i64, Status, usize)> = all_in(&dir).iter().map(|t| (t.id, t.status, t.lines.len())).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(after, vec![(3, Status::Failed, 2), (2, Status::Done, 2), (1, Status::Stopped, 2)]);
    }

    fn said(speaker: Speaker, start_ms: u64, text: &str) -> Line {
        Line { start_ms, end_ms: start_ms + 2_000, text: text.into(), speaker: Some(speaker) }
    }

    #[test]
    fn paragraphs_split_on_speaker_change_and_length() {
        let mut lines: Vec<Line> = (0..8).map(|i| said(Speaker::Others, i * 3_000, "a b c")).collect();
        lines.push(said(Speaker::You, 30_000, "d e f"));
        assert_eq!(paragraphs(&lines), vec![0..6, 6..8, 8..9]);
    }

    #[test]
    fn meeting_exports_name_the_speaker_once_per_paragraph() {
        let mut t = sample();
        t.lines = vec![said(Speaker::You, 0, "Hi all."), said(Speaker::You, 2_000, "Let's start."), said(Speaker::Others, 5_000, "Sounds good.")];
        assert_eq!(render(&t, Format::Txt), "You: Hi all. Let's start.\n\nOthers: Sounds good.\n");
        assert!(render(&t, Format::Md).contains("**[0:05]** Others: Sounds good."));
        assert!(render(&t, Format::Srt).contains("00:00:05,000 --> 00:00:07,000\nOthers: Sounds good."));
    }
}
