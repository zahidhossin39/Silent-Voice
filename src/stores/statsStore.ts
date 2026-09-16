import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { createTauriStorage } from "./tauriStorage";
import type { HistoryEntry } from "../types";

// Per-day dictation totals, kept SEPARATELY from history so the Home calendar
// and stats survive history pruning/clearing. History entries are the full text
// (capped by the user's limit/retention); this is just three running numbers
// per day — trivially small, so it's never pruned.
export interface DayStat {
  words: number;
  dictations: number;
  speakMs: number;
}

export function dayStartMs(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

function countWords(text: string): number {
  const t = text.trim();
  return t ? t.split(/\s+/).length : 0;
}

// Pure reducer: fold history entries into per-day totals keyed by local
// day-start ms. Exported so the same-day bucketing is unit-testable without the
// store or its async persistence.
export function aggregateByDay(entries: HistoryEntry[]): Record<number, DayStat> {
  const daily: Record<number, DayStat> = {};
  for (const e of entries) {
    const key = dayStartMs(e.timestamp);
    const cur = daily[key] ?? { words: 0, dictations: 0, speakMs: 0 };
    daily[key] = {
      words: cur.words + countWords(e.processed_text || e.raw_text),
      dictations: cur.dictations + 1,
      speakMs: cur.speakMs + e.duration_ms,
    };
  }
  return daily;
}

interface StatsState {
  // Keyed by local day-start ms. Record<number,…> serializes to JSON fine.
  daily: Record<number, DayStat>;
  seeded: boolean;
  record: (words: number, speakMs: number, at?: number) => void;
  // One-time backfill from whatever history still exists, so upgrading users
  // don't see today's numbers reset to zero. Runs once; future dictations are
  // counted live via record(), so there's no double counting.
  seedFromEntries: (entries: HistoryEntry[]) => void;
}

export const useStatsStore = create<StatsState>()(
  persist(
    (set, get) => ({
      daily: {},
      seeded: false,

      record: (words, speakMs, at = Date.now()) =>
        set((s) => {
          const key = dayStartMs(at);
          const cur = s.daily[key] ?? { words: 0, dictations: 0, speakMs: 0 };
          return {
            daily: {
              ...s.daily,
              [key]: {
                words: cur.words + words,
                dictations: cur.dictations + 1,
                speakMs: cur.speakMs + speakMs,
              },
            },
          };
        }),

      seedFromEntries: (entries) => {
        if (get().seeded) return;
        const seeded = aggregateByDay(entries);
        const daily: Record<number, DayStat> = { ...get().daily };
        for (const [k, v] of Object.entries(seeded)) {
          const key = Number(k);
          const cur = daily[key] ?? { words: 0, dictations: 0, speakMs: 0 };
          daily[key] = {
            words: cur.words + v.words,
            dictations: cur.dictations + v.dictations,
            speakMs: cur.speakMs + v.speakMs,
          };
        }
        set({ daily, seeded: true });
      },
    }),
    {
      name: "silent-voice-stats",
      storage: createJSONStorage(() => createTauriStorage("settings.json")),
    }
  )
);
