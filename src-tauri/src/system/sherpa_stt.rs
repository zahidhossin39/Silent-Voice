// FFI bindings to sherpa-onnx-c-api.dll for OFFLINE speech-to-text.
//
// This reuses the SAME DLLs already bundled for TTS (system/sherpa.rs) — no
// new binaries. It drives the OfflineRecognizer C-API, currently for the
// Moonshine model family (fast, natively-punctuated English).
//
// Struct layouts mirror sherpa-onnx v1.13.4 c-api.h EXACTLY. The bundled DLL
// was verified to be v1.13.4 (it exports the Moonshine/Qwen3/Cohere symbols
// and embeds "1.13.4"). SherpaOnnxOfflineModelConfig is a large nested struct
// with `moonshine` in the MIDDLE, so every field and the total size must match
// or the DLL reads past our allocation. Do not reorder or trim fields.
//
// Unused string sub-configs are left NULL (zeroed): the recognizer detects
// which model family is configured and ignores the rest — the same pattern the
// TTS side relies on in system/sherpa.rs.

#![allow(non_snake_case)]

use libloading::Symbol;
use std::ffi::{c_char, c_float, c_int, CString};
use std::path::Path;

#[repr(C)]
struct FeatureConfig {
    sample_rate: c_int,
    feature_dim: c_int,
}

#[repr(C)]
struct TransducerModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    joiner: *const c_char,
}
#[repr(C)]
struct ParaformerModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct NemoCtcModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct WhisperModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    task: *const c_char,
    tail_paddings: c_int,
    enable_token_timestamps: c_int,
    enable_segment_timestamps: c_int,
}
#[repr(C)]
struct TdnnModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct SenseVoiceModelConfig {
    model: *const c_char,
    language: *const c_char,
    use_itn: c_int,
}
#[repr(C)]
struct MoonshineModelConfig {
    preprocessor: *const c_char,
    encoder: *const c_char,
    uncached_decoder: *const c_char,
    cached_decoder: *const c_char,
    merged_decoder: *const c_char,
}
#[repr(C)]
struct FireRedAsrModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
}
#[repr(C)]
struct DolphinModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct ZipformerCtcModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct CanaryModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    src_lang: *const c_char,
    tgt_lang: *const c_char,
    use_pnc: c_int,
}
#[repr(C)]
struct WenetCtcModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct OmnilingualCtcModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct MedAsrCtcModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct FunAsrNanoModelConfig {
    encoder_adaptor: *const c_char,
    llm: *const c_char,
    embedding: *const c_char,
    tokenizer: *const c_char,
    system_prompt: *const c_char,
    user_prompt: *const c_char,
    max_new_tokens: c_int,
    temperature: c_float,
    top_p: c_float,
    seed: c_int,
    language: *const c_char,
    itn: c_int,
    hotwords: *const c_char,
}
#[repr(C)]
struct FireRedAsrCtcModelConfig {
    model: *const c_char,
}
#[repr(C)]
struct Qwen3AsrModelConfig {
    conv_frontend: *const c_char,
    encoder: *const c_char,
    decoder: *const c_char,
    tokenizer: *const c_char,
    max_total_len: c_int,
    max_new_tokens: c_int,
    temperature: c_float,
    top_p: c_float,
    seed: c_int,
    hotwords: *const c_char,
}
#[repr(C)]
struct CohereTranscribeModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    use_punct: c_int,
    use_itn: c_int,
}

