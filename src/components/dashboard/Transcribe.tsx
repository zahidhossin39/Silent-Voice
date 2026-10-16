import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Page from "../shared/Page";
import { CopyIcon, TrashIcon } from "../shared/TranscriptCard";
import { STT_MODELS } from "../../services/catalog";
import {
  copyToClipboard,
  isTauri,
  listenEvent,
  MEDIA_EXTENSIONS,
  pickMediaFile,
  transcribeCancel,
  transcribeStart,
  transcriptDelete,
  transcriptGet,
  transcriptRename,
  transcriptSaveAs,
  transcriptsList,
  TRANSCRIPT_PARAGRAPH,
  type Transcript,
} from "../../services/tauriBridge";

// "Transcribe a file": drop a recording in, read it back with timestamps.
// The Rust job (transcribe_job.rs) does the work and sends the whole transcript
// after every finished piece, so this page only mirrors what it's told.

function clock(ms: number): string {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

function minutes(ms: number): string {
  const min = Math.round(ms / 60_000);
  if (min < 1) return "under a minute";
  if (min < 60) return `${min} min`;
  return `${Math.floor(min / 60)} h ${min % 60} min`;
}

function modelLabel(id: string): string {
  return STT_MODELS.find((m) => m.id === id)?.label ?? id;
}

function paragraphs(t: Transcript) {
  const out: { start: number; text: string }[] = [];
  for (let i = 0; i < t.lines.length; i += TRANSCRIPT_PARAGRAPH) {
    const group = t.lines.slice(i, i + TRANSCRIPT_PARAGRAPH);
    out.push({ start: group[0].start_ms, text: group.map((l) => l.text).join(" ") });
  }
  return out;
}

export default function Transcribe() {
  const [list, setList] = useState<Transcript[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  // The list holds headers only; the open transcript's text lives here.
  const [full, setFull] = useState<Transcript | null>(null);

  const upsert = useCallback((t: Transcript) => {
    const head = { ...t, lines: [] };
    setList((l) => {
      const i = l.findIndex((x) => x.id === t.id);
      if (i === -1) return [head, ...l];
      const next = [...l];
      next[i] = head;
      return next;
    });
  }, []);

  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const onUpdate = useCallback(
    (t: Transcript) => {
      upsert(t);
      if (t.id === selectedRef.current) setFull(t);
    },
    [upsert]
  );

  useEffect(() => {
    if (selected === null) return setFull(null);
    let cancelled = false;
    transcriptGet(selected).then(
      (t) => !cancelled && setFull(t),
      () => {}
    );
    return () => {
      cancelled = true;
    };
  }, [selected]);

  useEffect(() => {
    transcriptsList().then((l) => {
      setList(l);
      setSelected((s) => s ?? l[0]?.id ?? null);
      setLoaded(true);
    });
    const un = listenEvent<Transcript>("transcript://update", onUpdate);
    return () => {
      un.then((f) => f());
    };
  }, [onUpdate]);

  const start = useCallback(
    async (opts: { path?: string; resume?: number }) => {
      setError(null);
      try {
        const t = await transcribeStart(opts);
        upsert(t);
        setSelected(t.id);
        setFull(t);
      } catch (e) {
        setError(String(e));
      }
    },
    [upsert]
  );

  // Files dropped anywhere on the window. Tauri hands over real paths, so a
  // 1 GB video is read in place, never copied through the page.
  useEffect(() => {
    if (!isTauri()) return;
    let un: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setDragging(true);
          else if (p.type === "leave") setDragging(false);
          else if (p.type === "drop") {
            setDragging(false);
            if (p.paths[0]) void start({ path: p.paths[0] });
          }
        })
        .then((f) => (cancelled ? f() : (un = f)))
    );
    return () => {
      cancelled = true;
      un?.();
    };
  }, [start]);

  const browse = async () => {
    const path = await pickMediaFile();
    if (path) void start({ path });
  };

  const busy = list.some((t) => t.status === "running");

  return (
    <Page
      fill
      title="Transcribe"
      subtitle="Turn a recording into text: lectures, calls, voice memos, videos. It all stays on this PC."
    >
      {loaded && list.length === 0 ? (
        <DropZone big dragging={dragging} busy={busy} error={error} onBrowse={browse} />
      ) : (
        <div className="grid min-h-0 flex-1 gap-4 md:grid-cols-[260px_minmax(0,1fr)] xl:grid-cols-[300px_minmax(0,1fr)]">
          <div className="flex min-h-0 flex-col gap-3">
            <DropZone dragging={dragging} busy={busy} error={error} onBrowse={browse} />
            <ul className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1" aria-label="Your transcripts">
              {list.map((t) => (
                <li key={t.id}>
                  <ListItem t={t} active={t.id === selected} onClick={() => setSelected(t.id)} />
                </li>
              ))}
            </ul>
          </div>
          {full && full.id === selected ? (
            <Reader
              key={full.id}
              t={full}
              busy={busy}
              onResume={() => start({ resume: full.id })}
              onDeleted={() => {
                const id = full.id;
                setList((l) => {
                  setSelected(l.find((x) => x.id !== id)?.id ?? null);
                  return l.filter((x) => x.id !== id);
                });
              }}
              onRenamed={(name) => {
                setList((l) => l.map((x) => (x.id === full.id ? { ...x, name } : x)));
                setFull({ ...full, name });
              }}
            />
          ) : (
            <div className="hidden rounded-2xl border border-sv-border bg-sv-surface md:block" />
          )}
        </div>
      )}
    </Page>
  );
}

