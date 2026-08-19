import { useState, useEffect } from "react";
import Page from "../shared/Page";
import { useHistoryStore } from "../../stores/historyStore";
import { useSettingsStore } from "../../stores/settingsStore";
import TranscriptList from "../shared/TranscriptList";
import ConfirmDialog from "../shared/ConfirmDialog";
import ScrollNumberPicker from "../shared/ScrollNumberPicker";
import { pruneAudioClips } from "../../services/tauriBridge";
import { Toggle } from "./Settings";
import type { Settings } from "../../types";

const RETENTION_OPTIONS: { value: Settings["history_retention"]; label: string }[] = [
  { value: "never", label: "Never" },
  { value: "3d", label: "After 3 days" },
  { value: "2w", label: "After 2 weeks" },
  { value: "3m", label: "After 3 months" },
  { value: "custom", label: "Custom…" },
];

const RETENTION_UNITS: Settings["history_retention_custom_unit"][] = [
  "hours",
  "days",
  "weeks",
  "months",
];

const SELECT_CLS =
  "h-9 rounded-lg border border-sv-border bg-sv-bg px-3 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent";

// Persisted so the first-run coachmark for the Storage gear shows exactly once,
// ever — not once per session.
const STORAGE_HINT_KEY = "sv-history-storage-hint";

