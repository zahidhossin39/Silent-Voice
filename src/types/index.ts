// ============================================================
// Silent Voice — Shared TypeScript types
// ============================================================

export type CompatibilityLevel = "good" | "warn" | "bad";

// ---------- Hardware ----------
export interface HardwareInfo {
  cpu_brand: string;
  physical_cores: number;
  logical_cores: number;
  total_ram_gb: number;
  available_ram_gb: number;
  has_avx2: boolean;
  has_avx512: boolean;
  gpu_vendor: string | null;
  gpu_name: string | null;
  gpu_vram_gb: number | null;
  free_disk_gb: number;
  os: string;
}

export interface DeviceRecommendation {
  tier: "low" | "balanced" | "high";
  stt_size: string;
  high_performance: boolean;
  performance_threads: number;
  coedit_enabled: boolean;
  use_gpu: boolean;
  reason: string;
}

// ---------- STT (Whisper) models ----------
export type SttPreset = "speed" | "balanced" | "accuracy" | "multilingual";

export interface SttModel {
  id: string; // e.g. "small.en"
  file: string; // e.g. "ggml-small.en.bin"
  family: string; // "Whisper"
  provider: string; // "OpenAI"
  label: string; // "Small (English)"
  size_mb: number;
  ram_mb: number;
  speed_label: string; // "~3x realtime"
  wer: string; // "~3.4%"
  multilingual: boolean;
  preset: SttPreset;
  best_for: string;
  url?: string; // Optional full download URL (overrides WHISPER_BASE_URL + file)
  // Which engine runs this model. Absent = "whisper" (whisper.cpp, .bin file).
  // "moonshine"/"sense-voice" = sherpa-onnx, downloaded as a .tar.bz2 archive
  // (`url`) and extracted into a folder. Faster + natively punctuated;
  // Moonshine is English-only, SenseVoice is multilingual.
  engine?: "whisper" | "moonshine" | "sense-voice" | "transducer";
}

// ---------- LLM models (for AI processing) ----------
export type ModelTier = "tiny" | "small" | "medium" | "large";  // medium = 7-8B models

export interface LlmModel {
  id: string; // GGUF storage id (filename stem), e.g. "phi-3.5-mini-instruct-q4"
  name: string;
  provider: string; // "Google", "Alibaba", "Microsoft", "Meta", ...
  url: string; // direct GGUF download URL
  params: string; // "3.8B"
  size_mb: number; // download size
  ram_gb: number;
  tier: ModelTier;
  speed_label: string;
  languages: string;
  license: string;
  best_for: string;
}

// ---------- TTS (Piper read-aloud) voices ----------
export type TtsQuality = "fast" | "balanced" | "natural";

export interface TtsModel {
  id: string; // Piper voice id (e.g. "en_US-amy-medium") or sherpa archive stem (e.g. "vits-coqui-bn-custom_female")
  label: string; // "Amy (US, female)"
  gender: "female" | "male" | "unknown";
  accent: "US" | "UK"; // legacy — only meaningful for English voices
  language: string; // display language, e.g. "English (US)", "German", "Bangla"
  quality: TtsQuality; // low → fast, medium → balanced, high → natural
  size_mb: number;
  engine: "piper" | "sherpa"; // which bundled TTS engine synthesizes this voice
  url_onnx: string; // piper: .onnx URL · sherpa: .tar.bz2 archive URL
  url_json: string; // piper: .onnx.json URL · sherpa: "" (everything is in the archive)
  // Kokoro archives bundle many voices, each picked by speaker index (sid).
  // Present only for Kokoro; absent = single-voice model.
  voices?: { sid: number; label: string; gender: 'female' | 'male' }[];
}

export interface PiperVoice {
  key: string;
  name: string;
  language_code: string;
  language_english: string;
  country_english: string;
  quality: string;
  num_speakers: number;
  onnx_path: string;
  onnx_size_bytes: number;
  json_path: string;
}

// ---------- Download state ----------
export type DownloadStatus =
  | "not_downloaded"
  | "downloading"
  | "downloaded"
  | "paused"
  | "cancelled"
  | "error";

export interface DownloadProgress {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  status: DownloadStatus;
  error?: string;
}

// ---------- AI processing modes ----------
export type ModelSource = "local" | "api" | "none";

export interface Mode {
  id: string;
  name: string;
  icon: string;
  system_prompt: string;
  description?: string; // plain-language outcome shown on the card, instead of the prompt
  model_source: ModelSource; // "none" => raw transcription, no LLM
  model_id: string;
  provider_id?: string; // for model_source "api": which ApiProvider to use
  hotkey?: string;
  builtin: boolean;
  pinned?: boolean;
  // Set when a user edits a built-in mode. Marks it as no longer safe to
  // silently refresh from BUILTIN_MODES when the app ships a prompt fix —
  // the user's own edit wins instead.
  customized?: boolean;
}

// ---------- API providers ----------
export type ApiUse = "stt" | "llm";

