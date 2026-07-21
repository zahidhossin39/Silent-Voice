// FFI bindings to sherpa-onnx-c-api.dll (offline TTS only).
//
// WHY FFI AND NOT THE CLI: sherpa-onnx-offline-tts.exe uses a narrow
// `main(char* argv[])`. On Windows the C runtime converts the process
// command line to the ANSI codepage, which cannot represent Bengali (or any
// non-Latin script) — the text arrives as `????` and the voice speaks
// gibberish. Passing UTF-8 bytes straight into the C API sidesteps argv
// entirely. (This bug was observed live: Bangla synth via CLI produced
// garbage; the same model via this API speaks correctly.)
//
// Struct layouts mirror sherpa-onnx v1.13.4 c-api.h EXACTLY — do not
// reorder or trim fields without re-checking the header for the bundled
// DLL's version. The DLL (+ onnxruntime.dll) lives in exe_dir/sherpa/.

#![allow(non_snake_case)]

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_float, c_int, CString};
use std::path::Path;
use std::sync::OnceLock;

#[repr(C)]
struct VitsModelConfig {
    model: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: c_float,
    noise_scale_w: c_float,
    length_scale: c_float,
    dict_dir: *const c_char,
}

#[repr(C)]
struct MatchaModelConfig {
    acoustic_model: *const c_char,
    vocoder: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: c_float,
    length_scale: c_float,
    dict_dir: *const c_char,
}

#[repr(C)]
struct KokoroModelConfig {
    model: *const c_char,
    voices: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    length_scale: c_float,
    dict_dir: *const c_char,
    lexicon: *const c_char,
    lang: *const c_char,
}

#[repr(C)]
struct KittenModelConfig {
    model: *const c_char,
    voices: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    length_scale: c_float,
}

#[repr(C)]
struct ZipvoiceModelConfig {
    tokens: *const c_char,
    encoder: *const c_char,
    decoder: *const c_char,
    vocoder: *const c_char,
    data_dir: *const c_char,
    lexicon: *const c_char,
    feat_scale: c_float,
    t_shift: c_float,
    target_rms: c_float,
    guidance_scale: c_float,
}

#[repr(C)]
struct PocketModelConfig {
    lm_flow: *const c_char,
    lm_main: *const c_char,
    encoder: *const c_char,
    decoder: *const c_char,
    text_conditioner: *const c_char,
    vocab_json: *const c_char,
    token_scores_json: *const c_char,
    voice_embedding_cache_capacity: c_int,
}

#[repr(C)]
struct SupertonicModelConfig {
    duration_predictor: *const c_char,
    text_encoder: *const c_char,
    vector_estimator: *const c_char,
    vocoder: *const c_char,
    tts_json: *const c_char,
    unicode_indexer: *const c_char,
    voice_style: *const c_char,
}

#[repr(C)]
struct TtsModelConfig {
    vits: VitsModelConfig,
    num_threads: c_int,
    debug: c_int,
    provider: *const c_char,
    matcha: MatchaModelConfig,
    kokoro: KokoroModelConfig,
    kitten: KittenModelConfig,
    zipvoice: ZipvoiceModelConfig,
    pocket: PocketModelConfig,
    supertonic: SupertonicModelConfig,
}

#[repr(C)]
struct TtsConfig {
    model: TtsModelConfig,
    rule_fsts: *const c_char,
    max_num_sentences: c_int,
    rule_fars: *const c_char,
    silence_scale: c_float,
}

#[repr(C)]
struct GeneratedAudio {
    samples: *const c_float,
    n: c_int,
    sample_rate: c_int,
}

type CreateFn = unsafe extern "C" fn(*const TtsConfig) -> *const std::ffi::c_void;
type DestroyFn = unsafe extern "C" fn(*const std::ffi::c_void);
type GenerateFn = unsafe extern "C" fn(
    *const std::ffi::c_void,
    *const c_char,
    c_int,
    c_float,
) -> *const GeneratedAudio;
type DestroyAudioFn = unsafe extern "C" fn(*const GeneratedAudio);
type WriteWaveFn =
    unsafe extern "C" fn(*const c_float, c_int, c_int, *const c_char) -> c_int;

static ORT: OnceLock<Result<Library, String>> = OnceLock::new();
static LIB: OnceLock<Result<Library, String>> = OnceLock::new();

