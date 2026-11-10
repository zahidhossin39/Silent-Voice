//! Meeting echo gate: mutes the parts of the mic track that are only the
//! speakers' sound, so the mic hearing the call doesn't turn into "You" lines.
//! Each mic piece it lets through then has the speakers' sound subtracted
//! (`without_echo`) before it's transcribed, so keeping you when you talk over
//! someone no longer keeps their echo too.
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
/// Delays up to ±0.5 s (a Bluetooth speaker adds ~0.2 s; the tracks can drift).
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

    /// How far (samples) the mic hears the speakers behind the computer's
    /// sound, from the recent peaks that agree; 0 before any.
    pub fn delay(&self) -> i64 {
        median(&self.peaks.iter().filter(|(s, _)| *s >= SCORE).map(|(_, l)| *l).collect::<Vec<_>>())
    }

    /// The rest of the mic audio, unchecked (less than a hop, or no speakers track).
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
            let heard = sys_end >= end;
            if mic_end < end || (!heard && mic_end < end + WAIT) {
                break;
            }
            let samples = self.mic[self.done - self.mic_start..][..HOP].to_vec();
            let (mut muted, mic, sys) = if heard { self.echo_only(end) } else { (false, 0.0, 0.0) };
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

/// Frames per fit (~4 s): short enough to follow the echo's loudness swings
/// (Windows' own mic processing moves it up to 8x), long enough not to fit
/// away the user's voice.
const FIT_FRAMES: usize = 62;

