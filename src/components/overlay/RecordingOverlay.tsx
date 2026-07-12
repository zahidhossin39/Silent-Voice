import type { RecordingState } from "../../types";
import type { TtsState } from "./OverlayApp";

// Content of the fixed-size pill window (68×22). The SAME five bars are
// always rendered and morph between states with CSS transitions
// (GPU-composited — perfectly smooth; the window itself never resizes):
//   idle       → single solid horizontal line in center (perfectly smooth capsule)
//   recording  → orange waveform (5 vertical bars scaled from guide: [6, 10, 13, 9, 5])
//   processing → gray waveform pulsing slower (same 5-bar shape, bg-sv-muted)
//   TTS        → BLUE waveform (sv-bar-tts wave) — read-aloud playback; the
//                different color + motion keeps it visually distinct from STT.
const BASE_HEIGHTS = [6, 10, 13, 9, 5];

// Sky blue — deliberately far from the orange accent so STT vs TTS is obvious.
const TTS_BLUE = "#38bdf8";

export default function RecordingOverlay({
  state,
  tts = "idle",
}: {
  state: RecordingState;
  tts?: TtsState;
}) {
  const recording = state === "recording";
  const processing = state === "processing";
  // Dictation states own the pill; TTS shows only when dictation is idle.
  const ttsActive = !recording && !processing && tts !== "idle";
  const idle = !recording && !processing && !ttsActive;

  return (
    <div
      data-tauri-drag-region
      title="Drag to move · right-click for options"
      className="flex h-full w-full cursor-move select-none items-center justify-center transition-all duration-300 ease-out"
      style={{
        gap: idle ? "0px" : "2px",
      }}
    >
      {[0, 1, 2, 3, 4].map((i) => {
        const isCenter = i === 2;
        let w = "2px";
        let h = "2px";
        let opacity = 1;

        if (idle) {
          w = isCenter ? "20px" : "0px";
          h = "2px";
          opacity = isCenter ? 1 : 0;
        } else {
          w = "2px";
          h = `${BASE_HEIGHTS[i]}px`;
          opacity = 1;
        }

        return (
          <span
            key={i}
            className={`rounded-full transition-all duration-300 ease-out ${recording
              ? "sv-bar bg-sv-accent"
              : processing
                ? "sv-bar-slow bg-sv-muted"
                : ttsActive
                  ? tts === "speaking"
                    ? "sv-bar-tts"
                    : "sv-bar-slow"
                  : "bg-sv-muted"
              }`}
            style={{
              width: w,
              height: h,
              opacity: ttsActive && tts === "synthesizing" ? 0.7 : opacity,
              background: ttsActive ? TTS_BLUE : undefined,
              animationDelay: `${i * 0.1}s`,
            }}
          />
        );
      })}
    </div>
  );
}