fn lib() -> Result<&'static Library, String> {
    LIB.get_or_init(|| {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("sherpa")))
            .ok_or("cannot locate exe dir")?;
        // Pre-load OUR onnxruntime.dll by absolute path FIRST. Without this,
        // Windows resolves the c-api DLL's `onnxruntime.dll` import through
        // the normal search order and can pick up an incompatible copy (e.g.
        // the OS-shipped one, or Piper's older 1.17 build) → API-version
        // mismatch → access violation. A module already loaded under that
        // name always wins dependency resolution, so loading ours first pins
        // it.
        let ort = ORT.get_or_init(|| {
            let p = dir.join("onnxruntime.dll");
            unsafe { Library::new(&p) }
                .map_err(|e| format!("could not load {}: {e} — reinstall the app.", p.display()))
        });
        if let Err(e) = ort {
            return Err(e.clone());
        }
        let dll = dir.join("sherpa-onnx-c-api.dll");
        unsafe { Library::new(&dll) }
            .map_err(|e| format!("could not load {}: {e} — reinstall the app.", dll.display()))
    })
    .as_ref()
    .map_err(|e| e.clone())
}

/// Synthesize `text` (UTF-8, any script) into a 16-bit WAV at `out_wav`.
/// `dir` is the extracted voice directory: contains the .onnx model,
/// tokens.txt, and optionally lexicon.txt / espeak-ng-data.
pub fn synthesize(
    dir: &Path,
    model: &Path,
    text: &str,
    out_wav: &Path,
    num_threads: i32,
) -> Result<(), String> {
    let lib = lib()?;

    let empty = CString::new("").unwrap();
    let c = |s: &str| CString::new(s).map_err(|e| e.to_string());

    let model_c = c(&model.to_string_lossy())?;
    let tokens_c = c(&dir.join("tokens.txt").to_string_lossy())?;
    let lexicon_path = dir.join("lexicon.txt");
    let lexicon_c = if lexicon_path.exists() {
        c(&lexicon_path.to_string_lossy())?
    } else {
        empty.clone()
    };
    let data_path = dir.join("espeak-ng-data");
    let data_c = if data_path.is_dir() {
        c(&data_path.to_string_lossy())?
    } else {
        empty.clone()
    };
    let provider_c = c("cpu")?;
    let text_c = c(text)?;
    let wav_c = c(&out_wav.to_string_lossy())?;

    // Zeroed config with only the VITS section populated — matches what the
    // CLI does for --vits-* flags.
    let mut cfg: TtsConfig = unsafe { std::mem::zeroed() };
    cfg.model.vits = VitsModelConfig {
        model: model_c.as_ptr(),
        lexicon: lexicon_c.as_ptr(),
        tokens: tokens_c.as_ptr(),
        data_dir: data_c.as_ptr(),
        noise_scale: 0.667,
        noise_scale_w: 0.8,
        length_scale: 1.0,
        dict_dir: empty.as_ptr(),
    };
    cfg.model.num_threads = num_threads;
    cfg.model.provider = provider_c.as_ptr();
    cfg.max_num_sentences = 1;
    cfg.rule_fsts = empty.as_ptr();
    cfg.rule_fars = empty.as_ptr();
    cfg.silence_scale = 0.2;

    unsafe {
        let create: Symbol<CreateFn> = lib
            .get(b"SherpaOnnxCreateOfflineTts\0")
            .map_err(|e| e.to_string())?;
        let destroy: Symbol<DestroyFn> = lib
            .get(b"SherpaOnnxDestroyOfflineTts\0")
            .map_err(|e| e.to_string())?;
        let generate: Symbol<GenerateFn> = lib
            .get(b"SherpaOnnxOfflineTtsGenerate\0")
            .map_err(|e| e.to_string())?;
        let destroy_audio: Symbol<DestroyAudioFn> = lib
            .get(b"SherpaOnnxDestroyOfflineTtsGeneratedAudio\0")
            .map_err(|e| e.to_string())?;
        let write_wave: Symbol<WriteWaveFn> = lib
            .get(b"SherpaOnnxWriteWave\0")
            .map_err(|e| e.to_string())?;

        let tts = create(&cfg);
        if tts.is_null() {
            return Err("sherpa-onnx could not load the voice (files may be corrupted — re-download it).".into());
        }
        let audio = generate(tts, text_c.as_ptr(), 0, 1.0);
        if audio.is_null() {
            destroy(tts);
            return Err("sherpa-onnx synthesis failed.".into());
        }
        let ok = write_wave((*audio).samples, (*audio).n, (*audio).sample_rate, wav_c.as_ptr());
        destroy_audio(audio);
        destroy(tts);
        if ok == 0 {
            return Err("could not write the synthesized WAV file.".into());
        }
    }
    Ok(())
}

