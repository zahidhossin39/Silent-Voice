// Input-sensitivity noise gate (Discord-style slider).
//
// The user's slider (0–100) sets how loud a sound must be to count as speech.
// Before transcription we scan the clip in 30ms frames, find the first/last
// frame whose RMS clears the threshold, and trim everything outside (plus a
// little padding). Wind rumbling into the mic after you stop talking gets cut
// instead of wasting transcription time and producing garbage; a clip that
// never clears the threshold is skipped entirely ("no speech").

use crate::audio::capture::WHISPER_SAMPLE_RATE;

const FRAME_MS: usize = 30;
const PAD_FRAMES: usize = 8; // ~240ms of context kept on each side

/// Map the 0–100 sensitivity slider to an RMS threshold (log scale).
/// 100 = very sensitive (whispers count as speech, threshold ~0.0015)
///   0 = very strict   (only loud speech counts,  threshold ~0.05)
fn threshold_for(sensitivity: u32) -> f32 {
    let s = sensitivity.min(100) as f32 / 100.0;
    let log_min = (0.05f32).ln(); // s = 0
    let log_max = (0.0015f32).ln(); // s = 1
    (log_min + (log_max - log_min) * s).exp()
}

fn frame_rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = frame.iter().map(|s| s * s).sum();
    (sum_sq / frame.len() as f32).sqrt()
}

/// Trim leading/trailing audio quieter than the sensitivity threshold.
/// Returns `None` when the whole clip is below threshold (no speech) or the
/// speech that remains is too short to transcribe meaningfully.
pub fn trim_silence(samples: &[f32], sensitivity: u32) -> Option<Vec<f32>> {
    let frame_len = WHISPER_SAMPLE_RATE as usize * FRAME_MS / 1000;
    if samples.len() < frame_len {
        return None;
    }
    let threshold = threshold_for(sensitivity);

    let frames: Vec<&[f32]> = samples.chunks(frame_len).collect();
    let first = frames.iter().position(|f| frame_rms(f) >= threshold)?;
    let last = frames.iter().rposition(|f| frame_rms(f) >= threshold)?;

    let start = first.saturating_sub(PAD_FRAMES) * frame_len;
    let end = ((last + 1 + PAD_FRAMES) * frame_len).min(samples.len());

    // Under ~0.3s of audio left → treat as no speech.
    if end.saturating_sub(start) < WHISPER_SAMPLE_RATE as usize * 3 / 10 {
        return None;
    }
    Some(samples[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(len: usize, amp: f32) -> Vec<f32> {
        (0..len)
            .map(|i| amp * (i as f32 * 0.3).sin())
            .collect()
    }

    #[test]
    fn silence_only_returns_none() {
        let quiet = tone(16_000, 0.001); // 1s of near-silence
        assert!(trim_silence(&quiet, 50).is_none());
    }

    #[test]
    fn speech_passes_through() {
        let speech = tone(16_000, 0.2); // 1s of loud "speech"
        let out = trim_silence(&speech, 50).expect("speech should survive");
        assert!(out.len() >= 15_000); // nearly everything kept
    }

    #[test]
    fn trailing_noise_is_trimmed() {
        // 1s speech + 2s wind-level noise.
        let mut clip = tone(16_000, 0.2);
        clip.extend(tone(32_000, 0.004));
        let out = trim_silence(&clip, 50).expect("speech present");
        // Speech (1s) + padding (~0.24s) — the 2s of wind is gone.
        assert!(out.len() < 24_000, "trailing noise not trimmed: {}", out.len());
    }

    #[test]
    fn sensitivity_extremes() {
        let soft = tone(16_000, 0.01);
        // Very sensitive → soft audio counts as speech.
        assert!(trim_silence(&soft, 100).is_some());
        // Very strict → the same audio is treated as silence.
        assert!(trim_silence(&soft, 0).is_none());
    }

    #[test]
    fn too_short_returns_none() {
        assert!(trim_silence(&tone(100, 0.5), 50).is_none());
    }
}
