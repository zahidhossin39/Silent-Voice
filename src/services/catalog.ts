import type { SttModel, LlmModel, TtsModel, TtsQuality } from "../types";

// Base URL for Whisper GGML models on Hugging Face
export const WHISPER_BASE_URL =
  "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

// ============================================================
// STT models (Whisper) — from build plan §4
// ============================================================
// Whether a model runs on the sherpa-onnx engine (downloaded as a .tar.bz2
// archive) instead of whisper.cpp. Single source of truth so adding a new
// sherpa family is one edit, not several scattered engine checks.
export function isSherpaEngine(engine: SttModel["engine"]): boolean {
  return (
    engine === "moonshine" || engine === "sense-voice" || engine === "transducer"
  );
}

// Whether a model actually uses the Language setting. Sherpa engines
// (Moonshine, SenseVoice, Parakeet) ignore it outright — the transcribe path
// never passes it — and Whisper's .en models only ever emit English. Picking a
// language for any of these is a silent no-op, which is exactly how dictating
// Bangla comes back as fluent English: the setting looked applied and was not.
export function honorsLanguage(model?: SttModel): boolean {
  if (!model) return true;
  return !isSherpaEngine(model.engine) && model.multilingual;
}

export const STT_MODELS: SttModel[] = [
  // ── Ranked best first; the Model Store shows them in this order ──
  {
    id: "parakeet-tdt-0.6b-v2",
    file: "parakeet-tdt-0.6b-v2",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
    engine: "transducer",
    family: "Parakeet",
    provider: "NVIDIA",
    label: "Parakeet TDT 0.6B v2 (English)",
    size_mb: 640,
    ram_mb: 1400,
    speed_label: "~8x realtime",
    wer: "~2.5%",
    multilingual: false,
    preset: "accuracy",
    best_for: "Highest accuracy English, punctuation + capitalization",
  },
  {
    id: "parakeet-tdt-0.6b-v3",
    file: "parakeet-tdt-0.6b-v3",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
    engine: "transducer",
    family: "Parakeet",
    provider: "NVIDIA",
    label: "Parakeet TDT 0.6B v3 (Multilingual)",
    size_mb: 640,
    ram_mb: 1400,
    speed_label: "~8x realtime",
    wer: "~3%",
    multilingual: true,
    preset: "accuracy",
    best_for: "Highest accuracy multilingual, punctuation + capitalization",
  },
  {
    id: "large-v3-turbo",
    file: "ggml-large-v3-turbo.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Large v3 Turbo",
    size_mb: 1638,
    ram_mb: 3277,
    speed_label: "~0.5x realtime",
    wer: "~2.9%",
    multilingual: true,
    preset: "accuracy",
    best_for: "Near-best accuracy, faster",
  },
  {
    id: "distil-large-v3.5",
    file: "ggml-distil-large-v3.5.bin",
    family: "Distil-Whisper",
    provider: "Hugging Face",
    label: "Distil Large v3.5",
    size_mb: 756,
    ram_mb: 1500,
    speed_label: "~3x realtime",
    wer: "~3.0%",
    multilingual: false,
    preset: "accuracy",
    best_for: "Fast + accurate English (6x faster than Large)",
    // This repo publishes a bare `ggml-model.bin`, not the ggml-<id>.bin name
    // the other Distil entries use — the local `file` name above is unaffected.
    url: "https://huggingface.co/distil-whisper/distil-large-v3.5-ggml/resolve/main/ggml-model.bin",
  },
  {
    id: "moonshine-base-en",
    file: "moonshine-base-en",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-moonshine-base-en-int8.tar.bz2",
    engine: "moonshine",
    family: "Moonshine",
    provider: "Useful Sensors",
    label: "Moonshine Base (English)",
    size_mb: 251,
    ram_mb: 600,
    speed_label: "~10x realtime",
    wer: "~3.4%",
    multilingual: false,
    preset: "balanced",
    best_for: "Fast + accurate English, auto-punctuation",
  },
  {
    id: "sense-voice-small",
    file: "sense-voice-small", // folder name; not a .bin
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
    engine: "sense-voice",
    family: "SenseVoice",
    provider: "Alibaba FunASR",
    label: "SenseVoice Small (Multilingual)",
    size_mb: 156,
    ram_mb: 500,
    speed_label: "~25x realtime",
    wer: "~5%",
    multilingual: true,
    preset: "balanced",
    best_for: "Fast multilingual (EN/ZH/JA/KO/Yue), auto-punctuation",
  },
  {
    id: "small",
    file: "ggml-small.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Small (Multilingual)",
    size_mb: 466,
    ram_mb: 1024,
    speed_label: "~3x realtime",
    wer: "~3.4%",
    multilingual: true,
    preset: "multilingual",
    best_for: "Good accuracy multilingual",
  },
  {
    id: "base",
    file: "ggml-base.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Base (Multilingual)",
    size_mb: 142,
    ram_mb: 500,
    speed_label: "~7x realtime",
    wer: "~5.5%",
    multilingual: true,
    preset: "balanced",
    best_for: "Balanced multilingual",
  },
  {
    id: "moonshine-tiny-en",
    file: "moonshine-tiny-en", // folder name; not a .bin
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-moonshine-tiny-en-int8.tar.bz2",
    engine: "moonshine",
    family: "Moonshine",
    provider: "Useful Sensors",
    label: "Moonshine Tiny (English)",
    size_mb: 108,
    ram_mb: 320,
    speed_label: "~15x realtime",
    wer: "~5%",
    multilingual: false,
    preset: "speed",
    best_for: "Fastest English, auto-punctuation",
  },
  {
    id: "base.en",
    file: "ggml-base.en.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Base (English)",
    size_mb: 142,
    ram_mb: 500,
    speed_label: "~7x realtime",
    wer: "~5.5%",
    multilingual: false,
    preset: "balanced",
    best_for: "Balanced English",
  },
  {
    id: "tiny.en",
    file: "ggml-tiny.en.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Tiny (English)",
    size_mb: 75,
    ram_mb: 390,
    speed_label: "~10x realtime",
    wer: "~8%",
    multilingual: false,
    preset: "speed",
    best_for: "Speed mode, quick notes",
  },
  {
    id: "tiny",
    file: "ggml-tiny.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Tiny (Multilingual)",
    size_mb: 75,
    ram_mb: 390,
    speed_label: "~10x realtime",
    wer: "~8%",
    multilingual: true,
    preset: "speed",
    best_for: "Speed + multilingual",
  },

  // ── One language each: shown under "More languages" unless it is the user's ──
  {
    id: "bangla-small",
    file: "ggml-bangla-small.bin",
    url: "https://huggingface.co/afridee/banglaasr-ggml/resolve/main/ggml-whisper-small-bangla-q5_0.bin",
    family: "Whisper",
    provider: "Bangla Speech Processing",
    label: "Bangla Small (Bengali)",
    size_mb: 167,
    ram_mb: 1024,
    speed_label: "~3x realtime",
    wer: "unmeasured",
    multilingual: true,
    preset: "multilingual",
    best_for: "Bengali dictation, small download",
  },
  {
    id: "bangla-medium",
    file: "ggml-bangla-medium.bin",
    url: "https://huggingface.co/SayedShaun/bengali-whisper-medium-ggml/resolve/main/ggml-model-q4_0.bin",
    family: "Whisper",
    provider: "SayedShaun",
    label: "Bangla Medium (Bengali)",
    size_mb: 424,
    ram_mb: 2662,
    speed_label: "~1x realtime",
    wer: "unmeasured",
    multilingual: true,
    preset: "multilingual",
    best_for: "Bengali dictation, best accuracy",
  },
  {
    id: "belle-whisper-large-v3-turbo-zh",
    file: "ggml-belle-whisper-large-v3-turbo-zh.bin",
    family: "Whisper",
    provider: "BELLE-2",
    label: "Belle Whisper Large v3 Turbo (Chinese)",
    size_mb: 1625,
    ram_mb: 3277,
    speed_label: "~0.5x realtime",
    wer: "~2.9%",
    multilingual: false,
    preset: "accuracy",
    best_for: "Best-in-class Mandarin Chinese accuracy",
    url: "https://huggingface.co/BELLE-2/Belle-whisper-large-v3-turbo-zh-ggml/resolve/main/ggml-model.bin",
  },
  {
    id: "kotoba-whisper-v2.0-q5_0",
    file: "ggml-kotoba-whisper-v2.0-q5_0.bin",
    family: "Distil-Whisper",
    provider: "Kotoba Technologies",
    label: "Kotoba Whisper v2.0 Q5 (Japanese)",
    size_mb: 538,
    ram_mb: 1100,
    speed_label: "~2x realtime",
    wer: "~3.1%",
    multilingual: false,
    preset: "balanced",
    best_for: "Lightweight fast Japanese ASR",
    url: "https://huggingface.co/kotoba-tech/kotoba-whisper-v2.0-ggml/resolve/main/ggml-kotoba-whisper-v2.0-q5_0.bin",
  },
  {
    id: "whisper-medium-korean-royshilkrot",
    file: "ggml-whisper-medium-korean-royshilkrot.bin",
    family: "Whisper",
    provider: "royshilkrot",
    label: "Whisper Medium (Korean)",
    size_mb: 1534,
    ram_mb: 2662,
    speed_label: "~1x realtime",
    wer: "~3%",
    multilingual: false,
    preset: "accuracy",
    best_for: "Solid general-purpose Korean ASR",
    url: "https://huggingface.co/royshilkrot/whisper-medium-korean-ggml/resolve/main/whisper-medium-korean.bin",
  },
  {
    id: "phowhisper-medium-vi",
    file: "ggml-phowhisper-medium-vi.bin",
    family: "Whisper",
    provider: "VinAI Research",
    label: "PhoWhisper Medium (Vietnamese)",
    size_mb: 1534,
    ram_mb: 2662,
    speed_label: "~1x realtime",
    wer: "~4%",
    multilingual: false,
    preset: "accuracy",
    best_for: "Best-in-class Vietnamese ASR across accents",
    url: "https://huggingface.co/dongxiat/ggml-PhoWhisper-medium/resolve/main/ggml-PhoWhisper-medium.bin",
  },
  {
    id: "whisper-large-v3-arabic-q5_0",
    file: "ggml-whisper-large-v3-arabic-q5_0.bin",
    family: "Whisper",
    provider: "Pomni",
    label: "Whisper Large v3 Q5 (Arabic)",
    size_mb: 1081,
    ram_mb: 2100,
    speed_label: "~0.8x realtime",
    wer: "~3.8%",
    multilingual: false,
    preset: "accuracy",
    best_for: "High-quality Arabic ASR, moderate RAM",
    url: "https://huggingface.co/Pomni/whisper-large-v3-arabic-ggml-allquants/resolve/main/ggml-large-v3-arabic-q5_0.bin",
  },
  {
    id: "whisper-medium-hebrew-imvladikon",
    file: "ggml-whisper-medium-hebrew-imvladikon.bin",
    family: "Whisper",
    provider: "imvladikon",
    label: "Whisper Medium (Hebrew)",
    size_mb: 1534,
    ram_mb: 2662,
    speed_label: "~1x realtime",
    wer: "~4%",
    multilingual: false,
    preset: "accuracy",
    best_for: "Best-available Hebrew ASR",
    url: "https://huggingface.co/imvladikon/whisper-medium-he/resolve/main/ggml-hebrew.bin",
  },
  {
    id: "nb-whisper-base-norwegian-q5_0",
    file: "ggml-nb-whisper-base-norwegian-q5_0.bin",
    family: "Whisper",
    provider: "National Library of Norway AI Lab",
    label: "NB-Whisper Base Q5 (Norwegian)",
    size_mb: 55,
    ram_mb: 300,
    speed_label: "~10x realtime",
    wer: "~5.5%",
    multilingual: false,
    preset: "speed",
    best_for: "Extremely lightweight Norwegian ASR",
    url: "https://huggingface.co/NbAiLabBeta/nb-whisper-base/resolve/main/ggml-model-q5_0.bin",
  },
];

