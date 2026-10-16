import { useEffect, useRef, useState, type ReactNode } from "react";
import { CopyIcon, TrashIcon } from "./TranscriptCard";
import { copyToClipboard, transcriptDelete, transcriptRename, transcriptSaveAs } from "../../services/tauriBridge";

// The head of an open transcript on the Transcribe and Notetaker pages:
// click-to-rename title, a details line, and Copy / Save… / two-step Delete.

/** "1:02:09" or "4:05". */
export function clock(ms: number): string {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export default function TranscriptHeader({
  id,
  title,
  name,
  details,
  copyText,
  busy,
  onRenamed,
  onDeleted,
}: {
  id: number;
  /** What's shown: the name, or a stand-in for an unnamed meeting. */
  title: string;
  /** The stored name, to edit. */
  name: string;
  details: ReactNode;
  /** The text Copy puts on the clipboard; empty disables Copy and Save. */
  copyText: string;
  /** Still being written: Save and Delete wait. */
  busy: boolean;
  onRenamed: (name: string) => void;
  onDeleted: () => void;
}) {
  const [note, setNote] = useState<{ msg: string; ok: boolean } | null>(null);
  const noteTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [renaming, setRenaming] = useState(false);

  useEffect(() => {
    if (!confirmDelete) return;
    const t = setTimeout(() => setConfirmDelete(false), 4000);
    return () => clearTimeout(t);
  }, [confirmDelete]);

  useEffect(() => () => clearTimeout(noteTimer.current), []);
  const flash = (msg: string, ok = true) => {
    clearTimeout(noteTimer.current);
    setNote({ msg, ok });
    noteTimer.current = setTimeout(() => setNote(null), ok ? 2000 : 4000);
  };

  const copy = async () => {
    await copyToClipboard(copyText);
    flash("Copied");
  };

  const saveAs = async () => {
    try {
      if (await transcriptSaveAs(id)) flash("Saved");
    } catch (e) {
      flash(String(e), false);
    }
  };

  const remove = async () => {
    if (!confirmDelete) return setConfirmDelete(true);
    try {
      await transcriptDelete(id);
      onDeleted();
    } catch (e) {
      flash(String(e), false);
    }
  };

  return (
    <header className="flex flex-wrap items-start justify-between gap-3 border-b border-sv-border px-6 py-4">
      <div className="min-w-0 flex-1">
        {renaming ? (
          <input
            autoFocus
            defaultValue={name}
            placeholder={title}
            aria-label="Name"
            className="w-full rounded-md border border-sv-accent bg-sv-bg px-2 py-0.5 text-lg font-semibold outline-none"
            onBlur={() => setRenaming(false)}
            onKeyDown={async (e) => {
              if (e.key === "Escape") setRenaming(false);
              if (e.key === "Enter") {
                const next = e.currentTarget.value.trim();
                if (next && next !== name) {
                  try {
                    await transcriptRename(id, next);
                    onRenamed(next);
                  } catch (err) {
                    flash(String(err), false);
                  }
                }
                setRenaming(false);
              }
            }}
          />
        ) : (
          <h2>
            <button
              onClick={() => setRenaming(true)}
              title="Rename"
              className="max-w-full truncate rounded-md text-left text-lg font-semibold hover:text-sv-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
            >
              {title}
            </button>
          </h2>
        )}
        <div className="mt-0.5 text-xs text-sv-muted">{details}</div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {note && (
          <span role="status" className={`mr-1 text-xs font-medium ${note.ok ? "text-sv-good" : "text-sv-bad"}`}>
            {note.msg}
          </span>
        )}
        <ToolButton onClick={copy} disabled={!copyText} label="Copy">
          <CopyIcon />
        </ToolButton>
        <ToolButton onClick={saveAs} disabled={!copyText || busy} label="Save…">
          <SaveIcon />
        </ToolButton>
        <ToolButton onClick={remove} disabled={busy} label={confirmDelete ? "Delete?" : "Delete"} danger={confirmDelete}>
          <TrashIcon />
        </ToolButton>
      </div>
    </header>
  );
}

function ToolButton({
  onClick,
  disabled,
  label,
  danger,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  label: string;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      className={`flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent disabled:cursor-not-allowed disabled:opacity-40 ${
        danger
          ? "border-sv-bad bg-sv-bad/10 text-sv-bad"
          : "border-sv-border text-sv-muted hover:border-sv-accent/60 hover:text-sv-text"
      }`}
    >
      {children}
      <span className={danger ? "" : "hidden xl:inline"}>{label}</span>
    </button>
  );
}

function SaveIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4"
      aria-hidden
    >
      <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5" />
      <path d="M4.5 19.5h15" />
    </svg>
  );
}