// ============================================================
// Offline ASR (Moonshine) — reuses the SAME sherpa-onnx-c-api.dll.
//
// Struct layouts mirror sherpa-onnx v1.13.4 c-api.h EXACTLY (verified against
// the tagged header for the bundled DLL). The whole config is zeroed and only
// the Moonshine sub-config is populated — but EVERY sub-struct must still be
// declared with the right field count/types so the byte offsets of the fields
// we DO set line up with what the DLL reads. Do not reorder or trim.
// ============================================================

#[repr(C)]
struct OffTransducer { encoder: *const c_char, decoder: *const c_char, joiner: *const c_char }
#[repr(C)]
struct OffParaformer { model: *const c_char }
#[repr(C)]
struct OffNemoCtc { model: *const c_char }
#[repr(C)]
struct OffWhisper {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    task: *const c_char,
    tail_paddings: c_int,
    enable_token_timestamps: c_int,
    enable_segment_timestamps: c_int,
}
#[repr(C)]
struct OffTdnn { model: *const c_char }
#[repr(C)]
struct OffSenseVoice { model: *const c_char, language: *const c_char, use_itn: c_int }
#[repr(C)]
struct OffMoonshine {
    preprocessor: *const c_char,
    encoder: *const c_char,
    uncached_decoder: *const c_char,
    cached_decoder: *const c_char,
    merged_decoder: *const c_char,
}
#[repr(C)]
struct OffFireRedAsr { encoder: *const c_char, decoder: *const c_char }
#[repr(C)]
struct OffDolphin { model: *const c_char }
#[repr(C)]
struct OffZipformerCtc { model: *const c_char }
#[repr(C)]
struct OffCanary {
    encoder: *const c_char,
    decoder: *const c_char,
    src_lang: *const c_char,
    tgt_lang: *const c_char,
    use_pnc: c_int,
}
#[repr(C)]
struct OffWenetCtc { model: *const c_char }
#[repr(C)]
struct OffOmnilingual { model: *const c_char }
#[repr(C)]
struct OffMedAsr { model: *const c_char }
#[repr(C)]
struct OffFunAsrNano {
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
struct OffFireRedAsrCtc { model: *const c_char }
#[repr(C)]
struct OffQwen3Asr {
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
struct OffCohereTranscribe {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    use_punct: c_int,
    use_itn: c_int,
}

#[repr(C)]
struct OfflineModelConfig {
    transducer: OffTransducer,
    paraformer: OffParaformer,
    nemo_ctc: OffNemoCtc,
    whisper: OffWhisper,
    tdnn: OffTdnn,
    tokens: *const c_char,
    num_threads: c_int,
    debug: c_int,
    provider: *const c_char,
    model_type: *const c_char,
    modeling_unit: *const c_char,
    bpe_vocab: *const c_char,
    telespeech_ctc: *const c_char,
    sense_voice: OffSenseVoice,
    moonshine: OffMoonshine,
    fire_red_asr: OffFireRedAsr,
    dolphin: OffDolphin,
    zipformer_ctc: OffZipformerCtc,
    canary: OffCanary,
    wenet_ctc: OffWenetCtc,
    omnilingual: OffOmnilingual,
    medasr: OffMedAsr,
    funasr_nano: OffFunAsrNano,
    fire_red_asr_ctc: OffFireRedAsrCtc,
    qwen3_asr: OffQwen3Asr,
    cohere_transcribe: OffCohereTranscribe,
}

#[repr(C)]
struct FeatureConfig { sample_rate: c_int, feature_dim: c_int }
#[repr(C)]
struct OfflineLmConfig { model: *const c_char, scale: c_float }
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

type Cptr = *const std::ffi::c_void;
type RecogCreateFn = unsafe extern "C" fn(*const OfflineRecognizerConfig) -> Cptr;
type RecogDestroyFn = unsafe extern "C" fn(Cptr);
type StreamCreateFn = unsafe extern "C" fn(Cptr) -> Cptr;
type StreamDestroyFn = unsafe extern "C" fn(Cptr);
type AcceptFn = unsafe extern "C" fn(Cptr, c_int, *const c_float, c_int);
type DecodeFn = unsafe extern "C" fn(Cptr, Cptr);
type GetResultFn = unsafe extern "C" fn(Cptr) -> *const OfflineRecognizerResult;
type DestroyResultFn = unsafe extern "C" fn(*const OfflineRecognizerResult);

/// Find the first `.onnx` in `dir` whose name contains `needle` but none of
/// `exclude` (case-insensitive). The exclude list is what keeps the
/// `cached_decode` lookup from matching `uncached_decode.int8.onnx`.
fn find_onnx(dir: &Path, needle: &str, exclude: &[&str]) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if name.ends_with(".onnx")
            && name.contains(needle)
            && !exclude.iter().any(|x| name.contains(x))
        {
            return Some(e.path());
        }
    }
    None
}

