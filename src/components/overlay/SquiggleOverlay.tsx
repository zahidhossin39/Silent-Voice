import { useEffect, useState } from "react";
import { listenEvent } from "../../services/tauriBridge";

// macOS underline renderer. Rust pushes the full list every poll and this just
// draws it — no local state to drift out of sync with the text underneath.
//
// The window hosting this is click-through, so nothing here can be hovered or
// clicked; it is purely visual. Keep it that way, or a stray pointer event
// starts landing on the overlay instead of the app the user is typing into.

type Squiggle = {
  x: number;
  y: number;
  w: number;
  h: number;
  spelling: boolean;
  message: string;
  suggestions: string[];
  start: number;
  end: number;
  expected: string;
};

// Same two colours the Windows renderer uses, so a word looks identical on
// both platforms: red for spelling, blue for grammar and style.
const SPELLING = "#ef4444";
const GRAMMAR = "#3b82f6";

const WAVE_PERIOD = 4;
const WAVE_HEIGHT = 2;

/// A wavy underline as a repeating SVG path, sized to the word.
function Wave({ width, color }: { width: number; color: string }) {
  const steps = Math.max(1, Math.round(width / WAVE_PERIOD));
  let d = `M 0 ${WAVE_HEIGHT}`;
  for (let i = 0; i < steps; i++) {
    const x1 = i * WAVE_PERIOD + WAVE_PERIOD / 2;
    const x2 = (i + 1) * WAVE_PERIOD;
    // Alternate the control point above and below the baseline.
    const cy = i % 2 === 0 ? 0 : WAVE_HEIGHT * 2;
    d += ` Q ${x1} ${cy} ${x2} ${WAVE_HEIGHT}`;
  }
  return (
    <svg
      width={steps * WAVE_PERIOD}
      height={WAVE_HEIGHT * 2 + 1}
      viewBox={`0 0 ${steps * WAVE_PERIOD} ${WAVE_HEIGHT * 2 + 1}`}
      style={{ display: "block", overflow: "visible" }}
      aria-hidden="true"
    >
      <path d={d} fill="none" stroke={color} strokeWidth={1.5} strokeLinecap="round" />
    </svg>
  );
}

export default function SquiggleOverlay() {
  const [squiggles, setSquiggles] = useState<Squiggle[]>([]);

  useEffect(() => {
    const un = listenEvent<Squiggle[]>("squiggle://set", (list) =>
      setSquiggles(Array.isArray(list) ? list : [])
    );
    return () => {
      un.then((f) => f());
    };
  }, []);

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "transparent",
        pointerEvents: "none",
        overflow: "hidden",
      }}
    >
      {squiggles.map((s, i) => (
        <div
          key={`${s.x}-${s.y}-${s.start}-${i}`}
          style={{
            position: "absolute",
            left: s.x,
            // Sit just under the word's baseline rather than on it.
            top: s.y + s.h - 1,
            width: s.w,
            height: WAVE_HEIGHT * 2 + 1,
            pointerEvents: "none",
          }}
        >
          <Wave width={s.w} color={s.spelling ? SPELLING : GRAMMAR} />
        </div>
      ))}
    </div>
  );
}
