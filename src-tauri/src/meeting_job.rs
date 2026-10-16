// "Record a meeting": the mic (You) and the computer's sound (Others) as two
// tracks, each cut at pauses and transcribed live into one transcript.
//
// Each capture thread writes its own WAV as audio arrives (so a backlog or a
// crash never costs the recording), then hands the chunk to one worker over a
// channel. The worker transcribes pieces one at a time in arrival order (the
// STT lock serializes them anyway). Stop ends the capture; the worker then
// finishes what's queued. Always local, like files.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::audio::live::{Live, Source};
use crate::audio::{capture, longform};
use crate::transcribe_job::{self, Cfg};
use crate::transcripts::{self, paragraphs, Kind, Line, Speaker, Status, Transcript};

const SR: u64 = capture::WHISPER_SAMPLE_RATE as u64;
/// A You line repeating Others lines this close in time is the mic hearing the speakers.
const ECHO_WINDOW_MS: u64 = 3_000;
/// With no new text the page needs no update (its timer runs on its own), so
/// progress is only saved now and then, for a crash to keep it.
const PROGRESS_EVERY: Duration = Duration::from_secs(10);

enum Msg {
    Audio(Speaker, Vec<f32>),
    /// That track's device stopped mid-way (an unplugged headset).
    Failed(Speaker, String),
}

/// The two running captures; Stop takes them.
static CAPTURE: Mutex<Option<(Live, Live)>> = Mutex::new(None);

pub fn start(app: &AppHandle) -> Result<Transcript, String> {
    let cfg = Cfg::load(app)?;
    let id = transcripts::new_id()?;
    transcribe_job::claim(id)?;
    // Unnamed until the user names it: the page shows the start time instead.
    let mut t = Transcript::new(id, Kind::Meeting, String::new(), String::new(), cfg.model_id.clone());
    let (tx, rx) = channel();
    let started = transcripts::save(&t).and_then(|()| {
        let mic = capture_track(Speaker::You, Source::Mic(cfg.mic.clone()), id, &tx)?;
        match capture_track(Speaker::Others, Source::Speakers, id, &tx) {
            Ok(speakers) => Ok((mic, speakers)),
            Err(e) => {
                let _ = mic.0.stop();
                Err(e)
            }
        }
    });
    drop(tx);
    let ((mic, mic_wav), (speakers, speakers_wav)) = match started {
        Ok(x) => x,
        Err(e) => {
            // Nothing was recorded: leave no entry behind, and free the slot.
            let _ = transcripts::delete(id);
            transcribe_job::release(id);
            return Err(e);
        }
    };
    if let Some(e) = mic_wav.or(speakers_wav) {
        t.error = Some(format!("The audio isn't being saved ({e}). The transcript still is."));
    }
    *CAPTURE.lock().map_err(|e| e.to_string())? = Some((mic, speakers));
    crate::system::tray::set_meeting_recording(app, true);
    let first = t.clone();
    let app = app.clone();
    std::thread::spawn(move || work(app, t, rx, cfg));
    Ok(first)
}

