import { useEffect } from "react";
import { useSettingsStore } from "../stores/settingsStore";
import {
  updateRuntimeConfig,
  setHotkey,
  setActiveMode,
  setOverlayOpacity,
  setTextReplacements,
  setBehavior,
  setAppProfiles,
  setAutostart,
  setTts,
  listenEvent,
} from "../services/tauriBridge";
import type { ResolvedAppProfile } from "../services/tauriBridge";
import type { Mode, ApiProvider } from "../types";

// Resolve a mode the same way set_active_mode does: for "api" modes pull the
// provider's base URL / key / model; local modes use the mode's own model id.
function resolveMode(
  mode: Mode,
  providers: ApiProvider[]
): Omit<ResolvedAppProfile, "app_match"> {
  if (mode.model_source === "api") {
    const provider = providers.find((p) => p.id === mode.provider_id);
    return {
      mode_source: "api",
      mode_prompt: mode.system_prompt,
      mode_model: provider?.model ?? mode.model_id,
      mode_base_url: provider?.base_url ?? "",
      mode_api_key: provider?.api_key ?? "",
    };
  }
  return {
    mode_source: mode.model_source,
    mode_prompt: mode.system_prompt,
    mode_model: mode.model_id,
    mode_base_url: "",
    mode_api_key: "",
  };
}

// Keeps the Rust-side runtime config (used by the global-hotkey pipeline) in
// sync with the frontend settings store.
export function useRuntimeSync() {
  const model = useSettingsStore((s) => s.settings.active_stt_model);
  const language = useSettingsStore((s) => s.settings.language);
  const device = useSettingsStore((s) => s.settings.audio_device);
  const hotkey = useSettingsStore((s) => s.settings.hotkey);
  const vocabulary = useSettingsStore((s) => s.settings.custom_vocabulary);
  const useGpu = useSettingsStore((s) => s.settings.use_gpu);
  const sttCloudProviderId = useSettingsStore((s) => s.settings.stt_cloud_provider_id);
  const activeModeId = useSettingsStore((s) => s.settings.active_mode_id);
  const modes = useSettingsStore((s) => s.modes);
  const providers = useSettingsStore((s) => s.providers);
  const snippets = useSettingsStore((s) => s.snippets);
  const appProfiles = useSettingsStore((s) => s.appProfiles);
  const toggleMode = useSettingsStore((s) => s.settings.toggle_mode);
  const inputSensitivity = useSettingsStore((s) => s.settings.input_sensitivity);
  const inlineProofread = useSettingsStore((s) => s.settings.inline_proofread);
  const highPerformance = useSettingsStore((s) => s.settings.high_performance);
  const ttsVoice = useSettingsStore((s) => s.settings.active_tts_voice);
  const ttsHotkey = useSettingsStore((s) => s.settings.tts_hotkey);
  const autoStart = useSettingsStore((s) => s.settings.auto_start);
  const overlayOpacity = useSettingsStore((s) => s.settings.overlay_opacity);
  const setSettings = useSettingsStore((s) => s.setSettings);

  useEffect(() => {
    const unsubPromise = listenEvent<string>("proofread://add-vocab", (word) => {
      const cleanWord = word.trim();
      if (!cleanWord) return;

      const currentVocab = useSettingsStore.getState().settings.custom_vocabulary || "";
      const entries = currentVocab
        .split(/[,\n]/)
        .map((entry) => entry.trim())
        .filter((entry) => entry.length > 0);

      const exists = entries.some(
        (entry) => entry.toLowerCase() === cleanWord.toLowerCase()
      );

      if (!exists) {
        const trimmedVocab = currentVocab.trim();
        const nextVocab = trimmedVocab ? `${trimmedVocab}, ${cleanWord}` : cleanWord;
        setSettings({ custom_vocabulary: nextVocab });
      }
    });

    return () => {
      unsubPromise.then((unsub) => unsub());
    };
  }, [setSettings]);

  useEffect(() => {
    setOverlayOpacity(overlayOpacity);
  }, [overlayOpacity]);

  useEffect(() => {
    const sttProvider = sttCloudProviderId
      ? providers.find((p) => p.id === sttCloudProviderId && p.uses.includes("stt"))
      : undefined;
    updateRuntimeConfig(
      model,
      language,
      device,
      vocabulary,
      sttProvider ? "cloud" : "local",
      sttProvider?.base_url ?? "",
      sttProvider?.api_key ?? "",
      sttProvider?.stt_model ?? "",
      useGpu
    );
  }, [model, language, device, vocabulary, sttCloudProviderId, providers, useGpu]);

  useEffect(() => {
    setHotkey(hotkey).catch(() => {});
  }, [hotkey]);

  useEffect(() => {
    setTextReplacements(snippets);
  }, [snippets]);

  useEffect(() => {
    setBehavior(toggleMode, inputSensitivity, inlineProofread, highPerformance);
  }, [toggleMode, inputSensitivity, inlineProofread, highPerformance]);

  useEffect(() => {
    setAutostart(autoStart);
  }, [autoStart]);

  useEffect(() => {
    setTts(ttsVoice ?? "", ttsHotkey);
  }, [ttsVoice, ttsHotkey]);

  // Resolve per-app profile rules to full mode configs and push to Rust.
  useEffect(() => {
    const resolved: ResolvedAppProfile[] = appProfiles
      .filter((r) => r.app_match.trim().length > 0)
      .flatMap((r) => {
        const mode = modes.find((m) => m.id === r.mode_id);
        if (!mode) return [];
        return [{ app_match: r.app_match.trim().toLowerCase(), ...resolveMode(mode, providers) }];
      });
    setAppProfiles(resolved);
  }, [appProfiles, modes, providers]);

  // Push the active AI mode (prompt + model + source) to the backend so the
  // hotkey pipeline can apply it after transcription. For "api" modes, resolve
  // the chosen provider's base URL / key / model.
  useEffect(() => {
    const mode = modes.find((m) => m.id === activeModeId);
    if (!mode) return;

    if (mode.model_source === "api") {
      const provider = providers.find((p) => p.id === mode.provider_id);
      setActiveMode(
        mode.id,
        "api",
        mode.system_prompt,
        provider?.model ?? mode.model_id,
        provider?.base_url ?? "",
        provider?.api_key ?? ""
      );
    } else {
      setActiveMode(
        mode.id,
        mode.model_source,
        mode.system_prompt,
        mode.model_id
      );
    }
  }, [activeModeId, modes, providers]);
}
