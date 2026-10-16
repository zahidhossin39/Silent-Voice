import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Page from "../shared/Page";
import { CopyIcon, TrashIcon } from "../shared/TranscriptCard";
import { clock, SaveIcon, ToolButton } from "./Transcribe";
import {
  copyToClipboard,
  listenEvent,
  meetingDevices,
  meetingStart,
  meetingStop,
  transcriptDelete,
  transcriptGet,
  transcriptParagraphs,
  transcriptRename,
  transcriptSaveAs,
  transcriptsList,
  type Transcript,
} from "../../services/tauriBridge";

// "Notetaker": record a call (your mic + the computer's sound) into a live,
// You/Others transcript. The Rust job (meeting_job.rs) does the work and sends
// the transcript as it grows, like the Transcribe page.

const CONSENT_KEY = "sv-notetaker-consent-seen";

/** True while a meeting records (for the sidebar dot). */
export function useMeetingRecording(): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => {
    transcriptsList().then((l) => setOn(l.some((t) => t.kind === "meeting" && t.status === "running")));
    const un = listenEvent<Transcript>("transcript://update", (t) => {
      if (t.kind === "meeting") setOn(t.status === "running");
    });
    return () => {
      un.then((f) => f());
    };
  }, []);
  return on;
}

function bluetooth(name: string | null): boolean {
  return !!name && /hands-free|headset|airpods|buds|bluetooth|\bbt\b/i.test(name);
}

function dayLabel(ms: number): string {
  const d = new Date(ms);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === yesterday.toDateString()) return "Yesterday";
  return d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
}