/// Takes the speakers' sound out of a mic piece and says whether the user's
/// own voice is left. `sys` is the computer's sound for the same moment,
/// already shifted by the mic's delay, as long as `mic`.
/// Per frequency, the part of the mic the speakers' sound explains (the best
/// linear filter, fitted per ~4 s) is subtracted and the rest rebuilt by
/// overlap-add. The voice check: what's left in the speech band must be loud
/// enough in at least 20% of the frames. Thumps the gate let through sit
/// below the band, so echo plus thumps alone fails it.
pub fn without_echo(mic: &[f32], sys: &[f32]) -> (Vec<f32>, bool) {
    let n = mic.len().min(sys.len());
    // Nothing playing: nothing to take out, nothing to mistake for you.
    if n < FRAME || rms(&sys[..n]) < FLOOR {
        return (mic.to_vec(), true);
    }
    let half = FRAME / 2;
    let mut planner = FftPlanner::new();
    let (fwd, inv) = (planner.plan_fft_forward(FRAME), planner.plan_fft_inverse(FRAME));
    // Periodic Hann at 50% overlap sums to exactly 1, so overlap-add of the
    // untouched frames gives the input back.
    let win: Vec<f32> = (0..FRAME).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FRAME as f32).cos()).collect();
    let count = n / half + 2;
    let padded = |x: &[f32]| {
        let mut p = vec![0.0; half];
        p.extend_from_slice(&x[..n]);
        p.resize((count + 1) * half, 0.0);
        p
    };
    let (ys, xs) = (padded(mic), padded(sys));
    let spectrum = |x: &[f32], i: usize| {
        let mut b: Vec<Complex32> = x[i * half..i * half + FRAME].iter().zip(&win).map(|(s, w)| Complex32::new(s * w, 0.0)).collect();
        fwd.process(&mut b);
        b
    };
    let x: Vec<Vec<Complex32>> = (0..count).map(|i| spectrum(&xs, i)).collect();
    let mut y: Vec<Vec<Complex32>> = (0..count).map(|i| spectrum(&ys, i)).collect();
    // A short last block joins the one before, so no fit rests on a few frames.
    let mut blocks: Vec<std::ops::Range<usize>> = (0..count).step_by(FIT_FRAMES).map(|s| s..(s + FIT_FRAMES).min(count)).collect();
    if blocks.len() > 1 && blocks[blocks.len() - 1].len() < FIT_FRAMES / 2 {
        let last = blocks.pop().unwrap();
        blocks.last_mut().unwrap().end = last.end;
    }
    for b in blocks {
        for k in 0..FRAME {
            let (sxy, sxx) = b.clone().fold((Complex32::default(), 0.0), |(a, e), i| (a + y[i][k] * x[i][k].conj(), e + x[i][k].norm_sqr()));
            let h = sxy / (sxx + 1e-12);
            for i in b.clone() {
                y[i][k] -= h * x[i][k];
            }
        }
    }
    let scale = win.iter().map(|w| w * w).sum::<f32>() * half as f32;
    let mut left: Vec<f32> = y.iter().map(|r| (r[BAND].iter().map(|c| c.norm_sqr()).sum::<f32>() / scale).sqrt()).collect();
    left.sort_by(f32::total_cmp);
    let voice = left[(left.len() - 1) * 4 / 5] >= YOUR_VOICE;
    let mut out = vec![0.0; (count + 1) * half];
    for (i, mut r) in y.into_iter().enumerate() {
        inv.process(&mut r);
        for (o, c) in out[i * half..i * half + FRAME].iter_mut().zip(&r) {
            *o += c.re / FRAME as f32;
        }
    }
    let mut cleaned = out[half..half + n].to_vec();
    cleaned.extend_from_slice(&mic[n..]);
    (cleaned, voice)
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
        assert!(!without_echo(&mic[480..], aligned).1);
    }

    #[test]
    fn your_voice_over_the_speakers_is_you() {
        let sys = talk(7, 4, 0.2);
        let you = talk(3, 4, 0.03);
        let mic: Vec<f32> = loud_echo_of(&sys).iter().zip(&you).map(|(e, y)| e + y).collect();
        let (out, voice) = without_echo(&mic[480..], &sys[..sys.len() - 480]);
        assert!(voice);
        // The echo is gone and your voice stays: what's left is close to you alone.
        let err: Vec<f32> = out.iter().zip(&you[480..]).map(|(o, y)| o - y).collect();
        assert_eq!(out.len(), mic.len() - 480);
        assert!(rms(&err) < 0.25 * rms(&you), "left after removal {:.4} vs your voice {:.4}", rms(&err), rms(&you));
    }

    #[test]
    fn with_nothing_playing_the_piece_is_left_alone() {
        let mic = talk(3, 2, 0.03);
        assert_eq!(without_echo(&mic, &vec![0.0; mic.len()]), (mic, true));
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
            without_echo(&mic[a..b], &sys[s0..(s0 + b - a).min(sys.len())]).1
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
    #[ignore] // SV_TEST_MEETING (path without "-you.wav") + SV_TEST_CLIPS (the user's dictation WAVs)
    fn echo_removal_on_a_real_call_keeps_your_voice() {
        let read = |p: &str| -> Vec<f32> { hound::WavReader::open(p).unwrap().samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect() };
        let base = std::env::var("SV_TEST_MEETING").unwrap();
        let (mic, sys) = (read(&format!("{base}-you.wav")), read(&format!("{base}-others.wav")));
        let mut clips: Vec<_> = std::fs::read_dir(std::env::var("SV_TEST_CLIPS").unwrap()).unwrap().map(|e| e.unwrap().path()).collect();
        clips.sort();
        let voice: Vec<f32> = clips.iter().flat_map(|c| read(c.to_str().unwrap())).collect();
        // Your voice laid over the call from 60 s for 40 s (others talk throughout).
        let (at, len) = (60 * 16_000, 40 * 16_000);
        let mut with_you = mic.clone();
        with_you[at..at + len].iter_mut().zip(&voice[..len]).for_each(|(m, v)| *m += v);
        // As a meeting runs it: the gate's delay, then each 10 s piece cleaned.
        let mut g = EchoGate::default();
        let delay: Vec<i64> = mic.chunks(1_600).zip(sys.chunks(1_600)).map(|(m, s)| {
            g.push_mic(m);
            g.push_sys(s);
            g.delay()
        }).collect();
        let clean = |m: &[f32]| -> Vec<f32> {
            (0..m.len()).step_by(160_000).flat_map(|a| {
                let b = (a + 160_000).min(m.len());
                let s0 = (a as i64 - delay[(b / 1_600).min(delay.len() - 1)]).max(0) as usize;
                let s = &sys[s0.min(sys.len())..(s0 + b - a).min(sys.len())];
                if s.len() < b - a { m[a..b].to_vec() } else { without_echo(&m[a..b], s).0 }
            }).collect()
        };
        let (plain, out) = (clean(&mic), clean(&with_you));
        let band = |x: &[f32]| {
            let mut b: Vec<Complex32> = x.iter().map(|s| Complex32::new(*s, 0.0)).collect();
            FftPlanner::new().plan_fft_forward(b.len()).process(&mut b);
            let n = b.len();
            ((300 * n / 16_000..3_400 * n / 16_000).map(|k| b[k].norm_sqr()).sum::<f32>() * 2.0 / (n * n) as f32).sqrt()
        };
        // Echo: speech-band level of the mic before and after, over hops with the speakers playing.
        let (mut raw, mut left) = (0.0, 0.0);
        for h in (0..mic.len().min(plain.len()) / HOP).filter(|h| rms(&sys[h * HOP..(h + 1) * HOP]) > 0.05 && !(at..at + len).contains(&(h * HOP))) {
            raw += band(&mic[h * HOP..(h + 1) * HOP]).powi(2);
            left += band(&plain[h * HOP..(h + 1) * HOP]).powi(2);
        }
        println!("echo in the speech band: {:.1} dB down", 10.0 * (raw / left.max(1e-12)).log10());
        // Your voice: the part of the output that is your voice (projection), vs your voice itself.
        // The canceller shifts its output by a few ms: compare at the best shift.
        let v = &voice[..len];
        let (shift, corr, kept) = (0..480)
            .map(|d| {
                let o = &out[at + d..at + d + len];
                let dot: f32 = o.iter().zip(v).map(|(a, b)| a * b).sum();
                (d, dot / (o.iter().map(|x| x * x).sum::<f32>() * v.iter().map(|x| x * x).sum::<f32>()).sqrt(), dot / v.iter().map(|x| x * x).sum::<f32>())
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        println!("your voice: {:.0}% of its level kept, output matches it at r = {corr:.2} ({shift} samples later)", kept * 100.0);
        // Level of the output over your voice vs your voice alone, speech band.
        println!("speech-band level over your voice: out {:.4} vs your voice {:.4}", band(&out[at..at + len]), band(v));
        // Loudness pattern (32 ms frames, dB) of the output vs your voice, where you're talking.
        let db = |x: &[f32]| 10.0 * (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32 + 1e-10).log10();
        let frames: Vec<(f32, f32)> = (0..len / 512 - 1)
            .filter(|f| rms(&v[f * 512..(f + 1) * 512]) > 0.01)
            .map(|f| (db(&out[at + shift + f * 512..][..512]), db(&v[f * 512..(f + 1) * 512])))
            .collect();
        let mean = |k: fn(&(f32, f32)) -> f32| frames.iter().map(k).sum::<f32>() / frames.len() as f32;
        let (mo, mv) = (mean(|f| f.0), mean(|f| f.1));
        let cov: f32 = frames.iter().map(|(o, v)| (o - mo) * (v - mv)).sum();
        let r = cov / (frames.iter().map(|(o, _)| (o - mo).powi(2)).sum::<f32>() * frames.iter().map(|(_, v)| (v - mv).powi(2)).sum::<f32>()).sqrt();
        println!("loudness pattern over {} voiced frames: r = {r:.2}, output {:.1} dB vs your voice", frames.len(), mo - mv);
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