// ============================================================
// LLM models (AI processing) — from build plan §5
// ============================================================
export const LLM_MODELS: LlmModel[] = [
  // ── Tiny tier (≤ 2B, runs anywhere) ───────────────────────
  {
    id: "llama-3.2-1b-instruct-q4",
    name: "Llama 3.2 1B Instruct",
    provider: "Meta",
    url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    params: "1B",
    size_mb: 770,
    ram_gb: 1.5,
    tier: "tiny",
    speed_label: "~20 tok/s CPU",
    languages: "Multilingual",
    license: "Llama 3.2",
    best_for: "Fast cleanup, quick edits",
  },
  {
    id: "gemma-3-1b-it-q4",
    name: "Gemma 3 1B",
    provider: "Google",
    url: "https://huggingface.co/ggml-org/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf",
    params: "1B",
    size_mb: 769,
    ram_gb: 1.5,
    tier: "tiny",
    speed_label: "~22 tok/s CPU",
    languages: "Multilingual",
    license: "Gemma",
    best_for: "Basic cleanup",
  },

  // ── Small tier (2B–4B, 4GB+ RAM) ─────────────────────────
  {
    id: "llama-3.2-3b-instruct-q4",
    name: "Llama 3.2 3B Instruct",
    provider: "Meta",
    url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    params: "3B",
    size_mb: 1926,
    ram_gb: 3.5,
    tier: "small",
    speed_label: "~10 tok/s CPU",
    languages: "Multilingual",
    license: "Llama 3.2",
    best_for: "Balanced quality cleanup",
  },
  {
    id: "gemma-3-4b-it-q4",
    name: "Gemma 3 4B",
    provider: "Google",
    url: "https://huggingface.co/bartowski/google_gemma-3-4b-it-GGUF/resolve/main/google_gemma-3-4b-it-Q4_K_M.gguf",
    params: "4B",
    size_mb: 2500,
    ram_gb: 4.0,
    tier: "small",
    speed_label: "~7 tok/s CPU",
    languages: "Multilingual",
    license: "Gemma",
    best_for: "Excellent instruction following",
  },
  {
    id: "phi-4-mini-instruct-q4",
    name: "Phi-4-mini Instruct",
    provider: "Microsoft",
    url: "https://huggingface.co/bartowski/microsoft_Phi-4-mini-instruct-GGUF/resolve/main/microsoft_Phi-4-mini-instruct-Q4_K_M.gguf",
    params: "3.8B",
    size_mb: 2490,
    ram_gb: 4.0,
    tier: "small",
    speed_label: "~7 tok/s CPU",
    languages: "English+",
    license: "MIT",
    best_for: "Advanced reasoning in a small model",
  },
  {
    id: "qwen3-4b-q4",
    name: "Qwen3 4B",
    provider: "Alibaba",
    url: "https://huggingface.co/bartowski/Qwen_Qwen3-4B-GGUF/resolve/main/Qwen_Qwen3-4B-Q4_K_M.gguf",
    params: "4B",
    size_mb: 2600,
    ram_gb: 4.0,
    tier: "small",
    speed_label: "~7 tok/s CPU",
    languages: "Multilingual",
    license: "Apache-2.0",
    best_for: "Latest-gen multilingual processing",
  },

  // ── Medium tier (7B–9B, 8GB+ RAM) ────────────────────────
  {
    id: "llama-3.1-8b-instruct-q4",
    name: "Llama 3.1 8B Instruct",
    provider: "Meta",
    url: "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
    params: "8B",
    size_mb: 4920,
    ram_gb: 8.0,
    tier: "medium",
    speed_label: "~5 tok/s CPU",
    languages: "Multilingual",
    license: "Llama 3.1",
    best_for: "High-quality general-purpose editing",
  },
  {
    id: "qwen3-8b-q4",
    name: "Qwen3 8B",
    provider: "Alibaba",
    url: "https://huggingface.co/bartowski/Qwen_Qwen3-8B-GGUF/resolve/main/Qwen_Qwen3-8B-Q4_K_M.gguf",
    params: "8B",
    size_mb: 4900,
    ram_gb: 8.0,
    tier: "medium",
    speed_label: "~4 tok/s CPU",
    languages: "Multilingual",
    license: "Apache-2.0",
    best_for: "Latest-gen thinking & reasoning",
  },
];

