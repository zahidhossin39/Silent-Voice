//! Meeting echo gate: mutes the parts of the mic track that are only the
//! speakers' sound, so the mic hearing the call doesn't turn into "You" lines.
//!
//! Wispr Flow's meeting recorder does the same thing. GCC-PHAT between the mic
//! and the computer's sound finds a steady delay while the mic hears the
//! speakers. A part is muted only when the mic is no louder than that echo
//! would be, so you talking over someone is kept. Tuned on a real call (laptop
//! speakers, Discord): 82% of the echo muted, none of the user's speech, and
//! unrelated audio (headphones) never locks.

use std::collections::VecDeque;
use std::sync::Arc;

use rustfft::{num_complex::Complex32, Fft, FftPlanner};

/// 0.25 s at 16 kHz: the unit that is muted or kept.
const HOP: usize = 4_000;
/// 1 s of both tracks compared per hop.
const WIN: usize = 16_000;
/// FFT size: WIN padded so a delay doesn't wrap around.
const N: usize = 32_768;
/// Delays up to Â±0.5 s (a Bluetooth speaker adds ~0.2 s; the tracks can drift).
const MAX_LAG: usize = 8_000;
/// Only the speech band is compared: a hum or DC shared by both tracks
/// otherwise peaks at a fake 0 ms delay.
const LO: usize = 150 * N / 16_000;
const HI: usize = 4_000 * N / 16_000;
/// RMS below this is silence.
const FLOOR: f32 = 0.003;
/// A peak this strong (0..1) counts as the mic hearing the speakers.
const SCORE: f32 = 0.05;
/// Delays within 35 ms count as the same.
const TOLERANCE: i64 = 35 * 16;
/// Locked when STABLE of the last HISTORY peaks agree on the delay.
const HISTORY: usize = 8;
const STABLE: usize = 5;
/// Mic audio is let through unchecked once it's this far ahead of the
/// computer's sound (that track failed, or there is none).
const WAIT: usize = 32_000;
/// Hops held back (2 s), so the ones heard just before a lock can be muted too.
const LOOKBACK: usize = 8;

/// What the gate saw, for the page's mic warnings.
#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    /// Mic seconds checked against the computer's sound.
    pub mic_secs: f32,
    /// Hops with the computer's sound playing, and how many of them were muted.
    pub playing: u32,
    pub muted: u32,
    /// The loudest 0.25 s of the mic (RMS).
    pub loudest: f32,
}

pub struct EchoGate {
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    /// Recent samples of each track; `*_start` is the absolute index of [0].
    mic: Vec<f32>,
    mic_start: usize,
    sys: Vec<f32>,
    sys_start: usize,
    /// Mic samples already released.
    done: usize,
    /// (score, delay in samples) of recent peaks.
    peaks: Vec<(f32, i64)>,
    /// mic / speakers RMS while locked: how loud the echo is.
    ratios: Vec<f32>,
    /// Recent mic hop RMS: its noise floor.
    levels: Vec<f32>,
    /// Checked hops held back for the walkback.
    pending: VecDeque<Hop>,
    pub stats: Stats,
}

struct Hop {
    samples: Vec<f32>,
    /// RMS on the mic and on the speakers.
    mic: f32,
    sys: f32,
    muted: bool,
}

impl Hop {
    fn write_to(self, out: &mut Vec<f32>) {
        if self.muted {
            out.resize(out.len() + self.samples.len(), 0.0);
        } else {
            out.extend(self.samples);
        }
    }
}

impl Default for EchoGate {
    fn default() -> Self {
        let mut planner = FftPlanner::new();
        Self {
            fft: planner.plan_fft_forward(N),
            ifft: planner.plan_fft_inverse(N),
            mic: Vec::new(),
            mic_start: 0,
            sys: Vec::new(),
            sys_start: 0,
            done: 0,
            peaks: Vec::new(),
            ratios: Vec::new(),
            levels: Vec::new(),
            pending: VecDeque::new(),
            stats: Stats::default(),
        }
    }
}

