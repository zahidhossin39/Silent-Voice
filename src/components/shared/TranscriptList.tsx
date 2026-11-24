import { useMemo, useRef, useState } from "react";
import { useHistoryStore } from "../../stores/historyStore";
import { useSettingsStore } from "../../stores/settingsStore";
import TranscriptCard from "./TranscriptCard";
import { useAnnounceStore } from "../../stores/announceStore";
import type { HistoryEntry } from "../../types";

export default function TranscriptList({ emptyState }: { emptyState?: React.ReactNode }) {
  const entries = useHistoryStore((s) => s.entries);
  const update = useHistoryStore((s) => s.update);
  const retranscribe = useHistoryStore((s) => s.retranscribe);
  const remove = useHistoryStore((s) => s.remove);
  const restore = useHistoryStore((s) => s.restore);
  const announce = useAnnounceStore((s) => s.announce);
  const blurHistory = useSettingsStore((s) => s.settings.blur_history);
  const hotkey = useSettingsStore((s) => s.settings.hotkey);

  const [query, setQuery] = useState("");
  const [learnedMsg, setLearnedMsg] = useState<string | null>(null);
  const [undoEntry, setUndoEntry] = useState<HistoryEntry | null>(null);
  const undoTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Memoized so the substring scan (and toLowerCase per entry) doesn't rerun on
  // every unrelated render — matters when history holds thousands of entries.
  const q = query.toLowerCase();
  const filtered = useMemo(
    () =>
      entries.filter((e) =>
        (e.processed_text + e.raw_text).toLowerCase().includes(q)
      ),
    [entries, q]
  );

  function handleLearn(msg: string) {
    setLearnedMsg(msg);
    announce(msg);
  }

  // Delete with a 6-second undo window instead of an instant, permanent loss.
  function handleRemove(id: number) {
    const entry = entries.find((e) => e.id === id);
    remove(id);
    if (!entry) return;
    setUndoEntry(entry);
    if (undoTimer.current) clearTimeout(undoTimer.current);
    undoTimer.current = setTimeout(() => setUndoEntry(null), 6000);
  }

  function handleUndo() {
    if (undoTimer.current) clearTimeout(undoTimer.current);
    if (undoEntry) restore(undoEntry);
    setUndoEntry(null);
  }

  const undoToast = undoEntry && (
    <div className="fixed bottom-5 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg border border-sv-border bg-sv-surface-2 px-4 py-2.5 text-sm shadow-xl">
      <span>Transcription deleted</span>
      <button
        onClick={handleUndo}
        className="font-semibold text-sv-accent hover:underline"
      >
        Undo
      </button>
    </div>
  );

  if (entries.length === 0) {
    return (
      <>
        {emptyState || (
          <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-sv-border px-8 py-12 text-center">
            <span className="grid h-11 w-11 place-items-center rounded-xl bg-sv-surface-2 text-sv-muted" aria-hidden>
              <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="3" width="6" height="11" rx="3" />
                <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
              </svg>
            </span>
            <p className="text-sm font-medium text-sv-text">No dictations yet</p>
            {/* The hotkey stays on one line: its keys never wrap apart. */}
            <div className="flex flex-wrap items-center justify-center gap-x-1.5 gap-y-2 text-sm text-sv-muted">
              Hold
              <span className="inline-flex items-center gap-1 whitespace-nowrap">
                {hotkey.split("+").map((k, i, arr) => (
                  <span key={i} className="inline-flex items-center gap-1">
                    <kbd className="rounded-md border border-sv-border bg-sv-surface-2 px-2 py-0.5 text-xs font-medium text-sv-text">{k}</kbd>
                    {i < arr.length - 1 && <span className="text-xs">+</span>}
                  </span>
                ))}
              </span>
              anywhere and speak.
            </div>
            <p className="text-xs text-sv-muted">What you say shows up here, ready to copy or fix.</p>
          </div>
        )}
        {undoToast}
      </>
    );
  }

  return (
    <>
      {undoToast}
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search transcriptions…"
        className="mb-4 w-full rounded-lg border border-sv-border bg-sv-surface px-3 py-2 text-sm"
      />

      {learnedMsg && (
        <div className="mb-4 rounded-lg border border-sv-good/30 bg-sv-good/10 px-3 py-2 text-xs text-sv-good">
          {learnedMsg}
        </div>
      )}

      {filtered.length === 0 ? (
        <div className="rounded-xl border border-dashed border-sv-border bg-sv-surface p-8 text-center text-sm text-sv-muted">
          No matches for your search.
        </div>
      ) : (
        <ul className="space-y-3">
          {filtered.map((e) => (
            <TranscriptCard
              key={e.id}
              entry={e}
              blurred={blurHistory}
              onUpdate={update}
              onRetranscribe={retranscribe}
              onRemove={handleRemove}
              onLearn={handleLearn}
            />
          ))}
        </ul>
      )}
    </>
  );
}
