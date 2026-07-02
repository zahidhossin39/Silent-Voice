import WaveformVisualizer from "../shared/WaveformVisualizer";
import type { RecordingState } from "../../types";

// Content centered inside the opaque pill window (the window itself is the
// pill). One visual element — the waveform — whose color carries the state:
//   idle       → short muted line
//   recording  → orange animated bars (brand accent)
//   processing → muted animated bars (still working, not listening)
// No red dot: red next to the orange brand accent clashed badly.
export default function RecordingOverlay({ state }: { state: RecordingState }) {
  const idle = state === "idle";
  const recording = state === "recording";

  return (
    <div
      data-tauri-drag-region
      title="Drag to move · right-click for options"
      className="flex h-full w-full cursor-move select-none items-center justify-center gap-2"
    >
      {idle ? (
        // A single short line — minimal at rest.
        <span className="h-[3px] w-5 rounded-full bg-sv-muted" />
      ) : (
        <WaveformVisualizer
          active
          bars={5}
          barClass={recording ? "bg-sv-accent" : "bg-sv-muted"}
        />
      )}
    </div>
  );
}
