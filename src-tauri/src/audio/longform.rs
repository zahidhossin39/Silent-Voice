// Split a long stream of 16 kHz audio (a dropped-in file now, a live meeting
// recording later) into pieces short enough for the speech engines, cutting in
// the pauses between sentences. Audio goes in as it arrives; finished pieces
// come out, so a 2-hour file never has to sit in memory whole.

use crate::audio::vad::{self, FRAME};

/// A finished piece of audio, ready to transcribe.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Offset of the first sample from the start of the stream.
    pub start: usize,
    pub samples: Vec<f32>,
    /// False only when the VAD heard no voice in it at all (silence, music):
    /// skip those rather than let a speech engine invent words for them.
    pub speech: bool,
}

/// The longest piece handed to a speech engine: Parakeet's memory grows with
/// length squared and Moonshine loops on long input; whisper's window is 30 s.
pub const MAX_SEGMENT: usize = 12 * 16_000;

/// A chunker on the real Silero VAD, at the user's sensitivity setting.
pub fn chunker(sensitivity: u32) -> Chunker<impl FnMut(&[f32]) -> Option<Vec<bool>>> {
    let threshold = vad::threshold_for(sensitivity);
    // One Silero state for the whole stream, so ~1 s pushes VAD like one clip.
    let mut state = vad::new_state();
    Chunker::with_vad(MAX_SEGMENT, move |s| vad::speech_mask_from(s, threshold, &mut state))
}

pub struct Chunker<V: FnMut(&[f32]) -> Option<Vec<bool>>> {
    vad: V,
    max: usize,
    buf: Vec<f32>,
    /// One speech flag per whole VAD frame of `buf`.
    mask: Vec<bool>,
    /// Stream offset of `buf[0]`.
    offset: usize,
}

impl<V: FnMut(&[f32]) -> Option<Vec<bool>>> Chunker<V> {
    pub fn with_vad(max_samples: usize, vad: V) -> Self {
        Self { vad, max: max_samples, buf: Vec::new(), mask: Vec::new(), offset: 0 }
    }

    pub fn push(&mut self, samples: &[f32]) -> Vec<Segment> {
        self.buf.extend_from_slice(samples);
        // VAD only the frames that just completed.
        let done = self.mask.len() * FRAME;
        let ready = self.buf.len() / FRAME * FRAME;
        if ready > done {
            // No VAD (not installed, or it failed on these frames): count them as
            // speech, so nothing is skipped and the next push tries again.
            let flags = (self.vad)(&self.buf[done..ready]).unwrap_or_else(|| vec![true; (ready - done) / FRAME]);
            self.mask.extend(flags);
        }
        let mut out = Vec::new();
        while self.buf.len() > self.max {
            let cut = silence_midpoints(&self.mask)
                .into_iter()
                .filter(|&c| c > 0 && c <= self.max)
                .last()
                // No real pause in reach: cut at the quietest moment (a breath)
                // in the last 1.5 s before the cap.
                .unwrap_or_else(|| quietest_boundary(&self.buf, self.max.saturating_sub(BREATH_SEARCH), self.max));
            out.push(self.take(cut));
        }
        out
    }

    /// Hand out `buf[..cut]` (cut on a frame boundary) as a segment.
    fn take(&mut self, cut: usize) -> Segment {
        let frames: Vec<bool> = self.mask.drain(..(cut / FRAME).min(self.mask.len())).collect();
        let seg = Segment { start: self.offset, samples: self.buf.drain(..cut).collect(), speech: frames.contains(&true) };
        self.offset += cut;
        seg
    }

    pub fn finish(self) -> Vec<Segment> {
        if self.buf.is_empty() {
            return Vec::new();
        }
        // A tail under one VAD frame (32 ms) has no flags and counts as silence.
        let speech = self.mask.contains(&true);
        vec![Segment { start: self.offset, samples: self.buf, speech }]
    }
}

/// How far back from the cap to look for a breath when there's no pause.
const BREATH_SEARCH: usize = 24_000; // 1.5 s

