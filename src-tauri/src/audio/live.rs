// Live audio for the meeting recorder: the mic, or the computer's own sound
// (WASAPI loopback), as 16 kHz mono chunks every ~100 ms.
//
// Loopback only delivers while something plays, so a quiet stretch would
// vanish and the Others track would slide out of step with the mic. The wall
// clock fills those gaps with silence.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio::capture::WHISPER_SAMPLE_RATE;
use crate::audio::decode::Resampler;

const SR: u64 = WHISPER_SAMPLE_RATE as u64;
const TICK: Duration = Duration::from_millis(100);
/// Silence is added only once a track is this far (0.2 s) behind the clock:
/// devices deliver in bursts a little late, and those must pass untouched.
const LAG: u64 = 3_200;
/// Most silence added per tick, so waking from sleep catches up in steps
/// instead of allocating hours of silence at once.
const MAX_PAD: u64 = 10 * SR;

pub enum Source {
    /// A microphone by name, or the default one.
    Mic(Option<String>),
    /// Whatever the default speakers/headphones are playing.
    Speakers,
}

/// A running capture. Audio goes to the callback given to `start`.
pub struct Live {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Result<(), String>>,
}

impl Live {
    /// Start capturing; `on_audio` gets 16 kHz mono chunks on a capture thread,
    /// and `on_failed` the reason if the device dies mid-way (unplugged).
    /// Fails at once if the device can't be opened.
    pub fn start(
        source: Source,
        on_audio: impl FnMut(Vec<f32>) + Send + 'static,
        on_failed: impl FnOnce(String) + Send + 'static,
    ) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel();
        let stop2 = stop.clone();
        // cpal streams can't move between threads on Windows: open it on the one that runs it.
        let thread = std::thread::spawn(move || {
            let r = run(source, on_audio, &stop2, ready_tx);
            if let Err(e) = &r {
                if !stop2.load(Ordering::Relaxed) {
                    on_failed(e.clone());
                }
            }
            r
        });
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { stop, thread }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(thread.join().map_or_else(|_| "audio thread crashed".into(), |r| r.err().unwrap_or_default())),
        }
    }

    /// Stop and wait for the last chunk to be handed over. Err if the device
    /// failed mid-way (unplugged): the audio before that was still delivered.
    pub fn stop(self) -> Result<(), String> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.join().map_err(|_| "audio thread crashed".to_string())?
    }
}

fn run(
    source: Source,
    mut on_audio: impl FnMut(Vec<f32>),
    stop: &AtomicBool,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let opened = open(&source);
    let (stream, rate, raw, failed) = match opened {
        Ok(x) => x,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
    };
    if let Err(e) = stream.play() {
        let _ = ready.send(Err(e.to_string()));
        return Err(e.to_string());
    }
    let _ = ready.send(Ok(()));
    let started = Instant::now();
    let mut resampler = Resampler::new(rate, WHISPER_SAMPLE_RATE);
    let mut pad = ClockPad { sent: 0 };
    loop {
        let stopping = stop.load(Ordering::Relaxed);
        if !stopping {
            std::thread::sleep(TICK);
        }
        let input = std::mem::take(&mut *raw.lock().map_err(|e| e.to_string())?);
        let mut out = Vec::new();
        resampler.push(&input, &mut out);
        let due = started.elapsed().as_millis() as u64 * SR / 1000;
        let out = pad.fill(out, due);
        if !out.is_empty() {
            on_audio(out);
        }
        if let Some(e) = failed.lock().map_err(|e| e.to_string())?.take() {
            return Err(e);
        }
        if stopping {
            return Ok(());
        }
    }
}

type Opened = (cpal::Stream, u32, Arc<Mutex<Vec<f32>>>, Arc<Mutex<Option<String>>>);

/// Open the device; its callback downmixes to mono into `raw`.
fn open(source: &Source) -> Result<Opened, String> {
    let host = cpal::default_host();
    let (device, config) = match source {
        Source::Mic(name) => {
            let d = match name {
                Some(n) => host
                    .input_devices()
                    .map_err(|e| e.to_string())?
                    .find(|d| d.name().is_ok_and(|x| &x == n))
                    .ok_or_else(|| format!("Microphone '{n}' not found."))?,
                None => host.default_input_device().ok_or("No microphone found.")?,
            };
            let c = d.default_input_config().map_err(|e| e.to_string())?;
            (d, c)
        }
        // An input stream on an output device is WASAPI loopback in cpal.
        Source::Speakers => {
            let d = host.default_output_device().ok_or("No speakers or headphones found.")?;
            let c = d.default_output_config().map_err(|e| e.to_string())?;
            (d, c)
        }
    };
    let rate = config.sample_rate().0;
    let channels = config.channels().max(1) as usize;
    let raw = Arc::new(Mutex::new(Vec::<f32>::new()));
    let failed = Arc::new(Mutex::new(None));
    let (r, f) = (raw.clone(), failed.clone());
    let err = move |e: cpal::StreamError| {
        if let Ok(mut slot) = f.lock() {
            slot.get_or_insert_with(|| format!("Audio device stopped: {e}"));
        }
    };
    let push = move |data: &[f32]| {
        if let Ok(mut buf) = r.lock() {
            buf.extend(data.chunks(channels).map(|fr| fr.iter().sum::<f32>() / channels as f32));
        }
    };
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(&config.into(), move |d: &[f32], _| push(d), err, None),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |d: &[i16], _| push(&d.iter().map(|s| *s as f32 / 32768.0).collect::<Vec<_>>()),
            err,
            None,
        ),
        fmt => return Err(format!("Unsupported audio format: {fmt:?}")),
    }
    .map_err(|e| e.to_string())?;
    Ok((stream, rate, raw, failed))
}