impl EchoGate {
    /// Mic audio in; the mic audio that's ready comes out, echo muted.
    pub fn push_mic(&mut self, chunk: &[f32]) -> Vec<f32> {
        self.mic.extend_from_slice(chunk);
        self.release()
    }

    /// The computer's sound in; mic audio it unblocks comes out.
    pub fn push_sys(&mut self, chunk: &[f32]) -> Vec<f32> {
        self.sys.extend_from_slice(chunk);
        self.release()
    }

    /// The rest of the mic audio, unchecked (less than a hop, or no speakers track).
    /// How far (samples) the mic hears the speakers behind the computer's
    /// sound, from the recent peaks that agree; 0 before any.
    pub fn delay(&self) -> i64 {
        median(&self.peaks.iter().filter(|(s, _)| *s >= SCORE).map(|(_, l)| *l).collect::<Vec<_>>())
    }

    pub fn finish(mut self) -> Vec<f32> {
        let mut out = self.release();
        for h in std::mem::take(&mut self.pending) {
            h.write_to(&mut out);
        }
        out.extend_from_slice(&self.mic[self.done - self.mic_start..]);
        out
    }

    fn release(&mut self) -> Vec<f32> {
        let mut out = Vec::new();
        loop {
            let end = self.done + HOP;
            let mic_end = self.mic_start + self.mic.len();
            let sys_end = self.sys_start + self.sys.len();
            if mic_end < end || (sys_end < end && mic_end < end + WAIT) {
                break;
            }
            let from = self.done - self.mic_start;
            let samples = self.mic[from..from + HOP].to_vec();
            let (mut muted, mic, sys) = if sys_end >= end { self.echo_only(end) } else { (false, 0.0, 0.0) };
            // Hangover: the room still rings for a moment after the speakers stop.
            if let Some(last) = self.pending.back().filter(|h| h.muted && !muted && sys < FLOOR) {
                muted = sounds_like_echo(&self.ratios, &self.levels, mic, last.sys);
            }
            if muted {
                // Walkback: what sounded like this echo just before the lock
                // (finding the delay takes 1-2 s) is muted too.
                let (ratios, levels) = (&self.ratios, &self.levels);
                for h in self.pending.iter_mut().rev() {
                    if !h.muted && sounds_like_echo(ratios, levels, h.mic, h.sys) {
                        h.muted = true;
                        self.stats.muted += 1;
                    }
                }
            }
            self.pending.push_back(Hop { samples, mic, sys, muted });
            while self.pending.len() > LOOKBACK {
                self.pending.pop_front().unwrap().write_to(&mut out);
            }
            self.done = end;
        }
        // Keep one window behind what's released.
        let keep_from = self.done.saturating_sub(WIN);
        trim(&mut self.mic, &mut self.mic_start, keep_from);
        trim(&mut self.sys, &mut self.sys_start, keep_from);
        out
    }

    /// Whether the mic hop ending at `end` holds nothing but the speakers'
    /// sound, and that hop's RMS on the mic and the speakers.
    fn echo_only(&mut self, end: usize) -> (bool, f32, f32) {
        let mic = window(&self.mic, self.mic_start, end);
        let sys = window(&self.sys, self.sys_start, end);
        let (hm, hs) = (rms(&mic[mic.len() - HOP..]), rms(&sys[sys.len().saturating_sub(HOP)..]));
        self.stats.mic_secs += HOP as f32 / 16_000.0;
        self.stats.loudest = self.stats.loudest.max(hm);
        push_capped(&mut self.levels, hm, 80);

        let peak = (rms(mic) >= FLOOR && rms(sys) >= FLOOR).then(|| self.gcc_phat(mic, sys));
        if let Some(p) = peak {
            push_capped(&mut self.peaks, p, HISTORY);
        }
        let agreed: Vec<i64> = self.peaks.iter().filter(|(s, _)| *s >= SCORE).map(|(_, l)| *l).collect();
        let lag = median(&agreed);
        let locked = agreed.iter().filter(|l| (**l - lag).abs() <= TOLERANCE).count() >= STABLE;
        if locked && hs >= FLOOR && peak.is_some_and(|(_, l)| (l - lag).abs() <= TOLERANCE) {
            push_capped(&mut self.ratios, hm / hs, 40);
        }
        if hs < FLOOR {
            return (false, hm, hs);
        }
        self.stats.playing += 1;
        let muted = locked && sounds_like_echo(&self.ratios, &self.levels, hm, hs);
        self.stats.muted += muted as u32;
        (muted, hm, hs)
    }