#[repr(C)]
struct OfflineModelConfig {
    transducer: TransducerModelConfig,
    paraformer: ParaformerModelConfig,
    nemo_ctc: NemoCtcModelConfig,
    whisper: WhisperModelConfig,
    tdnn: TdnnModelConfig,
    tokens: *const c_char,
    num_threads: c_int,
    debug: c_int,
    provider: *const c_char,
    model_type: *const c_char,
    modeling_unit: *const c_char,
    bpe_vocab: *const c_char,
    telespeech_ctc: *const c_char,
    sense_voice: SenseVoiceModelConfig,
    moonshine: MoonshineModelConfig,
    fire_red_asr: FireRedAsrModelConfig,
    dolphin: DolphinModelConfig,
    zipformer_ctc: ZipformerCtcModelConfig,
    canary: CanaryModelConfig,
    wenet_ctc: WenetCtcModelConfig,
    omnilingual: OmnilingualCtcModelConfig,
    medasr: MedAsrCtcModelConfig,
    funasr_nano: FunAsrNanoModelConfig,
    fire_red_asr_ctc: FireRedAsrCtcModelConfig,
    qwen3_asr: Qwen3AsrModelConfig,
    cohere_transcribe: CohereTranscribeModelConfig,
}

#[repr(C)]
struct OfflineLmConfig {
    model: *const c_char,
    scale: c_float,
}
#[repr(C)]
struct HomophoneReplacerConfig {
    dict_dir: *const c_char,
    lexicon: *const c_char,
    rule_fsts: *const c_char,
}

#[repr(C)]
struct OfflineRecognizerConfig {
    feat_config: FeatureConfig,
    model_config: OfflineModelConfig,
    lm_config: OfflineLmConfig,
    decoding_method: *const c_char,
    max_active_paths: c_int,
    hotwords_file: *const c_char,
    hotwords_score: c_float,
    rule_fsts: *const c_char,
    rule_fars: *const c_char,
    blank_penalty: c_float,
    hr: HomophoneReplacerConfig,
}

#[repr(C)]
struct OfflineRecognizerResult {
    text: *const c_char,
    timestamps: *const c_float,
    count: c_int,
    tokens: *const c_char,
    tokens_arr: *const *const c_char,
    json: *const c_char,
    lang: *const c_char,
    emotion: *const c_char,
    event: *const c_char,
    durations: *const c_float,
    ys_log_probs: *const c_float,
    segment_timestamps: *const c_float,
    segment_durations: *const c_float,
    segment_texts: *const c_char,
    segment_texts_arr: *const *const c_char,
    segment_count: c_int,
}

type CreateRecognizerFn =
    unsafe extern "C" fn(*const OfflineRecognizerConfig) -> *const std::ffi::c_void;
type DestroyRecognizerFn = unsafe extern "C" fn(*const std::ffi::c_void);
type CreateStreamFn =
    unsafe extern "C" fn(*const std::ffi::c_void) -> *const std::ffi::c_void;
type DestroyStreamFn = unsafe extern "C" fn(*const std::ffi::c_void);
type AcceptWaveformFn =
    unsafe extern "C" fn(*const std::ffi::c_void, c_int, *const c_float, c_int);
type DecodeFn = unsafe extern "C" fn(*const std::ffi::c_void, *const std::ffi::c_void);
type GetResultFn =
    unsafe extern "C" fn(*const std::ffi::c_void) -> *const OfflineRecognizerResult;
type DestroyResultFn = unsafe extern "C" fn(*const OfflineRecognizerResult);

/// A loaded sherpa-onnx offline recognizer — Moonshine or SenseVoice. Creating
/// it loads the ONNX graphs (~hundreds of ms), so callers should keep it alive
/// and reuse it across clips.
pub struct SherpaSttEngine {
    recognizer: *const std::ffi::c_void,
    _keepalive: Vec<CString>,
}

// The recognizer handle is an opaque C pointer; sherpa's offline recognizer is
// safe to call from one thread at a time. We only ever use it behind the app's
// single transcription path.
unsafe impl Send for SherpaSttEngine {}
unsafe impl Sync for SherpaSttEngine {}

