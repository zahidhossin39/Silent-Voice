import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Page from "../shared/Page";
import Select from "../shared/Select";
import { useSettingsStore } from "../../stores/settingsStore";
import { LANGUAGES, STT_MODELS, honorsLanguage } from "../../services/catalog";
import TranscriptHeader, { clock } from "../shared/TranscriptHeader";
import { TrashIcon } from "../shared/TranscriptCard";
import MeetingOverview, { GearIcon, SummarySettings, useSummaryChoice, type Choice, type SummaryChoice } from "./MeetingOverview";
import {
  listenEvent,
  meetingAudioClip,
  meetingDevices,
  meetingRetranscribe,
  meetingsSearch,
  meetingStart,
  meetingStop,
  transcriptDelete,
  transcriptGet,
  transcriptSaveAs,
  transcribeCancel,
  speakerLabel,
  transcriptParagraphs,
  transcriptsList,
  type Transcript,
} from "../../services/tauriBridge";

// "Notetaker": record a call (your mic + the computer's sound) into a live,
// You/Others transcript. The Rust job (meeting_job.rs) does the work and sends
// the transcript as it grows, like the Transcribe page.

const CONSENT_KEY = "sv-notetaker-consent-seen";
/** "local", or the id of the cloud provider that transcribes meetings. */
const STT_KEY = "sv-notetaker-stt";
/** The meetings' language code, or "" for the dictation language. */
const LANG_KEY = "sv-notetaker-language";

