import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { StateStorage } from "zustand/middleware";
import type {
  Settings,
  ApiProvider,
  Mode,
  TextReplacement,
  AppProfileRule,
} from "../types";
import { BUILTIN_MODES } from "../services/modes";
import { isTauri } from "../services/tauriBridge";

// Mirrors the Tauri global-shortcut parser's accepted main keys (see
// HotkeyRecorder.tsx's isSupportedMain). Used to sanitize hotkeys loaded
// from localStorage that predate that validation — without this, a
// previously-saved unsupported key (e.g. "ContextMenu") would be re-sent to
// Rust on every launch and fail forever with no way to self-heal.
const SUPPORTED_NAMED_MAIN = new Set([
  "Space", "Up", "Down", "Left", "Right", "Escape", "Tab", "Return",
  "Backspace", "Delete", "Home", "End", "PageUp", "PageDown", "Insert",
  "Pause", "ScrollLock", "PrintScreen", "NumLock", "CapsLock",
  "Alt", "Ctrl", "Shift", "Super",
]);

function isValidAccelerator(accel: string | undefined | null): boolean {
  if (!accel) return false;
  const parts = accel.split("+");
  const main = parts[parts.length - 1];
  return (
    /^[A-Z0-9]$/.test(main) ||
    /^F([1-9]|1[0-9]|2[0-4])$/.test(main) ||
    SUPPORTED_NAMED_MAIN.has(main)
  );
}

const DEFAULT_SETTINGS: Settings = {
  hotkey: "Ctrl+Shift+Space",
  active_stt_model: "base.en",
  active_mode_id: "raw",
  stt_preset: "balanced",
  language: "auto",
  use_gpu: false,
  high_performance: false,
  performance_threads: 0,
  audio_device: null,
  auto_start: false,
  theme: "dark",
  overlay_opacity: 92,
  custom_vocabulary: "",
  stt_cloud_provider_id: null,
  toggle_mode: true,
  input_sensitivity: 50,
  inline_proofread: true,
  active_tts_voice: null,
  tts_hotkey: "Ctrl+Alt+S",
  onboarded: false,
  pinned_stt: [],
  pinned_llm: [],
  pinned_tts: [],
};

interface SettingsState {
  settings: Settings;
  modes: Mode[];
  providers: ApiProvider[];
  snippets: TextReplacement[];
  appProfiles: AppProfileRule[];
  setSettings: (patch: Partial<Settings>) => void;
  togglePinnedStt: (id: string) => void;
  togglePinnedLlm: (id: string) => void;
  togglePinnedTts: (id: string) => void;
  setActiveMode: (id: string) => void;
  addMode: (mode: Mode) => void;
  updateMode: (id: string, patch: Partial<Mode>) => void;
  deleteMode: (id: string) => void;
  addProvider: (p: ApiProvider) => void;
  updateProvider: (id: string, patch: Partial<ApiProvider>) => void;
  deleteProvider: (id: string) => void;
  addSnippet: () => void;
  updateSnippet: (id: string, patch: Partial<TextReplacement>) => void;
  deleteSnippet: (id: string) => void;
  addAppProfile: () => void;
  updateAppProfile: (id: string, patch: Partial<AppProfileRule>) => void;
  deleteAppProfile: (id: string) => void;
}

// Non-obvious hydration guard and store cache
let storePromise: Promise<any> | null = null;
let isLoaded = false;