// ============================================================
// TTS voices (Piper) — read-aloud. Every entry verified against the
// rhasspy/piper-voices HF tree (en/<region>/<voice>/<tier>/<id>.onnx[.json]).
// Each voice needs BOTH files; download_tts_model fetches the pair.
// Tier naming: Piper low → "fast", medium → "balanced", high → "natural".
// ============================================================
const PIPER_BASE = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

const TIER: Record<string, TtsQuality> = {
  low: "fast",
  medium: "balanced",
  high: "natural",
};

function piperVoice(
  region: "en_US" | "en_GB",
  voice: string,
  tier: "low" | "medium" | "high",
  gender: "female" | "male" | "unknown",
  sizeMb: number,
  name: string
): TtsModel {
  const id = `${region}-${voice}-${tier}`;
  const base = `${PIPER_BASE}/en/${region}/${voice}/${tier}/${id}.onnx`;
  const accent = region === "en_US" ? "US" : "UK";
  return {
    id,
    label: `${name} (${accent}, ${gender === "unknown" ? "voice" : gender})`,
    gender,
    accent,
    language: `English (${accent})`,
    engine: "piper",
    quality: TIER[tier],
    size_mb: sizeMb,
    url_onnx: base,
    url_json: `${base}.json`,
  };
}

// Non-English Piper voice, e.g. piperVoiceLang("de", "de_DE", "thorsten", "high", "male", 109, "German").
function piperVoiceLang(
  langCode: string,
  region: string,
  voice: string,
  tier: "low" | "medium" | "high",
  gender: "female" | "male" | "unknown",
  sizeMb: number,
  language: string
): TtsModel {
  const id = `${region}-${voice}-${tier}`;
  const base = `${PIPER_BASE}/${langCode}/${region}/${voice}/${tier}/${id}.onnx`;
  return {
    id,
    label: `${voice[0].toUpperCase()}${voice.slice(1)} (${language}, ${gender === "unknown" ? "voice" : gender})`,
    gender,
    accent: "US", // unused for non-English voices — language carries the real info
    language,
    engine: "piper",
    quality: TIER[tier],
    size_mb: sizeMb,
    url_onnx: base,
    url_json: `${base}.json`,
  };
}