function DropZone({
  big,
  dragging,
  busy,
  error,
  onBrowse,
}: {
  big?: boolean;
  dragging: boolean;
  busy: boolean;
  error: string | null;
  onBrowse: () => void;
}) {
  return (
    <div
      className={`flex flex-col items-center justify-center rounded-2xl border-2 border-dashed text-center transition-colors duration-200 ${
        dragging ? "border-sv-accent bg-sv-accent/10" : "border-sv-border bg-sv-surface"
      } ${big ? "flex-1 px-8 py-16" : "px-5 py-6"}`}
    >
      <UploadIcon className={`${big ? "h-10 w-10" : "h-7 w-7"} ${dragging ? "text-sv-accent" : "text-sv-muted"}`} />
      <div className={`${big ? "mt-4 text-lg" : "mt-2 text-sm"} font-semibold`}>
        {dragging ? "Drop to transcribe" : busy ? "One file at a time" : "Drop an audio or video file"}
      </div>
      <p className={`mt-1 text-sv-muted ${big ? "max-w-md text-sm" : "text-xs"}`}>
        {busy
          ? "Wait for the current one to finish, or cancel it."
          : big
          ? "Up to a couple of hours. It uses your dictation model, and nothing leaves this PC. Close the app any time: what's done is kept, and you can resume."
          : MEDIA_EXTENSIONS.slice(0, 6).join(", ")}
      </p>
      {!busy && (
        <button
          onClick={onBrowse}
          className={`${big ? "mt-5" : "mt-3"} rounded-lg bg-sv-accent px-4 py-1.5 text-sm font-medium text-sv-on-accent transition hover:bg-sv-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface`}
        >
          Choose a file
        </button>
      )}
      {error && (
        <p role="alert" className="mt-3 max-w-md text-xs font-medium text-sv-bad">
          {error}
        </p>
      )}
    </div>
  );
}

const STATUS: Record<Transcript["status"], { label: string; dot: string }> = {
  running: { label: "Transcribing", dot: "bg-sv-accent animate-pulse" },
  done: { label: "Done", dot: "bg-sv-good" },
  stopped: { label: "Stopped", dot: "bg-sv-warn" },
  failed: { label: "Failed", dot: "bg-sv-bad" },
};