/// Names of the mic (`mic`, or the default) and speakers a recording would use,
/// so the page can warn about a Bluetooth headset before it starts.
pub fn device_names(mic: Option<&str>) -> (Option<String>, Option<String>) {
    let host = cpal::default_host();
    let mic = mic.map(str::to_string).or_else(|| host.default_input_device().and_then(|d| d.name().ok()));
    (mic, host.default_output_device().and_then(|d| d.name().ok()))
}

/// Keeps a track in step with the wall clock.
struct ClockPad {
    sent: u64,
}

impl ClockPad {
    /// `got`: new 16 kHz samples; `due`: how many the clock says exist by now.
    /// A track more than LAG behind gets silence before `got`: that's when
    /// nothing came in (loopback stalls while nothing plays, or the PC slept).
    fn fill(&mut self, got: Vec<f32>, due: u64) -> Vec<f32> {
        let have = self.sent + got.len() as u64;
        let out = if due > have + LAG {
            let mut out = vec![0.0; (due - LAG - have).min(MAX_PAD) as usize];
            out.extend(got);
            out
        } else {
            got
        };
        self.sent += out.len() as u64;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stalled_track_is_padded_with_silence_up_to_the_clock() {
        let mut pad = ClockPad { sent: 0 };
        let out = pad.fill(Vec::new(), 16_000); // 1 s passed, nothing played
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(out.len() as u64, 16_000 - LAG, "pads to just behind the clock");
    }

    #[test]
    fn a_flowing_track_is_passed_through_untouched() {
        let mut pad = ClockPad { sent: 0 };
        // Devices deliver in bursts a little behind the clock: never pad those.
        let out = pad.fill(vec![0.5; 1_400], 1_600);
        assert_eq!(out, vec![0.5; 1_400]);
        let out = pad.fill(vec![0.5; 1_700], 3_200);
        assert_eq!(out, vec![0.5; 1_700]);
    }

    #[test]
    fn audio_after_a_stall_follows_the_padding() {
        let mut pad = ClockPad { sent: 0 };
        let silence = pad.fill(Vec::new(), 32_000).len();
        let out = pad.fill(vec![0.5; 1_600], 33_600);
        assert_eq!(out, vec![0.5; 1_600], "real audio must not be padded again");
        assert_eq!(pad.sent, (silence + 1_600) as u64);
    }

    #[test]
    fn audio_arriving_after_a_stall_gets_the_missing_silence_before_it() {
        let mut pad = ClockPad { sent: 0 };
        // 2 s of nothing, then sound resumes within the same tick.
        let out = pad.fill(vec![0.5; 1_600], 32_000);
        assert_eq!(out.len() as u64, 32_000 - LAG);
        assert!(out[..out.len() - 1_600].iter().all(|&s| s == 0.0), "silence must come first");
        assert!(out[out.len() - 1_600..].iter().all(|&s| s == 0.5), "the real audio must come last, untouched");
    }

    #[test]
    fn waking_from_sleep_catches_up_in_steps() {
        let mut pad = ClockPad { sent: 0 };
        let hour = 3_600 * 16_000;
        assert_eq!(pad.fill(Vec::new(), hour).len() as u64, MAX_PAD);
        assert_eq!(pad.fill(Vec::new(), hour).len() as u64, MAX_PAD);
    }

    /// Real devices: plays a 1.5 s tone, then nothing, while capturing the
    /// speakers and the mic for 5 s. `cargo test --lib real_loopback -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_loopback_keeps_time_through_silence() {
        let got = Arc::new(Mutex::new(Vec::<f32>::new()));
        let mic_got = Arc::new(Mutex::new(0usize));
        let g = got.clone();
        let m = mic_got.clone();
        let t0 = Instant::now();
        let speakers = Live::start(Source::Speakers, move |c| g.lock().unwrap().extend(c), |e| panic!("{e}")).unwrap();
        let mic = Live::start(Source::Mic(None), move |c| *m.lock().unwrap() += c.len(), |e| panic!("{e}")).unwrap();
        {
            let (_out, handle) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&handle).unwrap();
            sink.append(rodio::source::Source::amplify(
                rodio::source::Source::take_duration(rodio::source::SineWave::new(440.0), Duration::from_millis(1500)),
                0.3,
            ));
            sink.sleep_until_end();
        }
        std::thread::sleep(Duration::from_millis(5000).saturating_sub(t0.elapsed()));
        speakers.stop().unwrap();
        mic.stop().unwrap();
        let secs = t0.elapsed().as_secs_f32();
        let a = got.lock().unwrap();
        let loud = a.iter().filter(|s| s.abs() > 0.05).count() as f32 / 16_000.0;
        let mic_s = *mic_got.lock().unwrap() as f32 / 16_000.0;
        println!("wall {secs:.2} s, speakers {:.2} s (tone {loud:.2} s), mic {mic_s:.2} s", a.len() as f32 / 16_000.0);
        assert!((a.len() as f32 / 16_000.0 - secs).abs() < 0.4, "speakers track out of step with the clock");
        assert!(loud > 0.5, "tone not captured");
        assert!((mic_s - secs).abs() < 0.4, "mic track out of step");
    }
}