    /// Strongest peak (0..1) of the phase-only cross-correlation, and the mic's
    /// delay behind the speakers in samples.
    fn gcc_phat(&self, mic: &[f32], sys: &[f32]) -> (f32, i64) {
        let spectrum = |x: &[f32]| {
            let mut b = vec![Complex32::default(); N];
            b.iter_mut().zip(x).for_each(|(c, &s)| c.re = s);
            self.fft.process(&mut b);
            b
        };
        let (x, y) = (spectrum(mic), spectrum(sys));
        let mut r = vec![Complex32::default(); N];
        for k in LO..HI {
            for i in [k, N - k] {
                let c = x[i] * y[i].conj();
                r[i] = c / (c.norm() + 1e-12);
            }
        }
        self.ifft.process(&mut r);
        let (mut best, mut lag) = (f32::MIN, 0i64);
        for d in -(MAX_LAG as i64)..=MAX_LAG as i64 {
            let v = r[d.rem_euclid(N as i64) as usize].re;
            if v > best {
                (best, lag) = (v, d);
            }
        }
        (best / (2 * (HI - LO)) as f32, lag)
    }
}

/// Mic this quiet next to the speakers is their echo, not you: under the
/// echo's own loudness (`ratios`, learned while locked) or the mic's noise
/// floor (`levels`), with room.
fn sounds_like_echo(ratios: &[f32], levels: &[f32], mic: f32, sys: f32) -> bool {
    // Echo gain: a low percentile, since you talking over them reads high.
    sys >= FLOOR && ratios.len() >= 3 && mic <= (3.0 * percentile(ratios, 0.3) * sys).max(3.0 * percentile(levels, 0.2))
}

/// Frame for the per-piece check: 128 ms, so a delay of tens of ms left after
/// alignment still mostly overlaps.
const FRAME: usize = 2_048;
/// The speech band. Keyboard and desk thumps (~250 Hz) and fan hum sit below it.
const BAND: std::ops::Range<usize> = (300 * FRAME / 16_000)..(3_400 * FRAME / 16_000);
/// Speech-band level (RMS) a piece needs once the speakers' sound is taken out.
/// Measured on two real calls: what echo left behind 0.0004-0.0025; the user's
/// own voice over the same calls 0.0034-0.045, even dictating quietly.
const YOUR_VOICE: f32 = 0.003;

/// Whether a mic piece holds the user's own voice and not only the speakers'
/// sound (plus thumps the gate let through). `sys` is the computer's sound for
/// the same moment, already shifted by the mic's delay, as long as `mic`.
/// Per frequency, the part of the mic the speakers' sound explains (the best
/// linear filter over the piece) is removed; what's left in the speech band
/// must be loud enough in at least 20% of the frames.
pub fn holds_your_voice(mic: &[f32], sys: &[f32]) -> bool {
    let n = mic.len().min(sys.len());
    // Nothing playing: nothing to mistake for you.
    if n < FRAME || rms(&sys[..n]) < FLOOR {
        return true;
    }
    let fft = FftPlanner::new().plan_fft_forward(FRAME);
    let win: Vec<f32> = (0..FRAME).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FRAME - 1) as f32).cos()).collect();
    let band = |x: &[f32]| {
        let mut b: Vec<Complex32> = x.iter().zip(&win).map(|(s, w)| Complex32::new(s * w, 0.0)).collect();
        fft.process(&mut b);
        b[BAND].to_vec()
    };
    let frames: Vec<(Vec<Complex32>, Vec<Complex32>)> = (0..=n - FRAME).step_by(FRAME / 2).map(|i| (band(&sys[i..i + FRAME]), band(&mic[i..i + FRAME]))).collect();
    // The speakers-to-mic filter that explains the most, per frequency.
    let h: Vec<Complex32> = (0..BAND.len())
        .map(|k| {
            let (sxy, sxx) = frames.iter().fold((Complex32::default(), 0.0), |(a, b), (x, y)| (a + y[k] * x[k].conj(), b + x[k].norm_sqr()));
            sxy / (sxx + 1e-12)
        })
        .collect();
    let scale = win.iter().map(|w| w * w).sum::<f32>() * FRAME as f32 / 2.0;
    let mut left: Vec<f32> = frames.iter().map(|(x, y)| (y.iter().zip(x).zip(&h).map(|((y, x), h)| (y - h * x).norm_sqr()).sum::<f32>() / scale).sqrt()).collect();
    left.sort_by(f32::total_cmp);
    left[(left.len() - 1) * 4 / 5] >= YOUR_VOICE
}