export const TTS_MODELS: TtsModel[] = [
  // ── US voices ─────────────────────────────────────────────
  piperVoice("en_US", "amy", "medium", "female", 61, "Amy"),
  piperVoice("en_US", "ryan", "high", "male", 116, "Ryan HD"),
  piperVoice("en_US", "lessac", "high", "female", 109, "Lessac HD"),
  piperVoice("en_US", "hfc_female", "medium", "female", 61, "Clara"),
  piperVoice("en_US", "hfc_male", "medium", "male", 61, "Henry"),
  // ── UK voices ─────────────────────────────────────────────
  piperVoice("en_GB", "alan", "medium", "male", 61, "Alan"),
  piperVoice("en_GB", "cori", "high", "female", 109, "Cori HD"),
  piperVoice("en_GB", "alba", "medium", "female", 61, "Alba (Scottish)"),

  // ── Other languages ──────────────────────────────────────────
  // Verified against rhasspy/piper-voices on Hugging Face (every .onnx and
  // .onnx.json URL checked with curl -I -L → 200 before being added — see
  // §15 of CLAUDE.md, the same invariant applies to TTS as STT).
  //
  // NOTE on Bangla: researched thoroughly (agy deep research + manual check
  // of rhasspy/piper-voices' voices.json) — Piper has NO Bangla voice, and no
  // community Piper-format (.onnx + .onnx.json) Bangla model exists either.
  // The closest things are VITS ONNX models built for the Mycroft Mimic3 /
  // Coqui engines (e.g. huggingface.co/csukuangfj/vits-mimic3-bn-multi_low),
  // but they use a different config schema and are NOT plug-and-play with
  // Piper's CLI — adding them here would silently break on synth. Bangla
  // would require either training a real Piper voice from scratch (e.g. on
  // OpenSLR SLR37) or integrating a second TTS engine (sherpa-onnx) — out of
  // scope for a catalog entry.
  piperVoiceLang("de", "de_DE", "thorsten", "high", "male", 109, "German"),
  piperVoiceLang("es", "es_AR", "daniela", "high", "female", 109, "Spanish (Argentina)"),
  piperVoiceLang("fr", "fr_FR", "tom", "medium", "male", 61, "French"),
  piperVoiceLang("hi", "hi_IN", "priyamvada", "medium", "female", 61, "Hindi"),
  piperVoiceLang("zh", "zh_CN", "xiao_ya", "medium", "female", 60, "Chinese"),
  piperVoiceLang("ar", "ar_JO", "kareem", "medium", "male", 60, "Arabic"),
  piperVoiceLang("pt", "pt_BR", "faber", "medium", "male", 60, "Portuguese (Brazil)"),
  piperVoiceLang("ru", "ru_RU", "irina", "medium", "female", 60, "Russian"),

  // ── Kokoro (sherpa-onnx engine) ─────────────────────────────
  // #1 open-weights TTS (tops TTS Arena). One archive = 11 voices, picked by
  // speaker index. url_json '' => downloaded + extracted as a sherpa archive;
  // the backend detects Kokoro by voices.bin and selects a voice via id#sid.
  {
    id: 'kokoro-int8-en-v0_19',
    label: 'Kokoro (English)',
    gender: 'female',
    accent: 'US',
    language: 'English (US)',
    quality: 'natural',
    size_mb: 98,
    engine: 'sherpa',
    url_onnx: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-en-v0_19.tar.bz2',
    url_json: '',
    voices: [
      { sid: 0, label: 'American female (default)', gender: 'female' },
      { sid: 1, label: 'Bella (US)', gender: 'female' },
      { sid: 2, label: 'Nicole (US)', gender: 'female' },
      { sid: 3, label: 'Sarah (US)', gender: 'female' },
      { sid: 4, label: 'Sky (US)', gender: 'female' },
      { sid: 5, label: 'Adam (US)', gender: 'male' },
      { sid: 6, label: 'Michael (US)', gender: 'male' },
      { sid: 7, label: 'Emma (UK)', gender: 'female' },
      { sid: 8, label: 'Isabella (UK)', gender: 'female' },
      { sid: 9, label: 'George (UK)', gender: 'male' },
      { sid: 10, label: 'Lewis (UK)', gender: 'male' },
    ],
  },
  // Full-precision (fp32) build of the same 11-voice Kokoro model — bigger on
  // disk, no int8 quantization, slightly cleaner audio. Same voices/sids.
  {
    id: 'kokoro-en-v0_19',
    label: 'Kokoro (English, HD)',
    gender: 'female',
    accent: 'US',
    language: 'English (US)',
    quality: 'natural',
    size_mb: 305,
    engine: 'sherpa',
    url_onnx: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-en-v0_19.tar.bz2',
    url_json: '',
    voices: [
      { sid: 0, label: 'American female (default)', gender: 'female' },
      { sid: 1, label: 'Bella (US)', gender: 'female' },
      { sid: 2, label: 'Nicole (US)', gender: 'female' },
      { sid: 3, label: 'Sarah (US)', gender: 'female' },
      { sid: 4, label: 'Sky (US)', gender: 'female' },
      { sid: 5, label: 'Adam (US)', gender: 'male' },
      { sid: 6, label: 'Michael (US)', gender: 'male' },
      { sid: 7, label: 'Emma (UK)', gender: 'female' },
      { sid: 8, label: 'Isabella (UK)', gender: 'female' },
      { sid: 9, label: 'George (UK)', gender: 'male' },
      { sid: 10, label: 'Lewis (UK)', gender: 'male' },
    ],
  },
  // MeloTTS — VITS, but 44.1 kHz output (vs Kokoro's 24 kHz), so crisper/
  // higher-fidelity. Runs on the existing sherpa VITS path (model.onnx +
  // tokens.txt + lexicon.txt, no voices.bin), single default English voice.
  {
    id: 'vits-melo-tts-en',
    label: 'MeloTTS (English, 44kHz)',
    gender: 'female',
    accent: 'US',
    language: 'English (US)',
    quality: 'natural',
    size_mb: 155,
    engine: 'sherpa',
    url_onnx: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-melo-tts-en.tar.bz2',
    url_json: '',
  },

  // ── Bangla (sherpa-onnx engine) ─────────────────────────────
  // Piper has no Bangla voices, so these run on the second bundled engine
  // (sherpa-onnx, sidecars/sherpa/). They're distributed as .tar.bz2 archives
  // by k2-fsa whose top-level folder equals the id — Rust extracts them into
  // %APPDATA%\SilentVoice\tts\<id>\. url_json is intentionally "" — that's
  // how download_tts_model (Rust) tells a sherpa archive from a Piper pair.
  // Both URLs verified with curl -I -L → 200; synthesis end-to-end tested
  // with sherpa-onnx-offline-tts.exe on real Bengali text before adding.
  {
    id: "vits-coqui-bn-custom_female",
    label: "Mitra (Bangla, female)",
    gender: "female",
    accent: "US",
    language: "Bangla",
    quality: "natural",
    size_mb: 103,
    engine: "sherpa",
    url_onnx:
      "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-coqui-bn-custom_female.tar.bz2",
    url_json: "",
  },
  // Meta MMS Bengali (sherpa-ready ONNX conversion). Distributed as two loose
  // files, not an archive — url_json pointing at tokens.txt is the marker the
  // Rust downloader uses for the two-file sherpa layout (dir/model.onnx +
  // dir/tokens.txt). Trained by Meta on much more data than the community
  // Coqui voice — offered as the higher-quality Bangla option.
  {
    id: "mms-tts-bengali",
    label: "Meta MMS (Bangla, female)",
    gender: "female",
    accent: "US",
    language: "Bangla",
    quality: "natural",
    size_mb: 109,
    engine: "sherpa",
    url_onnx:
      "https://huggingface.co/willwade/mms-tts-multilingual-models-onnx/resolve/main/ben/model.onnx",
    url_json:
      "https://huggingface.co/willwade/mms-tts-multilingual-models-onnx/resolve/main/ben/tokens.txt",
  },
  // NOTE: vits-mimic3-bn-multi_low was tried and REMOVED — its espeak-based
  // phonemization pipeline fails on Bengali script (produces a sub-second
  // gibberish blip; verified: 11 KB WAV vs Mitra's 645 KB for the same
  // sentence). Do not re-add it without solving that.
];

