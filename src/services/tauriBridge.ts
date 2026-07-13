// ============================================================
// Tauri bridge — safely calls Rust commands when running inside
// Tauri, and returns sensible mock data when running in a plain
// browser (Vite dev preview before the Rust backend is built).
// ============================================================
import type { HardwareInfo, HistoryEntry } from "../types";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const mod = await import("@tauri-apps/api/core");
  return mod.invoke<T>(cmd, args);
}

/** Subscribe to a Tauri event; returns an unlisten fn (no-op in browser). */
export async function listenEvent<T>(
  event: string,
  handler: (payload: T) => void
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<T>(event, (e) => handler(e.payload));
  return unlisten;
}

// ---------------- Overlay window self-control ----------------

/** Resize + re-center the overlay window (handled in Rust, bottom-anchored). */
export async function setOverlaySize(w: number, h: number): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_overlay_size", { width: w, height: h });
  } catch (e) {
    console.warn("set_overlay_size failed", e);
  }
}

/** Hide the overlay via the backend (sets the user-hidden flag so the
 * keep-alive loop won't bring it back). Re-show via the tray menu. */
export async function hideSelfWindow(): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("hide_overlay");
  } catch (e) {
    console.warn("hide_overlay failed", e);
  }
}

/** Broadcast the overlay opacity (0-100) to the overlay window. */
export async function setOverlayOpacity(value: number): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_overlay_opacity", { value });
  } catch (e) {
    console.warn("set_overlay_opacity failed", e);
  }
}

export async function quitApp(): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("quit_app");
  } catch (e) {
    console.warn("quit_app failed", e);
  }
}

// Mock hardware used when the Rust backend isn't available yet.
const MOCK_HARDWARE: HardwareInfo = {
  cpu_brand: "Intel(R) Core(TM) i7-8650U CPU @ 1.90GHz",
  physical_cores: 4,
  logical_cores: 8,
  total_ram_gb: 16,
  available_ram_gb: 9.2,
  has_avx2: true,
  has_avx512: false,
  gpu_vendor: "Intel",
  gpu_name: "Intel(R) UHD Graphics 620",
  gpu_vram_gb: 0.125,
  free_disk_gb: 346,
  os: "Windows 11 Pro",
};

export async function getHardwareInfo(): Promise<HardwareInfo> {
  if (!isTauri()) return MOCK_HARDWARE;
  try {
    return await invoke<HardwareInfo>("get_hardware_info");
  } catch {
    return MOCK_HARDWARE;
  }
}

export async function listInputDevices(): Promise<string[]> {
  if (!isTauri()) return ["Default microphone (preview)"];
  try {
    return await invoke<string[]>("list_input_devices");
  } catch {
    return [];
  }
}

export async function listDownloadedModels(): Promise<string[]> {
  if (!isTauri()) return [];
  try {
    return await invoke<string[]>("list_downloaded_models");
  } catch {
    return [];
  }
}

export async function downloadModel(
  modelId: string,
  url: string,
  fileName: string
): Promise<void> {
  if (!isTauri()) return; // simulated no-op in browser preview
  await invoke<void>("download_model", { modelId, url, fileName });
}

export async function deleteModel(modelId: string): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("delete_model", { modelId });
}

// ---------------- Runtime config + hotkey ----------------

export async function updateRuntimeConfig(
  modelId: string,
  language: string,
  audioDevice: string | null,
  vocabulary: string = "",
  sttSource: "local" | "cloud" = "local",
  sttBaseUrl: string = "",
  sttApiKey: string = "",
  sttCloudModel: string = "",
  useGpu: boolean = false
): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("update_runtime_config", {
      modelId,
      language,
      audioDevice,
      vocabulary,
      sttSource,
      sttBaseUrl,
      sttApiKey,
      sttCloudModel,
      useGpu,
    });
  } catch (e) {
    console.warn("update_runtime_config failed", e);
  }
}

export async function setHotkey(accelerator: string): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("set_hotkey", { accelerator });
}

