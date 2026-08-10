import { useEffect, useState } from "react";
import RecordingOverlay from "./RecordingOverlay";
import {
  listenEvent,
  setOverlaySize,
  hideSelfWindow,
  quitApp,
  ttsPause,
  ttsResume,
  ttsStop,
} from "../../services/tauriBridge";
import type { RecordingState } from "../../types";

// Read-aloud playback state (mirrors the Rust `tts://state` event).
export type TtsState = "idle" | "synthesizing" | "speaking" | "paused";

// Opaque pill window (matches overlay.rs). The window stays a FIXED size for
// all dictation states — resizing a WebView2 window is unavoidably janky on
// Windows, so every idle/recording/processing transition is a CSS animation
// inside the pill instead. Only the right-click menu changes the window size.
const PILL = { w: 58, h: 22 };
const MENU = { w: 190, h: 152 };
const TTS_BAR = { w: 132, h: 30 };

// Near-black pill fill (darker than the app surface) — matches the reference
// look: compact dark capsule + subtle outline + orange waveform.
const PILL_BG = "#0e1116";

export default function OverlayApp() {
  const [state, setState] = useState<RecordingState>("idle");
  const [tts, setTts] = useState<TtsState>("idle");
  const [level, setLevel] = useState(0);
  const [menuOpen, setMenuOpen] = useState(false);

  const ttsControls = (tts === "speaking" || tts === "paused") && state === "idle";

  // Opaque dark fill (this window is the pill; DWM rounds its corners).
  useEffect(() => {
    document.documentElement.style.background = PILL_BG;
    document.body.style.background = PILL_BG;
    document.body.style.overflow = "hidden";
  }, []);

  useEffect(() => {
    const unlisten = listenEvent<{ state: RecordingState }>(
      "pipeline://state",
      (p) => setState(p.state)
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Live mic loudness (0–100) drives the recording waveform's bar heights.
  useEffect(() => {
    const unlisten = listenEvent<number>("pipeline://level", (v) => setLevel(v));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Read-aloud (TTS) state — shows a distinct blue waveform in the pill so
  // the user can see TTS is working (and tell it apart from dictation).
  useEffect(() => {
    const unlisten = listenEvent<TtsState>("tts://state", (p) => setTts(p));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Window resize only when the menu opens/closes — never for state changes.
  useEffect(() => {
    if (menuOpen) setOverlaySize(MENU.w, MENU.h);
    else if (ttsControls) setOverlaySize(TTS_BAR.w, TTS_BAR.h);
    else setOverlaySize(PILL.w, PILL.h);
  }, [menuOpen, ttsControls]);

  return (
    <div
      data-tauri-drag-region
      onContextMenu={(e) => {
        e.preventDefault();
        setMenuOpen(true);
      }}
      className={`flex h-full w-full items-center justify-center overflow-hidden ${menuOpen ? "" : "rounded-full border border-[#262c3d]"
        }`}
      style={{ background: PILL_BG }}
    >
      {menuOpen ? (
        <ContextMenu
          onHide={() => {
            setMenuOpen(false);
            hideSelfWindow();
          }}
          onQuit={quitApp}
          onClose={() => setMenuOpen(false)}
        />
      ) : ttsControls ? (
        <TtsControlBar tts={tts} />
      ) : (
        <RecordingOverlay state={state} tts={tts} level={level} />
      )}
    </div>
  );
}

function TtsControlBar({ tts }: { tts: TtsState }) {
  const paused = tts === "paused";
  return (
    <div className="flex h-full w-full items-center justify-center gap-1.5 px-2">
      <button
        onClick={() => (paused ? ttsResume() : ttsPause())}
        title={paused ? "Resume" : "Pause"}
        className="flex h-5 w-5 items-center justify-center rounded-full text-[11px] text-[#38bdf8] hover:bg-white/10"
      >
        {paused ? "▶" : "❚❚"}
      </button>
      <button
        onClick={() => ttsStop()}
        title="Stop"
        className="flex h-5 w-5 items-center justify-center rounded-full text-[11px] text-sv-bad hover:bg-white/10"
      >
        ■
      </button>
    </div>
  );
}

function ContextMenu({
  onHide,
  onQuit,
  onClose,
}: {
  onHide: () => void;
  onQuit: () => void;
  onClose: () => void;
}) {
  return (
    <div className="w-full p-1.5">
      <div className="overflow-hidden rounded-lg border border-sv-border bg-sv-surface">
        <MenuButton label="✕  Hide overlay" onClick={onHide} />
        <MenuButton label="↩  Dismiss menu" onClick={onClose} />
        <div className="h-px bg-sv-border" />
        <MenuButton label="⏻  Quit Silent Voice" onClick={onQuit} danger />
      </div>
    </div>
  );
}

function MenuButton({
  label,
  onClick,
  danger,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`block w-full px-3 py-2 text-left text-xs hover:bg-sv-surface-2 ${danger ? "text-sv-bad" : "text-sv-text"
        }`}
    >
      {label}
    </button>
  );
}
