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
    /// The microphone. Also its track number in a meeting.
    You = 0,
    /// The computer's sound: everyone else on the call.
    Others = 1,
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
    /// The meeting's Overview (Markdown), written by an LLM on request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// A saved meeting being transcribed again from its audio (not recording).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub redo: bool,
}

impl Transcript {
    /// A new, running transcript with no text yet.
    pub fn new(id: i64, kind: Kind, name: String, source: String, model_id: String) -> Self {
        Self {
            id,
            kind,
            name,
            source,
            created: id,
            duration_ms: None,
            model_id,
            status: Status::Running,
            error: None,
            done_ms: 0,
            work_ms: 0,
            lines: Vec::new(),
            summary: None,
            redo: false,
        }
    }
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

/// Meetings whose name, Overview or words contain `query` (any case), newest first.
pub fn search(query: &str) -> Vec<i64> {
    search_in(&dir(), query)
}

fn search_in(dir: &Path, query: &str) -> Vec<i64> {
    let q = query.trim().to_lowercase();
    all_in(dir)
        .into_iter()
        .filter(|t| t.kind == Kind::Meeting)
        .filter(|t| {
            q.is_empty()
                || t.name.to_lowercase().contains(&q)
                || t.summary.as_ref().is_some_and(|s| s.to_lowercase().contains(&q))
                || t.lines.iter().any(|l| l.text.to_lowercase().contains(&q))
        })
        .map(|t| t.id)
        .collect()
}

/// `start_ms..end_ms` of one track as a small WAV, for the page to play.
pub fn audio_clip(id: i64, speaker: Speaker, start_ms: u64, end_ms: u64) -> Result<Vec<u8>, String> {
    let mut r = hound::WavReader::open(audio_path(id, speaker)).map_err(|_| "This meeting's audio wasn't saved.".to_string())?;
    let spec = r.spec();
    let rate = spec.sample_rate as u64;
    let from = (start_ms * rate / 1000).min(r.duration() as u64);
    let n = (end_ms.saturating_sub(start_ms).min(10 * 60_000) * rate / 1000) as usize;
    r.seek(from as u32).map_err(|e| e.to_string())?;
    let mut out = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut out, spec).map_err(|e| e.to_string())?;
    for s in r.samples::<i16>().take(n) {
        w.write_sample(s.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())?;
    Ok(out.into_inner())
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
            t.redo = false;
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

pub(crate) fn join(lines: &[Line]) -> String {
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

/// Runs of the same speaker, about PARAGRAPH lines each: one paragraph in
/// the reader and the exports, and one punctuation repair for meetings.
/// A full paragraph waits for the end of a sentence, up to twice as long, so
/// lines cut every few seconds don't split a sentence across paragraphs.
pub fn paragraphs(lines: &[Line]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    for i in 1..=lines.len() {
        let n = i - from;
        let full = n >= 2 * PARAGRAPH || (n >= PARAGRAPH && ends_sentence(&lines[i - 1].text));
        if i == lines.len() || full || lines[i].speaker != lines[from].speaker {
            out.push(from..i);
            from = i;
        }
    }
    out
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end().trim_end_matches(['"', '\'', '”', '’', ')']).ends_with(['.', '?', '!', '…', '。', '？', '！'])
}

pub(crate) fn label(l: &Line) -> &'static str {
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
            if let Some(sum) = &t.summary {
                s += &format!("{}\n\n## Transcript\n\n", sum.trim());
            }
            for r in paragraphs(&t.lines) {
                let p = &t.lines[r];
                s += &format!("**[{}]** {}{}\n\n", clock(p[0].start_ms, false), label(&p[0]), join(p));
            }
            s
        }
        Format::Txt => {
            let paras: Vec<String> = paragraphs(&t.lines).into_iter().map(|r| format!("{}{}", label(&t.lines[r.start]), join(&t.lines[r]))).collect();
            let head = t.summary.as_ref().map(|s| format!("{}\n\n---\n\n", s.trim())).unwrap_or_default();
            head + &paras.join("\n\n") + "\n"
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
            summary: None,
            redo: false,
        }
    }

    #[test]
    #[ignore] // reads a real saved meeting: SV_TEST_MEETING = its id
    fn a_real_meeting_clip_is_a_playable_wav_of_the_asked_length() {
        let id: i64 = std::env::var("SV_TEST_MEETING").unwrap().parse().unwrap();
        let bytes = audio_clip(id, Speaker::Others, 60_000, 63_500).unwrap();
        let r = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!((r.spec().sample_rate, r.duration()), (16_000, 56_000));
        assert!(!search("").is_empty());
    }

    #[test]
    fn search_finds_meetings_by_name_overview_or_words() {
        let dir = std::env::temp_dir().join(format!("sv-search-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let meeting = |id, name: &str, summary: Option<&str>, words: &str| Transcript {
            id,
            kind: Kind::Meeting,
            name: name.into(),
            summary: summary.map(Into::into),
            lines: vec![said(Speaker::Others, 0, words)],
            ..sample()
        };
        save_in(&dir, &meeting(1, "Standup", None, "the budget is fine")).unwrap();
        save_in(&dir, &meeting(2, "", Some("## Summary
Hiring plan"), "hello")).unwrap();
        save_in(&dir, &Transcript { id: 3, ..sample() }).unwrap(); // a file, never listed
        let found = |q| search_in(&dir, q);
        let r = (found("BUDGET"), found("hiring"), found("standup"), found("nothing"), found(" "));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(r, (vec![1], vec![2], vec![1], vec![], vec![2, 1]));
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
        let mut lines: Vec<Line> = (0..8).map(|i| said(Speaker::Others, i * 3_000, "a b c.")).collect();
        lines.push(said(Speaker::You, 30_000, "d e f"));
        assert_eq!(paragraphs(&lines), vec![0..6, 6..8, 8..9]);
    }

    #[test]
    fn a_full_paragraph_waits_for_the_end_of_a_sentence() {
        let mut lines: Vec<Line> = (0..30).map(|i| said(Speaker::Others, i * 3_000, "and then it")).collect();
        lines[7].text = "so that is how it works.".into();
        lines[8].text = "Is that \"clear?\"".into();
        // Breaks after the sentence ending at line 7, not mid-sentence at line 6;
        // then after line 8 is too soon; with no sentence end it stops at 12 lines.
        assert_eq!(paragraphs(&lines)[..3], [0..8, 8..20, 20..30]);
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