/** True while a meeting records (for the sidebar dot). */
export function useMeetingRecording(): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => {
    transcriptsList().then((l) => setOn(l.some((t) => t.kind === "meeting" && t.status === "running" && !t.redo)));
    const un = listenEvent<Transcript>("transcript://update", (t) => {
      if (t.kind === "meeting") setOn(t.status === "running" && !t.redo);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);
  return on;
}

/** Windows names a Bluetooth headset's mic "Headset (… Hands-Free AG Audio)". */
function bluetooth(name: string | null): boolean {
  return !!name && /hands-free|bluetooth/i.test(name);
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

/** Unnamed until the user names it: show when it was instead. */
function title(t: Transcript): string {
  return t.name || `Meeting at ${timeOf(t.created)}`;
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

  const recording = list.find((t) => t.status === "running" && !t.redo) ?? null;
  const busy = list.some((t) => t.status === "running");

  const [query, setQuery] = useState("");
  const [found, setFound] = useState<Set<number> | null>(null);
  useEffect(() => {
    if (!query.trim()) return setFound(null);
    let cancelled = false;
    // Searches every saved word; wait for a pause in typing.
    const timer = setTimeout(() => meetingsSearch(query).then((ids) => !cancelled && setFound(new Set(ids))), 200);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query, list.length]);
  const shown = found ? list.filter((t) => found.has(t.id)) : list;

  const removed = (id: number) => {
    const rest = list.filter((x) => x.id !== id);
    setList(rest);
    if (selected === id) setSelected(rest[0]?.id ?? null);
  };

  const sttProviders = useSettingsStore((s) => s.providers).filter((p) => p.uses.includes("stt") && p.base_url && p.stt_model);
  const [sttKey, setSttKey] = useState(() => {
    try {
      return localStorage.getItem(STT_KEY) ?? "local";
    } catch {
      return "local";
    }
  });
  const cloud = sttProviders.find((p) => p.id === sttKey);
  const [lang, setLang] = useState(() => {
    try {
      return localStorage.getItem(LANG_KEY) ?? "";
    } catch {
      return "";
    }
  });
  const pickLang = (v: string) => {
    setLang(v);
    try {
      localStorage.setItem(LANG_KEY, v);
    } catch {
      /* remembered for this visit only */
    }
  };
  const dictationLang = useSettingsStore((s) => s.settings.language);
  const localModel = STT_MODELS.find((m) => m.id === useSettingsStore.getState().settings.active_stt_model);
  const meetingLang = lang || dictationLang;
  const langName = (c: string) => LANGUAGES.find((l) => l.code === c)?.name ?? c;
  // Parakeet, Moonshine and .en models ignore the language: say so instead of failing quietly.
  const langNote = cloud
    ? meetingLang === "auto"
      ? `${cloud.name} detects the language of each part.`
      : `${cloud.name} is told the meeting is in ${langName(meetingLang)}.`
    : !honorsLanguage(localModel) && meetingLang !== "en"
      ? `Your voice model (${localModel?.label ?? "this PC's"}) only understands ${localModel?.id.startsWith("sense-voice") ? "a few languages" : "English"}. For ${meetingLang === "auto" ? "other languages" : langName(meetingLang)}, transcribe with a cloud provider or download a Whisper model.`
      : `Transcribed in ${meetingLang === "auto" ? "whatever language is spoken" : langName(meetingLang)}.`;
  const summary = useSummaryChoice();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const pickStt = (v: string) => {
    setSttKey(v);
    try {
      localStorage.setItem(STT_KEY, v);
    } catch {
      /* remembered for this visit only */
    }
  };

  const cloudArg = cloud && { base_url: cloud.base_url, api_key: cloud.api_key, model: cloud.stt_model };

  const redo = async (id: number) => {
    setError(null);
    try {
      const t = await meetingRetranscribe(id, cloudArg, lang || undefined);
      upsert(t);
      setFull(t);
    } catch (e) {
      setError(String(e));
    }
  };

  const start = async () => {
    setError(null);
    setStarting(true);
    try {
      const t = await meetingStart(cloudArg, lang || undefined);
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
    for (const t of shown) {
      const day = dayLabel(t.created);
      if (out[out.length - 1]?.day === day) out[out.length - 1].items.push(t);
      else out.push({ day, items: [t] });
    }
    return out;
  }, [shown]);

  const btMic = bluetooth(devices.mic);

  return (
    <Page
      fill
      title="Notetaker"
      subtitle={`Record a call and get a live transcript of who said what. ${cloud ? `Audio is sent to ${cloud.name} to transcribe; recordings and text stay on this PC.` : "Audio and text stay on this PC."}`}
      actions={
        <div className="relative flex items-center gap-2">
          <button
            onClick={() => setSettingsOpen((o) => !o)}
            aria-expanded={settingsOpen}
            aria-label="Notetaker settings"
            title="Notetaker settings"
            className={`flex h-10 w-10 items-center justify-center rounded-xl border transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
              settingsOpen ? "border-sv-accent text-sv-text" : "border-sv-border text-sv-muted hover:text-sv-text"
            }`}
          >
            <GearIcon className="h-[18px] w-[18px]" />
          </button>
        {recording ? (
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
        )}
          {settingsOpen && (
            <NotetakerSettings
              onClose={() => setSettingsOpen(false)}
              summary={summary}
              language={
                <Select value={lang} onChange={pickLang} className="w-full" disabled={!!recording}>
                  <option value="">Same as dictation ({langName(dictationLang)})</option>
                  {LANGUAGES.map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.name}
                    </option>
                  ))}
                </Select>
              }
              languageNote={langNote}
              stt={
                <Select value={cloud ? cloud.id : "local"} onChange={pickStt} className="w-full" disabled={!!recording}>
                  <option value="local">On this PC (your voice model)</option>
                  {sttProviders.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} · {p.stt_model}
                    </option>
                  ))}
                </Select>
              }
              sttNote={
                recording
                  ? "Applies to the next meeting."
                  : cloud
                    ? `Audio is sent to ${cloud.name}. If a call fails, that part is transcribed on this PC.`
                    : sttProviders.length === 0
                      ? "To transcribe in the cloud, turn on Speech-to-text for a provider in Cloud providers."
                      : "Nothing leaves this PC."
              }
            />
          )}
        </div>
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
          {list.length > 0 && (
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search meetings"
              aria-label="Search meetings"
              className="w-full rounded-xl border border-sv-border bg-sv-surface px-3.5 py-2 text-sm outline-none placeholder:text-sv-muted focus:border-sv-accent"
            />
          )}
          <section aria-label="Past meetings" className="min-h-0 flex-1 overflow-y-auto pr-1">
            {found && shown.length === 0 ? (
              <p className="px-1 pt-2 text-sm text-sv-muted">No meeting mentions "{query.trim()}".</p>
            ) : list.length === 0 ? (
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
                        <MeetingRow t={t} active={t.id === selected} onClick={() => setSelected(t.id)} onDeleted={() => removed(t.id)} onError={setError} />
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
            onDeleted={() => removed(full.id)}
            onRenamed={(name) => {
              setList((l) => l.map((x) => (x.id === full.id ? { ...x, name } : x)));
              setFull({ ...full, name });
            }}
            onSummarized={(next) => setFull((cur) => (cur?.id === next.id ? next : cur))}
            summaryWith={summary.chosen}
            onSettings={() => setSettingsOpen(true)}
            redoWith={busy ? null : cloud ? cloud.name : "this PC"}
            onRedo={() => redo(full.id)}
          />
        ) : (
          <div className="hidden rounded-2xl border border-sv-border bg-sv-surface md:block" />
        )}
      </div>
    </Page>
  );
}

