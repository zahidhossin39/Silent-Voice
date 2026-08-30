// Chunk-at-silence: transcribe finished sentences while the hotkey is still held.
//
// A background worker watches the growing recording, and when Silero VAD shows a
// real pause (enough speech, then enough trailing silence) it cuts there and
// transcribes that chunk immediately. By the time the user releases the key only
// the tail is left to transcribe, so the paste lands sooner.
//
// The cut always lands in the MIDDLE of the silence, never inside a word, which
// is what makes this lossless. Everything here is best-effort: no VAD model, no
// boundary, or any chunk error means the pipeline falls back to transcribing the
// whole clip exactly as it always has.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

use crate::audio::{capture, vad};
use crate::models::registry;
use crate::transcription::whisper;

const POLL_MS: u64 = 250;
const MIN_SPEECH_MS: usize = 2500; // a chunk must hold at least this much speech
const SILENCE_MS: usize = 700; // trailing silence that marks a boundary
/// Each Silero frame is 512 samples at 16 kHz.
const FRAME_MS: usize = vad::FRAME * 1000 / capture::WHISPER_SAMPLE_RATE as usize;

/// What the worker has finished so far. Both fields live under ONE lock: the
/// pipeline reads them together, and a torn read (text from before a chunk
/// landed, sample count from after) would silently drop that chunk's words.
#[derive(Default)]
pub struct Progress {
    text: Vec<String>,
    consumed_16k: usize,
}

#[derive(Default)]
pub struct Segmenter {
    done: Mutex<Progress>,
    /// Bumped by every `reset()`. A worker whose generation is stale exits
    /// without committing: releasing and immediately re-pressing the hotkey
    /// starts a new recording while the previous chunk may still be
    /// transcribing, and that text must not land in the new dictation.
    generation: AtomicUsize,
    /// Set when a chunk fails to transcribe. The pipeline then discards all
    /// chunk work and re-transcribes the whole clip, so no words are lost.
    pub failed: AtomicBool,
    pub stop: AtomicBool,
}

impl Segmenter {
    /// Clear all progress and return the generation new workers must carry.
    pub fn reset(&self) -> usize {
        if let Ok(mut done) = self.done.lock() {
            *done = Progress::default();
        }
        self.failed.store(false, Ordering::Relaxed);
        self.stop.store(false, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn is_current(&self, generation: usize) -> bool {
        self.generation.load(Ordering::Relaxed) == generation
    }

    /// Transcribed text so far, and how many 16 kHz samples it covers.
    /// Returns nothing once a chunk has failed.
    pub fn prefix(&self) -> (String, usize) {
        if self.failed.load(Ordering::Relaxed) {
            return (String::new(), 0);
        }
        match self.done.lock() {
            Ok(done) => (done.text.join(" "), done.consumed_16k),
            Err(_) => (String::new(), 0),
        }
    }

    fn commit(&self, generation: usize, text: &str, samples: usize) {
        if let Ok(mut done) = self.done.lock() {
            if !self.is_current(generation) {
                return;
            }
            if !text.is_empty() {
                done.text.push(text.to_string());
            }
            done.consumed_16k += samples;
        }
    }
}

pub struct ChunkCfg {
    pub model_id: String,
    pub language: String,
    pub vocabulary: String,
    pub use_gpu: bool,
    pub threads: u32,
    pub input_sensitivity: u32,
}

fn is_sentence_punct(c: char) -> bool {
    matches!(c, '.' | ',' | '!' | '?' | '-' | '…')
}

/// Whisper invents filler and ellipses when it is handed near-silence.
/// A chunk that comes back looking like that is dropped rather than
/// committed: leaving its audio unconsumed means the tail re-transcribes
/// it later with full surrounding context, which is what fixes it.
fn looks_hallucinated(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    let is_punct_strip = |c: char| is_sentence_punct(c) || c.is_whitespace();
    if t.chars().all(is_punct_strip) {
        return true;
    }
    if (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')')) {
        return true;
    }
    const FILLERS: &[&str] = &[
        "uh", "um", "umm", "uhh", "hmm", "mm", "mhm", "ah", "eh", "er", "erm", "huh",
    ];
    let lower = t.to_lowercase();
    for token in lower.split_whitespace() {
        let cleaned: String = token
            .chars()
            .filter(|&c| !is_sentence_punct(c))
            .collect();
        if !cleaned.is_empty() {
            if !FILLERS.contains(&cleaned.as_str()) {
                return false;
            }
        }
    }
    true
}

pub fn spawn(app: AppHandle, seg: Arc<Segmenter>, cfg: ChunkCfg, generation: usize) {
    tauri::async_runtime::spawn(async move {
        let mut raw_mark = 0usize;
        let mut pending: Vec<f32> = Vec::new();
        // Speech flags for `pending`, one per Silero frame. Kept across ticks so
        // each poll only runs VAD over the audio that just arrived — rescanning
        // the whole buffer every 250ms would burn power while the user is still
        // talking, which is exactly what this feature is trying to save.
        let mut mask: Vec<bool> = Vec::new();
        let mut analyzed = 0usize;

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
            if seg.stop.load(Ordering::Relaxed) || !seg.is_current(generation) {
                return;
            }

            let snapshot = {
                let state = app.state::<crate::AppState>();
                let lock = match state.recorder.lock() {
                    Ok(l) => l,
                    Err(_) => return,
                };
                match lock.as_ref() {
                    Some(r) => r.snapshot_16k(raw_mark),
                    // Recording ended.
                    None => return,
                }
            };
            let (new, mark) = snapshot;
            raw_mark = mark;
            pending.extend(new);

            // Whole frames only — Silero rejects a short frame, and a partial one
            // would be re-analyzed with different neighbours on the next tick.
            let ready = pending.len() / vad::FRAME * vad::FRAME;
            if ready > analyzed {
                if let Some(flags) = vad::speech_mask(
                    &pending[analyzed..ready],
                    vad::threshold_for(cfg.input_sensitivity),
                ) {
                    mask.extend(flags);
                    analyzed = ready;
                } else {
                    // No VAD model (or inference failed) — chunking is off entirely.
                    return;
                }
            }

            let Some(cut_frame) = find_boundary(&mask) else {
                continue;
            };
            let cut = cut_frame * vad::FRAME;

            match transcribe_chunk(&app, &pending[..cut], &cfg).await {
                Ok(text) => {
                    // `consumed_16k` is one offset from the start of the clip, so it
                    // cannot describe a gap. Committing a later chunk after skipping
                    // this one would leave its audio both inside the prefix text and
                    // inside the tail, pasting those words twice. Stop chunking
                    // instead: whatever is already committed stays valid, and the
                    // tail transcribes this audio with full surrounding context —
                    // which is what removes the hallucinated filler in the first place.
                    if looks_hallucinated(&text) {
                        crate::logging::log_info(
                            "segmenter",
                            "chunk looked hallucinated — leaving it to the final pass",
                        );
                        return;
                    }
                    seg.commit(generation, text.trim(), cut);
                    pending.drain(..cut);
                    mask.drain(..cut_frame);
                    analyzed -= cut;
                }
                Err(e) => {
                    crate::logging::log_error("segmenter", &format!("chunk STT failed: {e}"));
                    seg.failed.store(true, Ordering::Relaxed);
                    return;
                }
            }
        }
    });
}

