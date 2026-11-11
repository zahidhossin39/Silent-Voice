// "Record a meeting": the mic (You) and the computer's sound (Others) as two
// tracks, each cut at pauses and transcribed live into one transcript.
//
// Each capture thread writes its own WAV as audio arrives (so a backlog or a
// crash never costs the recording), then hands the chunk to one worker over a
// channel. The worker transcribes pieces one at a time in arrival order (the
// STT lock serializes them anyway). Stop ends the capture; the worker then
// finishes what's queued. Always local, like files.

use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::audio::live::{Live, Source};
use crate::audio::{capture, echo, longform};
use crate::transcribe_job::{self, Cfg};
use crate::transcripts::{self, paragraphs, Kind, Line, Speaker, Status, Transcript};

const SR: u64 = capture::WHISPER_SAMPLE_RATE as u64;
/// A You line repeating Others lines this close in time is the mic hearing the speakers.
const ECHO_WINDOW_MS: u64 = 3_000;
/// A meeting line ends at the first pause after this much audio: people take
/// turns in the pauses, and each line gets one speaker label.
const TURN: usize = 2 * SR as usize;
/// With no new text the page needs no update (its timer runs on its own), so
/// progress is only saved now and then, for a crash to keep it.
const PROGRESS_EVERY: Duration = Duration::from_secs(10);
/// Pieces failing in a row before the page is told.
const MAX_FAILS_IN_A_ROW: usize = 3;

enum Msg {
    Audio(Speaker, Vec<f32>),
    /// That track's device stopped mid-way (an unplugged headset).
    Failed(Speaker, String),
}

/// The two running captures; Stop takes them.
/// The mic, and the computer's sound when it can be captured here.
static CAPTURE: Mutex<Option<(Live, Option<Live>)>> = Mutex::new(None);

/// The user's settings, transcribing with `cloud` when given.
/// `language` overrides the dictation language for meetings ("auto" detects it).
fn cfg_for(app: &AppHandle, cloud: Option<transcribe_job::Cloud>, language: Option<String>) -> Result<Cfg, String> {
    let mut cfg = Cfg::load(app)?;
    if let Some(l) = language.filter(|l| !l.is_empty()) {
        // "bn,en": several languages, detected per piece and held to that list.
        let list: Vec<String> = l.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let l = if list.len() > 1 { "auto".to_string() } else { l };
        cfg.english = if list.len() > 1 { list.iter().all(|x| x == "en") } else { crate::system::hotkey::english_text(&l, &cfg.model_id) };
        cfg.language = l;
        if list.len() > 1 {
            cfg.languages = list;
        }
    }
    if let Some(c) = cloud {
        if c.base_url.trim().is_empty() || c.model.trim().is_empty() {
            return Err("Pick a speech-to-text model for this provider in Cloud providers.".into());
        }
        // Each cloud piece is punctuated alone, so pauses become full stops: the
        // English repair model fixes that at Stop. Other languages are left as sent.
        // cfg.model_id stays the local model: it's what a failed cloud call falls back to.
        cfg.english = cfg.language.starts_with("en");
        cfg.cloud = Some(c);
    }
    Ok(cfg)
}

/// The model a meeting is transcribed with, as the page shows it.
fn model_name(cfg: &Cfg) -> String {
    cfg.cloud.as_ref().map_or_else(|| cfg.model_id.clone(), |c| c.model.clone())
}

