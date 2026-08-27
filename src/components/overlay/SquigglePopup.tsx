import { useEffect, useState } from "react";
import { applyInlineFix } from "../../services/tauriBridge";
import { listenEvent } from "../../services/tauriBridge";

// Suggestion popup for one flagged word on macOS. Unlike the underline overlay
// this window takes clicks — Rust holds a retained reference to the field, so
// the fix still lands in the right text even though clicking here moves focus.

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

const MAX_ROWS = 4;

export default function SquigglePopup() {
  const [info, setInfo] = useState<Squiggle | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const un = listenEvent<Squiggle>("squiggle://popup", (next) => {
      setInfo(next);
      setBusy(false);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  if (!info) return null;

  const accent = info.spelling ? "#ef4444" : "#3b82f6";

  async function choose(replacement: string) {
    if (!info || busy) return;
    setBusy(true);
    await applyInlineFix(info.start, info.end, info.expected, replacement);
  }

  return (
    <div
      style={{
        fontFamily:
          "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif",
        background: "rgba(24,24,27,0.98)",
        border: "1px solid rgba(255,255,255,0.12)",
        borderRadius: 10,
        overflow: "hidden",
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        color: "#fafafa",
      }}
    >
      <div
        style={{
          padding: "8px 12px 6px",
          borderBottom: "1px solid rgba(255,255,255,0.08)",
          display: "flex",
          alignItems: "baseline",
          gap: 8,
        }}
      >
        <span style={{ fontWeight: 600, fontSize: 13, color: accent }}>
          {info.expected}
        </span>
        <span
          style={{
            fontSize: 11,
            color: "rgba(250,250,250,0.6)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {info.message}
        </span>
      </div>

      <div style={{ flex: 1, overflow: "hidden" }}>
        {info.suggestions.slice(0, MAX_ROWS).map((s) => (
          <button
            key={s}
            onClick={() => choose(s)}
            disabled={busy}
            style={{
              display: "block",
              width: "100%",
              textAlign: "left",
              padding: "8px 12px",
              fontSize: 13,
              background: "transparent",
              border: "none",
              color: "#fafafa",
              cursor: busy ? "default" : "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = "rgba(255,255,255,0.08)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
            }}
          >
            {s}
          </button>
        ))}
      </div>
    </div>
  );
}
