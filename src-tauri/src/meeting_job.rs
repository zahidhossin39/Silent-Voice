// "Record a meeting": the mic (You) and the computer's sound (Others) as two
// tracks, each cut at pauses and transcribed live into one transcript.
//
// Two capture threads feed one worker over a channel, so pieces are
// transcribed one at a time in arrival order (the STT lock serializes them
// anyway). Stop ends the capture; the worker then finishes what's queued.
// Both tracks are also kept as 16 kHz WAVs next to the transcript.
// Always local, like files.

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::audio::live::{Live, Source};
use crate::audio::{capture, longform};
use crate::transcribe_job::{self, Cfg};
use crate::transcripts::{self, paragraphs, Kind, Line, Speaker, Status, Transcript};

const SR: u64 = capture::WHISPER_SAMPLE_RATE as u64;
/// A You line repeating an Others line this close in time is the mic hearing the speakers.
const ECHO_WINDOW_MS: u64 = 3_000;
const PROGRESS_EVERY: Duration = Duration::from_secs(1);

/// The two running captures; Stop takes them.
static CAPTURE: Mutex<Option<(Live, Live)>> = Mutex::new(None);

pub fn start(app: &AppHandle) -> Result<Transcript, String> {
    let cfg = Cfg::load(app)?;
    let id = transcripts::new_id()?;
    transcribe_job::claim(id)?;
    let started = (|| {
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        let mic = Live::start(Source::Mic(cfg.mic.clone()), move |c| {
            let _ = tx.send((Speaker::You, c));
        })?;
        let speakers = match Live::start(Source::Speakers, move |c| {
            let _ = tx2.send((Speaker::Others, c));
        }) {
            Ok(s) => s,
            Err(e) => {
                let _ = mic.stop();
                return Err(e);
            }
        };
        *CAPTURE.lock().map_err(|e| e.to_string())? = Some((mic, speakers));
        Ok(rx)
    })();
    let t = Transcript {
        id,
        kind: Kind::Meeting,
        name: "Meeting".into(),
        source: String::new(),
        created: id,
        duration_ms: None,
        model_id: cfg.model_id.clone(),
        status: Status::Running,
        error: None,
        done_ms: 0,
        work_ms: 0,
        lines: Vec::new(),
    };
    let rx = match started {
        Ok(rx) => rx,
        Err(e) => {
            transcribe_job::finish(app, &Transcript { status: Status::Failed, error: Some(e.clone()), ..t });
            let _ = transcripts::delete(id);
            return Err(e);
        }
    };
    transcripts::save(&t)?;
    let first = t.clone();
    let app = app.clone();
    std::thread::spawn(move || work(app, t, rx, cfg));
    Ok(first)
}

/// End the capture; the worker finishes the queued pieces and marks it done.
pub fn stop() -> Result<(), String> {
    let Some((mic, speakers)) = CAPTURE.lock().map_err(|e| e.to_string())?.take() else {
        return Ok(());
    };
    // Both stop even if one failed (an unplugged headset): what was heard is kept.
    let (a, b) = (mic.stop(), speakers.stop());
    a.and(b)
}

