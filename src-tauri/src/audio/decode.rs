// Read any audio/video file the user drops in as 16 kHz mono f32, a piece at a
// time, so a 2-hour file never sits in memory whole (that alone is 460 MB).
//
// Pure Rust (symphonia): mp3, m4a/aac, mp4/mov audio, wav, flac, ogg/vorbis,
// mkv. Opus has no decoder there, so .opus voice notes and most .webm files get
// a plain "not supported" message instead of an error code.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL, CODEC_TYPE_OPUS};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::audio::capture::WHISPER_SAMPLE_RATE;

/// Hand out roughly this much 16 kHz audio per `next_chunk` (1 s).
const CHUNK: usize = WHISPER_SAMPLE_RATE as usize;

pub const UNSUPPORTED: &str =
    "This file type isn't supported yet. Try mp3, m4a, mp4, wav or flac.";

pub struct FileDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    resampler: Resampler,
    duration_ms: Option<u64>,
    out: Vec<f32>,
    done: bool,
}

impl FileDecoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("Couldn't open the file: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|_| UNSUPPORTED.to_string())?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.sample_rate.is_some())
            .ok_or("This file has no audio track.")?;
        if track.codec_params.codec == CODEC_TYPE_OPUS {
            return Err(UNSUPPORTED.into());
        }
        let params = track.codec_params.clone();
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|_| UNSUPPORTED.to_string())?;
        let rate = params.sample_rate.unwrap_or(WHISPER_SAMPLE_RATE);
        let duration_ms = params.n_frames.map(|n| n * 1000 / rate as u64);
        Ok(Self {
            track_id: track.id,
            format,
            decoder,
            resampler: Resampler::new(rate, WHISPER_SAMPLE_RATE),
            duration_ms,
            out: Vec::new(),
            done: false,
        })
    }

    /// Length of the audio, when the file says (most do; some mp3s don't).
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// The next ~1 s of 16 kHz mono audio, or None at the end of the file.
    pub fn next_chunk(&mut self) -> Result<Option<Vec<f32>>, String> {
        while self.out.len() < CHUNK && !self.done {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.done = true;
                    break;
                }
                Err(SymError::ResetRequired) => {
                    self.done = true;
                    break;
                }
                Err(e) => return Err(format!("Couldn't read the file: {e}")),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // One damaged frame: skip it, keep the rest of the file.
                Err(SymError::DecodeError(_)) => continue,
                Err(e) => return Err(format!("Couldn't decode the audio: {e}")),
            };
            let spec = *decoded.spec();
            let channels = spec.channels.count().max(1);
            let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            buf.copy_interleaved_ref(decoded);
            let mono: Vec<f32> = buf
                .samples()
                .chunks(channels)
                .map(|f| f.iter().sum::<f32>() / channels as f32)
                .collect();
            self.resampler.push(&mono, &mut self.out);
        }
        if self.out.is_empty() {
            return Ok(None);
        }
        let n = self.out.len().min(CHUNK);
        Ok(Some(self.out.drain(..n).collect()))
    }
}

/// Streaming resampler: a short moving average (cheap anti-alias for 44.1/48 kHz
/// down to 16 kHz) then linear interpolation, carrying its position across
/// pushes so chunk edges leave no clicks or drift.
// ponytail: box filter + linear, same family as the mic path (capture.rs);
// swap for a windowed-sinc (rubato) if accuracy on 44.1 kHz files measures worse.
struct Resampler {
    step: f64, // input samples per output sample
    pos: f64,  // next output position, in `buf` coordinates
    buf: Vec<f32>,
    width: usize,
    hist: Vec<f32>, // last width-1 raw samples, for the moving average
}

impl Resampler {
    fn new(from: u32, to: u32) -> Self {
        let step = from as f64 / to as f64;
        Self { step, pos: 0.0, buf: Vec::new(), width: step.round().max(1.0) as usize, hist: Vec::new() }
    }

    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.width > 1 {
            let mut all = std::mem::take(&mut self.hist);
            all.extend_from_slice(input);
            let w = self.width;
            if all.len() >= w {
                let mut sum: f32 = all[..w - 1].iter().sum();
                for i in w - 1..all.len() {
                    sum += all[i];
                    self.buf.push(sum / w as f32);
                    sum -= all[i + 1 - w];
                }
                self.hist = all[all.len() - (w - 1)..].to_vec();
            } else {
                self.hist = all;
            }
        } else {
            self.buf.extend_from_slice(input);
        }
        while self.pos + 1.0 < self.buf.len() as f64 {
            let i = self.pos as usize;
            let frac = (self.pos - i as f64) as f32;
            out.push(self.buf[i] + (self.buf[i + 1] - self.buf[i]) * frac);
            self.pos += self.step;
        }
        let used = (self.pos as usize).min(self.buf.len());
        self.buf.drain(..used);
        self.pos -= used as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(path: &Path) -> Result<Vec<f32>, String> {
        let mut d = FileDecoder::open(path)?;
        let mut all = Vec::new();
        while let Some(c) = d.next_chunk()? {
            all.extend(c);
        }
        Ok(all)
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("sv-decode-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn stereo_44k_wav_becomes_16k_mono_of_the_right_length() {
        let path = tmp("tone44k.wav");
        let spec = hound::WavSpec { channels: 2, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..44_100 * 3 {
            let s = ((i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin() * 12_000.0) as i16;
            w.write_sample(s).unwrap();
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        assert_eq!(FileDecoder::open(&path).unwrap().duration_ms(), Some(3000));
        let out = decode_all(&path).unwrap();
        assert!((out.len() as i64 - 48_000).abs() < 40, "got {} samples", out.len());
        // A 440 Hz tone survives: still loud after resampling.
        let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
        assert!(rms > 0.2, "rms {rms}");
    }

    #[test]
    fn resampler_is_seamless_across_pushes() {
        // Same input pushed whole vs in odd-sized pieces must give the same output.
        let input: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut a = Resampler::new(48_000, 16_000);
        let mut whole = Vec::new();
        a.push(&input, &mut whole);
        let mut b = Resampler::new(48_000, 16_000);
        let mut pieces = Vec::new();
        for c in input.chunks(1_153) {
            b.push(c, &mut pieces);
        }
        assert_eq!(whole.len(), pieces.len());
        assert!(whole.iter().zip(&pieces).all(|(x, y)| (x - y).abs() < 1e-5));
    }

    #[test]
    fn junk_file_says_unsupported() {
        let path = tmp("junk.bin");
        std::fs::write(&path, b"definitely not audio").unwrap();
        assert_eq!(FileDecoder::open(&path).err().as_deref(), Some(UNSUPPORTED));
    }

    /// Real files made with ffmpeg from a LibriVox recording.
    /// `$env:SV_AUDIO="<folder>"; cargo test --lib real_formats -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_formats() {
        let dir = std::path::PathBuf::from(std::env::var("SV_AUDIO").expect("SV_AUDIO"));
        for name in ["clip.m4a", "clip.mp4", "clip.flac", "clip44k.wav", "ref16k.wav", "war_10min.mp3"] {
            let t = std::time::Instant::now();
            let d = FileDecoder::open(&dir.join(name)).unwrap();
            let dur = d.duration_ms();
            let out = decode_all(&dir.join(name)).unwrap();
            let secs = out.len() as f64 / 16_000.0;
            println!("{name}: {secs:.1}s audio, header says {dur:?} ms, decoded in {:?}", t.elapsed());
            assert!(secs > 59.0, "{name} too short");
        }
        assert_eq!(FileDecoder::open(&dir.join("clip.opus")).err().as_deref(), Some(UNSUPPORTED));
    }
}
