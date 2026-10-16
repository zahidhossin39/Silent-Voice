// "Transcribe a file": decode → cut at pauses → transcribe each piece → save.
//
// One job at a time, on its own thread. New text is saved and sent to the page
// as each piece finishes, so progress is live and closing the app loses at
// most one piece. Each piece takes the STT lock on its own, so a dictation made
// during a long job only waits for the current piece (a second or two).
// Always local: a file never goes to a cloud STT provider.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{capture, decode::FileDecoder, longform};
use crate::system::textfmt;
use crate::transcripts::{self, Line, Status, Transcript, PARAGRAPH};

const SR: u64 = capture::WHISPER_SAMPLE_RATE as u64;
/// Pieces that fail in a row before the whole job gives up (a missing model
/// fails every piece; one bad piece shouldn't sink a 2-hour file).
const MAX_FAILS_IN_A_ROW: usize = 3;
/// Progress with no new text is sent at most this often.
const PROGRESS_EVERY: Duration = Duration::from_secs(1);

/// The running job's id and its cancel flag.
static JOB: Mutex<Option<(i64, Arc<AtomicBool>)>> = Mutex::new(None);

pub fn cancel() {
    if let Ok(j) = JOB.lock() {
        if let Some((_, stop)) = j.as_ref() {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Take the one job slot for `id` (files and meetings share it). The flag is
/// Cancel for a file; a meeting ends with Stop instead.
pub(crate) fn claim(id: i64) -> Result<Arc<AtomicBool>, String> {
    let mut job = JOB.lock().map_err(|e| e.to_string())?;
    if job.is_some() {
        return Err("A file is being transcribed or a meeting recorded. Wait for it to finish or stop it first.".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    *job = Some((id, stop.clone()));
    Ok(stop)
}

/// Free the slot of a job that never got going.
pub(crate) fn release(id: i64) {
    if let Ok(mut j) = JOB.lock() {
        if j.as_ref().is_some_and(|(jid, _)| *jid == id) {
            *j = None;
        }
    }
}

/// Save a job's final state and free the slot, under the job lock, so a rename
/// can't land in between and be overwritten; then tell the page.
pub(crate) fn finish(app: &AppHandle, t: &Transcript) {
    if let Ok(mut j) = JOB.lock() {
        let _ = transcripts::save(t);
        if j.as_ref().is_some_and(|(jid, _)| *jid == t.id) {
            *j = None;
        }
    }
    let _ = app.emit("transcript://update", t);
}

/// Run `f` on transcript `id` only while no job is writing it. Holds the job
/// lock throughout, so a job can't start (or finish) halfway through.
pub fn unless_running<T>(id: i64, busy: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let job = JOB.lock().map_err(|e| e.to_string())?;
    if job.as_ref().is_some_and(|(jid, _)| *jid == id) {
        return Err(busy.into());
    }
    f()
}

/// What each piece is transcribed with, read from settings when a job starts.
/// Shared with the meeting recorder.
pub(crate) struct Cfg {
    pub(crate) model_id: String,
    language: String,
    vocabulary: String,
    use_vocabulary: bool,
    use_gpu: bool,
    threads: u32,
    pub(crate) sensitivity: u32,
    pub(crate) english: bool,
    /// The dictation mic from settings (None = system default).
    pub(crate) mic: Option<String>,
}

impl Cfg {
    pub(crate) fn load(app: &AppHandle) -> Result<Self, String> {
        let state = app.state::<crate::AppState>();
        let c = state.config.lock().map_err(|e| e.to_string())?;
        Ok(Cfg {
            english: crate::system::hotkey::english_text(&c.language, &c.model_id),
            model_id: c.model_id.clone(),
            language: c.language.clone(),
            vocabulary: c.vocabulary.clone(),
            use_vocabulary: c.use_vocabulary,
            use_gpu: c.use_gpu,
            threads: crate::system::hotkey::resolve_thread_count(c.high_performance, c.performance_threads),
            sensitivity: c.input_sensitivity,
            mic: c.audio_device.clone(),
        })
    }
}

/// Start a new transcript for `path`, or carry on with `resume` from where it stopped.
pub fn start(app: &AppHandle, path: Option<String>, resume: Option<i64>) -> Result<Transcript, String> {
    let cfg = Cfg::load(app)?;
    let id = match resume {
        Some(id) => id,
        None => transcripts::new_id()?,
    };
    let stop = claim(id)?;
    let prepared = (|| {
        let mut t = match resume {
            Some(id) => transcripts::load(id)?,
            None => {
                let source = path.ok_or("No file given.")?;
                let name = std::path::Path::new(&source).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                Transcript::new(id, transcripts::Kind::File, name, source, cfg.model_id.clone())
            }
        };
        // Open before answering, so "not supported" shows up at once on the page.
        let decoder = FileDecoder::open(std::path::Path::new(&t.source))?;
        t.duration_ms = decoder.duration_ms().or(t.duration_ms);
        t.status = Status::Running;
        t.error = None;
        transcripts::save(&t)?;
        Ok::<_, String>((t, decoder))
    })();
    let (mut t, mut decoder) = prepared.inspect_err(|_| release(id))?;

    let first = t.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let work_before = t.work_ms;
        let mut last_sent = Instant::now();
        let result = run(
            &mut t,
            || decoder.next_chunk(),
            longform::chunker(cfg.sensitivity),
            &stop,
            |samples| {
                transcribe_piece(&app, samples, &cfg).inspect_err(|e| crate::logging::log_error("transcribe", &format!("piece skipped: {e}")))
            },
            |t, n| repunctuate_tail(t, n, cfg.english),
            |t, new_text| {
                if new_text || last_sent.elapsed() >= PROGRESS_EVERY {
                    t.work_ms = work_before + started.elapsed().as_millis() as u64;
                    let _ = transcripts::save(t);
                    let _ = app.emit("transcript://update", &*t);
                    last_sent = Instant::now();
                }
            },
        );
        t.work_ms = work_before + started.elapsed().as_millis() as u64;
        match result {
            Ok(()) if stop.load(Ordering::Relaxed) => t.status = Status::Stopped,
            Ok(()) => {
                t.status = Status::Done;
                t.done_ms = t.duration_ms.unwrap_or(t.done_ms).max(t.done_ms);
            }
            Err(e) => {
                crate::logging::log_error("transcribe", &e);
                t.status = Status::Failed;
                t.error = Some(e);
            }
        }
        crate::logging::log_info(
            "transcribe",
            &format!("{:?} after {:.1} min audio, {:.1} s work", t.status, t.done_ms as f64 / 60_000.0, t.work_ms as f64 / 1000.0),
        );
        finish(&app, &t);
    });
    Ok(first)
}

/// The job itself, with every outside dependency passed in so it runs in tests.
/// `changed(t, new_text)` fires after every piece; `repunct(t, n)` repairs the
/// last `n` lines as one paragraph.
fn run<V: FnMut(&[f32]) -> Option<Vec<bool>>>(
    t: &mut Transcript,
    mut next_chunk: impl FnMut() -> Result<Option<Vec<f32>>, String>,
    chunker: longform::Chunker<V>,
    stop: &AtomicBool,
    mut transcribe: impl FnMut(&[f32]) -> Result<String, String>,
    mut repunct: impl FnMut(&mut Transcript, usize),
    mut changed: impl FnMut(&mut Transcript, bool),
) -> Result<(), String> {
    // Resume: decode and drop the audio already done (lossless, no seeking).
    // Exact: pieces end on 32 ms VAD frames, so done_ms is a whole sample count.
    let skip = (t.done_ms * SR / 1000) as usize;
    let mut skipped = 0usize;
    let mut chunker = Some(chunker);
    // Lines not yet repunctuated as a paragraph (the tail of `t.lines`).
    let mut open = t.lines.len() % PARAGRAPH;
    let mut fails = 0;
    while let Some(c) = chunker.as_mut() {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pieces = match next_chunk()? {
            Some(mut chunk) => {
                if skipped < skip {
                    let drop_n = (skip - skipped).min(chunk.len());
                    skipped += drop_n;
                    chunk.drain(..drop_n);
                }
                c.push(&chunk)
            }
            None => chunker.take().map(|c| c.finish()).unwrap_or_default(),
        };
        for piece in pieces {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let start_ms = (skip + piece.start) as u64 * 1000 / SR;
            let end_ms = start_ms + piece.samples.len() as u64 * 1000 / SR;
            let mut new_text = false;
            if piece.speech {
                match transcribe(&piece.samples) {
                    Ok(text) => {
                        fails = 0;
                        if !text.is_empty() {
                            t.lines.push(Line { start_ms, end_ms, text, speaker: None });
                            new_text = true;
                            open += 1;
                            if open == PARAGRAPH {
                                repunct(t, open);
                                open = 0;
                            }
                        }
                    }
                    Err(e) => {
                        fails += 1;
                        if fails >= MAX_FAILS_IN_A_ROW {
                            return Err(e);
                        }
                    }
                }
            }
            t.done_ms = end_ms;
            changed(t, new_text);
        }
    }
    if open > 0 {
        repunct(t, open);
        changed(t, true);
    }
    Ok(())
}

/// Transcribe one piece locally and clean it (fillers, repeated words).
pub(crate) fn transcribe_piece(app: &AppHandle, samples: &[f32], cfg: &Cfg) -> Result<String, String> {
    // Its own temp file: the meeting recorder transcribes two tracks.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = crate::models::registry::audio_dir().join(format!("piece-{}.wav", NEXT.fetch_add(1, Ordering::Relaxed)));
    capture::write_wav(&path, samples)?;
    let text = tauri::async_runtime::block_on(crate::transcription::whisper::transcribe_dispatch(
        app,
        &path,
        &cfg.model_id,
        cfg.threads,
        &cfg.language,
        &cfg.vocabulary,
        cfg.use_vocabulary,
        cfg.use_gpu,
        "local",
        "",
        "",
        "",
    ));
    let _ = std::fs::remove_file(&path);
    let text = text?;
    Ok(textfmt::collapse_repeated_words(&textfmt::strip_fillers(text.trim())).trim().to_string())
}

/// Repair punctuation across the last `n` lines as one paragraph. Each piece was
/// decoded alone, so a pause mid-sentence became a full stop. The model never
/// changes words, so its output splits back into the same lines by word count.
/// The previous paragraph's last line goes in again as context: its forced
/// closing full stop is undone when the sentence actually runs on.
fn repunctuate_tail(t: &mut Transcript, n: usize, english: bool) {
    if !english {
        return;
    }
    let from = (t.lines.len() - n).saturating_sub(1);
    let sentence_start = from == 0 || t.lines[from - 1].text.ends_with(['.', '?', '!']);
    let parts: Vec<&str> = t.lines[from..].iter().map(|l| l.text.as_str()).collect();
    let fixed = crate::system::sherpa_stt::repunctuate_from(&parts, sentence_start);
    if let Some(split) = split_like(&fixed, &parts) {
        for (line, text) in t.lines[from..].iter_mut().zip(split) {
            line.text = text;
        }
    }
}

/// Cut `text` into pieces with the same word counts as `like`; None if the
/// total differs (then the lines are left as they were).
pub(crate) fn split_like(text: &str, like: &[&str]) -> Option<Vec<String>> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let counts: Vec<usize> = like.iter().map(|p| p.split_whitespace().count()).collect();
    if words.len() != counts.iter().sum::<usize>() {
        return None;
    }
    let mut i = 0;
    Some(counts.iter().map(|&c| { let s = words[i..i + c].join(" "); i += c; s }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::vad::FRAME;

    #[test]
    fn repaired_text_splits_back_into_the_same_lines() {
        let like = ["so we went to the", "market. And then we left."];
        assert_eq!(
            split_like("So we went to the market, and then we left.", &like),
            Some(vec!["So we went to the".into(), "market, and then we left.".into()])
        );
        assert_eq!(split_like("one two", &like), None, "word count changed: keep the originals");
    }

    fn fake_vad(s: &[f32]) -> Option<Vec<bool>> {
        Some(s.chunks_exact(FRAME).map(|f| f[0].abs() > 0.1).collect())
    }

    fn blank() -> Transcript {
        Transcript {
            kind: transcripts::Kind::File,
            id: 1,
            name: "a".into(),
            source: "a".into(),
            created: 1,
            duration_ms: None,
            model_id: "m".into(),
            status: Status::Running,
            error: None,
            done_ms: 0,
            work_ms: 0,
            lines: Vec::new(),
        }
    }

    /// Talk with pauses, ~100 s, fed in 1 s chunks like the decoder does.
    fn audio() -> Vec<f32> {
        let mut a = Vec::new();
        for i in 0..14 {
            a.extend(vec![0.5f32; (3 + i % 5) * 16_000 + i * 700]);
            a.extend(vec![0.0f32; 8_000]);
        }
        a
    }

    /// Run over `audio`; `stop_after` stops it after that many pieces. Returns
    /// the samples handed to the engine, and each repunctuation's (lines, n).
    fn run_on(t: &mut Transcript, audio: &[f32], stop_after: Option<usize>) -> (usize, Vec<(usize, usize)>) {
        let stop = AtomicBool::new(false);
        let mut chunks = audio.chunks(16_000).map(|c| c.to_vec());
        let (mut heard, mut repuncts, mut pieces) = (0, Vec::new(), 0);
        run(
            t,
            || Ok(chunks.next()),
            longform::Chunker::with_vad(4 * 16_000, fake_vad),
            &stop,
            |s| {
                heard += s.len();
                Ok(format!("w{}", s.len()))
            },
            |t, n| repuncts.push((t.lines.len(), n)),
            |_, _| {
                pieces += 1;
                if Some(pieces) == stop_after {
                    stop.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap();
        (heard, repuncts)
    }

    #[test]
    fn resume_carries_on_with_no_gap_and_no_repeat() {
        let audio = audio();
        let mut whole = blank();
        let (heard, _) = run_on(&mut whole, &audio, None);
        let mut t = blank();
        let (first, _) = run_on(&mut t, &audio, Some(9));
        assert!(t.done_ms > 0 && (t.done_ms as usize * 16) < audio.len(), "stopped mid-file");
        let (rest, _) = run_on(&mut t, &audio, None);
        assert_eq!(first + rest, heard, "audio lost or heard twice across the resume");
        assert_eq!(t.lines, whole.lines, "resumed transcript differs from an uninterrupted one");
        assert_eq!(t.lines.last().unwrap().end_ms, audio.len() as u64 / 16, "didn't reach the end");
    }

    #[test]
    fn resume_keeps_paragraphs_six_lines_long() {
        let audio = audio();
        let mut t = blank();
        let (_, mut repuncts) = run_on(&mut t, &audio, Some(9));
        let (_, more) = run_on(&mut t, &audio, None);
        repuncts.extend(more);
        let (last, whole) = repuncts.split_last().unwrap();
        assert!(whole.iter().all(|&(lines, n)| n == PARAGRAPH && lines % PARAGRAPH == 0), "{repuncts:?}");
        assert_eq!(*last, (t.lines.len(), t.lines.len() % PARAGRAPH), "{repuncts:?}");
    }

    #[test]
    fn one_failed_piece_is_skipped_but_failing_every_piece_stops_the_job() {
        let audio = audio();
        let stop = AtomicBool::new(false);
        let mut calls = 0;
        let mut t = blank();
        let mut chunks = audio.chunks(16_000).map(|c| c.to_vec());
        let r = run(
            &mut t,
            || Ok(chunks.next()),
            longform::Chunker::with_vad(4 * 16_000, fake_vad),
            &stop,
            |_| {
                calls += 1;
                if calls == 2 { Err("bad piece".into()) } else { Ok("ok".into()) }
            },
            |_, _| {},
            |_, _| {},
        );
        assert!(r.is_ok() && t.lines.len() == calls - 1, "one bad piece sank the job");

        let mut chunks = audio.chunks(16_000).map(|c| c.to_vec());
        let r = run(
            &mut blank(),
            || Ok(chunks.next()),
            longform::Chunker::with_vad(4 * 16_000, fake_vad),
            &stop,
            |_| Err("no model".into()),
            |_, _| {},
            |_, _| {},
        );
        assert_eq!(r, Err("no model".into()));
    }
}
