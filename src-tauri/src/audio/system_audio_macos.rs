// macOS Others capture: ScreenCaptureKit audio-only capture of the main
// display (macOS 13+, the "Screen & System Audio Recording" permission).
// docs/adr/0001.

use super::system_audio::{mix_to_mono, CaptureError};
use crate::audio::capture::WHISPER_SAMPLE_RATE;
use crate::audio::decode::Resampler;
use screencapturekit::prelude::*;
use std::any::Any;
use std::sync::{Arc, Mutex};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// Is the permission granted? Never prompts.
    fn CGPreflightScreenCaptureAccess() -> bool;
    /// Prompts the first time; later calls just report.
    fn CGRequestScreenCaptureAccess() -> bool;
}

fn macos_13_or_newer() -> bool {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).split('.').next()?.trim().parse::<u32>().ok())
        .is_some_and(|major| major >= 13)
}

pub fn support() -> Result<(), CaptureError> {
    if !macos_13_or_newer() {
        return Err(CaptureError::Unsupported("needs macOS 13 or newer".into()));
    }
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err(CaptureError::PermissionDenied);
    }
    Ok(())
}

struct Audio {
    raw: Arc<Mutex<Vec<f32>>>,
    /// Converts whatever rate arrives to 16 kHz; rebuilt if the rate changes.
    resampler: Mutex<Option<(u32, Resampler)>>,
}

impl SCStreamOutputTrait for Audio {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if !matches!(of_type, SCStreamOutputType::Audio) {
            return;
        }
        let rate = sample.format_description().and_then(|f| f.audio_sample_rate()).unwrap_or(48_000.0) as u32;
        let Ok(list) = sample.audio_buffer_list() else { return };
        // Float32 samples; one buffer per channel (planar) or channels interleaved in one.
        let bufs: Vec<(usize, Vec<f32>)> = (&list)
            .into_iter()
            .map(|b| {
                let s = b.data().chunks_exact(4).map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect();
                (b.number_channels as usize, s)
            })
            .collect();
        let mono = mix_to_mono(&bufs.iter().map(|(n, s)| (*n, s.as_slice())).collect::<Vec<_>>());
        let mut out = Vec::new();
        if let Ok(mut r) = self.resampler.lock() {
            if r.as_ref().map(|(at, _)| *at) != Some(rate) {
                *r = Some((rate, Resampler::new(rate, WHISPER_SAMPLE_RATE)));
            }
            if let Some((_, rs)) = r.as_mut() {
                rs.push(&mono, &mut out);
            }
        }
        if let Ok(mut raw) = self.raw.lock() {
            raw.extend(out);
        }
    }
}

struct Stopped(Arc<Mutex<Option<String>>>);

impl SCStreamDelegateTrait for Stopped {
    fn did_stop_with_error(&self, error: SCError) {
        if let Ok(mut slot) = self.0.lock() {
            slot.get_or_insert_with(|| format!("Computer sound stopped: {error}"));
        }
    }
}

/// Stops the stream when dropped, so no recording indicator is left behind.
struct Running(SCStream);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.stop_capture();
    }
}

pub fn open(raw: Arc<Mutex<Vec<f32>>>, failed: Arc<Mutex<Option<String>>>) -> Result<(Box<dyn Any>, u32), CaptureError> {
    if !macos_13_or_newer() {
        return Err(CaptureError::Unsupported("needs macOS 13 or newer".into()));
    }
    // The first time this shows the system prompt; the answer only counts
    // after the app restarts, so a first "no" is reported as denied.
    if !unsafe { CGPreflightScreenCaptureAccess() } && !unsafe { CGRequestScreenCaptureAccess() } {
        return Err(CaptureError::PermissionDenied);
    }
    let failed_msg = |e: SCError| CaptureError::Failed(format!("Could not record the computer's sound: {e}"));
    let content = SCShareableContent::get().map_err(failed_msg)?;
    let display = content.displays().into_iter().next().ok_or_else(|| CaptureError::Failed("No display to capture from.".into()))?;
    let filter = SCContentFilter::create().with_display(&display).with_excluding_windows(&[]).build().map_err(failed_msg)?;
    // Audio only: ScreenCaptureKit still needs a display, so ask for the
    // smallest, slowest video and never read it.
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_minimum_frame_interval(&CMTime::new(1, 1))
        .with_captures_audio(true)
        .with_sample_rate(WHISPER_SAMPLE_RATE as i32)
        .with_channel_count(1)
        .with_excludes_current_process_audio(true);
    let mut stream = SCStream::new_with_delegate(&filter, &config, Stopped(failed)).map_err(failed_msg)?;
    stream
        .add_output_handler(Audio { raw, resampler: Mutex::new(None) }, SCStreamOutputType::Audio)
        .map_err(failed_msg)?;
    stream.start_capture().map_err(failed_msg)?;
    Ok((Box::new(Running(stream)), WHISPER_SAMPLE_RATE))
}