function timeOf(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

/** "Meeting" until renamed: show when it was instead. */
function title(t: Transcript): string {
  return t.name === "Meeting" ? `Meeting at ${timeOf(t.created)}` : t.name;
}

export default function Notetaker() {
  const [list, setList] = useState<Transcript[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [full, setFull] = useState<Transcript | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [devices, setDevices] = useState<{ mic: string | null; speakers: string | null }>({ mic: null, speakers: null });
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [consentSeen, setConsentSeen] = useState(() => {
    try {
      return localStorage.getItem(CONSENT_KEY) === "1";
    } catch {
      return true;
    }
  });

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

  useEffect(() => {
    transcriptsList().then((all) => {
      const l = all.filter((t) => t.kind === "meeting");
      setList(l);
      setSelected((s) => s ?? l[0]?.id ?? null);
    });
    meetingDevices().then(setDevices, () => {});
    const un = listenEvent<Transcript>("transcript://update", (t) => {
      if (t.kind !== "meeting") return;
      upsert(t);
      if (t.status !== "running") setStopping(false);
      if (t.id === selectedRef.current) setFull(t);
    });
    return () => {
      un.then((f) => f());
    };
  }, [upsert]);

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

  const recording = list.find((t) => t.status === "running") ?? null;

  const start = async () => {
    setError(null);
    setStarting(true);
    try {
      const t = await meetingStart();
      upsert(t);
      setSelected(t.id);
      setFull(t);
      dismissConsent();
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };

  const stop = async () => {
    setStopping(true);
    try {
      await meetingStop();
    } catch (e) {
      setError(`Recording stopped with a problem: ${e}. Everything heard before it is kept.`);
    }
  };

  const dismissConsent = () => {
    setConsentSeen(true);
    try {
      localStorage.setItem(CONSENT_KEY, "1");
    } catch {
      /* private window: the note just shows again next time */
    }
  };

  const groups = useMemo(() => {
    const out: { day: string; items: Transcript[] }[] = [];
    for (const t of list) {
      const day = dayLabel(t.created);
      if (out[out.length - 1]?.day === day) out[out.length - 1].items.push(t);
      else out.push({ day, items: [t] });
    }
    return out;
  }, [list]);

  const btMic = bluetooth(devices.mic);

  return (
    <Page
      fill
      title="Notetaker"
      subtitle="Record a call and get a live transcript of who said what. Audio and text stay on this PC."
      actions={
        recording ? (
          <RecordingPill startedAt={recording.created} stopping={stopping} onStop={stop} />
        ) : (
          <button
            onClick={start}
            disabled={starting}
            className="flex shrink-0 items-center gap-2 rounded-xl bg-sv-accent px-4 py-2 text-sm font-semibold text-sv-on-accent shadow-sm transition hover:bg-sv-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-bg disabled:opacity-60"
          >
            <span className="relative flex h-3.5 w-3.5 items-center justify-center rounded-full border-2 border-current">
              <span className="h-1.5 w-1.5 rounded-full bg-current" />
            </span>
            {starting ? "Starting…" : "Start recording"}
          </button>
        )
      }
    >
      <div className="grid min-h-0 flex-1 gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.35fr)]">
        <div className="flex min-h-0 flex-col gap-3">
          {!recording && (
            <Setup
              mic={devices.mic}
              speakers={devices.speakers}
              btMic={btMic}
              showConsent={!consentSeen}
              onConsentSeen={dismissConsent}
            />
          )}
          {error && (
            <p role="alert" className="rounded-xl border border-sv-bad/40 bg-sv-bad/10 px-4 py-3 text-sm text-sv-bad">
              {error}
            </p>
          )}
          <section aria-label="Past meetings" className="min-h-0 flex-1 overflow-y-auto pr-1">
            {list.length === 0 ? (
              <p className="px-1 pt-2 text-sm text-sv-muted">
                Your meetings will be listed here. Start recording when a call begins, and stop when it ends.
              </p>
            ) : (
              groups.map((g) => (
                <div key={g.day} className="mb-4">
                  <h2 className="mb-1.5 px-1 text-xs font-semibold text-sv-muted">{g.day}</h2>
                  <ul className="space-y-1">
                    {g.items.map((t) => (
                      <li key={t.id}>
                        <MeetingRow t={t} active={t.id === selected} onClick={() => setSelected(t.id)} />
                      </li>
                    ))}
                  </ul>
                </div>
              ))
            )}
          </section>
        </div>

        {full && full.id === selected ? (
          <MeetingPanel
            key={full.id}
            t={full}
            stopping={stopping && full.status === "running"}
            onDeleted={() => {
              const id = full.id;
              const rest = list.filter((x) => x.id !== id);
              setList(rest);
              setSelected(rest[0]?.id ?? null);
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
    </Page>
  );
}

function RecordingPill({ startedAt, stopping, onStop }: { startedAt: number; stopping: boolean; onStop: () => void }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  return (
    <div
      role="status"
      aria-live="polite"
      className="flex shrink-0 items-center gap-3 rounded-xl border border-sv-bad/40 bg-sv-bad/10 py-1.5 pl-3.5 pr-1.5"
    >
      <span className="relative flex h-2.5 w-2.5">
        {!stopping && <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-sv-bad opacity-60" />}
        <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-sv-bad" />
      </span>
      <span className="text-sm font-semibold tabular-nums text-sv-bad">
        {stopping ? "Finishing up…" : `Recording ${clock(Math.max(0, now - startedAt))}`}
      </span>
      <button
        onClick={onStop}
        disabled={stopping}
        className="flex items-center gap-1.5 rounded-lg bg-sv-bad px-3 py-1 text-sm font-semibold text-white transition hover:bg-sv-bad/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-bad focus-visible:ring-offset-2 focus-visible:ring-offset-sv-bg disabled:opacity-50"
      >
        <span className="h-2.5 w-2.5 rounded-[2px] bg-current" aria-hidden />
        Stop
      </button>
    </div>
  );
}

function Setup({
  mic,
  speakers,
  btMic,
  showConsent,
  onConsentSeen,
}: {
  mic: string | null;
  speakers: string | null;
  btMic: boolean;
  showConsent: boolean;
  onConsentSeen: () => void;
}) {
  return (
    <div className="space-y-3 rounded-2xl border border-sv-border bg-sv-surface px-5 py-4">
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm">
        <dt className="text-sv-muted">You</dt>
        <dd className="truncate" title={mic ?? undefined}>
          {mic ?? "Default microphone"}
        </dd>
        <dt className="text-sv-muted">Others</dt>
        <dd className="truncate" title={speakers ?? undefined}>
          {speakers ? `What plays on ${speakers}` : "What plays on your speakers"}
        </dd>
      </dl>
      {btMic ? (
        <p className="rounded-lg bg-sv-warn/10 px-3 py-2 text-xs leading-relaxed text-sv-warn">
          This mic looks like a Bluetooth headset. Recording from it switches the headset to call quality, so everyone
          sounds worse. Pick your laptop's built-in mic in Settings for better sound.
        </p>
      ) : (
        <p className="text-xs leading-relaxed text-sv-muted">
          Best with headphones. On speakers your mic also hears the others; Silent Voice drops those repeats, but
          headphones keep "You" cleanest.
        </p>
      )}
      {showConsent && (
        <div className="flex items-start justify-between gap-3 border-t border-sv-border pt-3">
          <p className="text-xs leading-relaxed text-sv-text">
            Let people know you're recording. In many places everyone on a call has to agree to it.
          </p>
          <button
            onClick={onConsentSeen}
            className="shrink-0 rounded-md px-2 py-0.5 text-xs font-medium text-sv-accent hover:bg-sv-accent/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
          >
            Got it
          </button>
        </div>
      )}
    </div>
  );
}

function MeetingRow({ t, active, onClick }: { t: Transcript; active: boolean; onClick: () => void }) {
  const live = t.status === "running";
  return (
    <button
      onClick={onClick}
      aria-current={active}
      className={`flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
        active ? "bg-sv-surface-2" : "hover:bg-sv-surface"
      }`}
    >
      <span
        className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${
          live ? "bg-sv-bad/15 text-sv-bad" : "bg-sv-surface-2 text-sv-muted"
        }`}
        aria-hidden
      >
        {live ? (
          <span className="h-2.5 w-2.5 animate-pulse rounded-full bg-sv-bad" />
        ) : (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" className="h-4 w-4">
            <path d="M7 3.5h7l4 4V20a.5.5 0 0 1-.5.5h-10A.5.5 0 0 1 7 20z" />
            <path d="M10 11h5M10 14.5h5M10 18h3" />
          </svg>
        )}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium" title={title(t)}>
          {title(t)}
        </span>
        <span className="mt-0.5 block text-[11px] text-sv-muted">
          {live ? "Recording now" : `${timeOf(t.created)} · ${clock(t.duration_ms ?? t.done_ms)}`}
          {t.status === "stopped" && " · cut short"}
        </span>
      </span>
    </button>
  );
}

function MeetingPanel({
  t,
  stopping,
  onDeleted,
  onRenamed,
}: {
  t: Transcript;
  stopping: boolean;
  onDeleted: () => void;
  onRenamed: (name: string) => void;
}) {
  const paras = useMemo(() => transcriptParagraphs(t.lines), [t]);
  const [note, setNote] = useState<{ msg: string; ok: boolean } | null>(null);
  const noteTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const live = t.status === "running";

  useEffect(() => {
    const el = scroller.current;
    if (live && el && el.scrollHeight - el.scrollTop - el.clientHeight < 160) {
      bottom.current?.scrollIntoView({ block: "end" });
    }
  }, [live, t.lines.length]);

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
    const who = (s?: string) => (s === "you" ? "You: " : s === "others" ? "Others: " : "");
    await copyToClipboard(paras.map((p) => who(p.speaker) + p.text).join("\n\n"));
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

  return (
    <section className="flex min-h-0 flex-col overflow-hidden rounded-2xl border border-sv-border bg-sv-surface">
      <header className="flex flex-wrap items-start justify-between gap-3 border-b border-sv-border px-6 py-4">
        <div className="min-w-0 flex-1">
          {renaming ? (
            <input
              autoFocus
              defaultValue={t.name === "Meeting" ? "" : t.name}
              placeholder={title(t)}
              aria-label="Meeting name"
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
                {title(t)}
              </button>
            </h2>
          )}
          <div className="mt-0.5 text-xs text-sv-muted">
            {dayLabel(t.created)} · {timeOf(t.created)}
            {!live && ` · ${clock(t.duration_ms ?? t.done_ms)}`}
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
          <ToolButton onClick={saveAs} disabled={paras.length === 0 || live} label="Save…">
            <SaveIcon />
          </ToolButton>
          <ToolButton onClick={remove} disabled={live} label={confirmDelete ? "Delete?" : "Delete"} danger={confirmDelete}>
            <TrashIcon />
          </ToolButton>
        </div>
      </header>

      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        <section aria-labelledby="overview" className="mb-6">
          <h3 id="overview" className="text-xs font-semibold text-sv-muted">
            Overview
          </h3>
          <p className="mt-1.5 text-sm text-sv-muted">
            {live ? "The transcript fills in as people talk." : "A summary of this meeting will appear here in a later update."}
          </p>
        </section>

        {paras.length === 0 && !live ? (
          <p className="text-sm text-sv-muted">No speech was picked up in this meeting.</p>
        ) : (
          <ol className="space-y-5">
            {paras.map((p, i) => (
              <li key={i} className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-3">
                <span className="pt-0.5 text-right text-[11px] tabular-nums text-sv-muted">{clock(p.start)}</span>
                <div className="max-w-[72ch]">
                  <div className={`text-xs font-semibold ${p.speaker === "you" ? "text-sv-accent" : "text-sv-text"}`}>
                    {p.speaker === "you" ? "You" : "Others"}
                  </div>
                  <p className="mt-0.5 text-[15px] leading-relaxed text-sv-text">{p.text}</p>
                </div>
              </li>
            ))}
            {live && (
              <li className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-3" aria-label={stopping ? "Finishing the last words" : "Listening"}>
                <span />
                <div className="flex items-center gap-1.5 pt-1 text-xs text-sv-muted">
                  <span className="flex gap-0.5" aria-hidden>
                    {[0, 150, 300].map((d) => (
                      <span key={d} className="h-1.5 w-1.5 animate-pulse rounded-full bg-sv-muted" style={{ animationDelay: `${d}ms` }} />
                    ))}
                  </span>
                  {stopping ? "Finishing the last words…" : "Listening"}
                </div>
              </li>
            )}
          </ol>
        )}
        <div ref={bottom} />
      </div>
    </section>
  );
}