/// The VAD-frame boundary in `lo..=hi` with the least energy around it (±10 ms).
/// Only frame boundaries, so the speech flags stay aligned after the cut (why
/// this isn't sherpa_stt's `quietest_cut`, which snaps to any 10 ms step).
fn quietest_boundary(buf: &[f32], lo: usize, hi: usize) -> usize {
    const AROUND: usize = 160;
    let mut best = (f32::MAX, hi / FRAME * FRAME);
    let mut b = lo.div_ceil(FRAME).max(1) * FRAME;
    while b <= hi && b < buf.len() {
        let e: f32 = buf[b.saturating_sub(AROUND)..(b + AROUND).min(buf.len())].iter().map(|s| s * s).sum();
        if e < best.0 {
            best = (e, b);
        }
        b += FRAME;
    }
    best.1
}

/// Sample offsets at the middle of each silence gap long enough (~0.38 s) to be
/// a real between-sentence pause rather than a gap between words.
pub(crate) fn silence_midpoints(mask: &[bool]) -> Vec<usize> {
    const MIN_SILENCE_FRAMES: usize = 12; // 12 × 32 ms ≈ 0.38 s
    let mut cuts = Vec::new();
    let mut i = 0;
    while i < mask.len() {
        if mask[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < mask.len() && !mask[i] {
            i += 1;
        }
        if i - start >= MIN_SILENCE_FRAMES {
            cuts.push(((start + i) / 2) * FRAME);
        }
    }
    cuts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake VAD: a frame is "speech" when its first sample is loud.
    fn fake_vad(s: &[f32]) -> Option<Vec<bool>> {
        Some(s.chunks_exact(crate::audio::vad::FRAME).map(|f| f[0].abs() > 0.1).collect())
    }

    #[test]
    fn short_audio_comes_out_whole_on_finish() {
        let audio = vec![0.5f32; 16_000]; // 1 s of "speech"
        let mut c = Chunker::with_vad(12 * 16_000, fake_vad);
        assert!(c.push(&audio).is_empty(), "nothing is finished until the cap or the end");
        assert_eq!(c.finish(), vec![Segment { start: 0, samples: audio, speech: true }]);
    }

    #[test]
    fn a_silent_stretch_is_marked_as_no_speech() {
        let sr = 16_000;
        let mut audio = vec![0.0f32; 14 * sr]; // 14 s of nothing (music break, hold)
        audio.extend(vec![0.5f32; 3 * sr]);
        let mut c = Chunker::with_vad(12 * sr, fake_vad);
        let mut segs = c.push(&audio);
        segs.extend(c.finish());
        assert!(!segs[0].speech, "silence flagged as speech");
        assert!(segs.last().unwrap().speech, "talk flagged as silence");
    }

    #[test]
    fn without_a_vad_every_piece_counts_as_speech() {
        let mut c = Chunker::with_vad(12 * 16_000, |_: &[f32]| None);
        let mut segs = c.push(&vec![0.0f32; 20 * 16_000]);
        segs.extend(c.finish());
        assert!(segs.iter().all(|s| s.speech), "with no VAD we must not skip anything");
    }

    #[test]
    fn past_the_cap_it_cuts_inside_the_pause() {
        let sr = 16_000;
        let mut audio = vec![0.5f32; 8 * sr]; // 8 s talk
        audio.extend(vec![0.0f32; sr]); // 1 s pause
        audio.extend(vec![0.5f32; 8 * sr]); // 8 s talk
        let mut c = Chunker::with_vad(12 * sr, fake_vad);
        let first = c.push(&audio);
        assert_eq!(first.len(), 1);
        let cut = first[0].samples.len();
        assert!(cut > 8 * sr && cut < 9 * sr, "cut at {cut}, not in the pause");
        let rest = c.finish();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].start, cut);
        assert_eq!(cut + rest[0].samples.len(), audio.len(), "every sample kept once");
    }

    #[test]
    fn with_two_pauses_in_reach_it_cuts_in_the_later_one() {
        let sr = 16_000;
        let mut audio = vec![0.5f32; 3 * sr];
        audio.extend(vec![0.0f32; sr]); // pause at 3-4 s
        audio.extend(vec![0.5f32; 5 * sr]);
        audio.extend(vec![0.0f32; sr]); // pause at 9-10 s
        audio.extend(vec![0.5f32; 6 * sr]);
        let mut c = Chunker::with_vad(12 * sr, fake_vad);
        let cut = c.push(&audio)[0].samples.len();
        assert!(cut > 9 * sr && cut < 10 * sr, "cut at {cut}: the longest piece fits, so take the later pause");
    }

    #[test]
    fn a_vad_failure_counts_as_speech_and_the_next_push_retries() {
        let sr = 16_000;
        let mut calls = 0;
        let mut c = Chunker::with_vad(12 * sr, |s: &[f32]| {
            calls += 1;
            if calls == 1 { None } else { fake_vad(s) }
        });
        let mut segs = c.push(&vec![0.0f32; sr]); // fails: kept as speech
        segs.extend(c.push(&vec![0.0f32; 13 * sr])); // works: silence found
        segs.extend(c.finish());
        assert!(segs[0].speech, "a failed VAD must not drop audio");
        assert!(segs.iter().skip(1).any(|s| !s.speech), "VAD never came back after one failure");
    }

    #[test]
    fn nonstop_talk_splits_at_the_quietest_breath_within_the_cap() {
        let sr = 16_000;
        // 20 s with no real pause, only a 0.1 s breath at 11.5 s.
        let mut audio = vec![0.5f32; 20 * sr];
        let breath = 11 * sr + sr / 2;
        audio[breath..breath + sr / 10].fill(0.001);
        let mut c = Chunker::with_vad(12 * sr, fake_vad);
        let segs = c.push(&audio);
        let cut = segs[0].samples.len();
        assert!(cut <= 12 * sr, "segment over the cap: {cut}");
        assert!(cut >= breath && cut <= breath + sr / 10, "cut at {cut}, breath at {breath}");
    }

    /// Checkpoint A: a real 10-minute mp3 through decode + real VAD.
    /// `$env:SV_AUDIO="<folder>"; cargo test --lib real_file_chunks -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_file_chunks() {
        let path = std::path::PathBuf::from(std::env::var("SV_AUDIO").expect("SV_AUDIO")).join("war_10min.mp3");
        let t = std::time::Instant::now();
        let mut d = crate::audio::decode::FileDecoder::open(&path).unwrap();
        let mut c = chunker(50);
        let mut segs = Vec::new();
        while let Some(chunk) = d.next_chunk().unwrap() {
            segs.extend(c.push(&chunk));
        }
        segs.extend(c.finish());
        let secs: Vec<f32> = segs.iter().map(|s| s.samples.len() as f32 / 16_000.0).collect();
        let total: f32 = secs.iter().sum();
        println!(
            "{} pieces ({} with speech), {:.1}-{:.1} s each, {:.1} s total, in {:?}",
            segs.len(),
            segs.iter().filter(|s| s.speech).count(),
            secs.iter().cloned().fold(f32::MAX, f32::min),
            secs.iter().cloned().fold(0.0, f32::max),
            total,
            t.elapsed()
        );
        assert!(segs.iter().all(|s| s.samples.len() <= MAX_SEGMENT));
        assert!((total - 636.7).abs() < 1.0);
    }

    #[test]
    fn every_sample_comes_out_once_in_order_and_no_piece_exceeds_the_cap() {
        let mut seed = 12345u64;
        let mut rnd = |n: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % n
        };
        let cap = 12 * 16_000;
        for _ in 0..30 {
            // Random runs of talk and pause, 0-4 s each, ~2 minutes total.
            let mut audio = Vec::new();
            while audio.len() < 120 * 16_000 {
                let talk = rnd(2) == 0;
                let n = rnd(4 * 16_000) + 1;
                let start = audio.len();
                audio.extend((0..n).map(|i| if talk { 0.5 + ((start + i) % 7) as f32 * 0.01 } else { 0.0 }));
            }
            let mut c = Chunker::with_vad(cap, fake_vad);
            let mut segs = Vec::new();
            let mut i = 0;
            while i < audio.len() {
                let n = (rnd(40_000) + 1).min(audio.len() - i);
                segs.extend(c.push(&audio[i..i + n]));
                i += n;
            }
            segs.extend(c.finish());
            let mut next = 0;
            for s in &segs {
                assert_eq!(s.start, next, "gap or overlap");
                assert!(s.samples.len() <= cap, "piece of {} over the cap", s.samples.len());
                assert!(!s.samples.is_empty());
                assert_eq!(&s.samples[..], &audio[s.start..s.start + s.samples.len()]);
                next += s.samples.len();
            }
            assert_eq!(next, audio.len(), "samples lost at the end");
        }
    }
}
