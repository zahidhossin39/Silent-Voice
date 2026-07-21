import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { HistoryEntry } from "../types";
import {
  isTauri,
  loadHistory,
  saveHistory,
  clearHistoryFile,
} from "../services/tauriBridge";
import { useSettingsStore } from "./settingsStore";

const DAY_MS = 86_400_000;
const RETENTION_MS: Record<string, number> = {
  "3d": 3 * DAY_MS,
  "2w": 14 * DAY_MS,
  "3m": 90 * DAY_MS,
};

// Apply the user's history-limit (count cap) and retention window (age cap).
// Read non-reactively from the settings store so the two stores stay decoupled.
function prune(entries: HistoryEntry[]): HistoryEntry[] {
  const s = useSettingsStore.getState().settings;
  let out = entries;
  const window = RETENTION_MS[s.history_retention];
  if (window) {
    const now = Date.now();
    out = out.filter((e) => now - e.timestamp <= window);
  }
  const limit = s.history_limit > 0 ? s.history_limit : 1000;
  return out.length > limit ? out.slice(0, limit) : out;
}

interface HistoryState {
  entries: HistoryEntry[];
  hydrated: boolean;
  hydrate: () => Promise<void>;
  add: (entry: Omit<HistoryEntry, "id">) => void;
  addFull: (entry: HistoryEntry) => void;
  update: (id: number, processedText: string) => void;
  remove: (id: number) => void;
  clear: () => void;
  reprune: () => void;
}

// History is stored as a local JSON file (%APPDATA%/SilentVoice/history.json) in
// the desktop build via Rust commands, and in localStorage in the browser
// preview. No database — just local files. (Per project decision.)
export const useHistoryStore = create<HistoryState>()(
  persist(
    (set, get) => ({
      entries: [],
      hydrated: false,

      hydrate: async () => {
        const fromFile = await loadHistory();
        if (fromFile !== null) {
          // Running in Tauri: the JSON file is the source of truth. Prune on
          // load so a retention window applies even to already-saved entries.
          const pruned = prune(fromFile);
          set({ entries: pruned, hydrated: true });
          if (isTauri() && pruned.length !== fromFile.length) saveHistory(pruned);
        } else {
          set({ hydrated: true });
        }
      },

      add: (entry) => {
        const full: HistoryEntry = { ...entry, id: Date.now() };
        get().addFull(full);
      },

      addFull: (entry) => {
        const entries = prune([entry, ...get().entries]);
        set({ entries });
        if (isTauri()) saveHistory(entries);
      },

      update: (id, processedText) => {
        const entries = get().entries.map((e) =>
          e.id === id ? { ...e, processed_text: processedText } : e
        );
        set({ entries });
        if (isTauri()) saveHistory(entries);
      },

      remove: (id) => {
        const entries = get().entries.filter((e) => e.id !== id);
        set({ entries });
        if (isTauri()) saveHistory(entries);
      },

      clear: () => {
        set({ entries: [] });
        if (isTauri()) clearHistoryFile();
      },

      // Re-apply limit/retention to what's already stored — call when the
      // user changes either history setting.
      reprune: () => {
        const pruned = prune(get().entries);
        if (pruned.length !== get().entries.length) {
          set({ entries: pruned });
          if (isTauri()) saveHistory(pruned);
        }
      },
    }),
    {
      name: "silent-voice-history",
      // In Tauri the file is authoritative; skip rehydrating stale localStorage.
      skipHydration: false,
    }
  )
);
