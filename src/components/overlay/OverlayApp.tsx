import { useEffect, useState } from "react";
import RecordingOverlay from "./RecordingOverlay";
import {
  listenEvent,
  setOverlaySize,
  hideSelfWindow,
  quitApp,
} from "../../services/tauriBridge";
import type { RecordingState } from "../../types";

// Opaque pill window (matches overlay.rs). The window is sized to the pill and
// resized between idle and recording. Center-anchored resize (Rust) + shadow
// off keeps it exactly where you drag it (no drift).
const IDLE = { w: 54, h: 20 };
const ACTIVE = { w: 96, h: 26 };
const MENU = { w: 190, h: 152 };

export default function OverlayApp() {
  const [state, setState] = useState<RecordingState>("idle");
  const [menuOpen, setMenuOpen] = useState(false);

  // Opaque dark fill (this window is the pill; DWM rounds its corners).
  useEffect(() => {
    document.documentElement.style.background = "var(--color-sv-surface)";
    document.body.style.background = "var(--color-sv-surface)";
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

  // Resize the window: small idle, expanded while recording, large for menu.
  useEffect(() => {
    if (menuOpen) setOverlaySize(MENU.w, MENU.h);
    else if (state === "idle") setOverlaySize(IDLE.w, IDLE.h);
    else setOverlaySize(ACTIVE.w, ACTIVE.h);
  }, [state, menuOpen]);

  return (
    <div
      data-tauri-drag-region
      onContextMenu={(e) => {
        e.preventDefault();
        setMenuOpen(true);
      }}
      className="flex h-full w-full items-center justify-center overflow-hidden bg-sv-surface"
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
      ) : (
        <RecordingOverlay state={state} />
      )}
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
      className={`block w-full px-3 py-2 text-left text-xs hover:bg-sv-surface-2 ${
        danger ? "text-sv-bad" : "text-sv-text"
      }`}
    >
      {label}
    </button>
  );
}