// Push spoken-trigger → inserted-text pairs to the backend. Applied to the
// transcript after AI processing, right before pasting. Empty triggers are
// dropped here so the backend never has to guard against them.
export async function setTextReplacements(
  pairs: { trigger: string; replacement: string }[]
): Promise<void> {
  if (!isTauri()) return;
  const clean = pairs
    .filter((p) => p.trigger.trim().length > 0)
    .map((p) => [p.trigger, p.replacement] as [string, string]);
  try {
    await invoke<void>("set_text_replacements", { pairs: clean });
  } catch (e) {
    console.warn("set_text_replacements failed", e);
  }
}

// Push behavior settings (double-tap lock, input sensitivity, inline proofread, high performance).
export async function setBehavior(
  toggleMode: boolean,
  inputSensitivity: number,
  inlineProofread: boolean,
  highPerformance: boolean
): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_behavior", { toggleMode, inputSensitivity, inlineProofread, highPerformance });
  } catch (e) {
    console.warn("set_behavior failed", e);
  }
}

// Add/remove the per-user Windows Run-key entry ("Launch at startup").
export async function setAutostart(enabled: boolean): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_autostart", { enabled });
  } catch (e) {
    console.warn("set_autostart failed", e);
  }
}

/** Whether the Run-key entry actually exists right now (registry truth). */
export async function getAutostart(): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    return await invoke<boolean>("get_autostart");
  } catch {
    return false;
  }
}

// ---------------- Read aloud (TTS) ----------------

/** Push the active read-aloud voice + hotkey to Rust (registers the hotkey). */
export async function setTts(voiceId: string, hotkey: string): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_tts", { voiceId, hotkey });
  } catch (e) {
    console.warn("set_tts failed", e);
  }
}

export async function listDownloadedTts(): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>("list_downloaded_tts");
}

export async function downloadTtsModel(
  voiceId: string,
  urlOnnx: string,
  urlJson: string
): Promise<void> {
  if (!isTauri()) throw new Error("Voice downloads require the desktop app");
  await invoke<void>("download_tts_model", { voiceId, urlOnnx, urlJson });
}

export async function deleteTtsModel(voiceId: string): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("delete_tts_model", { voiceId });
}

/** Read the current text selection aloud (same as pressing the TTS hotkey). */
export async function ttsReadSelection(): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("tts_read_selection");
}

export async function ttsStop(): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("tts_stop");
}

/** Speak an explicit string with the active voice (Settings "Test voice"). */
export async function ttsSpeakText(text: string): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("tts_speak_text", { text });
}

/** One spelling/grammar issue found by Harper. Offsets are CHAR indices
 * (use Array.from(text), not text.slice, to map them). */
export interface ProofIssue {
  start: number;
  end: number;
  message: string;
  kind: string;
  suggestions: string[];
}

/** Check text for spelling/grammar issues (offline, Harper). Custom
 * vocabulary words are never flagged. */
export async function proofreadText(text: string): Promise<ProofIssue[]> {
  if (!isTauri()) return [];
  return invoke<ProofIssue[]>("proofread_text", { text });
}

// Push per-app profile rules, fully resolved (mode → prompt/model/keys) so the
// Rust pipeline never needs the frontend's mode/provider tables.
export interface ResolvedAppProfile {
  app_match: string;
  mode_source: string;
  mode_prompt: string;
  mode_model: string;
  mode_base_url: string;
  mode_api_key: string;
}

export async function setAppProfiles(
  profiles: ResolvedAppProfile[]
): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_app_profiles", { profiles });
  } catch (e) {
    console.warn("set_app_profiles failed", e);
  }
}

// ---------------- Storage location ----------------

export interface DataLocation {
  models_root: string | null;
  history_root: string | null;
}

export async function getDataLocation(): Promise<DataLocation> {
  if (!isTauri()) return { models_root: null, history_root: null };
  return invoke<DataLocation>("get_data_location");
}

export async function setDataLocation(loc: DataLocation): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("set_data_location", {
    modelsRoot: loc.models_root,
    historyRoot: loc.history_root,
  });
}

