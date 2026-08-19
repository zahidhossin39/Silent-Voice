import { useEffect, useRef, useState } from "react";

// Animated waveform used in the recording overlay and Home status.
// Two-mode behavior driven by real mic loudness (`level`, 0–100 from the
// backend's `pipeline://level`, ~every 60ms):
//   • recording but quiet  → the lively "waiting" loop animation (sv-bar)
//   • actually speaking     → bars track the real speaking envelope
//
// Loudness mapping: the backend level is rms*300 capped at 100, so ordinary
// speech only reaches ~6–25. We remap [FLOOR..FLOOR+SPAN] onto the full bar
// height so speech is dramatic, not a faint twitch. FLOOR/SPAN are the tuning
// knobs — a smaller SPAN is more sensitive/jumpier.
const FLOOR = 3;
const SPAN = 26;

function normLevel(level: number): number {
  const n = Math.min(1, Math.max(0, (level - FLOOR) / SPAN));
  return Math.pow(n, 0.7); // gentle curve so quiet speech still lifts the bars
}

export default function WaveformVisualizer({
  active,
  bars = 5,
  barClass = "bg-sv-accent",
  level,
  heightClass = "h-5",
}: {
  active: boolean;
  bars?: number;
  barClass?: string;
  level?: number; // live mic loudness 0–100
  heightClass?: string; // container height (e.g. "h-10" for a hero waveform)
}) {
  // Rolling history of recent normalized loudness (0–1), newest at the end.
  const [hist, setHist] = useState<number[]>(() => Array(bars).fill(0));
  const histLenRef = useRef(bars);

  useEffect(() => {
    histLenRef.current = bars;
    setHist(Array(bars).fill(0));
  }, [bars]);

  useEffect(() => {
    if (!active) {
      setHist(Array(histLenRef.current).fill(0));
      return;
    }
    setHist((h) => [...h.slice(1), normLevel(level ?? 0)]);
  }, [level, active]);

  // "Speaking" = recent loudness is meaningfully above silence. Below that we
  // fall back to the loop animation — which also covers browser/mock (no mic,
  // level stays 0, so it just animates).
  const speaking = active && Math.max(...hist) > 0.14;

  return (
    <div className={`flex items-center gap-[3px] ${heightClass}`}>
      {hist.map((val, i) => (
        <span
          key={i}
          className={`w-[3px] rounded-full ${barClass} ${
            active && !speaking ? "sv-bar" : "transition-[height] duration-75 ease-out"
          }`}
          style={{
            height: !active
              ? "30%"
              : speaking
                ? `${Math.round((0.15 + val * 0.85) * 100)}%`
                : "100%",
            animationDelay: `${i * 0.12}s`,
          }}
        />
      ))}
    </div>
  );
}
