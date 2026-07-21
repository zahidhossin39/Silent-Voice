import { useEffect, useState } from "react";
import Page from "../shared/Page";
import { useHistoryStore } from "../../stores/historyStore";
import { useSettingsStore } from "../../stores/settingsStore";
import type { Settings } from "../../types";

// Proofreading squiggles were removed from History at the user's request;
// inline system-wide proofreading in other apps is unaffected.
// Words from a correction worth learning: real words (letters, 3+ chars) that
// didn't appear anywhere in the original transcription.
function newWordsFromCorrection(original: string, corrected: string): string[] {
  const tokenize = (s: string) =>
    s
      .toLowerCase()
      .split(/[^\p{L}\p{N}'-]+/u)
      .filter(Boolean);
  const before = new Set(tokenize(original));
  const seen = new Set<string>();
  const out: string[] = [];
  for (const w of tokenize(corrected)) {
    if (w.length < 3) continue;
    if (!/\p{L}/u.test(w)) continue; // must contain a letter
    if (before.has(w) || seen.has(w)) continue;
    seen.add(w);
    out.push(w);
    if (out.length >= 10) break; // don't flood the vocabulary from one edit
  }
  return out;
}

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

export default function History() {
  const entries = useHistoryStore((s) => s.entries);
  const update = useHistoryStore((s) => s.update);
  const remove = useHistoryStore((s) => s.remove);
  const clear = useHistoryStore((s) => s.clear);
  const reprune = useHistoryStore((s) => s.reprune);
  const vocabulary = useSettingsStore((s) => s.settings.custom_vocabulary);
  const historyLimit = useSettingsStore((s) => s.settings.history_limit);
  const historyRetention = useSettingsStore((s) => s.settings.history_retention);
  const customValue = useSettingsStore((s) => s.settings.history_retention_custom_value);
  const customUnit = useSettingsStore((s) => s.settings.history_retention_custom_unit);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const [query, setQuery] = useState("");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [learnedMsg, setLearnedMsg] = useState<string | null>(null);

  // Re-apply the count/age caps whenever any of them change.
  useEffect(() => {
    reprune();
  }, [historyLimit, historyRetention, customValue, customUnit, reprune]);

  const filtered = entries.filter((e) =>
    (e.processed_text + e.raw_text)
      .toLowerCase()
      .includes(query.toLowerCase())
  );

  function startEdit(id: number, current: string) {
    setEditingId(id);
    setDraft(current);
    setLearnedMsg(null);
  }

  function saveEdit(id: number, original: string) {
    const corrected = draft.trim();
    setEditingId(null);
    if (!corrected || corrected === original) return;

    update(id, corrected);

    // Learn: any genuinely new words go into the custom vocabulary so Whisper
    // is primed to hear them correctly next time.
    const existing = new Set(
      vocabulary
        .split(/[,\n]/)
        .map((w) => w.trim().toLowerCase())
        .filter(Boolean)
    );
    const learned = newWordsFromCorrection(original, corrected).filter(
      (w) => !existing.has(w)
    );
    if (learned.length > 0) {
      const joined = vocabulary.trim()
        ? `${vocabulary.trim().replace(/,\s*$/, "")}, ${learned.join(", ")}`
        : learned.join(", ");
      setSettings({ custom_vocabulary: joined });
      setLearnedMsg(
        `Learned ${learned.length} new word${learned.length > 1 ? "s" : ""}: ${learned.join(", ")} — added to your custom vocabulary.`
      );
    }
  }

  return (
    <Page
      title="History"
      subtitle="Past transcriptions. Edit one to fix mistakes — corrections teach the app new words."
      actions={
        entries.length > 0 && (
          <button
            onClick={clear}
            className="rounded-lg border border-sv-border px-3 py-1.5 text-sm text-sv-muted hover:text-sv-bad"
          >
            Clear all
          </button>
        )
      }
    >
      {/* History controls: count cap + auto-delete window */}
      <div className="mb-4 rounded-xl border border-sv-border bg-sv-surface p-4">
        <div className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-sv-muted">
          History
        </div>
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center justify-between gap-3 sm:justify-start">
            <div>
              <div className="text-sm font-medium">History limit</div>
              <div className="text-[11px] text-sv-muted">
                Keep at most this many transcriptions
              </div>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="number"
                min={1}
                max={10000}
                value={historyLimit}
                onChange={(e) =>
                  setSettings({
                    history_limit: Math.max(1, Math.min(10000, Number(e.target.value) || 1)),
                  })
                }
                className="w-20 rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-right text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
              />
              <span className="text-xs text-sv-muted">entries</span>
            </div>
          </div>
          <div className="flex items-center justify-between gap-3 sm:justify-start">
            <div>
              <div className="text-sm font-medium">Auto-delete</div>
              <div className="text-[11px] text-sv-muted">
                Remove entries older than
              </div>
            </div>
            <div className="flex items-center gap-2">
              <select
                value={historyRetention}
                onChange={(e) =>
                  setSettings({
                    history_retention: e.target.value as Settings["history_retention"],
                  })
                }
                className="rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
              >
                {RETENTION_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
              {historyRetention === "custom" && (
                <>
                  <input
                    type="number"
                    min={1}
                    max={999}
                    value={customValue}
                    onChange={(e) =>
                      setSettings({
                        history_retention_custom_value: Math.max(
                          1,
                          Math.min(999, Number(e.target.value) || 1)
                        ),
                      })
                    }
                    className="w-16 rounded-lg border border-sv-border bg-sv-bg px-2 py-1.5 text-right text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
                  />
                  <select
                    value={customUnit}
                    onChange={(e) =>
                      setSettings({
                        history_retention_custom_unit: e.target
                          .value as Settings["history_retention_custom_unit"],
                      })
                    }
                    className="rounded-lg border border-sv-border bg-sv-bg px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
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
          </div>
        </div>
      </div>

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
          {entries.length === 0
            ? "No transcriptions yet."
            : "No matches for your search."}
        </div>
      ) : (
        <ul className="space-y-3">
          {filtered.map((e) => {
            const displayed = e.processed_text || e.raw_text;
            const isEditing = editingId === e.id;
            return (
              <li
                key={e.id}
                className="rounded-xl border border-sv-border bg-sv-surface p-4"
              >
                <div className="mb-1 flex items-center justify-between text-xs text-sv-muted">
                  <span>
                    {new Date(e.timestamp).toLocaleString()} · {e.mode_id} ·{" "}
                    {e.model_id}
                  </span>
                  <div className="flex gap-2">
                    {!isEditing && (
                      <button
                        onClick={() => startEdit(e.id, displayed)}
                        className="hover:text-sv-text"
                        title="Fix mistakes — new words are learned automatically"
                      >
                        Edit
                      </button>
                    )}
                    <button
                      onClick={() =>
                        navigator.clipboard.writeText(displayed)
                      }
                      className="hover:text-sv-text"
                    >
                      Copy
                    </button>
                    <button
                      onClick={() => remove(e.id)}
                      className="hover:text-sv-bad"
                    >
                      Delete
                    </button>
                  </div>
                </div>

                {isEditing ? (
                  <div>
                    <textarea
                      value={draft}
                      onChange={(ev) => setDraft(ev.target.value)}
                      rows={Math.min(6, Math.max(2, Math.ceil(draft.length / 90)))}
                      autoFocus
                      className="w-full resize-y rounded-lg border border-sv-accent/50 bg-sv-bg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
                    />
                    <div className="mt-2 flex gap-2">
                      <button
                        onClick={() => saveEdit(e.id, displayed)}
                        className="rounded-lg bg-sv-accent px-3 py-1.5 text-xs font-medium text-white hover:bg-sv-accent-hover"
                      >
                        Save correction
                      </button>
                      <button
                        onClick={() => setEditingId(null)}
                        className="rounded-lg border border-sv-border px-3 py-1.5 text-xs text-sv-muted hover:text-sv-text"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                ) : (
                  <p className="text-sm">{displayed}</p>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </Page>
  );
}
