import { useState } from "react";
import type { HistoryEntry } from "../../types";
import { pasteText, copyToClipboard } from "../../services/tauriBridge";
import { useSettingsStore } from "../../stores/settingsStore";

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

interface TranscriptCardProps {
  entry: HistoryEntry;
  blurred: boolean;
  onUpdate: (id: number, text: string) => void;
  onRemove: (id: number) => void;
  onLearn: (msg: string) => void;
}

export default function TranscriptCard({
  entry,
  blurred,
  onUpdate,
  onRemove,
  onLearn,
}: TranscriptCardProps) {
  const vocabulary = useSettingsStore((s) => s.settings.custom_vocabulary);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [copied, setCopied] = useState(false);

  const displayed = entry.processed_text || entry.raw_text;

  function startEdit() {
    setEditing(true);
    setDraft(displayed);
  }

  function saveEdit() {
    const corrected = draft.trim();
    setEditing(false);
    if (!corrected || corrected === displayed) return;

    onUpdate(entry.id, corrected);

    // Learn: any genuinely new words go into the custom vocabulary so Whisper
    // is primed to hear them correctly next time.
    const existing = new Set(
      vocabulary
        .split(/[,\n]/)
        .map((w) => w.trim().toLowerCase())
        .filter(Boolean)
    );
    const learned = newWordsFromCorrection(displayed, corrected).filter(
      (w) => !existing.has(w)
    );
    if (learned.length > 0) {
      const joined = vocabulary.trim()
        ? `${vocabulary.trim().replace(/,\s*$/, "")}, ${learned.join(", ")}`
        : learned.join(", ");
      setSettings({ custom_vocabulary: joined });
      onLearn(
        `Learned ${learned.length} new word${learned.length > 1 ? "s" : ""}: ${learned.join(", ")} — added to your custom vocabulary.`
      );
    }
  }

  async function handleCopy() {
    try {
      await copyToClipboard(displayed);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.warn("copy failed", e);
    }
  }

  async function handlePaste() {
    try {
      await pasteText(displayed);
    } catch (e) {
      console.warn("paste failed", e);
    }
  }

  return (
    <li className="group rounded-xl border border-sv-border bg-sv-surface p-4">
      <div className="mb-1 flex items-center justify-between text-xs text-sv-muted">
        <span>
          {new Date(entry.timestamp).toLocaleString()} · {entry.mode_id} ·{" "}
          {entry.model_id}
        </span>
        <div className="flex gap-2">
          {!editing && (
            <button
              onClick={startEdit}
              className="hover:text-sv-text"
              title="Fix mistakes — new words are learned automatically"
            >
              Edit
            </button>
          )}
          <button
            onClick={handlePaste}
            className="hover:text-sv-text"
          >
            Paste
          </button>
          <button
            onClick={handleCopy}
            className={copied ? "text-sv-good" : "hover:text-sv-text"}
          >
            {copied ? "Copied" : "Copy"}
          </button>
          <button
            onClick={() => onRemove(entry.id)}
            className="hover:text-sv-bad"
          >
            Delete
          </button>
        </div>
      </div>

      {editing ? (
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
              onClick={saveEdit}
              className="rounded-lg bg-sv-accent px-3 py-1.5 text-xs font-medium text-white hover:bg-sv-accent-hover"
            >
              Save correction
            </button>
            <button
              onClick={() => setEditing(false)}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs text-sv-muted hover:text-sv-text"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <p className={`text-sm ${blurred ? "blur-sm transition group-hover:blur-none" : ""}`}>{displayed}</p>
      )}
    </li>
  );
}