impl SherpaSttEngine {
    /// Load a Moonshine model from a directory containing preprocess.onnx,
    /// encode.int8.onnx, uncached_decode.int8.onnx, cached_decode.int8.onnx,
    /// and tokens.txt.
    pub fn load_moonshine(dir: &Path, num_threads: i32) -> Result<Self, String> {
        let lib = super::sherpa::lib()?;
        let c = |s: String| CString::new(s).map_err(|e| e.to_string());

        // Keep every CString alive until after create() copies the strings.
        let mut keep: Vec<CString> = Vec::new();
        let mut path = |name: &str| -> Result<*const c_char, String> {
            let s = c(dir.join(name).to_string_lossy().into_owned())?;
            let ptr = s.as_ptr();
            keep.push(s);
            Ok(ptr)
        };

        let preprocessor = path("preprocess.onnx")?;
        let encoder = path("encode.int8.onnx")?;
        let uncached = path("uncached_decode.int8.onnx")?;
        let cached = path("cached_decode.int8.onnx")?;
        let tokens = path("tokens.txt")?;

        let provider = c("cpu".into())?;
        let provider_ptr = provider.as_ptr();
        keep.push(provider);
        let method = c("greedy_search".into())?;
        let method_ptr = method.as_ptr();
        keep.push(method);

        let mut cfg: OfflineRecognizerConfig = unsafe { std::mem::zeroed() };
        cfg.feat_config.sample_rate = 16000;
        cfg.feat_config.feature_dim = 80;
        cfg.model_config.moonshine.preprocessor = preprocessor;
        cfg.model_config.moonshine.encoder = encoder;
        cfg.model_config.moonshine.uncached_decoder = uncached;
        cfg.model_config.moonshine.cached_decoder = cached;
        cfg.model_config.tokens = tokens;
        cfg.model_config.num_threads = num_threads;
        cfg.model_config.provider = provider_ptr;
        cfg.decoding_method = method_ptr;

        let recognizer = unsafe {
            let create: Symbol<CreateRecognizerFn> = lib
                .get(b"SherpaOnnxCreateOfflineRecognizer\0")
                .map_err(|e| e.to_string())?;
            create(&cfg)
        };
        if recognizer.is_null() {
            return Err("sherpa could not load the Moonshine model (files may be corrupted — re-download it).".into());
        }
        Ok(SherpaSttEngine {
            recognizer,
            _keepalive: keep,
        })
    }

    /// Load a SenseVoice model from a directory containing model.int8.onnx and
    /// tokens.txt. `language` is an ISO code ("en", "zh", "ja", "ko", "yue") or
    /// "auto" to detect it per clip.
    pub fn load_sense_voice(dir: &Path, num_threads: i32, language: &str) -> Result<Self, String> {
        let lib = super::sherpa::lib()?;
        let c = |s: String| CString::new(s).map_err(|e| e.to_string());

        let mut keep: Vec<CString> = Vec::new();
        let mut path = |name: &str| -> Result<*const c_char, String> {
            let s = c(dir.join(name).to_string_lossy().into_owned())?;
            let ptr = s.as_ptr();
            keep.push(s);
            Ok(ptr)
        };

        let model = path("model.int8.onnx")?;
        let tokens = path("tokens.txt")?;

        let lang = c(language.into())?;
        let lang_ptr = lang.as_ptr();
        keep.push(lang);
        let provider = c("cpu".into())?;
        let provider_ptr = provider.as_ptr();
        keep.push(provider);
        let method = c("greedy_search".into())?;
        let method_ptr = method.as_ptr();
        keep.push(method);

        let mut cfg: OfflineRecognizerConfig = unsafe { std::mem::zeroed() };
        cfg.feat_config.sample_rate = 16000;
        cfg.feat_config.feature_dim = 80;
        cfg.model_config.sense_voice.model = model;
        cfg.model_config.sense_voice.language = lang_ptr;
        cfg.model_config.sense_voice.use_itn = 1; // "two hundred" -> "200", etc.
        cfg.model_config.tokens = tokens;
        cfg.model_config.num_threads = num_threads;
        cfg.model_config.provider = provider_ptr;
        cfg.decoding_method = method_ptr;

        let recognizer = unsafe {
            let create: Symbol<CreateRecognizerFn> = lib
                .get(b"SherpaOnnxCreateOfflineRecognizer\0")
                .map_err(|e| e.to_string())?;
            create(&cfg)
        };
        if recognizer.is_null() {
            return Err("sherpa could not load the SenseVoice model (files may be corrupted — re-download it).".into());
        }
        Ok(SherpaSttEngine {
            recognizer,
            _keepalive: keep,
        })
    }