export interface ApiProvider {
  id: string;
  name: string; // "OpenAI", "OpenRouter", "Anthropic", ...
  api_key: string;
  base_url: string;
  model: string; // chat/completions model, used for "llm" (AI processing)
  stt_model: string; // audio/transcriptions model, used for "stt" (cloud Whisper)
  uses: ApiUse[];
}

// ---------- Text replacements (spoken trigger → inserted text) ----------
// e.g. { trigger: "my email", replacement: "zahidhosson28@gmail.com" }.
// Applied to the transcript after AI processing, just before pasting.
export interface TextReplacement {
  id: string;
  trigger: string;
  replacement: string;
}

// ---------- Per-app profiles ----------
// When the focused app's exe name contains `app_match`, use `mode_id` instead
// of the globally active mode. e.g. { app_match: "code", mode_id: "raw" }.
export interface AppProfileRule {
  id: string;
  app_match: string;
  mode_id: string;
}

// ---------- History ----------
export interface HistoryEntry {
  id: number;
  timestamp: number; // unix ms
  raw_text: string;
  processed_text: string;
  mode_id: string;
  model_id: string;
  duration_ms: number;
  audio_ms?: number;
  audio_file?: string;
}

// ---------- App settings ----------
export type RecordingState =
  | "idle"
  | "listening"
  | "recording"
  | "processing"
  | "done"; // brief success beat after text lands at the cursor, then back to idle

export interface Settings {
  hotkey: string;
  active_stt_model: string;
  active_mode_id: string;
  stt_preset: SttPreset;
  language: string; // "auto" or ISO code
  use_gpu: boolean;
  high_performance: boolean;
  performance_threads: number; // 0 = auto (all cores); only used when high_performance
  audio_device: string | null;
  auto_start: boolean;
  theme: "dark" | "light";
  custom_vocabulary: string; // comma/newline-separated words fed to whisper.cpp as a priming prompt
  stt_cloud_provider_id: string | null; // null = use local active_stt_model; else an ApiProvider.id with uses including "stt"
  toggle_mode: boolean; // double-tap the hotkey to lock recording on; single press stops
  input_sensitivity: number; // 0-100 (Discord-style): how loud a sound must be to count as speech
  inline_proofread: boolean; // squiggles under spelling/grammar errors in any app's text field (English)
  coedit_enabled: boolean; // AI grammar correction of dictated text (CoEdIT); off = raw transcript
  chunk_on_silence: boolean; // background transcription at natural silence boundaries
  proofread_disabled_rules: string[]; // Harper rule ids the user turned off (plus our "Filler" pseudo-rule)
  gector_sensitivity: "relaxed" | "balanced" | "aggressive";
  proofread_ignore_apps: string; // comma-separated exe-name substrings where squiggles are suppressed
  pill_auto_hide: boolean; // hide the recording pill ~5s after idle, reappear on next hotkey press
  append_trailing_space: boolean; // add a space after pasted text so the next dictation has a gap
  active_tts_voice: string | null; // Piper voice id for read-aloud; null = none selected
  tts_hotkey: string; // global hotkey that reads the current text selection aloud
  tts_enabled: boolean; // read-aloud on/off; when false the hotkey is unregistered
  onboarded: boolean; // true once the first-launch setup wizard has been completed/skipped
  pinned_stt: string[];
  pinned_llm: string[];
  pinned_tts: string[];
  hf_show_incompatible: boolean;
  history_limit: number; // max transcriptions to keep (hard count cap)
  blur_history: boolean; // hide transcript text until hover — history is visible on screen shares
  history_retention: "never" | "3d" | "2w" | "3m" | "custom"; // auto-delete entries older than this
  history_retention_custom_value: number; // used when history_retention === "custom"
  history_retention_custom_unit: "hours" | "days" | "weeks" | "months";
  save_audio: boolean;      // keep a copy of the audio for each dictation
  audio_clip_limit: number; // how many recordings to keep
  model_unload_minutes: number; // idle minutes before the STT model is freed; 0 = never
  popup_theme: PopupTheme;  // accent palette for the inline-proofread suggestion popup

  popup_surface: PopupSurface;  // card background for the inline-proofread popup
  app_theme: AppTheme;
}

// Accent palettes for the grammar-suggestion popup (surface stays dark; only
// the accent swaps). Order matches squiggle.rs PALETTES.
export type PopupTheme = "violet" | "teal" | "amber-blue" | "orange" | "brightness";
export type PopupSurface = "dark" | "light";

export type AppTheme = "orange" | "ocean" | "iris" | "reef" | "rose" | "meadow";

// ---------- Hugging Face ----------
export interface HfSearchItem {
  id: string;
  downloads: number;
  likes: number;
  last_modified: string;
  tags: string[];
  pipeline_tag: string | null;
  gated: boolean;
  params_b: number | null;
  siblings_gguf: number;
}

export interface HfModelDetails {
  id: string;
  downloads: number;
  likes: number;
  last_modified: string;
  tags: string[];
  pipeline_tag: string | null;
  gated: boolean;
  arch: string | null;
  params_b: number | null;
  context_length: number | null;
  has_tools: boolean;
  files: HfFile[];
  readme: string;
}

export interface HfFile {
  name: string;
  size_bytes: number;
}