function ListItem({ t, active, onClick }: { t: Transcript; active: boolean; onClick: () => void }) {
  const s = STATUS[t.status];
  return (
    <button
      onClick={onClick}
      aria-current={active}
      className={`w-full rounded-xl px-3 py-2.5 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
        active ? "bg-sv-surface-2" : "hover:bg-sv-surface"
      }`}
    >
      <div className="truncate text-sm font-medium" title={t.name}>
        {t.name}
      </div>
      <div className="mt-0.5 flex items-center gap-1.5 text-[11px] text-sv-muted">
        <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${s.dot}`} />
        <span>{t.status === "running" && t.duration_ms ? `${Math.min(100, Math.floor((t.done_ms / t.duration_ms) * 100))}%` : s.label}</span>
        <span aria-hidden>·</span>
        <span className="truncate">
          {t.duration_ms ? clock(t.duration_ms) : "?"} ·{" "}
          {new Date(t.created).toLocaleDateString(undefined, { month: "short", day: "numeric" })}
        </span>
      </div>
    </button>
  );
}

function Reader({
  t,
  busy,
  onResume,
  onDeleted,
  onRenamed,
}: {
  t: Transcript;
  busy: boolean;
  onResume: () => void;
  onDeleted: () => void;
  onRenamed: (name: string) => void;
}) {
  const paras = useMemo(() => paragraphs(t), [t]);
  const [note, setNote] = useState<{ msg: string; ok: boolean } | null>(null);
  const noteTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);
  const running = t.status === "running";

  // Follow the text as it fills in, unless the reader has scrolled up.
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = scroller.current;
    if (running && el && el.scrollHeight - el.scrollTop - el.clientHeight < 160) {
      bottom.current?.scrollIntoView({ block: "end" });
    }
  }, [running, t.lines.length]);

  useEffect(() => {
    if (!confirmDelete) return;
    const id = setTimeout(() => setConfirmDelete(false), 4000);
    return () => clearTimeout(id);
  }, [confirmDelete]);

  useEffect(() => () => clearTimeout(noteTimer.current), []);
  const flash = (msg: string, ok = true) => {
    clearTimeout(noteTimer.current);
    setNote({ msg, ok });
    noteTimer.current = setTimeout(() => setNote(null), ok ? 2000 : 4000);
  };

  const copy = async () => {
    await copyToClipboard(paras.map((p) => p.text).join("\n\n"));
    flash("Copied");
  };

  const saveAs = async () => {
    try {
      if (await transcriptSaveAs(t.id)) flash("Saved");
    } catch (e) {
      flash(String(e), false);
    }
  };

  const remove = async () => {
    if (!confirmDelete) return setConfirmDelete(true);
    try {
      await transcriptDelete(t.id);
      onDeleted();
    } catch (e) {
      flash(String(e), false);
    }
  };

  const pct = t.duration_ms ? Math.min(100, (t.done_ms / t.duration_ms) * 100) : 0;
  // Speed so far (audio per second of work) predicts the rest.
  const left = t.duration_ms && t.done_ms > 0 && t.work_ms > 0 ? Math.max(0, ((t.duration_ms - t.done_ms) / t.done_ms) * t.work_ms) : null;

  return (
    <section className="flex min-h-0 flex-col overflow-hidden rounded-2xl border border-sv-border bg-sv-surface">
      <header className="flex flex-wrap items-start justify-between gap-3 border-b border-sv-border px-6 py-4">
        <div className="min-w-0 flex-1">
          {renaming ? (
            <input
              autoFocus
              defaultValue={t.name}
              aria-label="Transcript name"
              className="w-full rounded-md border border-sv-accent bg-sv-bg px-2 py-0.5 text-lg font-semibold outline-none"
              onBlur={() => setRenaming(false)}
              onKeyDown={async (e) => {
                if (e.key === "Escape") setRenaming(false);
                if (e.key === "Enter") {
                  const name = e.currentTarget.value.trim();
                  if (name && name !== t.name) {
                    try {
                      await transcriptRename(t.id, name);
                      onRenamed(name);
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
                {t.name}
              </button>
            </h2>
          )}
          <div className="mt-0.5 text-xs text-sv-muted">
            {t.duration_ms ? clock(t.duration_ms) : "Length unknown"} · {modelLabel(t.model_id)} ·{" "}
            {new Date(t.created).toLocaleDateString(undefined, { dateStyle: "medium" })}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {note && (
            <span role="status" className={`mr-1 text-xs font-medium ${note.ok ? "text-sv-good" : "text-sv-bad"}`}>
              {note.msg}
            </span>
          )}
          <ToolButton onClick={copy} disabled={paras.length === 0} label="Copy">
            <CopyIcon />
          </ToolButton>
          <ToolButton onClick={saveAs} disabled={paras.length === 0} label="Save…">
            <SaveIcon />
          </ToolButton>
          <ToolButton onClick={remove} disabled={running} label={confirmDelete ? "Delete?" : "Delete"} danger={confirmDelete}>
            <TrashIcon />
          </ToolButton>
        </div>
      </header>

      {running && (
        <div className="border-b border-sv-border px-6 py-3">
          <div className="flex items-center justify-between gap-3 text-xs">
            <span className="tabular-nums text-sv-text">
              <span className="font-semibold">{t.duration_ms ? `${Math.floor(pct)}%` : clock(t.done_ms)}</span>
              <span className="text-sv-muted">
                {" "}· {clock(t.done_ms)} of {t.duration_ms ? clock(t.duration_ms) : "?"}
                {left !== null && (left < 60_000 ? " · under a minute left" : ` · about ${minutes(left)} left`)}
              </span>
            </span>
            <button
              onClick={() => void transcribeCancel()}
              className="rounded-lg border border-sv-border px-3 py-1 text-xs text-sv-muted transition hover:border-sv-bad hover:text-sv-bad"
            >
              Cancel
            </button>
          </div>
          <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-sv-border/60">
            <div className="h-full rounded-full bg-sv-accent transition-[width] duration-500 ease-out" style={{ width: `${Math.max(1, pct)}%` }} />
          </div>
        </div>
      )}

      {(t.status === "stopped" || t.status === "failed") && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-sv-border bg-sv-surface-2/60 px-6 py-3 text-sm">
          <span className={t.status === "failed" ? "text-sv-bad" : "text-sv-text"}>
            {t.status === "failed"
              ? `Stopped by an error at ${clock(t.done_ms)}: ${t.error ?? "unknown"}`
              : `Stopped at ${clock(t.done_ms)}${t.duration_ms ? ` of ${clock(t.duration_ms)}` : ""}. What's done is saved.`}
          </span>
          <button
            onClick={onResume}
            disabled={busy}
            title={busy ? "Another file is being transcribed" : undefined}
            className="rounded-lg bg-sv-accent px-3 py-1.5 text-xs font-medium text-sv-on-accent transition hover:bg-sv-accent/90 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t.status === "failed" ? "Try again" : "Resume"}
          </button>
        </div>
      )}

      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {paras.length === 0 && !running ? (
          <p className="text-sm text-sv-muted">No speech was found in this file.</p>
        ) : (
          <div className="space-y-5">
            {paras.map((p, i) => (
              <div key={i} className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-3">
                <span className="pt-0.5 text-right text-[11px] tabular-nums text-sv-muted">{clock(p.start)}</span>
                <p className="max-w-[72ch] text-[15px] leading-relaxed text-sv-text">{p.text}</p>
              </div>
            ))}
            {running && (
              <div className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-3" aria-label="Transcribing the next part">
                <span />
                <div className="max-w-[72ch] space-y-2 pt-1">
                  <div className="h-3 w-11/12 animate-pulse rounded bg-sv-border/60" />
                  <div className="h-3 w-3/5 animate-pulse rounded bg-sv-border/60" />
                </div>
              </div>
            )}
          </div>
        )}
        <div ref={bottom} />
      </div>
    </section>
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
  children: React.ReactNode;
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

const icon = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.75,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

function UploadIcon({ className }: { className?: string }) {
  return (
    <svg {...icon} className={className} aria-hidden>
      <path d="M12 15V4M7.5 8.5 12 4l4.5 4.5" />
      <path d="M4 14v4.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V14" />
    </svg>
  );
}

function SaveIcon() {
  return (
    <svg {...icon} className="h-4 w-4" aria-hidden>
      <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5" />
      <path d="M4.5 19.5h15" />
    </svg>
  );
}
