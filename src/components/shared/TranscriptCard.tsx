import { useState, useEffect, useRef } from "react";
import type { HistoryEntry } from "../../types";
import { copyToClipboard, readAudioClip, copyAudioFile, retranscribeClip } from "../../services/tauriBridge";
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

// ---- Icons: one consistent stroke set (Lucide geometry, 16px) ----
function Icon({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={16}
      height={16}
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

const EditIcon = () => (
  <Icon>
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z" />
  </Icon>
);
const CopyIcon = () => (
  <Icon>
    <rect x="9" y="9" width="13" height="13" rx="2" />
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
  </Icon>
);
const CheckIcon = () => (
  <Icon>
    <path d="M20 6 9 17l-5-5" />
  </Icon>
);
const RetranscribeIcon = ({ spinning }: { spinning?: boolean }) => (
  <Icon className={spinning ? "animate-spin" : undefined}>
    <path d="M21 2v6h-6" />
    <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
    <path d="M3 22v-6h6" />
    <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
  </Icon>
);
const TrashIcon = () => (
  <Icon>
    <path d="M3 6h18" />
    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
    <path d="M10 11v6M14 11v6" />
  </Icon>
);

type Tone = "default" | "danger";

function IconButton({
  label,
  onClick,
  disabled,
  active,
  tone = "default",
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  tone?: Tone;
  children: React.ReactNode;
}) {
  const rest =
    tone === "danger"
      ? "text-sv-muted hover:bg-sv-bad/10 hover:text-sv-bad"
      : active
        ? "text-sv-good"
        : "text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text";
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      className={`grid h-8 w-8 place-items-center rounded-lg transition-all duration-150 active:scale-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent/50 disabled:cursor-not-allowed disabled:opacity-45 ${rest}`}
    >
      {children}
    </button>
  );
}

interface TranscriptCardProps {
  entry: HistoryEntry;
  blurred: boolean;
  onUpdate: (id: number, text: string) => void;
  onRetranscribe: (id: number, text: string, modelId: string) => void;
  onRemove: (id: number) => void;
  onLearn: (msg: string) => void;
}

export default function TranscriptCard({
  entry,
  blurred,
  onUpdate,
  onRetranscribe,
  onRemove,
  onLearn,
}: TranscriptCardProps) {
  const vocabulary = useSettingsStore((s) => s.settings.custom_vocabulary);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [copied, setCopied] = useState(false);
  const [audioCopied, setAudioCopied] = useState(false);
  const [retranscribing, setRetranscribing] = useState(false);
  const [retranscribeErr, setRetranscribeErr] = useState<string | null>(null);

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

  async function handleRetranscribe() {
    if (!entry.audio_file || retranscribing) return;
    setRetranscribing(true);
    setRetranscribeErr(null);
    try {
      const { text, model_id } = await retranscribeClip(entry.audio_file);
      if (!text.trim()) {
        setRetranscribeErr("No speech detected in this recording.");
        return;
      }
      onRetranscribe(entry.id, text, model_id);
    } catch (e) {
      setRetranscribeErr(String(e));
    } finally {
      setRetranscribing(false);
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
    if (audioRef.current) return audioRef.current; // Concurrency check

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
    audio.onerror = () => {
      console.warn("audio decode failed", entry.audio_file);
      setIsPlaying(false);
    };
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

  async function seekTo(compute: (total: number) => number) {
    try {
      const audio = audioRef.current ?? (await loadAudio());
      if (!audio) return;
      // Before metadata lands, audio.duration is NaN — fall back to the length
      // Rust recorded so a seek works on the very first click.
      const total = Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : duration;
      const target = Math.max(0, Math.min(compute(total), total));
      audio.currentTime = target;
      setProgress(target);
    } catch (err) {
      console.warn("audio seek failed", err);
    }
  }

  function handleSeek(e: React.MouseEvent<HTMLDivElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    const fraction = rect.width > 0 ? x / rect.width : 0;
    seekTo((total) => total * fraction);
  }

  function handleSeekKey(e: React.KeyboardEvent<HTMLDivElement>) {
    const step = e.shiftKey ? 10 : 1;
    const keys: Record<string, (total: number) => number> = {
      ArrowRight: () => progress + step,
      ArrowUp: () => progress + step,
      ArrowLeft: () => progress - step,
      ArrowDown: () => progress - step,
      PageUp: () => progress + 10,
      PageDown: () => progress - 10,
      Home: () => 0,
      End: (total) => total,
    };
    const next = keys[e.key];
    if (!next) return;
    e.preventDefault();
    seekTo(next);
  }

  function formatTime(s: number) {
    const mins = Math.floor(s / 60);
    const secs = Math.floor(s % 60).toString().padStart(2, "0");
    return `${mins}:${secs}`;
  }

  const pct = duration > 0 ? Math.min(100, (progress / duration) * 100) : 0;
  const when = new Date(entry.timestamp);
  const dateLabel = when.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  const timeLabel = when.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });

  return (
    <li className="group rounded-xl border border-sv-border/70 bg-sv-surface p-4 transition-colors duration-200 hover:border-sv-border sm:p-5">
      <div className="mb-2.5 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-[13px] font-medium text-sv-text/90">
            {dateLabel} <span className="text-sv-muted">· {timeLabel}</span>
          </div>
          <div className="mt-1 inline-flex items-center gap-1.5 text-[11px] text-sv-muted">
            <span className="h-1.5 w-1.5 rounded-full bg-sv-accent/70" />
            <span className="truncate">{entry.model_id || "unknown model"}</span>
            {entry.mode_id && entry.mode_id !== "none" && (
              <span className="text-sv-muted/60">· {entry.mode_id}</span>
            )}
          </div>
        </div>

        {!editing && (
          <div className="flex shrink-0 items-center gap-0.5">
            <IconButton label="Edit — corrections teach new words" onClick={startEdit}>
              <EditIcon />
            </IconButton>
            {entry.audio_file && (
              <IconButton
                label="Re-transcribe with your current model"
                onClick={handleRetranscribe}
                disabled={retranscribing}
              >
                <RetranscribeIcon spinning={retranscribing} />
              </IconButton>
            )}
            <IconButton
              label={copied ? "Copied" : "Copy text"}
              onClick={handleCopy}
              active={copied}
            >
              {copied ? <CheckIcon /> : <CopyIcon />}
            </IconButton>
            <IconButton label="Delete" onClick={() => onRemove(entry.id)} tone="danger">
              <TrashIcon />
            </IconButton>
          </div>
        )}
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
              className="rounded-lg bg-sv-accent px-3 py-1.5 text-xs font-medium text-sv-on-accent transition-colors hover:bg-sv-accent-hover"
            >
              Save correction
            </button>
            <button
              onClick={() => setEditing(false)}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs text-sv-muted transition-colors hover:text-sv-text"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <>
          <p className={`text-[15px] leading-relaxed text-sv-text ${blurred ? "blur-sm transition group-hover:blur-none" : ""}`}>
            {displayed}
          </p>
          {retranscribeErr && (
            <p className="mt-2 text-xs text-sv-bad">{retranscribeErr}</p>
          )}
          {entry.audio_file && (
            <div className="mt-3.5 flex items-center gap-3 border-t border-sv-border/50 pt-3.5">
              <button
                onClick={togglePlay}
                aria-label={isPlaying ? "Pause recording" : "Play recording"}
                className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-colors duration-150 ${isPlaying ? "border-sv-accent bg-sv-accent/10 text-sv-accent" : "border-sv-border bg-sv-surface-2 text-sv-text hover:border-sv-accent hover:text-sv-accent"}`}
              >
                {isPlaying ? (
                  <svg viewBox="0 0 24 24" width={15} height={15} fill="currentColor" aria-hidden="true">
                    <rect x="6" y="5" width="4" height="14" rx="1" />
                    <rect x="14" y="5" width="4" height="14" rx="1" />
                  </svg>
                ) : (
                  <svg viewBox="0 0 24 24" width={15} height={15} fill="currentColor" aria-hidden="true" className="translate-x-[1px]">
                    <path d="M6 4.5v15l13-7.5-13-7.5Z" />
                  </svg>
                )}
              </button>

              <div
                className="relative h-4 flex-1 cursor-pointer rounded-full"
                role="slider"
                tabIndex={0}
                aria-label="Seek recording"
                aria-valuenow={Math.round(progress)}
                aria-valuemin={0}
                aria-valuemax={Math.round(duration) || 1}
                aria-valuetext={`${formatTime(progress)} of ${formatTime(duration)}`}
                onClick={handleSeek}
                onKeyDown={handleSeekKey}
              >
                <div className="absolute inset-x-0 top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-sv-surface-2">
                  <div
                    className="absolute left-0 top-0 h-full rounded-full bg-sv-accent"
                    style={{ width: `${pct}%` }}
                  />
                </div>
                {/* Playhead is always visible so the scrubber reads as one at
                    rest, not only on hover. The ring lifts it off the track. */}
                <div
                  className="pointer-events-none absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-sv-accent shadow ring-2 ring-sv-surface"
                  style={{ left: `${pct}%` }}
                />
              </div>

              <span className="shrink-0 tabular-nums text-[11px] text-sv-muted">
                {formatTime(progress)} / {formatTime(duration)}
              </span>

              <IconButton
                label={audioCopied ? "Audio copied" : "Copy audio"}
                onClick={handleCopyAudio}
                active={audioCopied}
              >
                {audioCopied ? (
                  <CheckIcon />
                ) : (
                  // Custom "copy audio": the copy metaphor (back + front sheet)
                  // with a small waveform on the front sheet so it clearly reads
                  // "copy this recording", not "play".
                  <Icon>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                    <rect x="9" y="9" width="13" height="13" rx="2" />
                    <path d="M12.5 16.5v-2" />
                    <path d="M15.5 18v-6" />
                    <path d="M18.5 17v-4" />
                  </Icon>
                )}
              </IconButton>
            </div>
          )}
        </>
      )}
    </li>
  );
}