/// One track: its device, writing its WAV as audio arrives and passing the
/// audio on. Also returns why the WAV couldn't be created, if it couldn't.
fn capture_track(speaker: Speaker, source: Source, id: i64, tx: &Sender<Msg>) -> Result<(Live, Option<String>), String> {
    let spec = hound::WavSpec { channels: 1, sample_rate: SR as u32, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let (mut wav, wav_err) = match hound::WavWriter::create(transcripts::audio_path(id, speaker), spec) {
        Ok(w) => (Some(w), None),
        Err(e) => {
            crate::logging::log_error("meeting", &format!("{speaker:?} audio not saved: {e}"));
            (None, Some(e.to_string()))
        }
    };
    let mut chunks = 0u32;
    let (tx, tx_failed) = (tx.clone(), tx.clone());
    let live = Live::start(
        source,
        move |chunk| {
            if let Some(w) = wav.as_mut() {
                for &s in &chunk {
                    let _ = w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
                }
                // Keep the header current (~every 10 s), so a crash still leaves a playable file.
                chunks += 1;
                if chunks % 100 == 0 {
                    let _ = w.flush();
                }
            }
            let _ = tx.send(Msg::Audio(speaker, chunk));
        },
        move |e| {
            crate::logging::log_error("meeting", &format!("{speaker:?} track stopped: {e}"));
            let _ = tx_failed.send(Msg::Failed(speaker, e));
        },
    )?;
    Ok((live, wav_err))
}

/// End the capture; the worker finishes the queued pieces and marks it done.
pub fn stop(app: &AppHandle) -> Result<(), String> {
    let Some((mic, speakers)) = CAPTURE.lock().map_err(|e| e.to_string())?.take() else {
        return Ok(());
    };
    crate::system::tray::set_meeting_recording(app, false);
    // Both stop even if one failed (an unplugged headset): what was heard is kept.
    let (a, b) = (mic.stop(), speakers.stop());
    a.and(b)
}

fn work(app: AppHandle, mut t: Transcript, rx: Receiver<Msg>, cfg: Cfg) {
    let started = Instant::now();
    let mut last_saved = Instant::now();
    run(
        &mut t,
        rx,
        [longform::chunker(cfg.sensitivity), longform::chunker(cfg.sensitivity)],
        |samples| {
            transcribe_job::transcribe_piece(&app, samples, &cfg)
                .inspect_err(|e| crate::logging::log_error("meeting", &format!("piece skipped: {e}")))
                .unwrap_or_default()
        },
        |t, news| {
            if news {
                let _ = app.emit("transcript://update", &*t);
            }
            if news || last_saved.elapsed() >= PROGRESS_EVERY {
                t.work_ms = started.elapsed().as_millis() as u64;
                let _ = transcripts::save(t);
                last_saved = Instant::now();
            }
        },
    );
    // ponytail: repaired once at Stop, so the live view (and a meeting cut short
    // by closing the app) keeps each piece's own punctuation; repair as each
    // paragraph closes if that reads badly.
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
/// and slot each line in by time with its speaker. `changed(t, news)` fires
/// after every chunk; `news` when there's new text or a new problem to show.
fn run<V: FnMut(&[f32]) -> Option<Vec<bool>>>(
    t: &mut Transcript,
    rx: Receiver<Msg>,
    chunkers: [longform::Chunker<V>; 2],
    mut transcribe: impl FnMut(&[f32]) -> String,
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
            added |= add_line(&mut t.lines, Line { start_ms, end_ms, text, speaker: Some(speaker) });
        }
        added
    };
    for msg in rx {
        let (speaker, chunk) = match msg {
            Msg::Audio(s, c) => (s, c),
            Msg::Failed(s, e) => {
                let who = if s == Speaker::You { "Your microphone" } else { "The computer's sound" };
                t.error = Some(format!("{who} stopped at {} ({e}). Everything before that is kept.", clock(t.done_ms)));
                changed(t, true);
                continue;
            }
        };
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

/// Insert `line` in time order. A You line that repeats what Others said at
/// the same time is the mic hearing the speakers: it's dropped (false), and an
/// Others line drops any You lines it echoes.
fn add_line(lines: &mut Vec<Line>, line: Line) -> bool {
    if line.speaker == Some(Speaker::You) {
        let others: Vec<&Line> = lines.iter().filter(|o| o.speaker == Some(Speaker::Others) && near(o, &line)).collect();
        if echoes(&line, &others) {
            return false;
        }
    } else {
        let others: Vec<Line> = lines.iter().filter(|o| o.speaker == Some(Speaker::Others)).cloned().chain([line.clone()]).collect();
        lines.retain(|y| {
            if y.speaker != Some(Speaker::You) || !near(y, &line) {
                return true;
            }
            let around: Vec<&Line> = others.iter().filter(|o| near(o, y)).collect();
            !echoes(y, &around)
        });
    }
    let at = lines.partition_point(|l| l.start_ms <= line.start_ms);
    lines.insert(at, line);
    true
}

fn near(a: &Line, b: &Line) -> bool {
    a.start_ms <= b.end_ms + ECHO_WINDOW_MS && b.start_ms <= a.end_ms + ECHO_WINDOW_MS
}

/// `you` repeats one of `others`, or (for longer lines) what they said together:
/// the two tracks cut at different pauses, so one echo can span two Others lines.
fn echoes(you: &Line, others: &[&Line]) -> bool {
    others.iter().any(|o| is_echo(&you.text, &o.text))
        || (others.len() > 1 && is_echo(&you.text, &others.iter().map(|o| o.text.as_str()).collect::<Vec<_>>().join(" ")))
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Most of `heard`'s words also appear in `said` (the mic picked up the speakers).
fn is_echo(heard: &str, said: &str) -> bool {
    let (heard, mut said) = (words(heard), words(said));
    if heard.is_empty() {
        return false;
    }
    let said_len = said.len();
    let mut found = 0;
    for w in &heard {
        if let Some(i) = said.iter().position(|x| x == w) {
            said.swap_remove(i);
            found += 1;
        }
    }
    if heard.len() <= 3 {
        // "okay" answering "is that okay with you?" is a reply, not an echo:
        // a short line only counts when it's (nearly) all that was said.
        found == heard.len() && said_len <= heard.len() + 2
    } else {
        found * 10 >= heard.len() * 6
    }
}

fn clock(ms: u64) -> String {
    format!("{}:{:02}", ms / 60_000, ms / 1000 % 60)
}

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
        assert!(!is_echo("okay", "Is that okay with you?"), "a one-word answer to a question is a reply");
        assert!(!is_echo("we can go", "we can't go"), "2 of 3 words isn't an echo");
    }

    #[test]
    fn longer_lines_are_an_echo_from_about_60_percent_shared_words() {
        assert!(is_echo("so the plan for next week is ready", "So the plan for next week, is it ready?"));
        assert!(is_echo("the numbers look much better this quarter honestly", "The numbers look better this quarter."), "75% shared");
        assert!(!is_echo("the numbers look fine but sales dropped a lot", "The numbers look great."), "33% shared");
    }

    #[test]
    fn an_echo_split_across_two_others_lines_is_dropped() {
        let mut lines = Vec::new();
        add_line(&mut lines, line(Speaker::Others, 1_000, "We should ship"));
        add_line(&mut lines, line(Speaker::Others, 3_000, "the release next week."));
        assert!(!add_line(&mut lines, line(Speaker::You, 1_500, "we should ship the release next week")));
    }

    #[test]
    fn same_words_long_before_are_not_an_echo() {
        let mut lines = vec![line(Speaker::You, 0, "let's move on to the budget")];
        add_line(&mut lines, line(Speaker::Others, 10_000, "Let's move on to the budget."));
        assert_eq!(lines.len(), 2, "a You line 8 s earlier was taken for an echo");
        // And a You piece that arrives late, from long before the Others line.
        let mut lines = vec![line(Speaker::Others, 10_000, "Let's move on to the budget.")];
        assert!(add_line(&mut lines, line(Speaker::You, 0, "let's move on to the budget")), "a You line 8 s earlier was dropped");
    }

    #[test]
    fn a_late_others_line_lands_before_a_later_you_line() {
        let mut lines = vec![line(Speaker::You, 5_000, "thanks everyone for joining")];
        add_line(&mut lines, line(Speaker::Others, 1_000, "Hi, can you hear me?"));
        assert_eq!(lines.iter().map(|l| l.start_ms).collect::<Vec<_>>(), vec![1_000, 5_000]);
    }

    fn fake_vad(s: &[f32]) -> Option<Vec<bool>> {
        Some(s.chunks_exact(FRAME).map(|f| f[0].abs() > 0.1).collect())
    }

    fn chunkers() -> [longform::Chunker<fn(&[f32]) -> Option<Vec<bool>>>; 2] {
        let v: fn(&[f32]) -> Option<Vec<bool>> = fake_vad;
        [longform::Chunker::with_vad(4 * 16_000, v), longform::Chunker::with_vad(4 * 16_000, v)]
    }

    #[test]
    fn two_tracks_become_one_transcript_in_time_order() {
        let (tx, rx) = channel();
        // You talk 0-3 s; Others talk 4-7 s; both tracks run 9 s in 0.1 s chunks.
        for i in 0..90 {
            let t = i as f32 / 10.0;
            tx.send(Msg::Audio(Speaker::You, vec![if t < 3.0 { 0.5 } else { 0.0 }; 1_600])).unwrap();
            tx.send(Msg::Audio(Speaker::Others, vec![if (4.0..7.0).contains(&t) { 0.4 } else { 0.0 }; 1_600])).unwrap();
        }
        drop(tx);
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        run(&mut t, rx, chunkers(), |s| if s.contains(&0.5) { "mine".into() } else { "theirs".into() }, |_, _| {});
        assert_eq!(t.done_ms, 9_000);
        let said: Vec<(Option<Speaker>, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(said, vec![(Some(Speaker::You), "mine"), (Some(Speaker::Others), "theirs")]);
    }

    #[test]
    fn a_device_failing_mid_meeting_is_reported_and_the_rest_kept() {
        let (tx, rx) = channel();
        tx.send(Msg::Audio(Speaker::You, vec![0.0; 16_000])).unwrap();
        tx.send(Msg::Failed(Speaker::Others, "device unplugged".into())).unwrap();
        tx.send(Msg::Audio(Speaker::You, vec![0.0; 16_000])).unwrap();
        drop(tx);
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        run(&mut t, rx, chunkers(), |_| String::new(), |_, _| {});
        assert_eq!(t.done_ms, 2_000, "audio after the failure was lost");
        assert!(t.error.as_deref().is_some_and(|e| e.contains("computer's sound") && e.contains("0:01")), "{:?}", t.error);
    }
}