export async function pickFolder(): Promise<string | null> {
  if (!isTauri()) return null;
  const result = await invoke<string | null>("pick_folder");
  return result;
}

/** Open the current models or history folder in the OS file explorer. */
export async function openDataFolder(kind: "models" | "history"): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("open_data_folder", { kind });
  } catch (e) {
    console.warn("open_data_folder failed", e);
  }
}

// ---------------- Local LLM (bundled llama.cpp) ----------------

export async function listDownloadedLlm(): Promise<string[]> {
  if (!isTauri()) return [];
  try {
    return await invoke<string[]>("list_downloaded_llm");
  } catch {
    return [];
  }
}

export async function downloadLlmModel(
  modelId: string,
  url: string
): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("download_llm_model", { modelId, url });
}

export async function deleteLlmModel(modelId: string): Promise<void> {
  if (!isTauri()) return;
  await invoke<void>("delete_llm_model", { modelId });
}

/** Run a downloaded local model via the bundled llama.cpp engine. */
export async function localLlmGenerate(
  modelId: string,
  systemPrompt: string,
  text: string
): Promise<string> {
  if (!isTauri()) return "(local AI requires the desktop app)";
  return invoke<string>("local_llm_generate", { modelId, systemPrompt, text });
}

// ---------------- AI modes (Ollama, optional) ----------------

export async function setActiveMode(
  modeId: string,
  modeSource: string,
  modePrompt: string,
  modeModel: string,
  modeBaseUrl = "",
  modeApiKey = ""
): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("set_active_mode", {
      modeId,
      modeSource,
      modePrompt,
      modeModel,
      modeBaseUrl,
      modeApiKey,
    });
  } catch (e) {
    console.warn("set_active_mode failed", e);
  }
}

/** Fetch all model ids a provider offers (GET {base_url}/models). */
export async function apiListModels(
  baseUrl: string,
  apiKey: string
): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>("api_list_models", { baseUrl, apiKey });
}

/** Generic OpenAI-compatible call (OpenAI, OpenRouter, Groq, Together…). */
export async function apiGenerate(
  baseUrl: string,
  apiKey: string,
  model: string,
  systemPrompt: string,
  text: string
): Promise<string> {
  if (!isTauri())
    return "(API processing requires the desktop app)";
  return invoke<string>("api_generate", {
    baseUrl,
    apiKey,
    model,
    systemPrompt,
    text,
  });
}

/** Round-trip test of a provider's cloud STT endpoint (sends a short silent clip). */
export async function apiTestStt(
  baseUrl: string,
  apiKey: string,
  sttModel: string
): Promise<string> {
  if (!isTauri()) return "(Cloud STT requires the desktop app)";
  return invoke<string>("api_test_stt", { baseUrl, apiKey, model: sttModel });
}

export interface OllamaStatus {
  running: boolean;
  models: string[];
}

export async function ollamaStatus(): Promise<OllamaStatus> {
  if (!isTauri()) return { running: false, models: [] };
  try {
    return await invoke<OllamaStatus>("ollama_status");
  } catch {
    return { running: false, models: [] };
  }
}

export async function ollamaGenerate(
  model: string,
  systemPrompt: string,
  text: string
): Promise<string> {
  if (!isTauri())
    return "(AI processing requires the desktop app + Ollama running)";
  return invoke<string>("ollama_generate", {
    model,
    systemPrompt,
    text,
  });
}

// ---------------- History (local JSON file via Rust) ----------------

export async function loadHistory(): Promise<HistoryEntry[] | null> {
  if (!isTauri()) return null; // signal: use localStorage fallback
  try {
    return await invoke<HistoryEntry[]>("load_history");
  } catch {
    return [];
  }
}

export async function saveHistory(entries: HistoryEntry[]): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("save_history", { entries });
  } catch (e) {
    console.warn("save_history failed", e);
  }
}

export async function clearHistoryFile(): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>("clear_history");
  } catch (e) {
    console.warn("clear_history failed", e);
  }
}
