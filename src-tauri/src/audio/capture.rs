use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Target sample rate Whisper expects.
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

pub enum Control {
    Stop,
}

/// A microphone recorder. Capture runs on its own thread because a cpal
/// `Stream` is not `Send` on all platforms, so we never move it across threads.
pub struct Recorder {
    ctrl_tx: Sender<Control>,
    samples_rx: Receiver<Vec<f32>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    in_sample_rate: Arc<AtomicU32>,
    // Live 0–100 loudness of the most recent audio frame (f32 bits), updated on
    // the capture thread so the pill/waveform can react to the real voice.
    level: Arc<AtomicU32>,
}

impl Recorder {
    /// Start capturing from the default (or named) input device.
    pub fn start(device_name: Option<String>) -> Result<Self, String> {
        let (ctrl_tx, ctrl_rx) = mpsc::channel::<Control>();
        let (samples_tx, samples_rx) = mpsc::channel::<Vec<f32>>();
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let in_sample_rate = Arc::new(AtomicU32::new(0));
        let level = Arc::new(AtomicU32::new(0));

        let buf_clone = buffer.clone();
        let rate_clone = in_sample_rate.clone();
        let level_clone = level.clone();

        thread::spawn(move || {
            if let Err(e) =
                capture_loop(device_name, ctrl_rx, &samples_tx, buf_clone, rate_clone, level_clone)
            {
                eprintln!("[audio] capture error: {e}");
                let _ = samples_tx.send(Vec::new());
            }
        });

        Ok(Self {
            ctrl_tx,
            samples_rx,
            buffer,
            in_sample_rate,
            level,
        })
    }

    /// A handle to the live 0–100 loudness value, for a throttled emitter to
    /// stream to the UI without touching the audio buffer or the RT thread.
    pub fn level_handle(&self) -> Arc<AtomicU32> {
        self.level.clone()
    }

    /// Resample and return native-rate mono samples in `[from_raw..]` as 16 kHz,
    /// plus the new raw watermark. Called repeatedly while recording.
    pub fn snapshot_16k(&self, from_raw: usize) -> (Vec<f32>, usize) {
        let rate = self.in_sample_rate.load(Ordering::Relaxed);
        if rate == 0 {
            return (Vec::new(), from_raw);
        }
        // Copy the new samples out under the lock, then release it BEFORE
        // resampling: the cpal capture callback locks this same buffer on every
        // audio frame, so holding it across the resample would stall the
        // real-time thread and risk dropped samples.
        let (native, len) = {
            let buf = match self.buffer.lock() {
                Ok(b) => b,
                Err(_) => return (Vec::new(), from_raw),
            };
            let len = buf.len();
            if from_raw >= len {
                return (Vec::new(), len);
            }
            (buf[from_raw..len].to_vec(), len)
        };
        let resampled = resample_linear(&native, rate, WHISPER_SAMPLE_RATE);
        (resampled, len)
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
    buffer: Arc<Mutex<Vec<f32>>>,
    in_sample_rate: Arc<AtomicU32>,
    level: Arc<AtomicU32>,
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
    let in_sample_rate_val = config.sample_rate().0;
    in_sample_rate.store(in_sample_rate_val, Ordering::Relaxed);
    let channels = config.channels() as usize;

    let buf_for_cb = buffer.clone();
    let level_for_cb = level.clone();

    let err_fn = |e| eprintln!("[audio] stream error: {e}");

    // Capture as f32 regardless of native sample format.
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| push_mono(&buf_for_cb, data, channels, &level_for_cb),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let floats: Vec<f32> = data.iter().map(|s| *s as f32 / 32768.0).collect();
                push_mono(&buf_for_cb, &floats, channels, &level_for_cb);
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
                push_mono(&buf_for_cb, &floats, channels, &level_for_cb);
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
    let resampled = resample_linear(&captured, in_sample_rate_val, WHISPER_SAMPLE_RATE);
    samples_tx.send(resampled).map_err(|e| e.to_string())?;
    Ok(())
}

/// Downmix interleaved frames to mono, append to the shared buffer, and update
/// the live loudness value for the reactive waveform. The RMS is over the frame
/// just added (a few hundred samples) — cheap enough for the RT callback.
fn push_mono(buffer: &Arc<Mutex<Vec<f32>>>, data: &[f32], channels: usize, level: &Arc<AtomicU32>) {
    if let Ok(mut buf) = buffer.lock() {
        let start = buf.len();
        if channels <= 1 {
            buf.extend_from_slice(data);
        } else {
            for frame in data.chunks(channels) {
                let avg = frame.iter().copied().sum::<f32>() / frame.len() as f32;
                buf.push(avg);
            }
        }
        level.store(level_of(&buf[start..]).to_bits(), Ordering::Relaxed);
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

/// Open the input device by name, or the system default when `None`.
fn open_device(device_name: Option<String>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    match device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| format!("input device '{name}' not found")),
        None => host
            .default_input_device()
            .ok_or_else(|| "no default input device".to_string()),
    }
}

/// Loudness of a buffer as a 0–100 meter value. Speech sits around RMS
/// 0.05–0.2, so it's scaled up to fill a visible bar.
fn level_of(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = data.iter().map(|s| s * s).sum();
    let rms = (sum_sq / data.len() as f32).sqrt();
    (rms * 300.0).min(100.0)
}

/// Stream live input loudness to `on_level` until `Control::Stop` is sent on
/// the returned channel. Used by the onboarding mic check — no resampling and
/// no buffering, since only the level matters.
pub fn start_level_probe(
    device_name: Option<String>,
    on_level: impl Fn(f32) + Send + Sync + 'static,
) -> Sender<Control> {
    let (ctrl_tx, ctrl_rx) = mpsc::channel::<Control>();
    thread::spawn(move || {
        if let Err(e) = probe_loop(device_name, ctrl_rx, on_level) {
            eprintln!("[audio] level probe error: {e}");
        }
    });
    ctrl_tx
}

fn probe_loop(
    device_name: Option<String>,
    ctrl_rx: Receiver<Control>,
    on_level: impl Fn(f32) + Send + Sync + 'static,
) -> Result<(), String> {
    let device = open_device(device_name)?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let cb = Arc::new(on_level);
    let err_fn = |e| eprintln!("[audio] probe stream error: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let cb = cb.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| cb(level_of(data)),
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let cb = cb.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    let floats: Vec<f32> = data.iter().map(|s| *s as f32 / 32768.0).collect();
                    cb(level_of(&floats));
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let cb = cb.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    let floats: Vec<f32> = data
                        .iter()
                        .map(|s| (*s as f32 - 32768.0) / 32768.0)
                        .collect();
                    cb(level_of(&floats));
                },
                err_fn,
                None,
            )
        }
        fmt => return Err(format!("unsupported sample format: {fmt:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    let _ = ctrl_rx.recv();
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
