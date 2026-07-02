use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Target sample rate Whisper expects.
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

enum Control {
    Stop,
}

/// A microphone recorder. Capture runs on its own thread because a cpal
/// `Stream` is not `Send` on all platforms, so we never move it across threads.
pub struct Recorder {
    ctrl_tx: Sender<Control>,
    samples_rx: Receiver<Vec<f32>>,
}

impl Recorder {
    /// Start capturing from the default (or named) input device.
    pub fn start(device_name: Option<String>) -> Result<Self, String> {
        let (ctrl_tx, ctrl_rx) = mpsc::channel::<Control>();
        let (samples_tx, samples_rx) = mpsc::channel::<Vec<f32>>();

        thread::spawn(move || {
            if let Err(e) = capture_loop(device_name, ctrl_rx, &samples_tx) {
                eprintln!("[audio] capture error: {e}");
                let _ = samples_tx.send(Vec::new());
            }
        });

        Ok(Self {
            ctrl_tx,
            samples_rx,
        })
    }

    /// Stop recording and return mono samples resampled to 16 kHz.
    pub fn stop(self) -> Vec<f32> {
        let _ = self.ctrl_tx.send(Control::Stop);
        self.samples_rx.recv().unwrap_or_default()
    }
}

fn capture_loop(
    device_name: Option<String>,
    ctrl_rx: Receiver<Control>,
    samples_tx: &Sender<Vec<f32>>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| format!("input device '{name}' not found"))?,
        None => host
            .default_input_device()
            .ok_or("no default input device")?,
    };

    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let in_sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let buf_for_cb = buffer.clone();

    let err_fn = |e| eprintln!("[audio] stream error: {e}");

    // Capture as f32 regardless of native sample format.
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| push_mono(&buf_for_cb, data, channels),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let floats: Vec<f32> = data.iter().map(|s| *s as f32 / 32768.0).collect();
                push_mono(&buf_for_cb, &floats, channels);
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| {
                let floats: Vec<f32> = data
                    .iter()
                    .map(|s| (*s as f32 - 32768.0) / 32768.0)
                    .collect();
                push_mono(&buf_for_cb, &floats, channels);
            },
            err_fn,
            None,
        ),
        fmt => return Err(format!("unsupported sample format: {fmt:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // Block until a stop signal arrives.
    let _ = ctrl_rx.recv();
    drop(stream);

    let captured = buffer.lock().map_err(|e| e.to_string())?.clone();
    let resampled = resample_linear(&captured, in_sample_rate, WHISPER_SAMPLE_RATE);
    samples_tx.send(resampled).map_err(|e| e.to_string())?;
    Ok(())
}

/// Downmix interleaved frames to mono and append to the shared buffer.
fn push_mono(buffer: &Arc<Mutex<Vec<f32>>>, data: &[f32], channels: usize) {
    if let Ok(mut buf) = buffer.lock() {
        if channels <= 1 {
            buf.extend_from_slice(data);
        } else {
            for frame in data.chunks(channels) {
                let avg = frame.iter().copied().sum::<f32>() / frame.len() as f32;
                buf.push(avg);
            }
        }
    }
}

/// Simple linear resampler — good enough for speech at 16 kHz.
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = (input.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Write mono 16 kHz f32 samples to a 16-bit PCM WAV file.
pub fn write_wav(path: &std::path::Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: WHISPER_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        writer
            .write_sample((clamped * i16::MAX as f32) as i16)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

/// List available input device names.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}
