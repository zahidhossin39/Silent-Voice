import { useState, useEffect, useRef } from "react";
import type { HistoryEntry } from "../../types";
import { pasteText, copyToClipboard, readAudioClip, copyAudioFile } from "../../services/tauriBridge";
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
  const [audioCopied, setAudioCopied] = useState(false);

  const [isPlaying, setIsPlaying] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [duration, setDuration] = useState(entry.audio_ms ? entry.audio_ms / 1000 : 0);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  
  useEffect(() => {
    return () => {
      if (audioUrl) URL.revokeObjectURL(audioUrl);
    };
  }, [audioUrl]);

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

  async function handleCopyAudio() {
    if (!entry.audio_file) return;
    try {
      await copyAudioFile(entry.audio_file);
      setAudioCopied(true);
      setTimeout(() => setAudioCopied(false), 1500);
    } catch (e) {
      console.warn("audio copy failed", e);
    }
  }

  // One place that turns a stored clip into a playable element. This used to be
  // copy-pasted into both play and seek, which is how a broken Blob() call
  // survived in both: the bytes arrive as an ArrayBuffer and must be passed
  // straight to Blob, never coerced.
  async function loadAudio(): Promise<HTMLAudioElement | null> {
    if (!entry.audio_file) return null;
    if (audioRef.current) return audioRef.current;
    const buffer = await readAudioClip(entry.audio_file);
    if (!buffer || buffer.byteLength === 0) {
      console.warn("audio clip missing or empty", entry.audio_file);
      return null;
    }
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    const url = URL.createObjectURL(new Blob([buffer], { type: "audio/wav" }));
    setAudioUrl(url);
    const audio = new Audio(url);
    audio.onplay = () => setIsPlaying(true);
    audio.onpause = () => setIsPlaying(false);
    audio.onended = () => {
      setIsPlaying(false);
      setProgress(0);
    };
    audio.ontimeupdate = () => setProgress(audio.currentTime);
    audio.onloadedmetadata = () => setDuration(audio.duration);
    audio.onerror = () => console.warn("audio decode failed", entry.audio_file);
    audioRef.current = audio;
    return audio;
  }

  async function togglePlay() {
    try {
      if (audioRef.current) {
        if (isPlaying) audioRef.current.pause();
        else await audioRef.current.play();
        return;
      }
      const audio = await loadAudio();
      if (audio) await audio.play();
    } catch (e) {
      console.warn("audio play failed", e);
    }
  }

  async function handleSeek(e: React.MouseEvent<HTMLDivElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    const fraction = rect.width > 0 ? x / rect.width : 0;
    try {
      const audio = audioRef.current ?? (await loadAudio());
      if (!audio) return;
      // Before metadata lands, audio.duration is NaN — fall back to the length
      // Rust recorded so a seek works on the very first click.
      const total = Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : duration;
      const target = total * fraction;
      audio.currentTime = target;
      setProgress(target);
    } catch (err) {
      console.warn("audio seek failed", err);
    }
  }

  function formatTime(s: number) {
    const mins = Math.floor(s / 60);
    const secs = Math.floor(s % 60).toString().padStart(2, "0");
    return `${mins}:${secs}`;
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
        <>
          <p className={`text-sm ${blurred ? "blur-sm transition group-hover:blur-none" : ""}`}>{displayed}</p>
          {entry.audio_file && (
            <div className="mt-3 flex items-center gap-3 border-t border-sv-border/60 pt-3">
              <button
                onClick={togglePlay}
                aria-label={isPlaying ? "Pause recording" : "Play recording"}
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-sv-border bg-sv-surface-2 text-sv-text transition-colors hover:border-sv-accent hover:text-sv-accent"
              >
                {isPlaying ? (
                  <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
                    <rect x="6" y="4" width="4" height="16" />
                    <rect x="14" y="4" width="4" height="16" />
                  </svg>
                ) : (
                  <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="5 3 19 12 5 21 5 3" />
                  </svg>
                )}
              </button>
              <div 
                className="relative h-1 flex-1 cursor-pointer rounded-full bg-sv-surface-2"
                role="slider"
                aria-valuenow={progress}
                aria-valuemin={0}
                aria-valuemax={duration || 1}
                onClick={handleSeek}
              >
                <div 
                  className="absolute left-0 top-0 h-full rounded-full bg-sv-accent" 
                  style={{ width: `${duration > 0 ? (progress / duration) * 100 : 0}%` }} 
                />
              </div>
              <span className="shrink-0 tabular-nums text-[11px] text-sv-muted">
                {formatTime(progress)} / {formatTime(duration)}
              </span>
              <button
                onClick={handleCopyAudio}
                className={`ml-auto rounded-lg border px-2.5 py-1.5 text-xs font-medium transition-colors duration-75 ${audioCopied ? "border-transparent text-sv-good" : "border-transparent text-sv-muted hover:border-sv-border hover:text-sv-text"}`}
              >
                {audioCopied ? "Copied" : "Copy audio"}
              </button>
            </div>
          )}
        </>
      )}
    </li>
  );
}
