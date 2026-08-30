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
import { applyAppTheme } from "../../services/appThemes";
import type { RecordingState, AppTheme } from "../../types";

// Read-aloud playback state (mirrors the Rust `tts://state` event).
export type TtsState = "idle" | "synthesizing" | "speaking" | "paused";

// Opaque pill window (matches overlay.rs). The window stays a FIXED size for
// all dictation states — resizing a WebView2 window is unavoidably janky on
// Windows, so every idle/recording/processing transition is a CSS animation
// inside the pill instead. Only the right-click menu changes the window size.
const PILL = { w: 58, h: 22 };
const MENU = { w: 190, h: 152 };
const TTS_BAR = { w: 132, h: 30 };
// Live transcript preview ("Transcribe while you speak"). Same one-shot resize
// the menu uses — never a tween, and only when a chunk has actually landed, so
// a dictation that never chunks keeps the plain pill it always had.
const NOTEPAD = { w: 360, h: 104 };

// Near-black pill fill (darker than the app surface) — matches the reference
// look: compact dark capsule + subtle outline + orange waveform.
const PILL_BG = "#0e1116";

export default function OverlayApp() {
  const [state, setState] = useState<RecordingState>("idle");
  const [tts, setTts] = useState<TtsState>("idle");
  const [level, setLevel] = useState(0);
  const [menuOpen, setMenuOpen] = useState(false);
  const [lines, setLines] = useState<string[]>([]);
  const [startedAt, setStartedAt] = useState(0);

  const ttsControls = (tts === "speaking" || tts === "paused") && state === "idle";
  // The notepad is earned, not announced: it opens on the first committed chunk
  // and closes the moment recording ends, so it never covers the user's work
  // while they are reading the result.
  const notepad = state === "recording" && lines.length > 0 && !menuOpen && !ttsControls;

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

  // Live app-accent changes from the main window (this pill has its own store
  // instance, so it never sees the dashboard's setSettings). The waveform is
  // bg-sv-accent, so re-pointing the accent tokens recolors it instantly.
  useEffect(() => {
    const unlisten = listenEvent<AppTheme>("app-theme://changed", (id) =>
      applyAppTheme(id)
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

  // Live transcript: each committed chunk arrives as its own line, and every
  // fresh recording clears the last one.
  useEffect(() => {
    const un = listenEvent<string>("pipeline://transcript", (t) => {
      const line = (t ?? "").trim();
      if (line) setLines((prev) => [...prev, line]);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const un = listenEvent<void>("pipeline://transcript-reset", () => {
      setLines([]);
      setStartedAt(Date.now());
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Window resize only when the menu opens/closes or the notepad appears —
  // never for state changes.
  useEffect(() => {
    if (menuOpen) setOverlaySize(MENU.w, MENU.h);
    else if (notepad) setOverlaySize(NOTEPAD.w, NOTEPAD.h);
    else if (ttsControls) setOverlaySize(TTS_BAR.w, TTS_BAR.h);
    else setOverlaySize(PILL.w, PILL.h);
  }, [menuOpen, notepad, ttsControls]);

  return (
    <div
      data-tauri-drag-region
      onContextMenu={(e) => {
        e.preventDefault();
        setMenuOpen(true);
      }}
      className={`flex h-full w-full items-center justify-center overflow-hidden ${menuOpen ? "" : notepad ? "rounded-xl border border-[#262c3d]" : "rounded-full border border-[#262c3d]"
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
      ) : notepad ? (
        <Notepad lines={lines} level={level} startedAt={startedAt} />
      ) : ttsControls ? (
        <TtsControlBar tts={tts} />
      ) : (
        <RecordingOverlay state={state} tts={tts} level={level} />
      )}
    </div>
  );
}

/// The live transcript panel. Newest line sits at the bottom where the eye
/// already is; older lines dim and slide up under a soft mask rather than
/// scrolling, so nothing ever appears half-clipped at the top edge.
function Notepad({
  lines,
  level,
  startedAt,
}: {
  lines: string[];
  level: number;
  startedAt: number;
}) {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (!startedAt) return;
    const id = setInterval(() => setElapsed(Math.floor((Date.now() - startedAt) / 1000)), 500);
    return () => clearInterval(id);
  }, [startedAt]);

  const words = lines.reduce((n, l) => n + l.split(/\s+/).filter(Boolean).length, 0);
  const mins = Math.floor(elapsed / 60);
  const secs = (elapsed % 60).toString().padStart(2, "0");

  return (
    <div className="flex h-full w-full flex-col px-3 pt-2">
      <div
        className="flex min-h-0 flex-1 flex-col justify-end overflow-hidden text-[11.5px] leading-[1.5] text-[#e9ebee]"
        style={{
          WebkitMaskImage: "linear-gradient(180deg, transparent 0, #000 18px, #000 100%)",
          maskImage: "linear-gradient(180deg, transparent 0, #000 18px, #000 100%)",
        }}
      >
        {lines.map((line, i) => (
          <p
            key={i}
            className={`m-0 mb-0.5 flex-none ${i === lines.length - 1 ? "text-[#e9ebee]" : "text-[#7b8496]"}`}
          >
            {line}
          </p>
        ))}
      </div>

      <div className="flex h-[22px] flex-none items-center gap-2 border-t border-[#262c3d]/80 text-[10px] tabular-nums text-[#7b8496]">
        <Bars level={level} />
        <span className="flex-1" />
        <span>{words} {words === 1 ? "word" : "words"}</span>
        <span aria-hidden="true">·</span>
        <span>{mins}:{secs}</span>
      </div>
    </div>
  );
}

/// The same five-bar mic meter the pill shows, shrunk to sit in the footer so
/// the notepad still says "I am listening" without a second visual language.
function Bars({ level }: { level: number }) {
  const heights = [0.45, 0.75, 1, 0.7, 0.4];
  return (
    <span className="flex flex-none items-end gap-[2px]" aria-hidden="true">
      {heights.map((h, i) => (
        <span
          key={i}
          className="w-[2px] rounded-[1px] bg-sv-accent transition-[height] duration-100"
          style={{ height: `${Math.max(2, Math.round(3 + (level / 100) * 9 * h))}px` }}
        />
      ))}
    </span>
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
        title="Stop" aria-label="Stop"
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