// Language-appropriate "Play sample" sentences for the Settings test button —
// a voice model can only pronounce its own language's script; feeding the
// English sample to e.g. a Bangla voice produces pure gibberish.
export const TTS_SAMPLE_TEXT: Record<string, string> = {
  default: "Hello! This is my voice reading your text aloud for you.",
  Bangla: "হ্যালো! এটি আমার কণ্ঠস্বর, যা আপনার লেখাটি জোরে পড়ে শোনাচ্ছে।",
  German: "Hallo! Das ist meine Stimme, die Ihren Text laut vorliest.",
  "Spanish (Argentina)": "¡Hola! Esta es mi voz leyendo tu texto en voz alta.",
  Polish: "Cześć! To jest mój głos, który głośno czyta Twój tekst.",
  Ukrainian: "Привіт! Це мій голос, який голосно читає ваш текст.",
  Kazakh: "Сәлем! Бұл сіздің жазған мәтініңізді дауыстап оқып жатқан дауысым.",
  Icelandic: "Halló! Þetta er röddin mín að lesa textann þinn upphátt.",
  Dutch: "Hallo! Dit is mijn stem die jouw tekst hardop voorleest.",
  French: "Bonjour ! Voici ma voix qui lit votre texte à haute voix.",
  Italian: "Ciao! Sono la tua voce che legge questo testo ad alta voce.",
  Hindi: "नमस्कार! यह मेरी आवाज़ है जो आपके पाठ को पढ़ रही है।",
  Greek: "Γεια σας! Αυτή είναι η φωνή μου που διαβάζει το κείμενό σας.",
  Chinese: "你好！这是我的声音在为你朗读这段文字。",
  Bulgarian: "Здравейте! Това е моят глас, който чете вашия текст на глас.",
  Arabic: "مرحباً! هذا هو صوتي وهو يقرأ نصك بصوت عالٍ.",
  "Portuguese (Brazil)": "Olá! Esta é minha voz lendo seu texto em voz alta.",
  Russian: "Привет! Это мой голос, который читает ваш текст вслух.",
  Vietnamese: "Xin chào! Đây là giọng tôi đang đọc văn bản của bạn.",
  Turkish: "Merhaba! Bu, metninizi yüksek sesle okuyan benim sesimdir.",
  Czech: "Ahoj! Tohle je můj hlas, který nahlas čte tvůj text.",
};

