// "Transcribe a file": decode → cut at pauses → transcribe each piece → save.
//
// One job at a time, on its own thread. Every finished piece is saved and sent
// to the page, so progress is live and closing the app loses at most one
// piece. Each piece takes the STT lock on its own, so a dictation made during
// a long job only waits for the current piece (a second or two).
// Always local: a file never goes to a cloud STT provider.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{capture, decode::FileDecoder, longform};
use crate::system::textfmt;
use crate::transcripts::{self, Line, Transcript};

const SR: u64 = capture::WHISPER_SAMPLE_RATE as u64;
/// Pieces repunctuated together, which also makes one paragraph (~1 min).
pub const PARAGRAPH: usize = 6;

/// The running job's id and its cancel flag.
static JOB: Mutex<Option<(i64, Arc<AtomicBool>)>> = Mutex::new(None);

pub fn running() -> Option<i64> {
    JOB.lock().ok().and_then(|j| j.as_ref().map(|(id, _)| *id))
}

pub fn cancel() {
    if let Ok(j) = JOB.lock() {
        if let Some((_, stop)) = j.as_ref() {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Start a new transcript for `path`, or carry on with `resume` from where it stopped.
pub fn start(app: &AppHandle, path: Option<String>, resume: Option<i64>) -> Result<Transcript, String> {
    let mut job = JOB.lock().map_err(|e| e.to_string())?;
    if job.is_some() {
        return Err("A file is already being transcribed. Wait for it or cancel it first.".into());
    }
    let (model_id, language, vocabulary, use_vocabulary, use_gpu, threads, sensitivity) = {
        let state = app.state::<crate::AppState>();
        let c = state.config.lock().map_err(|e| e.to_string())?;
        (
            c.model_id.clone(),
            c.language.clone(),
            c.vocabulary.clone(),
            c.use_vocabulary,
            c.use_gpu,
            crate::system::hotkey::resolve_thread_count(c.high_performance, c.performance_threads),
            c.input_sensitivity,
        )
    };
    let mut t = match resume {
        Some(id) => transcripts::load(id)?,
        None => {
            let source = path.ok_or("No file given.")?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
            Transcript {
                id: now,
                name: std::path::Path::new(&source).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                source,
                created: now,
                duration_ms: None,
                model_id: model_id.clone(),
                status: "running".into(),
                error: None,
                done_ms: 0,
                work_ms: 0,
                lines: Vec::new(),
            }
        }
    };
    // Open before answering, so "not supported" shows up at once on the page.
    let decoder = FileDecoder::open(std::path::Path::new(&t.source))?;
    t.duration_ms = decoder.duration_ms().or(t.duration_ms);
    t.status = "running".into();
    t.error = None;
    transcripts::save(&t)?;

    let stop = Arc::new(AtomicBool::new(false));
    *job = Some((t.id, stop.clone()));
    drop(job);

    let english = crate::system::hotkey::english_text(&language, &model_id);
    let cfg = Cfg { model_id, language, vocabulary, use_vocabulary, use_gpu, threads, english };
    let first = t.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let id = t.id;
        let started = Instant::now();
        let work_before = t.work_ms;
        let result = run(&app, &mut t, decoder, sensitivity, &cfg, &stop, |t| {
            t.work_ms = work_before + started.elapsed().as_millis() as u64;
            let _ = transcripts::save(t);
            let _ = app.emit("transcript://update", &*t);
        });
        t.work_ms = work_before + started.elapsed().as_millis() as u64;
        match result {
            Ok(()) if stop.load(Ordering::Relaxed) => t.status = "stopped".into(),
            Ok(()) => {
                t.status = "done".into();
                t.done_ms = t.duration_ms.unwrap_or(t.done_ms).max(t.done_ms);
            }
            Err(e) => {
                crate::logging::log_error("transcribe", &e);
                t.status = "failed".into();
                t.error = Some(e);
            }
        }
        crate::logging::log_info(
            "transcribe",
            &format!("{} after {:.1} min audio, {:.1} s work", t.status, t.done_ms as f64 / 60_000.0, t.work_ms as f64 / 1000.0),
        );
        let _ = transcripts::save(&t);
        let _ = app.emit("transcript://update", &t);
        if let Ok(mut j) = JOB.lock() {
            if j.as_ref().is_some_and(|(jid, _)| *jid == id) {
                *j = None;
            }
        }
    });
    Ok(first)
}

struct Cfg {
    model_id: String,
    language: String,
    vocabulary: String,
    use_vocabulary: bool,
    use_gpu: bool,
    threads: u32,
    english: bool,
}

fn run(
    app: &AppHandle,
    t: &mut Transcript,
    mut decoder: FileDecoder,
    sensitivity: u32,
    cfg: &Cfg,
    stop: &AtomicBool,
    mut changed: impl FnMut(&mut Transcript),
) -> Result<(), String> {
    // Resume: decode and drop the audio already done (lossless, no seeking).
    let skip = (t.done_ms * SR / 1000) as usize;
    let mut skipped = 0usize;
    let mut chunker = longform::chunker(sensitivity);
    // Lines not yet repunctuated as a paragraph (the tail of `t.lines`).
    let mut open = t.lines.len() % PARAGRAPH;
    let mut finished = false;
    while !finished {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pieces = match decoder.next_chunk()? {
            Some(mut chunk) => {
                if skipped < skip {
                    let drop_n = (skip - skipped).min(chunk.len());
                    skipped += drop_n;
                    chunk.drain(..drop_n);
                }
                chunker.push(&chunk)
            }
            None => {
                finished = true;
                std::mem::replace(&mut chunker, longform::chunker(sensitivity)).finish()
            }
        };
        for piece in pieces {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let start_ms = (skip + piece.start) as u64 * 1000 / SR;
            let end_ms = start_ms + piece.samples.len() as u64 * 1000 / SR;
            if piece.speech {
                let text = transcribe_piece(app, &piece.samples, cfg)?;
                if !text.is_empty() {
                    t.lines.push(Line { start_ms, end_ms, text });
                    open += 1;
                    if open == PARAGRAPH {
                        repunctuate_tail(t, open, cfg.english);
                        open = 0;
                    }
                }
            }
            t.done_ms = end_ms;
            changed(t);
        }
    }
    if open > 0 {
        repunctuate_tail(t, open, cfg.english);
        changed(t);
    }
    Ok(())
}

fn transcribe_piece(app: &AppHandle, samples: &[f32], cfg: &Cfg) -> Result<String, String> {
    let path = crate::models::registry::audio_dir().join("file-piece.wav");
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
    ))?;
    Ok(textfmt::collapse_repeated_words(&textfmt::strip_fillers(text.trim())).trim().to_string())
}

