import { useCallback, useEffect, useRef, useState } from "react";

// Keys that are valid as the "main" (non-modifier) key.
const IGNORED_AS_MAIN = new Set([
  "Control", "Meta", "Alt", "Shift",
  "CapsLock", "NumLock", "ScrollLock",
]);

// Map some special browser key names to Tauri names.
const KEY_ALIAS: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Escape: "Escape",
  Tab: "Tab",
  Enter: "Return",
  Backspace: "Backspace",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
};

// Single keys that are "safe" to use without any modifier (won't break typing).
const SAFE_SOLO_KEYS = new Set([
  "F1","F2","F3","F4","F5","F6","F7","F8","F9","F10","F11","F12",
  "PageUp","PageDown","Home","End","Insert","Pause","ScrollLock","PrintScreen",
]);

// Main keys the Tauri global-shortcut parser actually accepts. Anything
// outside this set (ContextMenu, media keys, IME keys, …) would fail to
// register in Rust — so refuse to capture it in the first place.
const SUPPORTED_NAMED_MAIN = new Set([
  "Space", "Up", "Down", "Left", "Right", "Escape", "Tab", "Return",
  "Backspace", "Delete", "Home", "End", "PageUp", "PageDown", "Insert",
  "Pause", "ScrollLock", "PrintScreen", "NumLock", "CapsLock",
]);

function isSupportedMain(main: string): boolean {
  return (
    /^[A-Z0-9]$/.test(main) ||
    /^F([1-9]|1[0-9]|2[0-4])$/.test(main) ||
    SUPPORTED_NAMED_MAIN.has(main)
  );
}

function buildAccelerator(e: KeyboardEvent): string | null {
  if (IGNORED_AS_MAIN.has(e.key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");

  const main = KEY_ALIAS[e.key] ?? (e.key.length === 1 ? e.key.toUpperCase() : e.key);
  if (!isSupportedMain(main)) return null;
  parts.push(main);

  return parts.join("+");
}

// Returns a warning string when a risky bare key is used, null otherwise.
function soloKeyWarning(accelerator: string): string | null {
  const parts = accelerator.split("+");
  if (parts.length !== 1) return null; // has modifiers — fine
  const key = parts[0];
  if (SAFE_SOLO_KEYS.has(key)) return null; // safe dedicated key — fine
  if (/^[A-Z0-9]$/.test(key))
    return "Single letter/digit keys will intercept all normal typing.";
  if (["Space","Tab","Return","Backspace","Delete","Escape","Up","Down","Left","Right"].includes(key))
    return "This key is used in normal navigation — it may interfere with apps.";
  return null;
}

// Render a hotkey string like "Ctrl+Shift+Space" as visual key chips.
function KeyChips({ value }: { value: string }) {
  const parts = (value || "").split("+");
  return (
    <div className="flex flex-wrap items-center gap-1">
      {parts.map((p, i) => (
        <span key={i}>
          <kbd className="rounded-md border border-sv-border bg-sv-surface-2 px-2 py-0.5 text-xs font-medium text-sv-text shadow-sm">
            {p}
          </kbd>
          {i < parts.length - 1 && (
            <span className="mx-0.5 text-xs text-sv-muted">+</span>
          )}
        </span>
      ))}
    </div>
  );
}

interface Props {
  value: string;
  onChange: (accelerator: string) => void;
  error?: string | null;
  warning?: string | null;
}

export default function HotkeyRecorder({ value, onChange, error }: Props) {
  const warning = soloKeyWarning(value);
  const [recording, setRecording] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const divRef = useRef<HTMLDivElement>(null);

  const stopRecording = useCallback(() => {
    setRecording(false);
    setPreview(null);
  }, []);

  useEffect(() => {
    if (!recording) return;

    function onKeyDown(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();

      // Escape cancels.
      if (e.key === "Escape") {
        stopRecording();
        return;
      }

      // Show live modifier preview even before a main key is pressed.
      const mods: string[] = [];
      if (e.ctrlKey) mods.push("Ctrl");
      if (e.altKey) mods.push("Alt");
      if (e.shiftKey) mods.push("Shift");
      if (e.metaKey) mods.push("Super");
      if (!IGNORED_AS_MAIN.has(e.key)) {
        const accel = buildAccelerator(e);
        if (accel) {
          onChange(accel);
          stopRecording();
          return;
        }
        // A real key was pressed but it can't be used as a global hotkey
        // (e.g. ContextMenu, media keys) — tell the user instead of
        // silently ignoring it.
        setPreview(`${e.key} — not supported, try another key`);
        return;
      }
      // Still holding only modifiers — show partial preview.
      setPreview(mods.length ? mods.join("+") + "+" : null);
    }

    function onClickOutside(e: MouseEvent) {
      if (divRef.current && !divRef.current.contains(e.target as Node)) {
        stopRecording();
      }
    }

    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("mousedown", onClickOutside);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("mousedown", onClickOutside);
    };
  }, [recording, onChange, stopRecording]);

  return (
    <div ref={divRef} className="flex flex-col items-start gap-1.5">
      <div
        onClick={() => setRecording(true)}
        tabIndex={0}
        onFocus={() => setRecording(true)}
        role="button"
        aria-label="Click to record hotkey"
        className={`flex min-w-[196px] cursor-pointer items-center justify-between gap-2 rounded-lg border px-3 py-2 transition focus:outline-none ${
          recording
            ? "border-sv-accent bg-sv-accent/10 ring-1 ring-sv-accent"
            : "border-sv-border bg-sv-bg hover:border-sv-accent/50"
        }`}
      >
        <div className="flex-1">
          {recording ? (
            preview ? (
              <div className="flex items-center gap-1 text-xs text-sv-muted">
                <kbd className="rounded border border-sv-border bg-sv-surface-2 px-1.5 py-0.5 text-xs text-sv-text">
                  {preview}
                </kbd>
                <span className="animate-pulse">…</span>
              </div>
            ) : (
              <span className="animate-pulse text-xs text-sv-accent">
                Press keys… (Esc to cancel)
              </span>
            )
          ) : (
            <KeyChips value={value} />
          )}
        </div>
        {!recording && (
          <span className="text-[10px] text-sv-muted">click to change</span>
        )}
      </div>

      {error && (
        <p className="text-[11px] text-sv-bad">{error}</p>
      )}
      {warning && !error && (
        <p className="text-[11px] text-sv-warn">⚠ {warning}</p>
      )}

      {/* Quick-pick presets — F9 is a genuine single-key option (bare
          modifier keys like solo "Alt" can't be registered as global
          hotkeys — the OS-level API has no code path for that). */}
      <div className="flex flex-wrap justify-start gap-1.5">
        {[
          "Alt+Space",
          "F9",
          "PageUp",
          "Alt+C",
        ].map((preset) => (
          <button
            key={preset}
            onClick={() => onChange(preset)}
            className={`rounded-full border px-2 py-0.5 text-[10px] transition ${
              value === preset
                ? "border-sv-accent bg-sv-accent/10 text-sv-accent"
                : "border-sv-border text-sv-muted hover:border-sv-accent/50 hover:text-sv-text"
            }`}
          >
            {preset}
          </button>
        ))}
      </div>
    </div>
  );
}
