// Silero VAD — neural speech detection for the input gate.
//
// The RMS gate in gate.rs can only ask "is this loud?", so a keyboard click or
// a fan burst reads as speech while a soft word reads as silence. Silero asks
// "is this a human voice?" instead. It's optional: when the ~2MB model isn't
// downloaded, gate.rs falls back to the RMS path unchanged.

use std::path::PathBuf;
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;

/// Silero v5 accepts exactly 512 samples per frame at 16kHz (32ms).
pub const FRAME: usize = 512;

static VAD: Mutex<Option<Session>> = Mutex::new(None);

fn model_path() -> PathBuf {
    crate::models::registry::vad_dir().join("silero_vad.onnx")
}

fn init_session() -> Option<Session> {
    let path = model_path();
    if !path.exists() {
        return None;
    }

    crate::onnx::ensure_runtime()?;

    match Session::builder()
        .and_then(|b| b.with_intra_threads(1))
        .and_then(|b| b.commit_from_file(&path))
    {
        Ok(s) => Some(s),
        Err(e) => {
            crate::logging::log_error("vad", &format!("failed to load ONNX session: {e}"));
            None
        }
    }
}

/// Map the 0–100 sensitivity slider to a Silero speech probability threshold.
/// 100 = very sensitive (0.15), 0 = very strict (0.9).
pub fn threshold_for(sensitivity: u32) -> f32 {
    let s = sensitivity.min(100) as f32 / 100.0;
    0.9 + (0.15 - 0.9) * s
}

/// One speech flag per 512-sample frame. `None` when the model isn't installed
/// or inference fails — the caller then falls back to the RMS gate.
pub fn speech_mask(samples: &[f32], threshold: f32) -> Option<Vec<bool>> {
    if samples.len() < FRAME {
        return None;
    }

    let mut slot = VAD.lock().ok()?;
    if slot.is_none() {
        *slot = init_session();
    }
    let session = slot.as_mut()?;

    let mut state = ndarray::Array3::<f32>::zeros((2, 1, 128));
    let mut mask = Vec::with_capacity(samples.len() / FRAME);

    for chunk in samples.chunks(FRAME) {
        // Silero rejects short frames; the trailing partial chunk inherits the
        // previous frame's verdict rather than being fed zero-padded garbage.
        if chunk.len() < FRAME {
            break;
        }
        let input = ndarray::Array2::from_shape_vec((1, FRAME), chunk.to_vec()).ok()?;
        let (Ok(input_t), Ok(state_t), Ok(sr_t)) = (
            Tensor::from_array(input),
            Tensor::from_array(state.clone()),
            Tensor::from_array(ndarray::arr0(16000i64)),
        ) else {
            return None;
        };

        let outputs = session
            .run(ort::inputs![
                "input" => input_t,
                "state" => state_t,
                "sr" => sr_t,
            ])
            .ok()?;

        let (_, prob) = outputs["output"].try_extract_tensor::<f32>().ok()?;
        mask.push(*prob.first()? >= threshold);

        let (_, next) = outputs["stateN"].try_extract_tensor::<f32>().ok()?;
        state = ndarray::Array3::from_shape_vec((2, 1, 128), next.to_vec()).ok()?;
    }

    if mask.is_empty() {
        None
    } else {
        Some(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_not_speech() {
        if !model_path().exists() {
            return;
        }
        let quiet = vec![0.0f32; 16_000];
        let mask = speech_mask(&quiet, threshold_for(50)).expect("model loaded");
        assert!(!mask.iter().any(|&s| s), "silence flagged as speech");
    }

    #[test]
    fn tone_is_not_speech() {
        if !model_path().exists() {
            return;
        }
        // A loud pure tone clears any RMS gate but isn't a voice — this is the
        // whole reason the neural gate exists.
        let tone: Vec<f32> = (0..16_000).map(|i| 0.4 * (i as f32 * 0.3).sin()).collect();
        let mask = speech_mask(&tone, threshold_for(50)).expect("model loaded");
        let speech_frames = mask.iter().filter(|&&s| s).count();
        assert!(
            speech_frames * 4 < mask.len(),
            "pure tone read as speech in {speech_frames}/{} frames",
            mask.len()
        );
    }
}
