// The computer's own sound (the meeting's "Others" track), one interface with
// a native backend per OS (docs/adr/0001). A backend pushes mono f32 samples
// at its own rate into `raw`; live.rs resamples and keeps it in step with the
// clock, the same as the mic.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex};

#[cfg_attr(windows, allow(dead_code))] // Windows only ever fails
pub enum CaptureError {
    /// This OS (or version, or audio system) can't capture the computer's sound.
    Unsupported(String),
    /// The OS asked the user, and the user said no (macOS Screen & System Audio Recording).
    PermissionDenied,
    Failed(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Unsupported(why) => write!(f, "Recording other people {why}."),
            Self::PermissionDenied => f.write_str(
                "Silent Voice isn't allowed to record the computer's sound: allow it in System Settings > Privacy & Security > Screen & System Audio Recording, then restart Silent Voice.",
            ),
            Self::Failed(e) => f.write_str(e),
        }
    }
}

/// Can the computer's sound be captured here, and if not, why.
pub fn support() -> Result<(), CaptureError> {
    imp::support()
}

/// Start capturing into `raw`; `failed` gets the reason if it dies mid-way.
/// Returns the sample rate and a handle that keeps the capture running until
/// dropped (on the thread that opened it).
pub(super) fn open(
    raw: Arc<Mutex<Vec<f32>>>,
    failed: Arc<Mutex<Option<String>>>,
) -> Result<(Box<dyn Any>, u32), CaptureError> {
    imp::open(raw, failed)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use cpal::traits::HostTrait;

    pub fn support() -> Result<(), CaptureError> {
        Ok(())
    }

    // An input stream on an output device is WASAPI loopback in cpal.
    pub fn open(raw: Arc<Mutex<Vec<f32>>>, failed: Arc<Mutex<Option<String>>>) -> Result<(Box<dyn Any>, u32), CaptureError> {
        use cpal::traits::DeviceTrait;
        let host = cpal::default_host();
        let d = host
            .default_output_device()
            .ok_or_else(|| CaptureError::Failed("No speakers or headphones found.".into()))?;
        let c = d.default_output_config().map_err(|e| CaptureError::Failed(e.to_string()))?;
        let rate = c.sample_rate().0;
        let stream = crate::audio::live::start_stream(&d, c, raw, failed).map_err(CaptureError::Failed)?;
        Ok((Box::new(stream), rate))
    }
}

#[cfg(target_os = "linux")]
use super::system_audio_linux as imp;

#[cfg(target_os = "macos")]
use super::system_audio_macos as imp;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use super::*;

    pub fn support() -> Result<(), CaptureError> {
        Err(CaptureError::Unsupported("isn't supported on this system".into()))
    }

    pub fn open(_: Arc<Mutex<Vec<f32>>>, _: Arc<Mutex<Option<String>>>) -> Result<(Box<dyn Any>, u32), CaptureError> {
        support().map(|_| unreachable!())
    }
}

/// Mix captured buffers down to one channel. Each entry is (channels in that
/// buffer, its samples): several 1-channel buffers are planar audio, one
/// n-channel buffer is interleaved. Platform-free so it is tested everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn mix_to_mono(bufs: &[(usize, &[f32])]) -> Vec<f32> {
    let total: usize = bufs.iter().map(|(n, _)| (*n).max(1)).sum();
    let frames = bufs.iter().map(|(n, s)| s.len() / (*n).max(1)).min().unwrap_or(0);
    (0..frames)
        .map(|i| {
            let sum: f32 = bufs.iter().map(|(n, s)| s[i * n.max(&1)..][..*n.max(&1)].iter().sum::<f32>()).sum();
            sum / total as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::mix_to_mono;

    #[test]
    fn planar_stereo_averages_the_two_channels() {
        let (l, r) = ([1.0, 0.5, 0.0], [0.0, 0.5, 1.0]);
        assert_eq!(mix_to_mono(&[(1, &l), (1, &r)]), vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn interleaved_stereo_averages_each_frame() {
        assert_eq!(mix_to_mono(&[(2, &[1.0, 0.0, 0.5, 0.5, 0.0, 1.0])]), vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn mono_passes_through() {
        assert_eq!(mix_to_mono(&[(1, &[0.1, -0.2])]), vec![0.1, -0.2]);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(mix_to_mono(&[]).is_empty());
        assert!(mix_to_mono(&[(1, &[]), (1, &[])]).is_empty());
    }

    // ScreenCaptureKit may deliver 48 kHz stereo even when 16 kHz mono is asked for.
    #[test]
    fn a_48k_stereo_sine_comes_out_16k_mono_with_the_same_length_and_level() {
        let n = 48_000;
        let sine: Vec<f32> = (0..n).map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48_000.0).sin()).collect();
        let mono = mix_to_mono(&[(1, &sine), (1, &sine)]);
        let mut out = Vec::new();
        crate::audio::decode::Resampler::new(48_000, 16_000).push(&mono, &mut out);
        assert!((out.len() as i64 - 16_000).abs() < 50, "{} samples for 1 s", out.len());
        let rms = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
        assert!((rms - 0.5 / 2f32.sqrt()).abs() < 0.02, "rms {rms}");
    }
}