/// Frame index to cut at, or None when this isn't a real pause yet.
fn find_boundary(mask: &[bool]) -> Option<usize> {
    let last_speech = mask.iter().rposition(|&s| s)?;
    if (mask.len() - 1 - last_speech) * FRAME_MS < SILENCE_MS {
        return None;
    }
    if mask[..=last_speech].iter().filter(|&&s| s).count() * FRAME_MS < MIN_SPEECH_MS {
        return None;
    }
    // Middle of the trailing silence — far from any word on either side.
    Some((last_speech + 1 + mask.len()) / 2)
}

async fn transcribe_chunk(
    app: &AppHandle,
    chunk: &[f32],
    cfg: &ChunkCfg,
) -> Result<String, String> {
    let Some(trimmed) = crate::audio::gate::trim_silence(chunk, cfg.input_sensitivity) else {
        return Ok(String::new());
    };
    let path = registry::audio_dir().join("chunk.wav");
    capture::write_wav(&path, &trimmed)?;
    // Always local: the caller only starts this worker for local STT, since
    // chunking a paid cloud endpoint would multiply the request count.
    let t = std::time::Instant::now();
    let res = whisper::transcribe_dispatch(
        app,
        &path,
        &cfg.model_id,
        cfg.threads,
        &cfg.language,
        &cfg.vocabulary,
        cfg.use_gpu,
        "local",
        "",
        "",
        "",
    )
    .await;
    let audio_secs = trimmed.len() as f64 / 16000.0;
    let ms = t.elapsed().as_millis();
    let ratio = if audio_secs > 0.0 {
        ms as f64 / (audio_secs * 1000.0)
    } else {
        0.0
    };
    crate::logging::log_info(
        "stt",
        &format!("chunk: {audio_secs:.2}s audio, {ms}ms decode, ratio {ratio:.2}"),
    );
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(speech: usize, silence: usize) -> Vec<bool> {
        let mut m = vec![true; speech];
        m.extend(vec![false; silence]);
        m
    }

    fn ms_to_frames(ms: usize) -> usize {
        ms / FRAME_MS + 1
    }

    #[test]
    fn no_pause_means_no_cut() {
        // The common case: one short sentence, still talking. Must not cut.
        assert!(find_boundary(&frames(ms_to_frames(3000), 0)).is_none());
    }

    #[test]
    fn brief_gap_is_not_a_boundary() {
        // A gap between words is not a sentence break.
        assert!(find_boundary(&frames(ms_to_frames(3000), ms_to_frames(150))).is_none());
    }

    #[test]
    fn too_little_speech_is_not_a_boundary() {
        // Cutting here would hand Whisper a fragment it tends to hallucinate on.
        assert!(find_boundary(&frames(ms_to_frames(400), ms_to_frames(900))).is_none());
    }

    #[test]
    fn real_pause_cuts_inside_the_silence() {
        let speech = ms_to_frames(3000);
        let silence = ms_to_frames(900);
        let cut = find_boundary(&frames(speech, silence)).expect("boundary");
        assert!(cut > speech, "cut landed in speech at {cut} (speech ends {speech})");
        assert!(cut < speech + silence, "cut ran past the silence at {cut}");
    }

    #[test]
    fn hallucinated_outputs() {
        assert!(looks_hallucinated("..."));
        assert!(looks_hallucinated("[BLANK_AUDIO]"));
        assert!(looks_hallucinated("(sighs)"));
        assert!(looks_hallucinated("Uh... um."));
        assert!(looks_hallucinated(""));

        assert!(!looks_hallucinated("Uh, I need the report."));
        assert!(!looks_hallucinated("So the meeting is at three."));
    }
}