/// Repair punctuation across the last `n` lines as one paragraph. Each piece was
/// decoded alone, so a pause mid-sentence became a full stop. The model never
/// changes words, so its output splits back into the same lines by word count.
fn repunctuate_tail(t: &mut Transcript, n: usize, english: bool) {
    if !english {
        return;
    }
    let from = t.lines.len() - n;
    let parts: Vec<&str> = t.lines[from..].iter().map(|l| l.text.as_str()).collect();
    let fixed = crate::system::sherpa_stt::repunctuate(&parts);
    if let Some(split) = split_like(&fixed, &parts) {
        for (line, text) in t.lines[from..].iter_mut().zip(split) {
            line.text = text;
        }
    }
}

/// Cut `text` into pieces with the same word counts as `like`; None if the
/// total differs (then the lines are left as they were).
fn split_like(text: &str, like: &[&str]) -> Option<Vec<String>> {
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
    use super::split_like;

    #[test]
    fn repaired_text_splits_back_into_the_same_lines() {
        let like = ["so we went to the", "market. And then we left."];
        assert_eq!(
            split_like("So we went to the market, and then we left.", &like),
            Some(vec!["So we went to the".into(), "market, and then we left.".into()])
        );
        assert_eq!(split_like("one two", &like), None, "word count changed: keep the originals");
    }
}
