import { describe, it, expect } from "vitest";
import { aggregateByDay, dayStartMs } from "./statsStore";
import type { HistoryEntry } from "../types";

function entry(over: Partial<HistoryEntry>): HistoryEntry {
  return {
    id: 1,
    timestamp: Date.now(),
    raw_text: "",
    processed_text: "",
    model_id: "x",
    duration_ms: 0,
    ...over,
  } as HistoryEntry;
}

describe("aggregateByDay", () => {
  it("sums entries from the same local day into one bucket", () => {
    const day = new Date(2026, 8, 11, 0, 0, 0, 0).getTime(); // Sep 11 2026 local
    const morning = new Date(2026, 8, 11, 9).getTime();
    const evening = new Date(2026, 8, 11, 21).getTime();

    const daily = aggregateByDay([
      entry({ timestamp: morning, processed_text: "one two three", duration_ms: 1000 }),
      entry({ timestamp: evening, processed_text: "four five", duration_ms: 500 }),
    ]);

    expect(Object.keys(daily)).toHaveLength(1);
    expect(daily[day]).toEqual({ words: 5, dictations: 2, speakMs: 1500 });
  });

  it("splits entries on different days into separate buckets", () => {
    const d1 = new Date(2026, 8, 10, 12).getTime();
    const d2 = new Date(2026, 8, 11, 12).getTime();
    const daily = aggregateByDay([
      entry({ timestamp: d1, processed_text: "a b", duration_ms: 100 }),
      entry({ timestamp: d2, processed_text: "c", duration_ms: 200 }),
    ]);

    expect(Object.keys(daily)).toHaveLength(2);
    expect(daily[dayStartMs(d1)].words).toBe(2);
    expect(daily[dayStartMs(d2)].words).toBe(1);
  });

  it("falls back to raw_text when processed_text is empty", () => {
    const t = Date.now();
    const daily = aggregateByDay([
      entry({ timestamp: t, raw_text: "hello there world", processed_text: "" }),
    ]);
    expect(daily[dayStartMs(t)].words).toBe(3);
  });
});
