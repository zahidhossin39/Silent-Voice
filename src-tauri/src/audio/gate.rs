// Input-sensitivity noise gate (Discord-style slider).
//
// The user's slider (0–100) sets how loud a sound must be to count as speech.
// Before transcription we scan the clip in 30ms frames, find the first/last
// frame whose RMS clears the threshold, and trim everything outside (plus a
// little padding). Wind rumbling into the mic after you stop talking gets cut
// instead of wasting transcription time and producing garbage; a clip that
// never clears the threshold is skipped entirely ("no speech").

use crate::audio::capture::WHISPER_SAMPLE_RATE;
use crate::audio::vad;

const FRAME_MS: usize = 30;
// Words often start softly (breathy onsets like "h", "wh") and only clear the
// RMS threshold mid-word — keep extra context BEFORE the first loud frame so
// the first word isn't clipped. Trailing padding stays shorter: silence after
// speech is exactly what Whisper hallucinates extra words on.
const PAD_FRAMES_LEAD: usize = 14; // ~420ms before speech
const PAD_FRAMES_TAIL: usize = 8; // ~240ms after speech

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

fn rms_bounds(samples: &[f32], sensitivity: u32) -> Option<(usize, usize)> {
    let frame_len = WHISPER_SAMPLE_RATE as usize * FRAME_MS / 1000;
    let threshold = threshold_for(sensitivity);
    let frames: Vec<&[f32]> = samples.chunks(frame_len).collect();
    let first = frames.iter().position(|f| frame_rms(f) >= threshold)?;
    let last = frames.iter().rposition(|f| frame_rms(f) >= threshold)?;
    Some((first * frame_len, (last + 1) * frame_len))
}

/// Trim leading/trailing audio that isn't speech. Returns `None` when the clip
/// holds no speech at all, or when what remains is too short to transcribe
/// meaningfully.
pub fn trim_silence(samples: &[f32], sensitivity: u32) -> Option<Vec<f32>> {
    let frame_len = WHISPER_SAMPLE_RATE as usize * FRAME_MS / 1000;
    if samples.len() < frame_len {
        return None;
    }

    let (speech_start, speech_end) = match vad::speech_mask(samples, vad::threshold_for(sensitivity))
    {
        // VAD ran, so trust it — including a "nothing here was a voice"
        // verdict. Falling back to RMS on an empty mask would re-admit exactly
        // the loud non-speech (fan, keyboard, music) it just rejected.
        Some(mask) => {
            let first = mask.iter().position(|&s| s)?;
            let last = mask.iter().rposition(|&s| s)?;
            (first * vad::FRAME, (last + 1) * vad::FRAME)
        }
        // Model not installed or inference failed — fall back to the RMS scan.
        None => rms_bounds(samples, sensitivity)?,
    };

    let start = speech_start.saturating_sub(PAD_FRAMES_LEAD * frame_len);
    let end = (speech_end + PAD_FRAMES_TAIL * frame_len).min(samples.len());

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

    // These target rms_bounds rather than trim_silence: a synthetic tone is not
    // a voice, so once Silero VAD is installed it (correctly) rejects the whole
    // clip and the RMS path never runs. VAD itself is covered in vad.rs.

    #[test]
    fn silence_only_returns_none() {
        let quiet = tone(16_000, 0.001); // 1s of near-silence
        assert!(rms_bounds(&quiet, 50).is_none());
    }

    #[test]
    fn loud_audio_passes_through() {
        let loud = tone(16_000, 0.2);
        let (start, end) = rms_bounds(&loud, 50).expect("loud audio should survive");
        assert_eq!(start, 0);
        assert!(end >= 15_000); // nearly everything kept
    }

    #[test]
    fn trailing_noise_is_trimmed() {
        // 1s loud + 2s wind-level noise.
        let mut clip = tone(16_000, 0.2);
        clip.extend(tone(32_000, 0.004));
        let (_, end) = rms_bounds(&clip, 50).expect("loud section present");
        assert!(end < 20_000, "trailing noise not trimmed: {end}");
    }

    #[test]
    fn sensitivity_extremes() {
        let soft = tone(16_000, 0.01);
        // Very sensitive → soft audio clears the threshold.
        assert!(rms_bounds(&soft, 100).is_some());
        // Very strict → the same audio is treated as silence.
        assert!(rms_bounds(&soft, 0).is_none());
    }

    #[test]
    fn too_short_returns_none() {
        assert!(trim_silence(&tone(100, 0.5), 50).is_none());
    }
}