    pub fn load_transducer(dir: &Path, num_threads: i32) -> Result<Self, String> {
        let lib = super::sherpa::lib()?;
        let c = |s: String| CString::new(s).map_err(|e| e.to_string());

        let mut keep: Vec<CString> = Vec::new();
        let mut path = |name: &str| -> Result<*const c_char, String> {
            let s = c(dir.join(name).to_string_lossy().into_owned())?;
            let ptr = s.as_ptr();
            keep.push(s);
            Ok(ptr)
        };

        let encoder = path("encoder.int8.onnx")?;
        let decoder = path("decoder.int8.onnx")?;
        let joiner = path("joiner.int8.onnx")?;
        let tokens = path("tokens.txt")?;

        let provider = c("cpu".into())?;
        let provider_ptr = provider.as_ptr();
        keep.push(provider);
        let method = c("greedy_search".into())?;
        let method_ptr = method.as_ptr();
        keep.push(method);

        let mut cfg: OfflineRecognizerConfig = unsafe { std::mem::zeroed() };
        cfg.feat_config.sample_rate = 16000;
        cfg.feat_config.feature_dim = 80;
        cfg.model_config.transducer.encoder = encoder;
        cfg.model_config.transducer.decoder = decoder;
        cfg.model_config.transducer.joiner = joiner;
        cfg.model_config.tokens = tokens;
        cfg.model_config.num_threads = num_threads;
        cfg.model_config.provider = provider_ptr;
        cfg.decoding_method = method_ptr;

        let recognizer = unsafe {
            let create: Symbol<CreateRecognizerFn> = lib
                .get(b"SherpaOnnxCreateOfflineRecognizer\0")
                .map_err(|e| e.to_string())?;
            create(&cfg)
        };
        if recognizer.is_null() {
            return Err("sherpa could not load the Parakeet model (files may be corrupted — re-download it).".into());
        }
        Ok(SherpaSttEngine {
            recognizer,
            _keepalive: keep,
        })
    }

    /// Transcribe 16 kHz mono f32 samples. Returns the recognized text.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        let lib = super::sherpa::lib()?;
        unsafe {
            let create_stream: Symbol<CreateStreamFn> = lib
                .get(b"SherpaOnnxCreateOfflineStream\0")
                .map_err(|e| e.to_string())?;
            let destroy_stream: Symbol<DestroyStreamFn> = lib
                .get(b"SherpaOnnxDestroyOfflineStream\0")
                .map_err(|e| e.to_string())?;
            let accept: Symbol<AcceptWaveformFn> = lib
                .get(b"SherpaOnnxAcceptWaveformOffline\0")
                .map_err(|e| e.to_string())?;
            let decode: Symbol<DecodeFn> = lib
                .get(b"SherpaOnnxDecodeOfflineStream\0")
                .map_err(|e| e.to_string())?;
            let get_result: Symbol<GetResultFn> = lib
                .get(b"SherpaOnnxGetOfflineStreamResult\0")
                .map_err(|e| e.to_string())?;
            let destroy_result: Symbol<DestroyResultFn> = lib
                .get(b"SherpaOnnxDestroyOfflineRecognizerResult\0")
                .map_err(|e| e.to_string())?;

            let stream = create_stream(self.recognizer);
            if stream.is_null() {
                return Err("sherpa could not create an offline stream.".into());
            }
            accept(stream, 16000, samples.as_ptr(), samples.len() as c_int);
            decode(self.recognizer, stream);
            let result = get_result(stream);
            let text = if result.is_null() || (*result).text.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr((*result).text)
                    .to_string_lossy()
                    .into_owned()
            };
            if !result.is_null() {
                destroy_result(result);
            }
            destroy_stream(stream);
            Ok(text.trim().to_string())
        }
    }
}

impl Drop for SherpaSttEngine {
    fn drop(&mut self) {
        if let Ok(lib) = super::sherpa::lib() {
            unsafe {
                if let Ok(destroy) =
                    lib.get::<DestroyRecognizerFn>(b"SherpaOnnxDestroyOfflineRecognizer\0")
                {
                    destroy(self.recognizer);
                }
            }
        }
    }
}