/// Transcribe 16 kHz mono f32 `samples` with a Moonshine model in `dir`.
/// `dir` is the extracted k2-fsa archive (preprocess/encode/uncached_decode/
/// cached_decode `.onnx` + tokens.txt).
pub fn transcribe_moonshine(
    dir: &Path,
    samples: &[f32],
    num_threads: i32,
) -> Result<String, String> {
    let lib = lib()?;

    let pre = find_onnx(dir, "preprocess", &[]).ok_or("moonshine: preprocess model missing")?;
    let enc = find_onnx(dir, "encode", &["preprocess", "decode"]).ok_or("moonshine: encode model missing")?;
    let unc = find_onnx(dir, "uncached_decode", &[]).ok_or("moonshine: uncached_decode model missing")?;
    let cac = find_onnx(dir, "cached_decode", &["uncached"]).ok_or("moonshine: cached_decode model missing")?;
    let tokens = dir.join("tokens.txt");
    if !tokens.exists() {
        return Err("moonshine: tokens.txt missing".into());
    }

    let c = |s: &str| CString::new(s).map_err(|e| e.to_string());
    let empty = CString::new("").unwrap();
    let pre_c = c(&pre.to_string_lossy())?;
    let enc_c = c(&enc.to_string_lossy())?;
    let unc_c = c(&unc.to_string_lossy())?;
    let cac_c = c(&cac.to_string_lossy())?;
    let tokens_c = c(&tokens.to_string_lossy())?;
    let provider_c = c("cpu")?;
    let method_c = c("greedy_search")?;

    let mut cfg: OfflineRecognizerConfig = unsafe { std::mem::zeroed() };
    cfg.feat_config = FeatureConfig { sample_rate: 16000, feature_dim: 80 };
    cfg.model_config.moonshine = OffMoonshine {
        preprocessor: pre_c.as_ptr(),
        encoder: enc_c.as_ptr(),
        uncached_decoder: unc_c.as_ptr(),
        cached_decoder: cac_c.as_ptr(),
        merged_decoder: empty.as_ptr(),
    };
    cfg.model_config.tokens = tokens_c.as_ptr();
    cfg.model_config.num_threads = num_threads.max(1);
    cfg.model_config.provider = provider_c.as_ptr();
    cfg.decoding_method = method_c.as_ptr();

    unsafe {
        let create: Symbol<RecogCreateFn> =
            lib.get(b"SherpaOnnxCreateOfflineRecognizer\0").map_err(|e| e.to_string())?;
        let destroy: Symbol<RecogDestroyFn> =
            lib.get(b"SherpaOnnxDestroyOfflineRecognizer\0").map_err(|e| e.to_string())?;
        let stream_create: Symbol<StreamCreateFn> =
            lib.get(b"SherpaOnnxCreateOfflineStream\0").map_err(|e| e.to_string())?;
        let stream_destroy: Symbol<StreamDestroyFn> =
            lib.get(b"SherpaOnnxDestroyOfflineStream\0").map_err(|e| e.to_string())?;
        let accept: Symbol<AcceptFn> =
            lib.get(b"SherpaOnnxAcceptWaveformOffline\0").map_err(|e| e.to_string())?;
        let decode: Symbol<DecodeFn> =
            lib.get(b"SherpaOnnxDecodeOfflineStream\0").map_err(|e| e.to_string())?;
        let get_result: Symbol<GetResultFn> =
            lib.get(b"SherpaOnnxGetOfflineStreamResult\0").map_err(|e| e.to_string())?;
        let destroy_result: Symbol<DestroyResultFn> =
            lib.get(b"SherpaOnnxDestroyOfflineRecognizerResult\0").map_err(|e| e.to_string())?;

        let recognizer = create(&cfg);
        if recognizer.is_null() {
            return Err("sherpa-onnx could not load the Moonshine model (files may be corrupted — re-download it).".into());
        }
        let stream = stream_create(recognizer);
        if stream.is_null() {
            destroy(recognizer);
            return Err("sherpa-onnx could not create a decode stream.".into());
        }
        accept(stream, 16000, samples.as_ptr(), samples.len() as c_int);
        decode(recognizer, stream);
        let result = get_result(stream);
        let text = if result.is_null() || (*result).text.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr((*result).text).to_string_lossy().into_owned()
        };
        if !result.is_null() {
            destroy_result(result);
        }
        stream_destroy(stream);
        destroy(recognizer);
        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Requires the Bangla voice in %APPDATA%\SilentVoice\tts and the sherpa
    // DLLs next to the test exe (copy target/debug/sherpa → target/debug/deps/sherpa).
    // Verifies UTF-8 Bengali survives the FFI boundary — the exact thing the
    // CLI path broke.
    fn synth_voice(voice_id: &str) {
        let dir = crate::models::registry::sherpa_voice_dir(voice_id);
        let Some(model) = crate::models::registry::sherpa_voice_model(voice_id) else {
            eprintln!("{voice_id} not downloaded — skipping");
            return;
        };
        let out = std::env::temp_dir().join(format!("sv_sherpa_{voice_id}.wav"));
        let _ = std::fs::remove_file(&out);
        synthesize(&dir, &model, "আমার সোনার বাংলা, আমি তোমায় ভালোবাসি।", &out, 2).unwrap();
        let len = std::fs::metadata(&out).unwrap().len();
        // Real speech for this sentence is several seconds of audio; the
        // mangled-text failure mode produced a sub-second (~11 KB) blip.
        // (MMS voices run at 16 kHz, so bytes/sec is lower than 22 kHz VITS.)
        assert!(len > 60_000, "{voice_id}: WAV too small ({len} bytes) — text likely mangled");
        eprintln!("{voice_id}: {len} bytes → {}", out.display());
    }

    #[test]
    fn bengali_text_synthesizes() {
        synth_voice("vits-coqui-bn-custom_female");
        synth_voice("mms-tts-bengali");
    }

    // Decodes the Moonshine archive's OWN bundled test WAV (ground truth in
    // test_wavs/trans.txt) through the FFI — proves the offline recognizer +
    // Moonshine struct layout work end-to-end without a live mic. Requires the
    // tiny model extracted into the STT models dir and the sherpa DLLs next to
    // the test exe (copy target/debug/sherpa → target/debug/deps/sherpa).
    #[test]
    fn moonshine_decodes_bundled_wav() {
        let dir = crate::models::registry::moonshine_dir("sherpa-onnx-moonshine-tiny-en-int8");
        let wav = dir.join("test_wavs").join("0.wav");
        if !wav.exists() {
            eprintln!("moonshine tiny model not extracted — skipping");
            return;
        }
        let reader = hound::WavReader::open(&wav).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16000, "expected a 16 kHz test wav");
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => reader
                .into_samples::<i16>()
                .map(|s| s.unwrap() as f32 / 32768.0)
                .collect(),
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>().map(|s| s.unwrap()).collect()
            }
        };
        let text = transcribe_moonshine(&dir, &samples, 2).unwrap();
        eprintln!("moonshine decoded: {text:?}");
        let upper = text.to_uppercase();
        assert!(
            upper.contains("NIGHTFALL") && upper.contains("YELLOW LAMPS"),
            "decode did not match ground truth, got: {text:?}"
        );
    }
}
