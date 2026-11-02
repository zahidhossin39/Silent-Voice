// The computer's own sound (the meeting's "Others" track), one interface with
// a native backend per OS (docs/adr/0001). A backend pushes mono f32 samples
// at its own rate into `raw`; live.rs resamples and keeps it in step with the
// clock, the same as the mic.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex};

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
            Self::Unsupported(why) => write!(f, "Recording other people isn't possible here: {why}"),
            Self::PermissionDenied => f.write_str("Silent Voice isn't allowed to record the computer's sound."),
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

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn support() -> Result<(), CaptureError> {
        Err(CaptureError::Unsupported("not built for this system yet".into()))
    }

    pub fn open(_: Arc<Mutex<Vec<f32>>>, _: Arc<Mutex<Option<String>>>) -> Result<(Box<dyn Any>, u32), CaptureError> {
        support().map(|_| unreachable!())
    }
}