const customStorage: StateStorage = {
  getItem: async (name) => {
    if (isTauri()) {
      if (!storePromise) {
        storePromise = (async () => {
          try {
            const { Store } = await import("@tauri-apps/plugin-store");
            const store = await Store.load("settings.json");
            // One-time migration: copy from localStorage if store is empty
            const existing = await store.get(name);
            if (existing === null || existing === undefined) {
              const oldVal = window.localStorage.getItem(name);
              if (oldVal !== null) {
                await store.set(name, oldVal);
                await store.save();
              }
            }
            return store;
          } catch (e) {
            console.error("Tauri store load failed", e);
            return null;
          }
        })();
      }
      const store = await storePromise;
      if (store) {
        const val = await store.get(name);
        isLoaded = true;
        return typeof val === "string" ? val : null;
      }
      isLoaded = true;
      return null;
    } else {
      const val = window.localStorage.getItem(name);
      isLoaded = true;
      return val;
    }
  },
  setItem: async (name, value) => {
    // Block write attempts before the existing file state is loaded
    if (!isLoaded) return;
    if (isTauri()) {
      const store = await storePromise;
      if (store) {
        await store.set(name, value);
        await store.save();
      }
    } else {
      window.localStorage.setItem(name, value);
    }
  },
  removeItem: async (name) => {
    if (isTauri()) {
      const store = await storePromise;
      if (store) {
        await store.delete(name);
        await store.save();
      }
    } else {
      window.localStorage.removeItem(name);
    }
  },
};

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      settings: DEFAULT_SETTINGS,
      modes: BUILTIN_MODES,
      providers: [],
      snippets: [],
      appProfiles: [],
      setSettings: (patch) =>
        set((s) => ({ settings: { ...s.settings, ...patch } })),
      togglePinnedStt: (id) =>
        set((s) => {
          const current = s.settings.pinned_stt || [];
          return {
            settings: {
              ...s.settings,
              pinned_stt: current.includes(id)
                ? current.filter((x) => x !== id)
                : [...current, id],
            },
          };
        }),
      togglePinnedLlm: (id) =>
        set((s) => {
          const current = s.settings.pinned_llm || [];
          return {
            settings: {
              ...s.settings,
              pinned_llm: current.includes(id)
                ? current.filter((x) => x !== id)
                : [...current, id],
            },
          };
        }),
      togglePinnedTts: (id) =>
        set((s) => {
          const current = s.settings.pinned_tts || [];
          return {
            settings: {
              ...s.settings,
              pinned_tts: current.includes(id)
                ? current.filter((x) => x !== id)
                : [...current, id],
            },
          };
        }),
      setActiveMode: (id) =>
        set((s) => ({ settings: { ...s.settings, active_mode_id: id } })),
      addMode: (mode) => set((s) => ({ modes: [...s.modes, mode] })),
      updateMode: (id, patch) =>
        set((s) => ({
          modes: s.modes.map((m) => (m.id === id ? { ...m, ...patch } : m)),
        })),
      deleteMode: (id) =>
        set((s) => {
          const nextActive = s.settings.active_mode_id === id ? "raw" : s.settings.active_mode_id;
          return {
            modes: s.modes.filter((m) => m.id !== id),
            settings: { ...s.settings, active_mode_id: nextActive },
          };
        }),
      addProvider: (p) => set((s) => ({ providers: [...s.providers, p] })),
      updateProvider: (id, patch) =>
        set((s) => ({
          providers: s.providers.map((p) =>
            p.id === id ? { ...p, ...patch } : p
          ),
        })),
      deleteProvider: (id) =>
        set((s) => ({ providers: s.providers.filter((p) => p.id !== id) })),
      addSnippet: () =>
        set((s) => ({
          snippets: [
            ...s.snippets,
            { id: `snip_${Date.now()}`, trigger: "", replacement: "" },
          ],
        })),
      updateSnippet: (id, patch) =>
        set((s) => ({
          snippets: s.snippets.map((sn) =>
            sn.id === id ? { ...sn, ...patch } : sn
          ),
        })),
      deleteSnippet: (id) =>
        set((s) => ({ snippets: s.snippets.filter((sn) => sn.id !== id) })),
      addAppProfile: () =>
        set((s) => ({
          appProfiles: [
            ...s.appProfiles,
            { id: `prof_${Date.now()}`, app_match: "", mode_id: "raw" },
          ],
        })),
      updateAppProfile: (id, patch) =>
        set((s) => ({
          appProfiles: s.appProfiles.map((p) =>
            p.id === id ? { ...p, ...patch } : p
          ),
        })),
      deleteAppProfile: (id) =>
        set((s) => ({
          appProfiles: s.appProfiles.filter((p) => p.id !== id),
        })),
    }),
    {
      name: "silent-voice-settings",
      storage: createJSONStorage(() => customStorage),
      merge: (persistedState: any, currentState) => {
        // Deep merge the settings object so new keys added to DEFAULT_SETTINGS
        // aren't lost when loading an older persisted state.
        const mergedSettings = {
          ...currentState.settings,
          ...(persistedState?.settings || {}),
        };
        // Also ensure new keys are strictly populated if undefined in persisted state
        for (const key in currentState.settings) {
          if (mergedSettings[key] === undefined) {
            mergedSettings[key] = currentState.settings[key as keyof Settings];
          }
        }
        // Self-heal hotkeys saved before Tauri-accelerator validation
        // existed — an unsupported key would otherwise fail on the Rust
        // side forever with the same error banner on every launch.
        if (!isValidAccelerator(mergedSettings.hotkey)) {
          mergedSettings.hotkey = DEFAULT_SETTINGS.hotkey;
        }
        if (!isValidAccelerator(mergedSettings.tts_hotkey)) {
          mergedSettings.tts_hotkey = DEFAULT_SETTINGS.tts_hotkey;
        }
        return {
          ...currentState,
          ...persistedState,
          settings: mergedSettings,
        };
      },
    }
  )
);
