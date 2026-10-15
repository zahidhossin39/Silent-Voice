import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { createTauriStorage } from "./tauriStorage";
import type { HistoryEntry } from "../types";

// Per-day dictation totals, kept SEPARATELY from history so the Home calendar
// and stats survive history pruning/clearing. History entries are the full text
// (capped by the user's limit/retention); this is just a few running numbers
// per day — trivially small, so it's never pruned.
export interface DayStat {
  words: number;
  dictations: number;
  /** Legacy: this was the processing time after release, not speaking time. Unused. */
  speakMs: number;
  /** Real speaking time (key held), only for dictations made since it was recorded. */
  talkMs?: number;
  /** Words those timed dictations produced, so speed = talkWords / talkMs. */
  talkWords?: number;
}

/** Average typing speed the "time saved" figure compares against. */
export const TYPING_WPM = 40;
/** Typical dictation speed, used for days recorded before speaking time was. */
const SPEAK_WPM = 130;

export function dayStartMs(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function countWords(text: string): number {
  const t = text.trim();
  return t ? t.split(/\s+/).length : 0;
}

/** Time spent speaking those words: measured where known, estimated otherwise. */
export function speakingMs(d: DayStat): number {
  const untimed = Math.max(0, d.words - (d.talkWords ?? 0));
  return (d.talkMs ?? 0) + (untimed / SPEAK_WPM) * 60_000;
}

/** Minutes saved vs typing the same words at TYPING_WPM. */
export function savedMinutes(days: DayStat[]): number {
  let ms = 0;
  for (const d of days) ms += Math.max(0, (d.words / TYPING_WPM) * 60_000 - speakingMs(d));
  return ms / 60_000;
}

function add(cur: DayStat | undefined, words: number, talkMs?: number): DayStat {
  const c = cur ?? { words: 0, dictations: 0, speakMs: 0 };
  const timed = talkMs !== undefined && talkMs > 0;
  return {
    ...c,
    words: c.words + words,
    dictations: c.dictations + 1,
    talkMs: (c.talkMs ?? 0) + (timed ? talkMs : 0),
    talkWords: (c.talkWords ?? 0) + (timed ? words : 0),
  };
}

// Pure reducer: fold history entries into per-day totals keyed by local
// day-start ms. Exported so the same-day bucketing is unit-testable without the
// store or its async persistence.
export function aggregateByDay(entries: HistoryEntry[]): Record<number, DayStat> {
  const daily: Record<number, DayStat> = {};
  for (const e of entries) {
    const key = dayStartMs(e.timestamp);
    daily[key] = add(daily[key], countWords(e.processed_text || e.raw_text), e.speak_ms);
  }
  return daily;
}

interface StatsState {
  // Keyed by local day-start ms. Record<number,…> serializes to JSON fine.
  daily: Record<number, DayStat>;
  /** Words per hour of the day (0–23), since this was added. */
  hours: number[];
  /** Words per app (exe name), since this was added. */
  apps: Record<string, number>;
  seeded: boolean;
  record: (words: number, talkMs?: number, at?: number, app?: string) => void;
  // One-time backfill from whatever history still exists, so upgrading users
  // don't see today's numbers reset to zero. Runs once; future dictations are
  // counted live via record(), so there's no double counting.
  seedFromEntries: (entries: HistoryEntry[]) => void;
}

export const useStatsStore = create<StatsState>()(
  persist(
    (set, get) => ({
      daily: {},
      hours: Array(24).fill(0),
      apps: {},
      seeded: false,

      record: (words, talkMs, at = Date.now(), app) =>
        set((s) => {
          const key = dayStartMs(at);
          const hours = [...(s.hours?.length === 24 ? s.hours : Array(24).fill(0))];
          hours[new Date(at).getHours()] += words;
          const apps = { ...(s.apps ?? {}) };
          if (app) apps[app] = (apps[app] ?? 0) + words;
          return { daily: { ...s.daily, [key]: add(s.daily[key], words, talkMs) }, hours, apps };
        }),

      seedFromEntries: (entries) => {
        if (get().seeded) return;
        const seeded = aggregateByDay(entries);
        const daily: Record<number, DayStat> = { ...get().daily };
        for (const [k, v] of Object.entries(seeded)) {
          const key = Number(k);
          const cur = daily[key] ?? { words: 0, dictations: 0, speakMs: 0 };
          daily[key] = {
            ...cur,
            words: cur.words + v.words,
            dictations: cur.dictations + v.dictations,
            talkMs: (cur.talkMs ?? 0) + (v.talkMs ?? 0),
            talkWords: (cur.talkWords ?? 0) + (v.talkWords ?? 0),
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