/// Read a WAV into 16 kHz mono f32 samples. The app records at 16 kHz mono, so
/// this is normally a straight i16→f32 pass; stereo is averaged to mono.
fn read_wav_mono_f32(path: &Path) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .into_samples::<i32>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 32768.0)
            .collect(),
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };
    if spec.channels <= 1 {
        return Ok(raw);
    }
    // Downmix interleaved channels to mono.
    let ch = spec.channels as usize;
    Ok(raw
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect())
}

// Moonshine is trained on short utterances and its decoder has no
// temperature-fallback loop-breaker (whisper's safety net), so feeding it one
// long clip can spiral into a repetition loop ("So, X. So, X. So, X…"). The fix
// — also sherpa-onnx's own recommended usage — is to split long audio at
// silence into short segments and transcribe each. Clips this short or shorter
// go in one shot, so normal dictation is unaffected. Applied uniformly to every
// sherpa engine (SenseVoice doesn't need it — non-autoregressive, no loop risk
// — but segmenting it too is harmless and keeps this path simple).
const SHERPA_MAX_WHOLE_SECS: usize = 18;
/// When segmenting, aim for pieces around this long, always cutting at a pause.
const SHERPA_TARGET_SEG_SECS: usize = 12;

/// Transcribe a WAV file with a sherpa engine (Moonshine or SenseVoice),
/// reusing (or lazily loading) a recognizer kept resident on AppState so the
/// ~2 s model load happens only once. Synchronous/CPU-bound — callers should
/// run it on a blocking thread. `threads` is baked into the recognizer, so a
/// change re-loads it; switching model_id also reloads.
pub fn ensure_engine(app: &tauri::AppHandle, model_id: &str, threads: u32) -> Result<std::sync::Arc<SherpaSttEngine>, String> {
    use tauri::Manager;
    let state = app.state::<crate::AppState>();
    let mut slot = state.sherpa_stt.lock().map_err(|e| e.to_string())?;
    let key = format!("{model_id}|{threads}");
    let hit = slot.as_ref().is_some_and(|(k, _)| *k == key);
    if !hit {
        let dir = crate::models::registry::stt_model_dir(model_id);
        let eng = std::sync::Arc::new(
            match crate::models::registry::stt_engine(model_id) {
                crate::models::registry::SttEngine::SenseVoice => {
                    SherpaSttEngine::load_sense_voice(&dir, threads as i32, "auto")?
                }
                crate::models::registry::SttEngine::Transducer => {
                    SherpaSttEngine::load_transducer(&dir, threads as i32)?
                }
                crate::models::registry::SttEngine::Moonshine => {
                    SherpaSttEngine::load_moonshine(&dir, threads as i32)?
                }
                crate::models::registry::SttEngine::Whisper => unreachable!(),
            },
        );
        *slot = Some((key, eng));
    }
    Ok(slot.as_ref().unwrap().1.clone())
}

