import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { HistoryEntry } from "../types";
import {
  isTauri,
  loadHistory,
  saveHistory,
  clearHistoryFile,
} from "../services/tauriBridge";

interface HistoryState {
  entries: HistoryEntry[];
  hydrated: boolean;
  hydrate: () => Promise<void>;
  add: (entry: Omit<HistoryEntry, "id">) => void;
  addFull: (entry: HistoryEntry) => void;
  update: (id: number, processedText: string) => void;
  remove: (id: number) => void;
  clear: () => void;
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
          // Running in Tauri: the JSON file is the source of truth.
          set({ entries: fromFile, hydrated: true });
        } else {
          set({ hydrated: true });
        }
      },

      add: (entry) => {
        const full: HistoryEntry = { ...entry, id: Date.now() };
        get().addFull(full);
      },

      addFull: (entry) => {
        const entries = [entry, ...get().entries].slice(0, 1000);
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
    }),
    {
      name: "silent-voice-history",
      // In Tauri the file is authoritative; skip rehydrating stale localStorage.
      skipHydration: false,
    }
  )
);