// Human-readable language of an STT model, derived from its label/flags — used
// to power the Model Store language filter. Language-specific models put the
// language in parentheses in their label (e.g. "… (Chinese)"); OpenAI base
// models are either "(English)"/"(Multilingual)" or the plain large models
// (multilingual). Distil models are English.
export function sttLanguage(m: SttModel): string {
  const paren = m.label.match(/\(([^)]+)\)/);
  if (paren) {
    const inner = paren[1].trim();
    if (/multilingual/i.test(inner)) return "Multilingual";
    // "Quranic Arabic" -> "Arabic" so it groups with other Arabic models.
    return inner.replace(/^Quranic\s+/i, "");
  }
  return m.multilingual ? "Multilingual" : "English";
}

// Language-specific models the Model Store ranks up for this user. From the
// system language and the time zone (Windows is often English even where
// people dictate in another language); both are read offline.
const BY_CODE: Record<string, string> = {
  bn: "Bengali", ar: "Arabic", zh: "Chinese", ja: "Japanese", ko: "Korean", vi: "Vietnamese", he: "Hebrew", iw: "Hebrew", nb: "Norwegian", nn: "Norwegian", no: "Norwegian",
};
const BY_ZONE: Record<string, string> = {
  "Asia/Dhaka": "Bengali",
  "Asia/Riyadh": "Arabic", "Asia/Dubai": "Arabic", "Africa/Cairo": "Arabic", "Asia/Baghdad": "Arabic", "Asia/Kuwait": "Arabic", "Asia/Qatar": "Arabic",
  "Asia/Bahrain": "Arabic", "Asia/Muscat": "Arabic", "Asia/Amman": "Arabic", "Asia/Beirut": "Arabic", "Asia/Damascus": "Arabic", "Asia/Aden": "Arabic",
  "Africa/Casablanca": "Arabic", "Africa/Algiers": "Arabic", "Africa/Tunis": "Arabic", "Africa/Tripoli": "Arabic", "Africa/Khartoum": "Arabic",
  "Asia/Shanghai": "Chinese", "Asia/Chongqing": "Chinese", "Asia/Urumqi": "Chinese", "Asia/Taipei": "Chinese", "Asia/Hong_Kong": "Chinese", "Asia/Macau": "Chinese",
  "Asia/Tokyo": "Japanese", "Asia/Seoul": "Korean", "Asia/Ho_Chi_Minh": "Vietnamese", "Asia/Saigon": "Vietnamese",
  "Asia/Jerusalem": "Hebrew", "Asia/Tel_Aviv": "Hebrew", "Europe/Oslo": "Norwegian",
};