pub fn transcribe_file(
    app: &tauri::AppHandle,
    audio_path: &str,
    model_id: &str,
    threads: u32,
) -> Result<String, String> {
    use tauri::Manager;
    let samples = read_wav_mono_f32(Path::new(audio_path))?;

    let engine = ensure_engine(app, model_id, threads)?;
    let sensitivity = app
        .state::<crate::AppState>()
        .config
        .lock()
        .map(|c| c.input_sensitivity)
        .unwrap_or(50);

    let sr = crate::audio::capture::WHISPER_SAMPLE_RATE as usize;
    if samples.len() <= SHERPA_MAX_WHOLE_SECS * sr {
        return engine.transcribe(&samples);
    }

    // Long clip: transcribe silence-bounded segments and join. Moonshine is a
    // short-form model — hand it a chunk that STARTS mid-word (which happens
    // when continuous speech is hard-split at the cap) and it silently returns
    // nothing, dropping that whole span. Two guards below make this lossless.
    let mut segs = segment_bounds(&samples, sensitivity);

    // 1) Snap every internal boundary to the quietest point nearby, so cuts
    // land between words instead of through one. VAD-chosen cuts are already in
    // silence and barely move; hard splits snap to the smallest gap available.
    let win = sr * 2 / 5; // ±0.4 s search
    for i in 0..segs.len().saturating_sub(1) {
        let b = segs[i].1;
        let nb = quietest_cut(&samples, b, win);
        if nb > segs[i].0 && nb < segs[i + 1].1 {
            segs[i].1 = nb;
            segs[i + 1].0 = nb;
        }
    }

    let mut parts: Vec<String> = Vec::new();
    for (seg_idx, (start, end)) in segs.into_iter().enumerate() {
        let t0 = std::time::Instant::now();
        let mut text = engine.transcribe(&samples[start..end])?;
        let elapsed = t0.elapsed();
        let audio_seconds = (end - start) as f64 / sr as f64;
        let decode_ms = elapsed.as_millis();
        let ratio = decode_ms as f64 / (audio_seconds * 1000.0);
        crate::logging::log_info(
            "sherpa_stt",
            &format!(
                "segment {seg_idx}: {audio_seconds:.2}s audio, {decode_ms}ms decode, ratio {ratio:.2}"
            ),
        );
        // 2) Never drop silently. A non-trivial span that comes back empty was
        // almost certainly clipped mid-word — retry with ~1 s of lead-in so the
        // model has a running start. Any duplicated words at the join are
        // collapsed later in the pipeline (collapse_repeated_words).
        if text.trim().is_empty() && end - start > sr / 2 {
            let pad_start = start.saturating_sub(sr);
            let retry = engine
                .transcribe(&samples[pad_start..end])
                .unwrap_or_default();
            if retry.trim().is_empty() {
                crate::logging::log_error(
                    "sherpa_stt",
                    &format!("segment [{start}..{end}] transcribed empty — words may be lost"),
                );
            } else {
                crate::logging::log_info(
                    "sherpa_stt",
                    "recovered an empty segment via padded retry",
                );
                text = retry;
            }
        }
        let text = text.trim();
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }
    Ok(parts.join(" "))
}

/// The center of the lowest-energy 10 ms frame within `window` samples either
/// side of `around` — i.e. the quietest place to cut so a word isn't sliced in
/// half. Falls back to `around` if the range is degenerate.
fn quietest_cut(samples: &[f32], around: usize, window: usize) -> usize {
    let lo = around.saturating_sub(window);
    let hi = (around + window).min(samples.len());
    if hi <= lo + 160 {
        return around;
    }
    const FRAME: usize = 160; // 10 ms @ 16 kHz
    let mut best = around;
    let mut best_e = f32::MAX;
    let mut i = lo;
    while i + FRAME <= hi {
        let e: f32 = samples[i..i + FRAME].iter().map(|s| s * s).sum();
        if e < best_e {
            best_e = e;
            best = i + FRAME / 2;
        }
        i += FRAME;
    }
    best
}

/// Split a long clip into segments of at most ~SHERPA_TARGET_SEG_SECS,
/// cutting at silences found by Silero VAD. Falls back to fixed windows when the
/// VAD model isn't installed. Returns non-overlapping `[start, end)` ranges that
/// together cover the whole clip.
fn segment_bounds(samples: &[f32], sensitivity: u32) -> Vec<(usize, usize)> {
    let sr = crate::audio::capture::WHISPER_SAMPLE_RATE as usize;
    let max_seg = SHERPA_TARGET_SEG_SECS * sr;

    let cuts = match crate::audio::vad::speech_mask(
        samples,
        crate::audio::vad::threshold_for(sensitivity),
    ) {
        Some(mask) => silence_midpoints(&mask),
        // No VAD model — clean fixed windows (no overlap, so no duplicated words
        // at the joins; a word may rarely be clipped at a hard boundary).
        None => return fixed_windows(samples.len(), max_seg),
    };
    build_segments(samples.len(), &cuts, max_seg)
}