fn work(app: AppHandle, mut t: Transcript, rx: Receiver<(Speaker, Vec<f32>)>, cfg: Cfg) {
    let spec = hound::WavSpec { channels: 1, sample_rate: SR as u32, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut wavs = [Speaker::You, Speaker::Others].map(|s| hound::WavWriter::create(transcripts::audio_path(t.id, s), spec).ok());
    let mut since_flush = 0;
    let started = Instant::now();
    let mut last_sent = Instant::now();
    run(
        &mut t,
        rx,
        [longform::chunker(cfg.sensitivity), longform::chunker(cfg.sensitivity)],
        |samples| {
            transcribe_job::transcribe_piece(&app, samples, &cfg)
                .inspect_err(|e| crate::logging::log_error("meeting", &format!("piece skipped: {e}")))
                .unwrap_or_default()
        },
        |speaker, chunk| {
            if let Some(w) = wavs[speaker as usize].as_mut() {
                for &s in chunk {
                    let _ = w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
                }
            }
            // Keep the headers current, so a crash still leaves playable files.
            since_flush += 1;
            if since_flush >= 100 {
                since_flush = 0;
                wavs.iter_mut().flatten().for_each(|w| drop(w.flush()));
            }
        },
        |t, new_text| {
            if new_text || last_sent.elapsed() >= PROGRESS_EVERY {
                t.work_ms = started.elapsed().as_millis() as u64;
                let _ = transcripts::save(t);
                let _ = app.emit("transcript://update", &*t);
                last_sent = Instant::now();
            }
        },
    );
    wavs.into_iter().flatten().for_each(|w| drop(w.finalize()));
    for range in paragraphs(&t.lines) {
        repunctuate(&mut t, range, cfg.english);
    }
    t.work_ms = started.elapsed().as_millis() as u64;
    t.duration_ms = Some(t.done_ms);
    t.status = Status::Done;
    crate::logging::log_info("meeting", &format!("done: {:.1} min, {} lines", t.done_ms as f64 / 60_000.0, t.lines.len()));
    transcribe_job::finish(&app, &t);
}

/// The worker: until both tracks end, cut each into pieces, transcribe them,
/// and slot each line in by time with its speaker.
fn run<V: FnMut(&[f32]) -> Option<Vec<bool>>>(
    t: &mut Transcript,
    rx: Receiver<(Speaker, Vec<f32>)>,
    chunkers: [longform::Chunker<V>; 2],
    mut transcribe: impl FnMut(&[f32]) -> String,
    mut keep_audio: impl FnMut(Speaker, &[f32]),
    mut changed: impl FnMut(&mut Transcript, bool),
) {
    let mut heard = [0u64; 2];
    let mut chunkers = chunkers.map(Some);
    // True when a line was added.
    let mut add = |t: &mut Transcript, speaker: Speaker, pieces: Vec<longform::Segment>| {
        let mut added = false;
        for piece in pieces.into_iter().filter(|p| p.speech) {
            let text = transcribe(&piece.samples);
            if text.is_empty() {
                continue;
            }
            let start_ms = piece.start as u64 * 1000 / SR;
            let end_ms = start_ms + piece.samples.len() as u64 * 1000 / SR;
            let line = Line { start_ms, end_ms, text, speaker: Some(speaker) };
            added |= add_line(&mut t.lines, line);
        }
        added
    };
    for (speaker, chunk) in rx {
        keep_audio(speaker, &chunk);
        let i = speaker as usize;
        heard[i] += chunk.len() as u64;
        t.done_ms = heard.iter().max().unwrap() * 1000 / SR;
        let pieces = chunkers[i].as_mut().map(|c| c.push(&chunk)).unwrap_or_default();
        let added = add(t, speaker, pieces);
        changed(t, added);
    }
    for (i, speaker) in [Speaker::You, Speaker::Others].into_iter().enumerate() {
        let pieces = chunkers[i].take().map(|c| c.finish()).unwrap_or_default();
        let added = add(t, speaker, pieces);
        changed(t, added);
    }
}

/// Insert `line` in time order. A You line that repeats an Others line said at
/// the same time is the mic hearing the speakers: it's dropped (false), and an
/// Others line drops any You lines it echoes.
fn add_line(lines: &mut Vec<Line>, line: Line) -> bool {
    let near = |a: &Line, b: &Line| a.start_ms <= b.end_ms + ECHO_WINDOW_MS && b.start_ms <= a.end_ms + ECHO_WINDOW_MS;
    match line.speaker {
        Some(Speaker::You) => {
            if lines.iter().any(|o| o.speaker == Some(Speaker::Others) && near(o, &line) && is_echo(&line.text, &o.text)) {
                return false;
            }
        }
        _ => lines.retain(|y| !(y.speaker == Some(Speaker::You) && near(y, &line) && is_echo(&y.text, &line.text))),
    }
    let at = lines.partition_point(|l| l.start_ms <= line.start_ms);
    lines.insert(at, line);
    true
}

/// Most of `heard`'s words also appear in `said` (the mic picked up the speakers).
fn is_echo(heard: &str, said: &str) -> bool {
    let words = |s: &str| -> Vec<String> {
        s.split_whitespace()
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect()
    };
    let (h, mut s) = (words(heard), words(said));
    if h.is_empty() {
        return false;
    }
    let mut found = 0;
    for w in &h {
        if let Some(i) = s.iter().position(|x| x == w) {
            s.swap_remove(i);
            found += 1;
        }
    }
    // Short replies ("yeah", "okay") must match fully to count as an echo.
    if h.len() <= 3 { found == h.len() } else { found * 10 >= h.len() * 6 }
}

// ponytail: repaired once at Stop, so the live view shows each piece's own
// punctuation; repair as each paragraph closes if that reads badly live.
fn repunctuate(t: &mut Transcript, range: std::ops::Range<usize>, english: bool) {
    if !english || range.is_empty() {
        return;
    }
    let parts: Vec<&str> = t.lines[range.clone()].iter().map(|l| l.text.as_str()).collect();
    let fixed = crate::system::sherpa_stt::repunctuate(&parts);
    if let Some(split) = transcribe_job::split_like(&fixed, &parts) {
        for (line, text) in t.lines[range].iter_mut().zip(split) {
            line.text = text;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::vad::FRAME;

    fn line(speaker: Speaker, start_ms: u64, text: &str) -> Line {
        Line { start_ms, end_ms: start_ms + 2_000, text: text.into(), speaker: Some(speaker) }
    }

    #[test]
    fn the_mic_hearing_the_speakers_is_dropped_but_a_reply_is_kept() {
        let mut lines = Vec::new();
        assert!(add_line(&mut lines, line(Speaker::Others, 1_000, "Can you send me the report by Friday?")));
        assert!(!add_line(&mut lines, line(Speaker::You, 1_200, "can you send me the report by friday")), "echo kept");
        assert!(add_line(&mut lines, line(Speaker::You, 3_500, "Sure, I'll send it tomorrow.")), "real reply dropped");
        assert!(add_line(&mut lines, line(Speaker::You, 30_000, "Can you send me the report by Friday?")), "same words much later aren't an echo");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn a_late_others_line_removes_the_echo_heard_before_it() {
        let mut lines = vec![line(Speaker::You, 1_100, "we should ship it next week")];
        add_line(&mut lines, line(Speaker::Others, 1_000, "We should ship it next week."));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].speaker, Some(Speaker::Others));
    }

    #[test]
    fn short_replies_only_count_as_echo_when_identical() {
        assert!(is_echo("yeah", "Yeah."));
        assert!(!is_echo("yeah okay", "okay so the numbers"));
    }

    fn fake_vad(s: &[f32]) -> Option<Vec<bool>> {
        Some(s.chunks_exact(FRAME).map(|f| f[0].abs() > 0.1).collect())
    }

    #[test]
    fn two_tracks_become_one_transcript_in_time_order() {
        let (tx, rx) = channel();
        // You talk 0-3 s; Others talk 4-7 s; both tracks run 9 s in 0.1 s chunks.
        for i in 0..90 {
            let t = i as f32 / 10.0;
            tx.send((Speaker::You, vec![if t < 3.0 { 0.5 } else { 0.0 }; 1_600])).unwrap();
            tx.send((Speaker::Others, vec![if (4.0..7.0).contains(&t) { 0.4 } else { 0.0 }; 1_600])).unwrap();
        }
        drop(tx);
        let mut t = Transcript {
            id: 1,
            kind: Kind::Meeting,
            name: "m".into(),
            source: String::new(),
            created: 1,
            duration_ms: None,
            model_id: "m".into(),
            status: Status::Running,
            error: None,
            done_ms: 0,
            work_ms: 0,
            lines: Vec::new(),
        };
        let mut kept = [0usize; 2];
        run(
            &mut t,
            rx,
            [longform::Chunker::with_vad(4 * 16_000, fake_vad), longform::Chunker::with_vad(4 * 16_000, fake_vad)],
            |s| if s[s.len() / 2..].iter().chain(&s[..s.len() / 2]).any(|&x| x == 0.5) { "mine".into() } else { "theirs".into() },
            |sp, c| kept[sp as usize] += c.len(),
            |_, _| {},
        );
        assert_eq!(kept, [144_000, 144_000], "audio not kept whole");
        assert_eq!(t.done_ms, 9_000);
        let said: Vec<(Option<Speaker>, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(said, vec![(Some(Speaker::You), "mine"), (Some(Speaker::Others), "theirs")]);
        assert!(t.lines.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    }
}