export function localLanguages(
  locales: readonly string[] = typeof navigator === "undefined" ? [] : navigator.languages ?? [navigator.language],
  zone: string = Intl.DateTimeFormat().resolvedOptions().timeZone ?? ""
): Set<string> {
  const out = new Set<string>();
  for (const l of locales) {
    const name = BY_CODE[l.toLowerCase().split("-")[0]];
    if (name) out.add(name);
  }
  if (BY_ZONE[zone]) out.add(BY_ZONE[zone]);
  return out;
}

// Top 20 prioritized languages — build plan §10
export const LANGUAGES: { code: string; name: string }[] = [
  { code: "auto", name: "Auto-detect" },
  { code: "en", name: "English" },
  { code: "zh", name: "Mandarin Chinese" },
  { code: "hi", name: "Hindi" },
  { code: "es", name: "Spanish" },
  { code: "fr", name: "French" },
  { code: "ar", name: "Arabic" },
  { code: "bn", name: "Bengali" },
  { code: "pt", name: "Portuguese" },
  { code: "ru", name: "Russian" },
  { code: "ja", name: "Japanese" },
  { code: "de", name: "German" },
  { code: "ko", name: "Korean" },
  { code: "tr", name: "Turkish" },
  { code: "vi", name: "Vietnamese" },
  { code: "it", name: "Italian" },
  { code: "th", name: "Thai" },
  { code: "nl", name: "Dutch" },
  { code: "pl", name: "Polish" },
  { code: "uk", name: "Ukrainian" },
  { code: "id", name: "Indonesian" },
];
