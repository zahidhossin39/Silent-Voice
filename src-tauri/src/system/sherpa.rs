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
}