pub fn start(app: &AppHandle, cloud: Option<transcribe_job::Cloud>, language: Option<String>, speakers: bool) -> Result<Transcript, String> {
    let mut cfg = cfg_for(app, cloud, language)?;
    cfg.speakers = speakers;
    let id = transcripts::new_id()?;
    transcribe_job::claim(id)?;
    // Unnamed until the user names it: the page shows the start time instead.
    let mut t = Transcript::new(id, Kind::Meeting, String::new(), String::new(), model_name(&cfg));
    let (tx, rx) = channel();
    let started = transcripts::save(&t).and_then(|()| {
        let mic = capture_track(Speaker::You, Source::Mic(cfg.mic.clone()), id, &tx)?;
        // Without the computer's sound (old macOS, permission refused, no
        // sound server) the meeting still records You, and says why.
        let speakers = capture_track(Speaker::Others, Source::Speakers, id, &tx)
            .inspect_err(|e| crate::logging::log_error("meeting", &format!("recording You only: {e}")));
        Ok((mic, speakers))
    });
    drop(tx);
    let ((mic, mic_wav), speakers) = match started {
        Ok(x) => x,
        Err(e) => {
            // Nothing was recorded: leave no entry behind, and free the slot.
            let _ = transcripts::delete(id);
            transcribe_job::release(id);
            return Err(e);
        }
    };
    let (speakers, speakers_wav) = match speakers {
        Ok((live, wav)) => (Some(live), wav),
        Err(e) => {
            t.error = Some(format!("Only your side was recorded. {e}"));
            (None, None)
        }
    };
    if let Some(e) = mic_wav.or(speakers_wav) {
        t.error = Some(format!("The audio isn't being saved ({e}). The transcript still is."));
    }
    if t.error.is_some() {
        transcripts::save(&t)?;
    }
    *CAPTURE.lock().map_err(|e| e.to_string())? = Some((mic, speakers));
    crate::system::tray::set_meeting_recording(app, true);
    let first = t.clone();
    let app = app.clone();
    std::thread::spawn(move || work(app, t, rx, cfg, None));
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

/// Transcribe a saved meeting again from its two WAVs, with today's model
/// (or `cloud`). The new text replaces the old as it comes; the Overview stays.
pub fn retranscribe(app: &AppHandle, id: i64, cloud: Option<transcribe_job::Cloud>, language: Option<String>, speakers: bool) -> Result<Transcript, String> {
    let mut cfg = cfg_for(app, cloud, language)?;
    cfg.speakers = speakers;
    let readers = [Speaker::You, Speaker::Others]
        .map(|s| hound::WavReader::open(transcripts::audio_path(id, s)).map_err(|_| "This meeting's audio wasn't saved, so it can't be transcribed again.".to_string()));
    let [you, others] = readers;
    let (you, others) = (you?, others?);
    let cancel = transcribe_job::claim(id)?;
    let mut t = match transcripts::load(id) {
        Ok(t) if t.kind == Kind::Meeting => t,
        Ok(_) => {
            transcribe_job::release(id);
            return Err("Only meetings can be transcribed again here.".into());
        }
        Err(e) => {
            transcribe_job::release(id);
            return Err(e);
        }
    };
    // Cancel puts this back.
    let before = t.clone();
    if let Err(e) = transcripts::save_backup(&before) {
        transcribe_job::release(id);
        return Err(e);
    }
    t.lines.clear();
    t.error = None;
    t.done_ms = 0;
    t.work_ms = 0;
    t.status = Status::Running;
    t.redo = true;
    t.model_id = model_name(&cfg);
    if let Err(e) = transcripts::save(&t) {
        transcribe_job::release(id);
        return Err(e);
    }
    // Bounded: the reader waits for the worker instead of loading hours of audio.
    let (tx, rx) = std::sync::mpsc::sync_channel(64);
    let stop = cancel.clone();
    std::thread::spawn(move || feed([you, others], tx, &stop));
    let first = t.clone();
    let app = app.clone();
    std::thread::spawn(move || work(app, t, rx, cfg, Some((cancel, before))));
    Ok(first)
}

/// Both tracks to the worker in 0.1 s turns, as if they were being recorded.
fn feed<R: std::io::Read>(readers: [hound::WavReader<R>; 2], tx: std::sync::mpsc::SyncSender<Msg>, cancel: &std::sync::atomic::AtomicBool) {
    const CHUNK: usize = SR as usize / 10;
    let mut samples = readers.map(|r| r.into_samples::<i16>());
    let mut open = [true, true];
    while open.iter().any(|o| *o) && !cancel.load(std::sync::atomic::Ordering::Relaxed) {
        for (i, speaker) in [Speaker::You, Speaker::Others].into_iter().enumerate() {
            if !open[i] {
                continue;
            }
            let chunk: Vec<f32> = samples[i].by_ref().take(CHUNK).map_while(Result::ok).map(|s| s as f32 / i16::MAX as f32).collect();
            open[i] = chunk.len() == CHUNK;
            if !chunk.is_empty() && tx.send(Msg::Audio(speaker, chunk)).is_err() {
                return;
            }
        }
    }
}

/// End the capture; the worker finishes the queued pieces and marks it done.
pub fn stop(app: &AppHandle) -> Result<(), String> {
    let Some((mic, speakers)) = CAPTURE.lock().map_err(|e| e.to_string())?.take() else {
        return Ok(());
    };
    crate::system::tray::set_meeting_recording(app, false);
    // Both stop even if one failed (an unplugged headset): what was heard is kept.
    let (a, b) = (mic.stop(), speakers.map_or(Ok(()), Live::stop));
    a.and(b)
}

/// `undo`: a Transcribe again, with its Cancel flag and the text to put back.
fn work(app: AppHandle, mut t: Transcript, rx: impl IntoIterator<Item = Msg>, cfg: Cfg, undo: Option<(std::sync::Arc<std::sync::atomic::AtomicBool>, Transcript)>) {
    let started = Instant::now();
    let mut last_saved = Instant::now();
    run(
        &mut t,
        rx,
        [longform::chunker(cfg.sensitivity).cut_at_pauses(TURN), longform::chunker(cfg.sensitivity).cut_at_pauses(TURN)],
        |samples| {
            // Cancelled: skip the pieces still queued; the old text comes back below.
            if undo.as_ref().is_some_and(|(c, _)| c.load(std::sync::atomic::Ordering::Relaxed)) {
                return Ok(String::new());
            }
            transcribe_job::transcribe_piece(&app, samples, &cfg)
                .inspect_err(|e| crate::logging::log_error("meeting", &format!("piece skipped: {e}")))
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
    if let Some((c, before)) = &undo {
        if c.load(std::sync::atomic::Ordering::Relaxed) {
            crate::logging::log_info("meeting", "transcribe again cancelled; old text kept");
            transcribe_job::finish(&app, before);
            return transcripts::drop_backup(before.id);
        }
    }
    // ponytail: repaired once at Stop, so the live view (and a meeting cut short
    // by closing the app) keeps each piece's own punctuation; repair as each
    // paragraph closes if that reads badly.
    for range in paragraphs(&t.lines) {
        repunctuate(&mut t, range, cfg.english);
    }
    t.work_ms = started.elapsed().as_millis() as u64;
    t.duration_ms = Some(t.done_ms);
    t.status = Status::Done;
    t.redo = false;
    crate::logging::log_info("meeting", &format!("done: {:.1} min, {} lines", t.done_ms as f64 / 60_000.0, t.lines.len()));
    transcribe_job::finish(&app, &t);
    if undo.is_some() {
        transcripts::drop_backup(t.id);
    }
    if cfg.speakers {
        tell_speakers_apart(&app, t.id, Some(&cfg));
    }
}

/// Label each Others line with who said it (Speaker 1, 2, ...) when the speaker
/// models are downloaded. Runs after the meeting is saved: about 15 s per minute
/// of audio, so the text is readable meanwhile and the labels arrive after.
/// `split`: the settings the meeting was just transcribed with; a line holding
/// two people is then cut between them and its parts transcribed again.
pub(crate) fn tell_speakers_apart(app: &AppHandle, id: i64, split: Option<&Cfg>) {
    if !crate::models::registry::diarize_installed() {
        return;
    }
    use crate::system::diarize;
    let started = Instant::now();
    let dir = crate::models::registry::diarize_dir();
    let found = (|| {
        let t = transcripts::load(id)?;
        let mut r = hound::WavReader::open(transcripts::audio_path(id, Speaker::Others)).map_err(|e| e.to_string())?;
        let samples: Vec<f32> = r.samples::<i16>().map_while(Result::ok).map(|s| s as f32 / 32768.0).collect();
        let turns = diarize::turns(&dir, &samples, t.others)?;
        let lines: Vec<(u64, u64)> = t.lines.iter().filter(|l| l.speaker == Some(Speaker::Others)).map(|l| (l.start_ms, l.end_ms)).collect();
        let mut who: Vec<Option<u32>> = lines.iter().map(|&(a, b)| diarize::who_spoke(&turns, a, b)).collect();
        let clip = |&(a, b): &(u64, u64)| &samples[(a as usize * 16).min(samples.len())..(b as usize * 16).min(samples.len())];
        // Lines no turn covered get the voice they sound like.
        if who.contains(&None) {
            match diarize::voiceprints(&dir, &lines.iter().map(clip).collect::<Vec<_>>()) {
                Ok(prints) => diarize::fill_in(&mut who, &prints),
                Err(e) => crate::logging::log_error("meeting", &format!("unlabelled lines left as they are: {e}")),
            }
        }
        let mut parts: Vec<Option<Vec<transcripts::Line>>> = vec![None; lines.len()];
        for (i, &(a, b)) in lines.iter().enumerate() {
            let Some(cfg) = split else { break };
            let cuts = diarize::split_at_speakers(&turns, a, b);
            if cuts.is_empty() {
                continue;
            }
            let texts: Result<Vec<String>, String> = cuts.iter().map(|c| transcribe_job::transcribe_piece(app, clip(c), cfg)).collect();
            // A part with no words, or a failed call: the line stays whole.
            if let Some(texts) = texts.ok().filter(|t| t.iter().all(|t| !t.is_empty())) {
                parts[i] = Some(
                    cuts.iter()
                        .zip(texts)
                        .map(|(&(a, b), text)| transcripts::Line { start_ms: a, end_ms: b, text, speaker: Some(Speaker::Others), who: diarize::who_spoke(&turns, a, b) })
                        .collect(),
                );
            }
        }
        Ok::<_, String>((turns, lines, who, parts))
    })();
    let (turns, lines, who, mut parts) = match found {
        Ok(x) => x,
        Err(e) => return crate::logging::log_error("meeting", &format!("speakers not told apart: {e}")),
    };
    let split = parts.iter().filter(|p| p.is_some()).count();
    // Reloaded under the job lock: the user may have renamed it meanwhile, or
    // started Transcribe again (that run labels its own lines).
    let saved = transcribe_job::unless_running(id, "transcribing", || {
        let mut t = transcripts::load(id)?;
        let others = t.lines.iter().filter(|l| l.speaker == Some(Speaker::Others));
        // The same lines as above (only renamed meanwhile): use the voice matches and splits too.
        let same = others.clone().count() == lines.len() && others.zip(&lines).all(|(l, &(a, b))| l.start_ms == a && l.end_ms == b);
        let mut i = 0;
        let mut out = Vec::with_capacity(t.lines.len());
        for mut l in std::mem::take(&mut t.lines) {
            if l.speaker != Some(Speaker::Others) {
                out.push(l);
                continue;
            }
            if !same {
                l.who = diarize::who_spoke(&turns, l.start_ms, l.end_ms);
                out.push(l);
                continue;
            }
            match parts[i].take() {
                Some(p) => out.extend(p),
                None => out.push(transcripts::Line { who: who[i], ..l }),
            }
            i += 1;
        }
        t.lines = out;
        transcripts::save(&t)?;
        Ok(t)
    });
    if let Ok(t) = saved {
        let n = t.lines.iter().filter_map(|l| l.who).max().unwrap_or(0);
        crate::logging::log_info("meeting", &format!("{n} other speakers told apart in {:.0} s; {split} lines split between two of them", started.elapsed().as_secs_f32()));
        let _ = app.emit("transcript://update", &t);
    }
}

/// The worker: until both tracks end, cut each into pieces, transcribe them,
/// and slot each line in by time with its speaker. `changed(t, news)` fires
/// after every chunk; `news` when there's new text or a new problem to show.
fn run<V: FnMut(&[f32]) -> Option<Vec<bool>>>(
    t: &mut Transcript,
    rx: impl IntoIterator<Item = Msg>,
    chunkers: [longform::Chunker<V>; 2],
    mut transcribe: impl FnMut(&[f32]) -> Result<String, String>,
    mut changed: impl FnMut(&mut Transcript, bool),
) {
    let mut heard = [0u64; 2];
    let mut gate = echo::EchoGate::default();
    let mut recent = Recent::default();
    let mut fails = 0;
    let mut chunkers = chunkers.map(Some);
    // True when a line was added.
    let mut add = |t: &mut Transcript, speaker: Speaker, pieces: Vec<longform::Segment>| {
        let mut added = false;
        for piece in pieces.into_iter().filter(|p| p.speech) {
            let text = match transcribe(&piece.samples) {
                Ok(text) => {
                    fails = 0;
                    text
                }
                // One bad piece is skipped quietly; pieces failing in a row
                // (no model, say) mean nothing is being written: say why.
                Err(e) => {
                    fails += 1;
                    if fails == MAX_FAILS_IN_A_ROW {
                        t.error = Some(format!("Nothing is being transcribed: {e}. The audio is still being saved."));
                        added = true;
                    }
                    continue;
                }
            };
            if text.is_empty() {
                continue;
            }
            if fails < MAX_FAILS_IN_A_ROW && t.error.as_deref().is_some_and(|e| e.starts_with("Nothing is being transcribed")) {
                t.error = None;
            }
            let start_ms = piece.start as u64 * 1000 / SR;
            let end_ms = start_ms + piece.samples.len() as u64 * 1000 / SR;
            added |= add_line(&mut t.lines, Line { start_ms, end_ms, text, speaker: Some(speaker), who: None });
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
        // The mic goes through the echo gate first, and comes out once the
        // computer's sound for the same moment has arrived.
        let mut added = false;
        let mic = match speaker {
            Speaker::You => gate.push_mic(&chunk),
            Speaker::Others => {
                let pieces = chunkers[i].as_mut().map(|c| c.push(&chunk)).unwrap_or_default();
                added = add(t, speaker, pieces);
                recent.push(&chunk);
                gate.push_sys(&chunk)
            }
        };
        if !mic.is_empty() {
            let pieces = chunkers[0].as_mut().map(|c| c.push(&mic)).unwrap_or_default();
            added |= add(t, Speaker::You, only_yours(pieces, &recent, gate.delay()));
        }
        added |= mic_hint(t, &gate.stats);
        changed(t, added);
    }
    let delay = gate.delay();
    let rest = gate.finish();
    let pieces = chunkers[0].as_mut().map(|c| c.push(&rest)).unwrap_or_default();
    let added = add(t, Speaker::You, only_yours(pieces, &recent, delay));
    changed(t, added);
    for (i, speaker) in [Speaker::You, Speaker::Others].into_iter().enumerate() {
        let mut pieces = chunkers[i].take().map(|c| c.finish()).unwrap_or_default();
        if speaker == Speaker::You {
            pieces = only_yours(pieces, &recent, delay);
        }
        let added = add(t, speaker, pieces);
        changed(t, added);
    }
}

/// The computer's sound of the last minute, to check You pieces against.
#[derive(Default)]
struct Recent {
    samples: Vec<f32>,
    /// Track position of samples[0].
    start: usize,
}

impl Recent {
    const KEEP: usize = 60 * SR as usize;

    fn push(&mut self, chunk: &[f32]) {
        self.samples.extend_from_slice(chunk);
        // Trimmed 10 s at a time, not every chunk.
        if self.samples.len() > Self::KEEP + 10 * SR as usize {
            let over = self.samples.len() - Self::KEEP;
            self.samples.drain(..over);
            self.start += over;
        }
    }

    /// `a..b` of the track, or None when it isn't (or is no longer) all here.
    fn get(&self, a: usize, b: usize) -> Option<&[f32]> {
        self.samples.get(a.checked_sub(self.start)?..b.checked_sub(self.start)?)
    }
}

/// You pieces with the speakers' sound taken out, minus the ones where only
/// that sound (with thumps the gate let through) was there, which would
/// otherwise be transcribed as you.
/// `delay`: how far the mic hears the speakers behind the computer's sound.
fn only_yours(pieces: Vec<longform::Segment>, recent: &Recent, delay: i64) -> Vec<longform::Segment> {
    pieces
        .into_iter()
        .filter_map(|mut p| {
            let a = (p.start as i64 - delay).max(0) as usize;
            let Some(sys) = recent.get(a, a + p.samples.len()).filter(|_| p.speech) else { return Some(p) };
            let (cleaned, voice) = echo::without_echo(&p.samples, sys);
            if !voice {
                crate::logging::log_info("meeting", &format!("You piece at {} was only the speakers' sound; skipped", clock(p.start as u64 * 1000 / SR)));
                return None;
            }
            p.samples = cleaned;
            Some(p)
        })
        .collect()
}

const SILENT_MIC: &str = "Your mic is sending silence. It may be muted, or not the mic you talk into (Settings > Microphone).";
const ECHO_MIC: &str = "Your mic is hearing your speakers, so those parts of your side are muted. Headphones keep every word you say.";

/// Warn on the page when the mic sends only silence, or mostly hears the
/// speakers. Only these warnings are set or cleared here; a real problem
/// stays. True when the page needs an update.
fn mic_hint(t: &mut Transcript, s: &echo::Stats) -> bool {
    let silent = s.mic_secs >= 20.0 && s.loudest < 0.001;
    let echo = s.playing >= 40 && s.muted * 2 >= s.playing;
    let current = t.error.as_deref();
    let want = if silent {
        Some(SILENT_MIC)
    } else if echo || current == Some(ECHO_MIC) {
        // Kept once shown: the share can dip as the call goes on.
        Some(ECHO_MIC)
    } else {
        None
    };
    let ours = current.is_none_or(|e| e == SILENT_MIC || e == ECHO_MIC);
    if !ours || current == want {
        return false;
    }
    t.error = want.map(str::to_string);
    true
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
    // Context on both sides, so a paragraph cut mid-sentence doesn't get a
    // full stop at the cut: the line after is read but not rewritten.
    let speaker = t.lines[range.start].speaker;
    let sentence_start = range.start == 0
        || t.lines[range.start - 1].speaker != speaker
        || t.lines[range.start - 1].text.ends_with(['.', '?', '!']);
    let end = if t.lines.get(range.end).is_some_and(|l| l.speaker == speaker) { range.end + 1 } else { range.end };
    let parts: Vec<&str> = t.lines[range.start..end].iter().map(|l| l.text.as_str()).collect();
    let fixed = crate::system::sherpa_stt::repunctuate_from(&parts, sentence_start);
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
        Line { start_ms, end_ms: start_ms + 2_000, text: text.into(), speaker: Some(speaker), who: None }
    }

    #[test]
    #[ignore] // needs the downloaded punctuation model; SV_TEST_TRANSCRIPT = a saved meeting .json
    fn repair_keeps_sentences_whole_across_paragraphs() {
        let path = std::env::var("SV_TEST_TRANSCRIPT").expect("SV_TEST_TRANSCRIPT");
        let mut t: Transcript = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for range in paragraphs(&t.lines) {
            repunctuate(&mut t, range, true);
        }
        for r in paragraphs(&t.lines) {
            println!("[{}..{}] ...{}", r.start, r.end, t.lines[r.end - 1].text.chars().rev().take(50).collect::<String>().chars().rev().collect::<String>());
        }
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
        run(&mut t, rx, chunkers(), |s| Ok(if s.contains(&0.5) { "mine".into() } else { "theirs".into() }), |_, _| {});
        assert_eq!(t.done_ms, 9_000);
        let said: Vec<(Option<Speaker>, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(said, vec![(Some(Speaker::You), "mine"), (Some(Speaker::Others), "theirs")]);
    }

    #[test]
    fn the_mic_hearing_the_speakers_writes_no_you_lines() {
        // Others talk in bursts; the mic hears them 30 ms later, loud enough to
        // pass as speech. 20 s in 0.1 s chunks.
        let mut x = 7u64;
        let others: Vec<f32> = (0..320_000)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                if (i / 8_000) % 3 == 0 { 0.0 } else { (x >> 40) as f32 / (1u64 << 24) as f32 - 0.5 }
            })
            .collect();
        let mic: Vec<f32> = (0..others.len()).map(|i| i.checked_sub(480).map_or(0.0, |j| 0.4 * others[j])).collect();
        let (tx, rx) = channel();
        for (m, o) in mic.chunks(1_600).zip(others.chunks(1_600)) {
            tx.send(Msg::Audio(Speaker::You, m.to_vec())).unwrap();
            tx.send(Msg::Audio(Speaker::Others, o.to_vec())).unwrap();
        }
        drop(tx);
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        // Distinct text per piece, so the word-level echo filter can't be what drops them.
        let mut n = 0;
        run(&mut t, rx, chunkers(), |_| { n += 1; Ok(format!("piece {n}")) }, |_, _| {});
        let you = t.lines.iter().filter(|l| l.speaker == Some(Speaker::You)).count();
        assert!(t.lines.iter().any(|l| l.speaker == Some(Speaker::Others)));
        assert!(you <= 1, "{you} You lines from the mic hearing the speakers");
        assert_eq!(t.error.as_deref(), Some(ECHO_MIC));
    }

    #[test]
    fn mic_warnings_come_and_go_but_never_hide_a_real_problem() {
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        let silent = echo::Stats { mic_secs: 25.0, loudest: 0.0, ..Default::default() };
        assert!(mic_hint(&mut t, &silent));
        assert_eq!(t.error.as_deref(), Some(SILENT_MIC));
        // You unmuted: the warning goes.
        assert!(mic_hint(&mut t, &echo::Stats { loudest: 0.1, ..silent }));
        assert_eq!(t.error, None);
        // Echo: shown, and kept when the share dips later.
        assert!(mic_hint(&mut t, &echo::Stats { mic_secs: 30.0, loudest: 0.1, playing: 50, muted: 40 }));
        assert!(!mic_hint(&mut t, &echo::Stats { mic_secs: 60.0, loudest: 0.1, playing: 100, muted: 40 }));
        assert_eq!(t.error.as_deref(), Some(ECHO_MIC));
        // A real problem is never replaced.
        t.error = Some("Only your side was recorded.".into());
        assert!(!mic_hint(&mut t, &silent));
        assert_eq!(t.error.as_deref(), Some("Only your side was recorded."));
    }

    #[test]
    fn a_device_failing_mid_meeting_is_reported_and_the_rest_kept() {
        let (tx, rx) = channel();
        tx.send(Msg::Audio(Speaker::You, vec![0.0; 16_000])).unwrap();
        tx.send(Msg::Failed(Speaker::Others, "device unplugged".into())).unwrap();
        tx.send(Msg::Audio(Speaker::You, vec![0.0; 16_000])).unwrap();
        drop(tx);
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        run(&mut t, rx, chunkers(), |_| Ok(String::new()), |_, _| {});
        assert_eq!(t.done_ms, 2_000, "audio after the failure was lost");
        assert!(t.error.as_deref().is_some_and(|e| e.contains("computer's sound") && e.contains("0:01")), "{:?}", t.error);
    }

    #[test]
    fn saved_tracks_are_fed_back_in_turns_until_both_end() {
        let wav = |n: usize, v: i16| {
            let mut out = std::io::Cursor::new(Vec::new());
            let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
            let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
            (0..n).for_each(|_| w.write_sample(v).unwrap());
            w.finalize().unwrap();
            hound::WavReader::new(std::io::Cursor::new(out.into_inner())).unwrap()
        };
        let (tx, rx) = std::sync::mpsc::sync_channel(1_000);
        feed([wav(4_000, 1_000), wav(1_700, -1_000)], tx, &std::sync::atomic::AtomicBool::new(false));
        let got: Vec<(Speaker, usize)> = rx.iter().map(|m| match m {
            Msg::Audio(s, c) => (s, c.len()),
            Msg::Failed(..) => panic!(),
        }).collect();
        use Speaker::*;
        assert_eq!(got, vec![(You, 1_600), (Others, 1_600), (You, 1_600), (Others, 100), (You, 800)]);
    }

    #[test]
    fn pieces_failing_in_a_row_say_why_on_the_page() {
        let (tx, rx) = channel();
        // 20 s of talk on the mic: several pieces, none of them transcribable.
        for _ in 0..200 {
            tx.send(Msg::Audio(Speaker::You, vec![0.5; 1_600])).unwrap();
        }
        drop(tx);
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        run(&mut t, rx, chunkers(), |_| Err("model 'base.en' is not downloaded".into()), |_, _| {});
        assert!(t.error.as_deref().is_some_and(|e| e.contains("not downloaded")), "{:?}", t.error);
    }
}