export default function History() {
  const entries = useHistoryStore((s) => s.entries);
  const clear = useHistoryStore((s) => s.clear);
  const reprune = useHistoryStore((s) => s.reprune);
  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const [confirmClear, setConfirmClear] = useState(false);
  const [storageOpen, setStorageOpen] = useState(false);
  const [coach, setCoach] = useState(false);

  const dismissCoach = () => {
    localStorage.setItem(STORAGE_HINT_KEY, "1");
    setCoach(false);
  };

  // First-ever visit: after a beat, point the user at the gear once. The short
  // delay lets the page settle so the callout animates in, not mid-navigation.
  useEffect(() => {
    if (localStorage.getItem(STORAGE_HINT_KEY)) return;
    const t = setTimeout(() => setCoach(true), 550);
    return () => clearTimeout(t);
  }, []);

  // Enforce the limit/retention whenever those settings change. This lived in
  // Settings before the controls moved here — it must run wherever the controls
  // are, or trimming never happens on change.
  useEffect(() => {
    reprune();
  }, [
    settings.history_limit,
    settings.history_retention,
    settings.history_retention_custom_value,
    settings.history_retention_custom_unit,
    reprune,
  ]);

  // Esc closes the slide-over.
  useEffect(() => {
    if (!storageOpen) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setStorageOpen(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [storageOpen]);

  const openStorage = () => {
    setStorageOpen(true);
    if (coach) dismissCoach();
  };

  return (
    <Page
      title="History"
      subtitle="Past transcriptions. Edit one to fix mistakes — corrections teach the app new words."
      actions={
        <div className="flex items-center gap-2">
          <div className="relative">
            <button
              onClick={openStorage}
              aria-label="Storage settings"
              title="Storage settings"
              className={`sv-gear grid h-9 w-9 place-items-center rounded-lg border text-sv-muted transition hover:border-sv-accent hover:text-sv-accent ${
                coach ? "sv-gear-coach border-sv-accent text-sv-accent" : "border-sv-border"
              }`}
            >
              <GearIcon />
            </button>
            {coach && <StorageCoach onOpen={openStorage} onDismiss={dismissCoach} />}
          </div>
          {entries.length > 0 && (
            <button
              onClick={() => setConfirmClear(true)}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-sm text-sv-muted hover:text-sv-bad"
            >
              Clear all
            </button>
          )}
        </div>
      }
    >
      <TranscriptList />

      <StorageSheet
        open={storageOpen}
        onClose={() => setStorageOpen(false)}
        settings={settings}
        setSettings={setSettings}
      />

      <ConfirmDialog
        open={confirmClear}
        title="Clear all history?"
        message={
          <>
            This permanently deletes all {entries.length} saved transcription
            {entries.length === 1 ? "" : "s"} and their audio. This can't be undone.
          </>
        }
        confirmLabel="Clear all"
        onConfirm={() => {
          clear();
          setConfirmClear(false);
        }}
        onCancel={() => setConfirmClear(false)}
      />
    </Page>
  );
}

// Right-side slide-over holding the Storage controls. Deliberately styled as
// rounded "tiles" (not the flat divided rows of the Settings page) so it reads
// as its own focused surface.
function StorageSheet({
  open,
  onClose,
  settings,
  setSettings,
}: {
  open: boolean;
  onClose: () => void;
  settings: Settings;
  setSettings: (patch: Partial<Settings>) => void;
}) {
  return (
    <div className={`fixed inset-0 z-50 ${open ? "" : "pointer-events-none"}`}>
      {/* Scrim */}
      <div
        onClick={onClose}
        className={`absolute inset-0 bg-black/50 backdrop-blur-[2px] transition-opacity duration-200 ${
          open ? "opacity-100" : "opacity-0"
        }`}
      />
      {/* Panel */}
      <aside
        role="dialog"
        aria-modal="true"
        aria-label="Storage settings"
        className={`sv-sheet absolute right-0 top-0 flex h-full w-[400px] max-w-[92vw] flex-col border-l border-sv-border bg-sv-surface shadow-2xl transition-transform duration-300 ease-out ${
          open ? "translate-x-0 is-open" : "translate-x-full"
        }`}
      >
        {/* Accent strip at the very top — a small signature that sets the panel
            apart from the plain Settings cards. */}
        <div className="h-1 shrink-0 bg-gradient-to-r from-sv-accent via-sv-accent/60 to-transparent" />

        <header className="flex items-start gap-3 px-5 pb-4 pt-4">
          <span className="mt-0.5 grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-sv-accent/15 text-sv-accent">
            <ArchiveIcon />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="text-[15px] font-semibold text-sv-text">Storage</h2>
            <p className="mt-0.5 text-[12px] leading-relaxed text-sv-muted">
              How much history to keep and whether to save the original audio.
            </p>
          </div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="grid h-8 w-8 shrink-0 place-items-center rounded-lg text-sv-muted transition hover:bg-sv-surface-2 hover:text-sv-text"
          >
            <CloseIcon />
          </button>
        </header>

        <div className="flex-1 space-y-2.5 overflow-y-auto px-5 pb-6">
          <Tile label="History limit" hint="Keep at most this many transcriptions">
            <div className="flex items-center gap-2.5">
              <ScrollNumberPicker
                value={settings.history_limit}
                onChange={(v) => setSettings({ history_limit: v })}
                min={1}
                max={10000}
              />
              <span className="text-sm text-sv-muted">entries</span>
            </div>
          </Tile>

          <Tile label="Auto-delete" hint="Remove entries older than" stack>
            <div className="flex items-center gap-2.5">
              <select
                value={settings.history_retention}
                onChange={(e) =>
                  setSettings({
                    history_retention: e.target.value as Settings["history_retention"],
                  })
                }
                className={SELECT_CLS}
              >
                {RETENTION_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
              {settings.history_retention === "custom" && (
                <>
                  <ScrollNumberPicker
                    value={settings.history_retention_custom_value}
                    onChange={(v) => setSettings({ history_retention_custom_value: v })}
                    min={1}
                    max={999}
                    width={60}
                  />
                  <select
                    value={settings.history_retention_custom_unit}
                    onChange={(e) =>
                      setSettings({
                        history_retention_custom_unit: e.target
                          .value as Settings["history_retention_custom_unit"],
                      })
                    }
                    className={SELECT_CLS}
                  >
                    {RETENTION_UNITS.map((u) => (
                      <option key={u} value={u}>
                        {u}
                      </option>
                    ))}
                  </select>
                </>
              )}
            </div>
          </Tile>

          <Tile label="Save audio of each dictation" hint="Lets you replay and copy the original recording.">
            <Toggle
              checked={settings.save_audio}
              onChange={async (v) => {
                setSettings({ save_audio: v });
                if (v) await pruneAudioClips(settings.audio_clip_limit);
              }}
            />
          </Tile>

          {settings.save_audio && (
            <Tile label="Recordings to keep" hint="About 1 MB each. Older recordings are deleted first.">
              <div className="flex items-center gap-2.5">
                <ScrollNumberPicker
                  value={settings.audio_clip_limit}
                  onChange={async (v) => {
                    setSettings({ audio_clip_limit: v });
                    await pruneAudioClips(v);
                  }}
                  min={1}
                  max={500}
                />
                <span className="text-sm text-sv-muted">recordings</span>
              </div>
            </Tile>
          )}

          <Tile label="Blur transcripts until hover" hint="Keeps what you dictated hidden when your screen is visible to others">
            <Toggle
              checked={settings.blur_history}
              onChange={(v) => setSettings({ blur_history: v })}
            />
          </Tile>
        </div>
      </aside>
    </div>
  );
}

// A single setting as a rounded tile. `stack` drops the control to its own line
// (used by Auto-delete, whose custom row of pickers is too wide to sit inline).
function Tile({
  label,
  hint,
  stack,
  children,
}: {
  label: string;
  hint?: string;
  stack?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className="sv-sheet-row rounded-xl border border-sv-border bg-sv-surface-2/40 p-3.5 transition-colors hover:border-sv-border hover:bg-sv-surface-2/70">
      <div className={stack ? "space-y-3" : "flex items-center justify-between gap-4"}>
        <div className="min-w-0">
          <div className="text-[13px] font-medium text-sv-text">{label}</div>
          {hint && (
            <div className="mt-0.5 text-[11px] leading-relaxed text-sv-muted">{hint}</div>
          )}
        </div>
        <div className={stack ? "" : "shrink-0"}>{children}</div>
      </div>
    </div>
  );
}

// First-run callout that points at the Storage gear. Shown once, ever.
function StorageCoach({ onOpen, onDismiss }: { onOpen: () => void; onDismiss: () => void }) {
  return (
    <div
      role="dialog"
      aria-label="New: Storage settings"
      className="sv-coach absolute right-0 top-full z-40 mt-3 w-[264px] rounded-xl border border-sv-accent/50 bg-sv-surface p-3.5 shadow-2xl"
    >
      {/* Arrow pointing up to the gear */}
      <span className="absolute -top-[6px] right-3.5 h-3 w-3 rotate-45 border-l border-t border-sv-accent/50 bg-sv-surface" />
      <div className="flex items-start gap-2.5">
        <span className="mt-0.5 grid h-7 w-7 shrink-0 place-items-center rounded-lg bg-sv-accent/15 text-sv-accent">
          <SparkleIcon />
        </span>
        <div className="min-w-0">
          <div className="text-[13px] font-semibold text-sv-text">History settings live here</div>
          <div className="mt-0.5 text-[11.5px] leading-relaxed text-sv-muted">
            Set how many transcriptions to keep, auto-delete, save audio, and more.
          </div>
        </div>
      </div>
      <div className="mt-3 flex items-center justify-end gap-2">
        <button
          onClick={onDismiss}
          className="rounded-lg px-2.5 py-1 text-[12px] text-sv-muted transition hover:text-sv-text"
        >
          Got it
        </button>
        <button
          onClick={onOpen}
          className="rounded-lg bg-sv-accent px-3 py-1 text-[12px] font-semibold text-sv-on-accent transition hover:bg-sv-accent-hover"
        >
          Open settings
        </button>
      </div>
    </div>
  );
}

function SparkleIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3l1.8 4.9L19 9.7l-4.2 2.9L16 18l-4-3-4 3 1.2-5.4L5 9.7l5.2-1.8z" />
    </svg>
  );
}

function GearIcon() {
  return (
    <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19 12a7 7 0 0 0-.1-1l2-1.6-2-3.4-2.3 1a7 7 0 0 0-1.7-1L14.5 2h-4l-.4 2.4a7 7 0 0 0-1.7 1l-2.3-1-2 3.4L4.1 11a7 7 0 0 0 0 2l-2 1.6 2 3.4 2.3-1a7 7 0 0 0 1.7 1L10.5 22h4l.4-2.4a7 7 0 0 0 1.7-1l2.3 1 2-3.4-2-1.6c.1-.3.1-.7.1-1z" />
    </svg>
  );
}

function ArchiveIcon() {
  return (
    <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="4" width="18" height="4" rx="1" />
      <path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8" />
      <path d="M10 12h4" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  );
}