/// Sample offsets at the middle of each silence gap long enough (~0.38 s) to be
/// a real between-sentence pause rather than a gap between words.
fn silence_midpoints(mask: &[bool]) -> Vec<usize> {
    const MIN_SILENCE_FRAMES: usize = 12; // 12 × 32 ms ≈ 0.38 s
    let frame = crate::audio::vad::FRAME;
    let mut cuts = Vec::new();
    let mut i = 0;
    while i < mask.len() {
        if mask[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < mask.len() && !mask[i] {
            i += 1;
        }
        if i - start >= MIN_SILENCE_FRAMES {
            cuts.push(((start + i) / 2) * frame);
        }
    }
    cuts
}

/// Greedily pack the spans between cut points into segments no longer than
/// `max_seg`, preferring to break at a pause; a stretch of continuous speech
/// longer than `max_seg` is hard-split so no segment can ever exceed the cap.
fn build_segments(len: usize, cuts: &[usize], max_seg: usize) -> Vec<(usize, usize)> {
    let mut points = vec![0usize];
    points.extend(cuts.iter().copied().filter(|&c| c > 0 && c < len));
    points.push(len);
    points.dedup();

    let mut segs = Vec::new();
    let mut start = 0usize;
    let mut prev = 0usize;
    for &p in &points[1..] {
        if p - start <= max_seg {
            prev = p;
            continue;
        }
        if prev > start {
            segs.push((start, prev));
            start = prev;
        }
        while p - start > max_seg {
            segs.push((start, start + max_seg));
            start += max_seg;
        }
        prev = p;
    }
    if start < len {
        segs.push((start, len));
    }
    segs
}

fn fixed_windows(len: usize, win: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut start = 0;
    while start < len {
        let end = (start + win).min(len);
        v.push((start, end));
        start = end;
    }
    v
}

#[cfg(test)]
mod seg_tests {
    use super::*;

    #[test]
    fn short_span_is_one_segment() {
        // No cuts, whole thing fits — one segment covering everything.
        assert_eq!(build_segments(1000, &[], 5000), vec![(0, 1000)]);
    }

    #[test]
    fn breaks_at_a_pause_when_over_the_cap() {
        // Cut at 600; cap 700. [0,1000] over cap → close at 600, then [600,1000].
        assert_eq!(build_segments(1000, &[600], 700), vec![(0, 600), (600, 1000)]);
    }

    #[test]
    fn continuous_speech_is_hard_split() {
        // No usable cut and 1000 > cap 400 → hard 400-sample pieces + remainder.
        assert_eq!(
            build_segments(1000, &[], 400),
            vec![(0, 400), (400, 800), (800, 1000)]
        );
    }

    #[test]
    fn segments_are_gapless_and_cover_everything() {
        let segs = build_segments(10_000, &[1500, 3100, 7000, 9500], 3000);
        assert_eq!(segs.first().unwrap().0, 0);
        assert_eq!(segs.last().unwrap().1, 10_000);
        for w in segs.windows(2) {
            assert_eq!(w[0].1, w[1].0, "gap or overlap between segments");
        }
        for (s, e) in segs {
            assert!(e - s <= 3000, "segment exceeds cap: {}", e - s);
        }
    }

    #[test]
    fn quietest_cut_finds_the_gap() {
        // Loud, then a quiet dip around 5000, then loud again. A cut targeted
        // at 4800 should snap into the dip, not stay on a loud sample.
        let mut s = vec![0.5f32; 10_000];
        for v in s.iter_mut().take(5200).skip(4800) {
            *v = 0.0;
        }
        let cut = quietest_cut(&s, 4600, 600);
        assert!((4800..=5200).contains(&cut), "cut {cut} not in the quiet dip");
    }

    #[test]
    fn quietest_cut_degenerate_range_is_noop() {
        let s = vec![0.1f32; 100];
        assert_eq!(quietest_cut(&s, 50, 0), 50);
    }

    #[test]
    fn fixed_windows_tile_the_clip() {
        assert_eq!(fixed_windows(1000, 400), vec![(0, 400), (400, 800), (800, 1000)]);
        assert_eq!(fixed_windows(800, 400), vec![(0, 400), (400, 800)]);
    }

    #[test]
    fn only_long_gaps_become_cuts() {
        // frame = vad::FRAME. speech, then a 5-frame gap (too short), speech,
        // then a 15-frame gap (a real pause), speech.
        let mut mask = vec![true; 3];
        mask.extend(vec![false; 5]);
        mask.extend(vec![true; 3]);
        mask.extend(vec![false; 15]);
        mask.extend(vec![true; 3]);
        let cuts = silence_midpoints(&mask);
        assert_eq!(cuts.len(), 1, "only the long gap should produce a cut");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Spike benchmark. Point it at the extracted Moonshine dir and a real
    // 16 kHz mono dictation clip via env vars, then:
    //   copy target\debug\sherpa -> target\debug\deps\sherpa   (DLLs next to test exe)
    //   set SV_MOONSHINE_DIR=...    set SV_SAMPLE_WAV=...
    //   cargo test --release sherpa_stt::tests::bench -- --nocapture --ignored
    #[test]
    #[ignore]
    fn bench() {
        let dir = std::env::var("SV_MOONSHINE_DIR").expect("set SV_MOONSHINE_DIR");
        let wav = std::env::var("SV_SAMPLE_WAV").expect("set SV_SAMPLE_WAV");

        // Exercise the PRODUCTION wav reader, not an inline copy.
        let samples = read_wav_mono_f32(Path::new(&wav)).expect("read wav");
        let secs = samples.len() as f32 / 16000.0;
        eprintln!("clip: {:.2}s ({} samples)", secs, samples.len());

        let t_load = std::time::Instant::now();
        let engine = SherpaSttEngine::load_moonshine(Path::new(&dir), 4).expect("load moonshine");
        eprintln!("model load: {} ms", t_load.elapsed().as_millis());

        // Warm run + timed runs (reuse engine — measures pure inference).
        for i in 0..4 {
            let t = std::time::Instant::now();
            let text = engine.transcribe(&samples).expect("transcribe");
            let ms = t.elapsed().as_millis();
            eprintln!("run {i}: {ms} ms  |  \"{text}\"");
        }

        // Segmented path (the long-clip fix): split at silence, transcribe each,
        // join. Prints segment layout + joined result so we can eyeball that it
        // stays coherent and bounded vs a single whole-clip call.
        let bounds = segment_bounds(&samples, 50);
        eprintln!("--- segmented into {} piece(s) ---", bounds.len());
        let t = std::time::Instant::now();
        let mut parts = Vec::new();
        for (s, e) in &bounds {
            eprintln!("  seg {:.1}s..{:.1}s", *s as f32 / 16000.0, *e as f32 / 16000.0);
            let txt = engine.transcribe(&samples[*s..*e]).expect("seg transcribe");
            let txt = txt.trim().to_string();
            if !txt.is_empty() {
                parts.push(txt);
            }
        }
        eprintln!(
            "segmented total: {} ms  |  \"{}\"",
            t.elapsed().as_millis(),
            parts.join(" ")
        );
    }

    // set SV_SENSEVOICE_DIR=...   set SV_SAMPLE_WAV=...
    // cargo test --release sherpa_stt::tests::bench_sense_voice -- --nocapture --ignored
    #[test]
    #[ignore]
    fn bench_sense_voice() {
        let dir = std::env::var("SV_SENSEVOICE_DIR").expect("set SV_SENSEVOICE_DIR");
        let wav = std::env::var("SV_SAMPLE_WAV").expect("set SV_SAMPLE_WAV");

        let samples = read_wav_mono_f32(Path::new(&wav)).expect("read wav");
        eprintln!("clip: {:.2}s", samples.len() as f32 / 16000.0);

        let t_load = std::time::Instant::now();
        let engine =
            SherpaSttEngine::load_sense_voice(Path::new(&dir), 4, "auto").expect("load sense voice");
        eprintln!("model load: {} ms", t_load.elapsed().as_millis());

        for i in 0..3 {
            let t = std::time::Instant::now();
            let text = engine.transcribe(&samples).expect("transcribe");
            eprintln!("run {i}: {} ms  |  \"{text}\"", t.elapsed().as_millis());
        }
    }
}