fn trim(buf: &mut Vec<f32>, start: &mut usize, keep_from: usize) {
    let n = keep_from.saturating_sub(*start).min(buf.len());
    buf.drain(..n);
    *start += n;
}

/// Up to WIN samples of `buf` ending at absolute index `end`.
fn window(buf: &[f32], start: usize, end: usize) -> &[f32] {
    let to = (end - start).min(buf.len());
    &buf[to.saturating_sub(WIN)..to]
}

fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
}

fn push_capped<T>(v: &mut Vec<T>, x: T, cap: usize) {
    v.push(x);
    if v.len() > cap {
        v.remove(0);
    }
}

fn percentile(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    s[((s.len() - 1) as f32 * p).round() as usize]
}

fn median(v: &[i64]) -> i64 {
    let mut s = v.to_vec();
    s.sort_unstable();
    s.get(s.len() / 2).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Speech-like test audio: noise in 0.5 s bursts with pauses between.
    fn talk(seed: u64, secs: usize, level: f32) -> Vec<f32> {
        let mut x = seed;
        (0..secs * 16_000)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let on = (i / 8_000 + seed as usize) % 3 != 0;
                if on { ((x >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 2.0 * level } else { 0.0 }
            })
            .collect()
    }

    /// The mic hearing `sys` 30 ms late at 4% loudness, over a little room noise.
    fn echo_of(sys: &[f32]) -> Vec<f32> {
        let noise = talk(99, sys.len() / 16_000 + 1, 0.004);
        (0..sys.len()).map(|i| i.checked_sub(480).map_or(0.0, |j| 0.04 * sys[j]) + noise[i] * 0.5).collect()
    }

    /// Both tracks through the gate in 0.1 s chunks, as a meeting sends them.
    fn run(mic: &[f32], sys: &[f32]) -> (Vec<f32>, Stats) {
        let mut g = EchoGate::default();
        let mut out = Vec::new();
        for (m, s) in mic.chunks(1_600).zip(sys.chunks(1_600)) {
            out.extend(g.push_mic(m));
            out.extend(g.push_sys(s));
        }
        let stats = g.stats;
        out.extend(g.finish());
        (out, stats)
    }

    fn hop_rms(x: &[f32], at: usize) -> f32 {
        rms(&x[at * HOP..(at + 1) * HOP])
    }

    #[test]
    fn the_mic_hearing_the_speakers_is_muted() {
        let sys = talk(7, 20, 0.2);
        let (out, stats) = run(&echo_of(&sys), &sys);
        assert_eq!(out.len(), sys.len(), "the mic track changed length");
        assert!(stats.muted * 10 >= stats.playing * 7, "only {} of {} echo hops muted", stats.muted, stats.playing);
    }

    #[test]
    fn you_talking_over_them_is_kept() {
        let sys = talk(7, 20, 0.2);
        let you = talk(3, 20, 0.08);
        let mic: Vec<f32> = echo_of(&sys).iter().zip(&you).map(|(e, y)| e + y).collect();
        let (out, _) = run(&mic, &sys);
        for h in 0..sys.len() / HOP {
            if hop_rms(&you, h) > 0.03 {
                assert!(hop_rms(&out, h) > 0.03, "your speech at {:.2} s was muted", h as f32 / 4.0);
            }
        }
    }

    #[test]
    fn with_headphones_nothing_is_muted() {
        let sys = talk(7, 20, 0.2);
        let mic = talk(11, 20, 0.01);
        let (out, stats) = run(&mic, &sys);
        assert_eq!(stats.muted, 0);
        assert_eq!(out, mic);
    }

    #[test]
    fn without_the_computers_sound_the_mic_passes_unchecked() {
        let mic = talk(5, 5, 0.1);
        let mut g = EchoGate::default();
        let mut out = Vec::new();
        for m in mic.chunks(1_600) {
            out.extend(g.push_mic(m));
        }
        assert!(out.len() >= mic.len() - WAIT - HOP * (LOOKBACK + 1), "the mic waited for a track that isn't there: {} of {}", out.len(), mic.len());
        out.extend(g.finish());
        assert_eq!(out, mic);
    }

    /// Low thumps like a keyboard or desk: a 20 ms swell of 200 Hz every 0.3 s.
    fn thumps(len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let k = i % 4_800;
                let env = if k < 320 { (std::f32::consts::PI * k as f32 / 320.0).sin().powi(2) } else { 0.0 };
                0.2 * env * (i as f32 * 2.0 * std::f32::consts::PI * 200.0 / 16_000.0).sin()
            })
            .collect()
    }

    /// The mic hearing `sys` 30 ms late at 20% loudness over a quiet room, as
    /// on the real calls (speech-band noise ~0.0004).
    fn loud_echo_of(sys: &[f32]) -> Vec<f32> {
        let noise = talk(99, sys.len() / 16_000 + 1, 0.001);
        (0..sys.len()).map(|i| i.checked_sub(480).map_or(0.0, |j| 0.2 * sys[j]) + noise[i]).collect()
    }

    #[test]
    fn a_piece_that_is_only_the_speakers_and_thumps_is_not_you() {
        let sys = talk(7, 4, 0.2);
        let mic: Vec<f32> = loud_echo_of(&sys).iter().zip(thumps(sys.len())).map(|(e, t)| e + t).collect();
        let aligned = &sys[..sys.len() - 480];
        assert!(!holds_your_voice(&mic[480..], aligned));
    }

    #[test]
    fn your_voice_over_the_speakers_is_you() {
        let sys = talk(7, 4, 0.2);
        let you = talk(3, 4, 0.03);
        let mic: Vec<f32> = loud_echo_of(&sys).iter().zip(&you).map(|(e, y)| e + y).collect();
        assert!(holds_your_voice(&mic[480..], &sys[..sys.len() - 480]));
    }

    #[test]
    #[ignore] // SV_TEST_MEETINGS = ';'-separated meeting paths (no extension), SV_TEST_CLIPS = a dir of the user's dictation WAVs
    fn fake_you_lines_of_real_calls_are_dropped_and_your_voice_kept() {
        let read = |p: &str| -> Vec<f32> { hound::WavReader::open(p).unwrap().samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect() };
        // As a meeting runs it: the gate's delay at the piece's end, then the piece check.
        let delays = |mic: &[f32], sys: &[f32]| {
            let mut g = EchoGate::default();
            mic.chunks(1_600).zip(sys.chunks(1_600)).map(|(m, s)| {
                g.push_mic(m);
                g.push_sys(s);
                g.delay()
            }).collect::<Vec<i64>>()
        };
        let check = |mic: &[f32], sys: &[f32], delay: &[i64], a: usize, b: usize| {
            let d = delay[(b / 1_600).min(delay.len() - 1)];
            let s0 = (a as i64 - d).max(0) as usize;
            holds_your_voice(&mic[a..b], &sys[s0..(s0 + b - a).min(sys.len())])
        };
        let meetings = std::env::var("SV_TEST_MEETINGS").unwrap();
        let mut voice = Vec::new();
        let mut clips: Vec<_> = std::fs::read_dir(std::env::var("SV_TEST_CLIPS").unwrap()).unwrap().map(|e| e.unwrap().path()).collect();
        clips.sort();
        clips.iter().for_each(|c| voice.extend(read(c.to_str().unwrap())));
        let (mut kept_fake, mut lost_you) = (Vec::new(), Vec::new());
        for base in meetings.split(';') {
            let (mic, sys) = (read(&format!("{base}-you.wav")), read(&format!("{base}-others.wav")));
            let delay = delays(&mic, &sys);
            let t: crate::transcripts::Transcript = serde_json::from_str(&std::fs::read_to_string(format!("{base}.json")).unwrap()).unwrap();
            for l in t.lines.iter().filter(|l| l.speaker == Some(crate::transcripts::Speaker::You)) {
                let keep = check(&mic, &sys, &delay, l.start_ms as usize * 16, l.end_ms as usize * 16);
                println!("{base:.0} {:>6} {} {}", l.start_ms, if keep { "KEPT   " } else { "dropped" }, l.text);
                if keep {
                    kept_fake.push(l.start_ms);
                }
            }
            // The user's own voice laid over the call, 3 s at a time.
            // 3 s stretches of the dictation where the user is talking (30%+ of 32 ms frames voiced), not pauses.
            let talking: Vec<&[f32]> = voice.chunks_exact(48_000).filter(|c| c.chunks_exact(512).filter(|f| rms(f) > 0.01).count() * 10 >= 93 * 3).collect();
            for (k, at) in [20usize, 30, 50, 60, 100, 140, 210, 240, 265].into_iter().enumerate().filter(|(_, at)| (at + 3) * 16_000 < mic.len()) {
                let (a, v) = (at * 16_000, talking[k]);
                let mut m = mic.clone();
                m[a..a + v.len()].iter_mut().zip(v).for_each(|(m, v)| *m += v);
                if !check(&m, &sys, &delay, a, a + v.len()) {
                    lost_you.push(at);
                }
            }
        }
        println!("fake lines kept: {kept_fake:?}; your voice lost at: {lost_you:?}");
        assert!(lost_you.is_empty(), "your voice was dropped at {lost_you:?} s");
        assert!(kept_fake.len() <= 3, "{} fake You lines kept", kept_fake.len());
    }

    #[test]
    #[ignore] // SV_TEST_MEETING = a meeting's audio path without "-you.wav"/"-others.wav"
    fn a_real_call_heard_through_laptop_speakers() {
        let base = std::env::var("SV_TEST_MEETING").expect("SV_TEST_MEETING");
        let read = |s: &str| -> Vec<f32> {
            hound::WavReader::open(format!("{base}-{s}.wav")).unwrap().samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
        };
        let (mic, sys) = (read("you"), read("others"));
        let (out, stats) = run(&mic, &sys);
        println!("{stats:?}");
        if let Ok(path) = std::env::var("SV_TEST_OUT") {
            // The mic as the transcriber hears it, to listen to or transcribe.
            let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
            let mut w = hound::WavWriter::create(path, spec).unwrap();
            out.iter().for_each(|s| w.write_sample((s * 32767.0) as i16).unwrap());
            w.finalize().unwrap();
        }
        let speech = |x: &[f32]| {
            let mut c = crate::audio::longform::chunker(100).cut_at_pauses(32_000);
            let mut p = c.push(x);
            p.extend(c.finish());
            p.iter().filter(|s| s.speech).map(|s| format!("{:.1}-{:.1}", s.start as f32 / 16e3, (s.start + s.samples.len()) as f32 / 16e3)).collect::<Vec<_>>()
        };
        println!("mic pieces before: {:?}\nmic pieces after:  {:?}", speech(&mic), speech(&out));
        assert!(stats.muted * 10 >= stats.playing * 7, "only {} of {} hops with sound playing muted", stats.muted, stats.playing);
        for h in 0..out.len() / HOP {
            if hop_rms(&mic, h) > 0.012 {
                assert!(hop_rms(&out, h) > 0.0, "the mic's loud part at {:.2} s was muted", h as f32 / 4.0);
            }
        }
    }
}