/** The models the Notetaker uses, kept out of the way behind the gear button. */
function NotetakerSettings({
  onClose,
  summary,
  stt,
  sttNote,
  language,
  languageNote,
}: {
  onClose: () => void;
  summary: SummaryChoice;
  stt: React.ReactNode;
  sttNote: string;
  language: React.ReactNode;
  languageNote: string;
}) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => {
      // The model list opens inside the panel; anything outside closes it.
      if (!box.current?.contains(e.target as Node) && !(e.target as Element).closest?.('[aria-label="Notetaker settings"]')) onClose();
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [onClose]);

  return (
    <div
      ref={box}
      role="dialog"
      aria-label="Notetaker settings"
      className="absolute right-0 top-[calc(100%+8px)] z-40 w-[min(26rem,calc(100vw-2rem))] space-y-5 rounded-2xl border border-sv-border bg-sv-surface p-5 text-left shadow-[0_16px_40px_-12px_rgb(0_0_0/0.5)]"
    >
      <section>
        <h3 className="text-sm font-semibold">Transcription</h3>
        <p className="mb-2 mt-0.5 text-xs text-sv-muted">Turns the meeting's audio into text as people talk.</p>
        {stt}
        <p className="mt-1.5 text-xs text-sv-muted">{sttNote}</p>
        <h4 className="mb-1.5 mt-4 text-xs font-semibold text-sv-muted">Language spoken</h4>
        {language}
        <p className="mt-1.5 text-xs text-sv-muted">{languageNote}</p>
      </section>
      <section className="border-t border-sv-border pt-4">
        <h3 className="text-sm font-semibold">Summary</h3>
        <p className="mb-2 mt-0.5 text-xs text-sv-muted">Writes the Overview: key points, decisions and action items.</p>
        <SummarySettings s={summary} />
      </section>
    </div>
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

function MeetingRow({
  t,
  active,
  onClick,
  onDeleted,
  onError,
}: {
  t: Transcript;
  active: boolean;
  onClick: () => void;
  onDeleted: () => void;
  onError: (e: string) => void;
}) {
  const live = t.status === "running";
  const [confirm, setConfirm] = useState(false);
  useEffect(() => {
    if (!confirm) return;
    const timer = setTimeout(() => setConfirm(false), 4000);
    return () => clearTimeout(timer);
  }, [confirm]);
  const act =
    "flex h-7 items-center justify-center rounded-md px-1.5 text-sv-muted transition hover:bg-sv-surface hover:text-sv-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent";
  return (
    <div className="group relative">
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
      {!live && (
        <div
          className={`absolute right-2 top-1/2 flex -translate-y-1/2 items-center gap-0.5 rounded-lg bg-sv-surface-2 p-0.5 ${
            confirm ? "" : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
          }`}
        >
          <button className={act} title="Save…" aria-label={`Save ${title(t)}`} onClick={() => transcriptSaveAs(t.id).catch((e) => onError(String(e)))}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round" className="h-4 w-4" aria-hidden>
              <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14" />
            </svg>
          </button>
          <button
            className={`${act} ${confirm ? "gap-1 text-sv-bad hover:text-sv-bad" : ""}`}
            title={confirm ? "Click again to delete" : "Delete"}
            aria-label={confirm ? `Confirm delete ${title(t)}` : `Delete ${title(t)}`}
            onClick={async () => {
              if (!confirm) return setConfirm(true);
              try {
                await transcriptDelete(t.id);
                onDeleted();
              } catch (e) {
                onError(String(e));
              }
            }}
          >
            <TrashIcon />
            {confirm && <span className="text-xs font-medium">Delete?</span>}
          </button>
        </div>
      )}
    </div>
  );
}

function MeetingPanel({
  t,
  stopping,
  onDeleted,
  onRenamed,
  onSummarized,
  summaryWith,
  onSettings,
  redoWith,
  onRedo,
}: {
  t: Transcript;
  stopping: boolean;
  onDeleted: () => void;
  onRenamed: (name: string) => void;
  onSummarized: (t: Transcript) => void;
  summaryWith?: Choice;
  onSettings: () => void;
  /** Where "Transcribe again" would send the audio; null while a job runs. */
  redoWith: string | null;
  onRedo: () => void;
}) {
  const [confirmRedo, setConfirmRedo] = useState(false);
  useEffect(() => {
    if (!confirmRedo) return;
    const timer = setTimeout(() => setConfirmRedo(false), 5000);
    return () => clearTimeout(timer);
  }, [confirmRedo]);
  const player = usePlayer(t.id);
  const paras = useMemo(() => transcriptParagraphs(t.lines), [t]);
  const scroller = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const live = t.status === "running";

  useEffect(() => {
    const el = scroller.current;
    if (live && el && el.scrollHeight - el.scrollTop - el.clientHeight < 160) {
      bottom.current?.scrollIntoView({ block: "end" });
    }
  }, [live, t.lines.length]);

  return (
    <section className="flex min-h-0 flex-col overflow-hidden rounded-2xl border border-sv-border bg-sv-surface">
      <TranscriptHeader
        id={t.id}
        title={title(t)}
        name={t.name}
        details={
          <>
            {dayLabel(t.created)} · {timeOf(t.created)}
            {!live && ` · ${clock(t.duration_ms ?? t.done_ms)}`}
            {t.redo && (
              <>
                {" · transcribing again… "}
                <button
                  onClick={() => transcribeCancel()}
                  title="Stop and keep the old text"
                  className="rounded font-medium text-sv-accent hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
                >
                  Cancel
                </button>
              </>
            )}
            {!live && redoWith && (
              <>
                {" · "}
                <button
                  onClick={() => {
                    if (!confirmRedo) return setConfirmRedo(true);
                    setConfirmRedo(false);
                    onRedo();
                  }}
                  title={`Transcribe this meeting's audio again with ${redoWith}. The Overview is kept.`}
                  className={`rounded font-medium hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
                    confirmRedo ? "text-sv-warn" : "text-sv-accent"
                  }`}
                >
                  {confirmRedo ? `Replace the text? Transcribe with ${redoWith}` : "Transcribe again"}
                </button>
              </>
            )}
          </>
        }
        copyText={(t.summary ? t.summary.trim() + "\n\n---\n\n" : "") + paras.map((p) => speakerLabel(p.speaker) + p.text).join("\n\n")}
        busy={live}
        onRenamed={onRenamed}
        onDeleted={onDeleted}
      />

      {t.error && (
        <p role="alert" className="border-b border-sv-border bg-sv-warn/10 px-6 py-2.5 text-xs text-sv-warn">
          {t.error}
        </p>
      )}

      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        <MeetingOverview t={t} onSummarized={onSummarized} chosen={summaryWith} onSettings={onSettings} />

        {paras.length === 0 && !live ? (
          <p className="text-sm text-sv-muted">No speech was picked up in this meeting.</p>
        ) : (
          <ol className="space-y-5">
            {paras.map((p, i) => (
              <li key={i} className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-3">
                {live || !p.speaker ? (
                  <span className="pt-0.5 text-right text-[11px] tabular-nums text-sv-muted">{clock(p.start)}</span>
                ) : (
                  <button
                    onClick={() => player.toggle(i, p.speaker!, p.start, p.end)}
                    aria-label={player.playing === i ? "Stop playing" : `Play from ${clock(p.start)}`}
                    title={player.playing === i ? "Stop" : "Play this part"}
                    className={`flex h-fit items-center justify-end gap-1 rounded pt-0.5 text-[11px] tabular-nums transition hover:text-sv-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
                      player.playing === i ? "text-sv-accent" : "text-sv-muted"
                    }`}
                  >
                    <svg viewBox="0 0 12 12" className="h-2.5 w-2.5 shrink-0" fill="currentColor" aria-hidden>
                      {player.playing === i ? <rect x="2" y="2" width="8" height="8" rx="1" /> : <path d="M3 1.5v9l7.5-4.5z" />}
                    </svg>
                    {clock(p.start)}
                  </button>
                )}
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

/** Plays one paragraph of a meeting's audio at a time; a second click stops it. */
function usePlayer(id: number) {
  const [playing, setPlaying] = useState<number | null>(null);
  const audio = useRef<{ el: HTMLAudioElement; url: string } | null>(null);
  const stop = useCallback(() => {
    if (audio.current) {
      audio.current.el.pause();
      URL.revokeObjectURL(audio.current.url);
      audio.current = null;
    }
    setPlaying(null);
  }, []);
  useEffect(() => stop, [id, stop]);
  const toggle = async (i: number, speaker: "you" | "others", start: number, end: number) => {
    const again = playing === i;
    stop();
    if (again) return;
    setPlaying(i);
    try {
      const bytes = await meetingAudioClip(id, speaker, start, end);
      const url = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
      const el = new Audio(url);
      audio.current = { el, url };
      el.onended = stop;
      await el.play();
    } catch {
      stop();
    }
  };
  return { playing, toggle };
}
