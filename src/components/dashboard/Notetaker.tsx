import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import Page from "../shared/Page";
import Select from "../shared/Select";
import { useNotetakerSettings, useSettingsStore } from "../../stores/settingsStore";
import { DownloadButton, RemoveButton } from "./settings/downloads";
import { Info, Toggle } from "./settings/parts";
import { LANGUAGES, STT_MODELS, honorsLanguage } from "../../services/catalog";
import TranscriptHeader, { ToolButton, clock } from "../shared/TranscriptHeader";
import { TrashIcon } from "../shared/TranscriptCard";
import MeetingOverview, { GearIcon, Picker, SummarySettings, useSummaryChoice, type Choice, type SummaryChoice } from "./MeetingOverview";
import { ModelPicker } from "./providers/shared";
import {
  listenEvent,
  meetingAudioClip,
  meetingCaptureSupport,
  meetingDevices,
  meetingOpenCaptureSettings,
  type CaptureSupport,
  meetingRetranscribe,
  meetingSetPeople,
  meetingsSearch,
  meetingStart,
  meetingUndoAgent,
  agentOf,
  meetingStop,
  transcriptDelete,
  transcriptGet,
  transcriptSaveAs,
  transcribeCancel,
  speakerLabel,
  speakerName,
  diarizeInstalled,
  downloadDiarizeModel,
  deleteDiarizeModel,
  transcriptParagraphs,
  transcriptsList,
  type Transcript,
} from "../../services/tauriBridge";

// "Notetaker": record a call (your mic + the computer's sound) into a live,
// You/Others transcript. The Rust job (meeting_job.rs) does the work and sends
// the transcript as it grows, like the Transcribe page.

/** A gap this long between words gets an "N min of silence" line. */
const SILENCE_MARK_MS = 60_000;

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
  const [capture, setCapture] = useState<CaptureSupport>(null);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [nt, setNt] = useNotetakerSettings();
  const consentSeen = nt.consent_seen;

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

  // An AI agent saves over MCP from another process, which sends the page no
  // event: read the meetings again whenever the window comes back into view.
  const reload = useCallback(() => {
    transcriptsList().then((all) => {
      const l = all.filter((t) => t.kind === "meeting");
      setList(l);
      setSelected((s) => s ?? l[0]?.id ?? null);
    });
    const id = selectedRef.current;
    if (id !== null) transcriptGet(id).then((t) => selectedRef.current === t.id && setFull(t), () => {});
  }, []);

  useEffect(() => {
    const onVisible = () => document.visibilityState === "visible" && reload();
    window.addEventListener("focus", reload);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", reload);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [reload]);

  useEffect(() => {
    reload();
    meetingDevices().then(setDevices, () => {});
    meetingCaptureSupport().then(setCapture, () => {});
    const un = listenEvent<Transcript>("transcript://update", (t) => {
      if (t.kind !== "meeting") return;
      upsert(t);
      if (t.status !== "running") setStopping(false);
      if (t.id === selectedRef.current) setFull(t);
    });
    return () => {
      un.then((f) => f());
    };
  }, [upsert, reload]);

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

  const sttProviders = useSettingsStore((s) => s.providers).filter((p) => p.uses.includes("stt") && p.base_url);
  const pickSttModel = (providerId: string, model: string) => setNt({ stt_models: { ...nt.stt_models, [providerId]: model } });
  const sttModelOf = (p: { id: string; stt_model: string }) => nt.stt_models[p.id] || p.stt_model;
  const sttKey = nt.stt;
  const cloudProvider = sttProviders.find((p) => p.id === sttKey);
  // "Record only": keep the audio; an AI agent transcribes it over MCP later.
  // stt "record" is how it was stored before it became its own toggle.
  const recordOnly = nt.record_only ?? sttKey === "record";
  const setRecordOnly = (v: boolean) => setNt({ record_only: v, ...(sttKey === "record" ? { stt: "local" } : {}) });
  // A provider with no speech-to-text model picked can't transcribe yet.
  const cloud = cloudProvider && sttModelOf(cloudProvider) ? cloudProvider : undefined;
  const lang = nt.language;
  const pickLang = (v: string) => setNt({ language: v });
  const dictationLang = useSettingsStore((s) => s.settings.language);
  const sttChoices: Choice[] = [
    ...sttProviders.map((p) => ({
      key: `api:${p.id}`,
      id: p.id,
      label: p.name,
      detail: sttModelOf(p) || "Choose a model",
      source: "api" as const,
      model: sttModelOf(p),
      base_url: p.base_url,
      api_key: p.api_key,
    })),
    {
      key: "local:local",
      id: "local",
      label: "Your voice model",
      detail: STT_MODELS.find((m) => m.id === useSettingsStore.getState().settings.active_stt_model)?.label ?? "",
      source: "local" as const,
      model: "",
      base_url: "",
      api_key: "",
    },
  ];
  const localModel = STT_MODELS.find((m) => m.id === useSettingsStore.getState().settings.active_stt_model);
  // "bn,en": the first is the main language, the rest are also spoken.
  const [firstLang = "", ...alsoLangs] = lang.split(",");
  const meetingLang = firstLang || dictationLang;
  const listed = alsoLangs.length > 0 ? [meetingLang, ...alsoLangs] : null;
  const langName = (c: string) => LANGUAGES.find((l) => l.code === c)?.name ?? c;
  // Parakeet, Moonshine and .en models ignore the language: say so instead of failing quietly.
  // Measured on a Bangla meeting: Whisper turbo wrote English, v3 garbled it; these got it right.
  const langNote = listed
    ? `Each part is checked for ${listed.map(langName).join(", ")}. A part that comes back in another script (Hindi letters in a Bangla call, say) is transcribed again in the first of them with its own letters (Bengali rather than English, say).`
    : cloud
    ? meetingLang !== "en" && /whisper/i.test(sttModelOf(cloud))
      ? `Whisper models handle ${meetingLang === "auto" ? "languages other than English" : langName(meetingLang)} poorly (turbo turns them into English). For better results pick microsoft/mai-transcribe-2 or elevenlabs/scribe-v2 from OpenRouter.`
      : meetingLang === "auto"
        ? `${cloud.name} detects the language as people talk, mixed languages included.`
        : `${cloud.name} is told the meeting is in ${langName(meetingLang)}.`
    : !honorsLanguage(localModel) && meetingLang !== "en"
      ? `Your voice model (${localModel?.label ?? "this PC's"}) only understands ${localModel?.id.startsWith("sense-voice") ? "a few languages" : "English"}. For ${meetingLang === "auto" ? "other languages" : langName(meetingLang)}, transcribe with a cloud provider or download a Whisper model.`
      : `Transcribed in ${meetingLang === "auto" ? "whatever language is spoken" : langName(meetingLang)}.`;
  // A language the chosen model handles badly is a warning, shown in full; the rest sits behind an info icon.
  const langWarn = !listed && (cloud ? meetingLang !== "en" && /whisper/i.test(sttModelOf(cloud)) : !honorsLanguage(localModel) && meetingLang !== "en");
  const summary = useSummaryChoice();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const pickStt = (v: string) => setNt({ stt: v });

  const cloudArg = cloud && { base_url: cloud.base_url, api_key: cloud.api_key, model: sttModelOf(cloud) };

  const redo = async (id: number) => {
    setError(null);
    try {
      const t = await meetingRetranscribe(id, cloudArg, lang || undefined, nt.speakers);
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
      const t = await meetingStart(cloudArg, lang || undefined, nt.speakers, recordOnly);
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

  const dismissConsent = () => setNt({ consent_seen: true });

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
      subtitle={
        <span className="inline-flex items-center gap-1.5">
          {recordOnly ? "Record a call now, transcribe it later." : "Record a call and get a live transcript of who said what."}
          <Info
            label="where your audio goes"
            text={
              recordOnly
                ? "Only the audio is kept, on this PC. An AI agent you connect in Settings transcribes it later, or pick Transcribe here on the meeting."
                : cloud
                  ? `Audio is sent to ${cloud.name} to transcribe. Recordings and text stay on this PC.`
                  : "Audio and text stay on this PC. Nothing is uploaded."
            }
          />
        </span>
      }
      actions={
        <div className="relative flex items-center gap-2">
          <button
            onClick={() => setSettingsOpen((o) => !o)}
            aria-expanded={settingsOpen}
            aria-label="Notetaker settings"
            title="Notetaker settings"
            className={`grid h-10 w-10 shrink-0 place-items-center rounded-xl border bg-sv-surface transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
              settingsOpen ? "border-sv-accent/70 bg-sv-surface-2 text-sv-text" : "border-sv-border text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text"
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
            className="flex h-10 shrink-0 items-center gap-2 rounded-xl bg-sv-accent px-4 text-sm font-semibold text-sv-on-accent shadow-sm transition hover:bg-sv-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-bg disabled:opacity-60"
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
                <div className="flex flex-col gap-2">
                  <Select value={firstLang} onChange={(v) => pickLang(v && v !== "auto" ? [v, ...alsoLangs.filter((x) => x !== v)].join(",") : v)} className="w-full" disabled={!!recording}>
                    <option value="">Same as dictation ({langName(dictationLang)})</option>
                    {LANGUAGES.map((l) => (
                      <option key={l.code} value={l.code}>
                        {l.name}
                      </option>
                    ))}
                  </Select>
                  {meetingLang !== "auto" && (
                    <div className="flex flex-wrap items-center gap-1.5 text-xs">
                      <span className="text-sv-muted">Also spoken:</span>
                      {alsoLangs.map((code) => (
                        <button
                          key={code}
                          onClick={() => pickLang([firstLang || meetingLang, ...alsoLangs.filter((x) => x !== code)].join(","))}
                          disabled={!!recording}
                          aria-label={`Remove ${langName(code)}`}
                          className="flex items-center gap-1 rounded-full border border-sv-border px-2 py-0.5 hover:border-sv-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
                        >
                          {langName(code)} <span aria-hidden>×</span>
                        </button>
                      ))}
                      <select
                        value=""
                        onChange={(e) => e.target.value && pickLang([firstLang || meetingLang, ...alsoLangs, e.target.value].join(","))}
                        disabled={!!recording}
                        aria-label="Add a language"
                        className="h-6 rounded-md border border-sv-border bg-sv-bg px-1 text-xs text-sv-muted outline-none focus:border-sv-accent"
                      >
                        <option value="">+ Add</option>
                        {LANGUAGES.filter((l) => l.code !== "auto" && l.code !== meetingLang && !alsoLangs.includes(l.code)).map((l) => (
                          <option key={l.code} value={l.code}>
                            {l.name}
                          </option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>
              }
              languageNote={langNote}
              languageWarn={langWarn}
              locked={!!recording}
              recordOnly={recordOnly}
              onRecordOnly={setRecordOnly}
              stt={
                <div className="flex flex-col gap-2">
                  <Picker
                    choices={sttChoices}
                    chosen={sttChoices.find((c) => c.id === (cloudProvider?.id ?? "local")) ?? sttChoices[0]}
                    disabled={!!recording}
                    onPick={(k) => pickStt(k.replace(/^(api|local):/, ""))}
                    action="Transcribe with"
                    localTitle="On this PC · offline"
                  />
                  {cloudProvider && (
                    <ModelPicker
                      p={cloudProvider}
                      api={summary.providerApi}
                      stt
                      value={sttModelOf(cloudProvider)}
                      onPick={(m) => pickSttModel(cloudProvider.id, m)}
                    />
                  )}
                </div>
              }
              sttNote={
                recordOnly
                    ? "Record only is on, so this is used when you press Transcribe here on a recorded meeting."
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
              capture={capture}
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
              <p className="px-1 pt-2 text-sm text-sv-muted">No meetings yet.</p>
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
            onUndoAgent={async () => {
              try {
                const t = await meetingUndoAgent(full.id);
                upsert(t);
                setFull(t);
              } catch (e) {
                setError(String(e));
              }
            }}
          />
        ) : (
          <div className="hidden flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-sv-border px-8 text-center md:flex">
            <span className="grid h-11 w-11 place-items-center rounded-xl bg-sv-surface-2 text-sv-muted" aria-hidden>
              <MicIcon />
            </span>
            <div>
              <p className="text-sm font-medium text-sv-text">{list.length ? "Pick a meeting to open it" : "Your meetings show up here"}</p>
              <p className="mx-auto mt-1 max-w-[34ch] text-xs leading-relaxed text-sv-muted">
                {list.length ? "Its transcript, overview and recording open on this side." : "Start recording when a call begins and stop when it ends. The transcript fills in as people talk."}
              </p>
            </div>
          </div>
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
  languageWarn,
  locked,
  recordOnly,
  onRecordOnly,
}: {
  onClose: () => void;
  summary: SummaryChoice;
  stt: React.ReactNode;
  sttNote: string;
  language: React.ReactNode;
  languageNote: string;
  languageWarn: boolean;
  /** A meeting is recording: changes wait for the next one. */
  locked: boolean;
  recordOnly: boolean;
  onRecordOnly: (v: boolean) => void;
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
      className="absolute right-0 top-[calc(100%+8px)] z-40 max-h-[calc(100vh-8rem)] w-[min(24rem,calc(100vw-2rem))] divide-y divide-sv-border overflow-y-auto rounded-2xl border border-sv-border bg-sv-surface text-left shadow-[0_16px_40px_-12px_rgb(0_0_0/0.5)]"
    >
      {locked && <p className="bg-sv-surface-2/60 px-5 py-2.5 text-xs text-sv-muted">Recording now. Changes apply to the next meeting.</p>}
      <section className="px-5 py-4">
        <SettingHead title="Language spoken" about={`What people speak in your meetings.${languageWarn ? "" : ` ${languageNote}`}`} />
        {language}
        {languageWarn && <p className="mt-2 text-xs leading-relaxed text-sv-warn">{languageNote}</p>}
      </section>
      <section className="px-5 py-4">
        <SettingHead
          title="Record only"
          about="Keep only the audio while you record, and transcribe it later: with an AI agent you connect in Settings > Connect an AI agent (no API key needed here), or with Transcribe here on the meeting."
        >
          <Toggle label="Record only" checked={recordOnly} onChange={onRecordOnly} disabled={locked} />
        </SettingHead>
        {recordOnly && <p className="-mt-1 text-xs text-sv-muted">Nothing is transcribed while you record.</p>}
      </section>
      <section className="px-5 py-4">
        <SettingHead
          title={recordOnly ? "Transcribe later with" : "Transcription"}
          about={`Turns the meeting's audio into text${recordOnly ? "" : " as people talk"}. ${sttNote}`}
        />
        {stt}
      </section>
      <SpeakersSetting />
      <section className="px-5 py-4">
        <SettingHead title="Summary" about="Writes the Overview: key points, decisions and action items." />
        <SummarySettings s={summary} />
      </section>
    </div>
  );
}

/** A settings heading with its explanation behind an info icon, and an optional control on the right. */
function SettingHead({ title, about, children }: { title: string; about: string; children?: React.ReactNode }) {
  return (
    <div className="mb-2.5 flex min-h-6 items-center justify-between gap-3">
      <div className="flex items-center gap-1.5">
        <h3 className="text-sm font-semibold">{title}</h3>
        <Info label={title} text={about} />
      </div>
      {children}
    </div>
  );
}

/** "Tell speakers apart": download the speaker models first, then switch it on or off. */
function SpeakersSetting() {
  const [nt, setNt] = useNotetakerSettings();
  const [ready, setReady] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    diarizeInstalled().then(setReady);
  }, []);
  return (
    <section className="px-5 py-4">
      <SettingHead
        title="Tell speakers apart"
        about="After you stop, Others is split into Speaker 1, Speaker 2… by voice, on this PC. It takes about 15 s per minute of meeting; the text shows meanwhile."
      >
        {ready === null ? null : ready ? (
          <div className="flex items-center gap-3">
            <RemoveButton
              onClick={async () => {
                await deleteDiarizeModel();
                setReady(false);
              }}
            />
            <Toggle label="Tell speakers apart" checked={nt.speakers} onChange={(v) => setNt({ speakers: v })} />
          </div>
        ) : (
          <DownloadButton
            modelId="speakers"
            sizeLabel="47 MB"
            start={async () => {
              setError(null);
              try {
                if (await downloadDiarizeModel()) {
                  setReady(true);
                  setNt({ speakers: true });
                }
              } catch (e) {
                setError(String(e));
              }
            }}
          />
        )}
      </SettingHead>
      {error && <p className="text-xs text-sv-warn">Download failed: {error}</p>}
    </section>
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

/** Why Others can't be recorded here, and the fix where there is one. Start still records You. */
function CaptureNote({ capture }: { capture: NonNullable<CaptureSupport> }) {
  return (
    <div className="space-y-2 rounded-lg bg-sv-warn/10 px-3 py-2 text-xs leading-relaxed text-sv-warn" role="status">
      {capture.kind === "permission" ? (
        <>
          <p>
            Silent Voice needs permission to record the computer's sound. Allow it in System Settings &gt; Privacy &amp;
            Security &gt; Screen &amp; System Audio Recording, then restart Silent Voice. macOS may ask again from time
            to time. Until then, recording keeps only your side.
          </p>
          <button
            onClick={() => meetingOpenCaptureSettings()}
            className="rounded-md border border-sv-warn/40 px-2 py-0.5 font-medium hover:bg-sv-warn/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-warn"
          >
            Open System Settings
          </button>
        </>
      ) : capture.kind === "unsupported" ? (
        <p>Recording other people {capture.reason}. Recording keeps only your side.</p>
      ) : (
        <p>Can't record the computer's sound: {capture.reason} Recording keeps only your side.</p>
      )}
    </div>
  );
}

function Setup({
  mic,
  speakers,
  capture,
  btMic,
  showConsent,
  onConsentSeen,
}: {
  mic: string | null;
  speakers: string | null;
  capture: CaptureSupport;
  btMic: boolean;
  showConsent: boolean;
  onConsentSeen: () => void;
}) {
  return (
    <section aria-labelledby="nt-sources" className="rounded-2xl border border-sv-border bg-sv-surface">
      <div className="flex items-center justify-between gap-2 px-4 pb-1 pt-3">
        <h2 id="nt-sources" className="text-xs font-semibold text-sv-muted">
          What gets recorded
        </h2>
        <Info
          label="getting a clean recording"
          text={`Best with headphones. On speakers your mic also hears the others; Silent Voice removes those repeats, but headphones keep "You" cleanest. To change the mic, open Settings.`}
        />
      </div>
      <ul className="px-2 pb-2">
        <Source
          icon={<MicIcon />}
          who="You"
          device={mic ?? "Default microphone"}
          state={btMic ? "warn" : "ok"}
          note={
            btMic && (
              <span className="flex items-center gap-1 text-sv-warn">
                Bluetooth headset: everyone sounds worse
                <Info
                  label="Bluetooth microphones"
                  text="Recording from a Bluetooth headset switches it to call quality, so everyone sounds worse. Pick your laptop's built-in mic in Settings for better sound."
                />
              </span>
            )
          }
        />
        <Source
          icon={<SpeakerIcon />}
          who="Others"
          device={capture ? "Not recorded" : speakers ? `What plays on ${speakers}` : "What plays on your speakers"}
          state={capture ? "off" : "ok"}
        />
      </ul>
      {capture && (
        <div className="px-4 pb-3">
          <CaptureNote capture={capture} />
        </div>
      )}
      {showConsent && (
        <div className="flex items-center gap-2.5 border-t border-sv-border px-4 py-2.5">
          <p className="min-w-0 flex-1 text-xs leading-5 text-sv-text">
            Tell people you're recording.{" "}
            <span className="inline-block align-middle">
              <Info label="recording consent" text="In many places everyone on a call has to agree to being recorded." />
            </span>
          </p>
          <button
            onClick={onConsentSeen}
            className="shrink-0 rounded-md px-2 py-1 text-xs font-medium text-sv-accent hover:bg-sv-accent/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
          >
            Got it
          </button>
        </div>
      )}
    </section>
  );
}

/** One recorded side: an icon, who it is, the device, and a status dot. */
function Source({
  icon,
  who,
  device,
  state,
  note,
}: {
  icon: React.ReactNode;
  who: string;
  device: string;
  state: "ok" | "warn" | "off";
  note?: React.ReactNode;
}) {
  const dot = { ok: ["bg-sv-good", "Ready"], warn: ["bg-sv-warn", "Check this"], off: ["bg-sv-muted/50", "Off"] }[state];
  return (
    <li className="flex items-center gap-3 rounded-xl px-2 py-2">
      <span
        className={`grid h-9 w-9 shrink-0 place-items-center rounded-lg ${state === "off" ? "bg-sv-surface-2/50 text-sv-muted/60" : "bg-sv-surface-2 text-sv-text"}`}
        aria-hidden
      >
        {icon}
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-medium uppercase tracking-wide text-sv-muted">{who}</div>
        <div className={`truncate text-sm ${state === "off" ? "text-sv-muted" : "text-sv-text"}`} title={device}>
          {device}
        </div>
        {note && <div className="mt-0.5 text-xs">{note}</div>}
      </div>
      <span className={`h-2 w-2 shrink-0 rounded-full ${dot[0]}`} role="img" aria-label={dot[1]} title={dot[1]} />
    </li>
  );
}

function MicIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-[18px] w-[18px]" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
    </svg>
  );
}

function SpeakerIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-[18px] w-[18px]" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 9.5v5h3.5L12 18.5v-13L7.5 9.5H4Z" />
      <path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" />
    </svg>
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
          {t.status === "recorded" && " · waiting for an AI agent"}
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
  onUndoAgent,
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
  /** Put back the meeting as it was before an AI agent changed it. */
  onUndoAgent: () => void;
}) {
  const [confirmRedo, setConfirmRedo] = useState(false);
  useEffect(() => {
    if (!confirmRedo) return;
    const timer = setTimeout(() => setConfirmRedo(false), 5000);
    return () => clearTimeout(timer);
  }, [confirmRedo]);
  const duration = t.duration_ms ?? t.done_ms;
  const player = usePlayer(t.id, duration);
  const paras = useMemo(() => transcriptParagraphs(t.lines), [t]);
  // The paragraph being heard: the last one started by now (the tracks overlap).
  const current = player.playing || player.pos > 0 ? paras.reduce((c, p, i) => (p.start <= player.pos ? i : c), -1) : -1;
  // The quiet before each paragraph. The two tracks overlap, so it starts at the latest end so far.
  const quietBefore = useMemo(() => {
    let heard = Infinity;
    return paras.map((p) => {
      const quiet = p.start - heard;
      heard = Math.max(heard === Infinity ? 0 : heard, p.end);
      return quiet;
    });
  }, [paras]);
  const scroller = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const live = t.status === "running";
  // Record only: audio, no text yet. Nothing to replace, so no confirm step.
  const waiting = t.status === "recorded";
  const agent = agentOf(t);
  const [confirmUndo, setConfirmUndo] = useState(false);
  useEffect(() => {
    if (!confirmUndo) return;
    const timer = setTimeout(() => setConfirmUndo(false), 5000);
    return () => clearTimeout(timer);
  }, [confirmUndo]);

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
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
            <span className="flex items-center gap-1.5">
              <CalendarIcon />
              {dayLabel(t.created)}, {timeOf(t.created)}
            </span>
            {!live && (
              <span className="flex items-center gap-1.5 tabular-nums">
                <ClockIcon />
                {clock(t.duration_ms ?? t.done_ms)}
              </span>
            )}
            {!live && !t.redo && (t.others != null || t.lines.some((l) => l.who)) && <PeopleCount t={t} />}
            {agent && !live && (
              <span className="flex items-center gap-1 rounded-full bg-sv-surface-2 py-0.5 pl-2.5 pr-1 text-sv-text">
                Transcribed by {agent}
                <button
                  onClick={() => {
                    if (!confirmUndo) return setConfirmUndo(true);
                    setConfirmUndo(false);
                    onUndoAgent();
                  }}
                  aria-label={confirmUndo ? `Confirm: undo ${agent}'s changes` : `Undo ${agent}'s changes`}
                  className={`rounded-full px-2 py-0.5 font-medium transition hover:bg-sv-bg/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
                    confirmUndo ? "text-sv-warn" : "text-sv-accent"
                  }`}
                >
                  {confirmUndo ? "Undo changes?" : "Undo"}
                </button>
              </span>
            )}
            {t.redo && (
              <span className="flex items-center gap-1.5 rounded-full bg-sv-accent/10 py-0.5 pl-2.5 pr-1 text-sv-text" role="status">
                <span className="h-3 w-3 animate-spin rounded-full border-2 border-sv-accent/30 border-t-sv-accent" aria-hidden />
                Transcribing again
                <button
                  onClick={() => transcribeCancel()}
                  aria-label="Stop transcribing again and keep the old text"
                  className="rounded-full px-2 py-0.5 font-medium text-sv-accent transition hover:bg-sv-bg/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
                >
                  Cancel
                </button>
              </span>
            )}
          </div>
        }
        tools={
          !live &&
          !waiting &&
          redoWith && (
            <ToolButton
              onClick={() => {
                if (!confirmRedo) return setConfirmRedo(true);
                setConfirmRedo(false);
                onRedo();
              }}
              label={confirmRedo ? "Replace text?" : "Transcribe again"}
              danger={confirmRedo}
            >
              <RedoIcon />
            </ToolButton>
          )
        }
        copyText={(t.summary ? t.summary.trim() + "\n\n---\n\n" : "") + paras.map((p) => speakerLabel(p.speaker, p.who) + p.text).join("\n\n")}
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
        {/* Nothing to summarize until there is text. */}
        {!waiting && <MeetingOverview t={t} onSummarized={onSummarized} chosen={summaryWith} onSettings={onSettings} />}

        {waiting ? (
          <div className="rounded-xl border border-sv-border bg-sv-surface-2/50 px-4 py-3.5 text-sm">
            <p className="font-medium text-sv-text">Only the audio was recorded</p>
            <p className="mt-1 text-sv-muted">
              An AI agent on this PC can transcribe it: connect one in Settings, then ask it to transcribe your latest meeting.
            </p>
            {redoWith && (
              <button
                onClick={onRedo}
                className="mt-3 rounded-lg border border-sv-border px-3 py-1.5 text-sm font-medium text-sv-text transition hover:border-sv-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
              >
                Transcribe here with {redoWith}
              </button>
            )}
          </div>
        ) : paras.length === 0 && !live ? (
          <p className="text-sm text-sv-muted">No speech was picked up in this meeting.</p>
        ) : (
          <ol className="space-y-5">
            {paras.map((p, i) => (
              <Fragment key={i}>
                {quietBefore[i] >= SILENCE_MARK_MS && (
                  <li className="flex items-center gap-3 text-[11px] text-sv-muted">
                    <span className="h-px flex-1 bg-sv-border" aria-hidden />
                    {Math.round(quietBefore[i] / 60_000)} min of silence
                    <span className="h-px flex-1 bg-sv-border" aria-hidden />
                  </li>
                )}
              <li className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-3">
                {live || !p.speaker ? (
                  <span className="pt-0.5 text-right text-[11px] tabular-nums text-sv-muted">{clock(p.start)}</span>
                ) : (
                  <button
                    onClick={() => player.playFrom(p.start)}
                    aria-label={`Play from ${clock(p.start)}`}
                    title="Play from here"
                    className={`flex h-fit items-center justify-end gap-1 rounded pt-0.5 text-[11px] tabular-nums transition hover:text-sv-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
                      current === i ? "text-sv-accent" : "text-sv-muted"
                    }`}
                  >
                    <svg viewBox="0 0 12 12" className="h-2.5 w-2.5 shrink-0" fill="currentColor" aria-hidden>
                      <path d="M3 1.5v9l7.5-4.5z" />
                    </svg>
                    {clock(p.start)}
                  </button>
                )}
                <div className={`max-w-[72ch] rounded-md transition-colors ${current === i ? "-mx-2 bg-sv-accent/10 px-2" : ""}`} aria-current={current === i || undefined}>
                  <div className={`text-xs font-semibold ${p.speaker === "you" ? "text-sv-accent" : "text-sv-text"}`}>
                    {speakerName(p.speaker, p.who) || "Others"}
                  </div>
                  <p className="mt-0.5 text-[15px] leading-relaxed text-sv-text">{p.text}</p>
                </div>
              </li>
              </Fragment>
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
      {/* A record-only meeting has audio before it has any lines. */}
      {!live && (waiting || paras.some((p) => p.speaker)) && duration > 0 && <PlayerBar player={player} duration={duration} />}
    </section>
  );
}

/** Plays the whole meeting, both tracks mixed, fetched a window at a time
 * (an hour of audio is ~115 MB). `pos` is the playing time in the meeting. */
// shortcut: a tiny gap where one window hands over to the next; stream one file if it's heard.
const WINDOW_MS = 120_000;
function usePlayer(id: number, duration: number) {
  const [playing, setPlaying] = useState(false);
  const [pos, setPos] = useState(0);
  const [rate, setRateState] = useState(1);
  const audio = useRef<{ el: HTMLAudioElement; url: string } | null>(null);
  const run = useRef(0);
  const rateRef = useRef(1);
  const release = () => {
    if (audio.current) {
      audio.current.el.pause();
      URL.revokeObjectURL(audio.current.url);
      audio.current = null;
    }
  };
  const pause = useCallback(() => {
    run.current++;
    release();
    setPlaying(false);
  }, []);
  useEffect(() => {
    setPos(0);
    return pause;
  }, [id, pause]);
  const playFrom = async (ms: number) => {
    const mine = ++run.current;
    release();
    setPlaying(true);
    setPos(ms);
    try {
      let next = meetingAudioClip(id, null, ms, ms + WINDOW_MS);
      for (let from = ms; from < duration; from += WINDOW_MS) {
        const bytes = await next;
        if (run.current !== mine) return;
        // Fetch the next window while this one plays.
        if (from + WINDOW_MS < duration) next = meetingAudioClip(id, null, from + WINDOW_MS, from + 2 * WINDOW_MS);
        const url = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
        const el = new Audio(url);
        el.playbackRate = rateRef.current;
        audio.current = { el, url };
        el.ontimeupdate = () => run.current === mine && setPos(from + el.currentTime * 1000);
        const ended = new Promise((done) => (el.onended = done));
        await el.play();
        await ended;
        if (run.current !== mine) return;
        release();
      }
      setPos(duration);
    } catch {
      /* audio not saved, or the meeting was deleted */
    }
    if (run.current === mine) setPlaying(false);
  };
  const setRate = (r: number) => {
    rateRef.current = r;
    setRateState(r);
    if (audio.current) audio.current.el.playbackRate = r;
  };
  return { playing, pos, rate, playFrom, pause, setRate };
}

const SPEEDS = [1, 1.25, 1.5, 2];

/** Play/pause, a seek bar and the speed, under the transcript. */
function PlayerBar({ player, duration }: { player: ReturnType<typeof usePlayer>; duration: number }) {
  return (
    <div className="flex items-center gap-3 border-t border-sv-border px-6 py-2.5">
      <button
        onClick={() => (player.playing ? player.pause() : player.playFrom(player.pos >= duration ? 0 : player.pos))}
        aria-label={player.playing ? "Pause" : "Play the meeting"}
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-sv-accent text-white transition hover:brightness-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface"
      >
        <svg viewBox="0 0 12 12" className="h-3 w-3" fill="currentColor" aria-hidden>
          {player.playing ? <path d="M2.5 1.5h2.5v9H2.5zM7 1.5h2.5v9H7z" /> : <path d="M3 1.5v9l7.5-4.5z" />}
        </svg>
      </button>
      <span className="w-12 shrink-0 text-right text-[11px] tabular-nums text-sv-muted">{clock(player.pos)}</span>
      <input
        type="range"
        min={0}
        max={duration}
        step={1000}
        value={Math.min(player.pos, duration)}
        onChange={(e) => player.playFrom(Number(e.target.value))}
        aria-label="Position in the meeting"
        aria-valuetext={clock(player.pos)}
        className="min-w-0 flex-1 accent-sv-accent"
      />
      <span className="w-12 shrink-0 text-[11px] tabular-nums text-sv-muted">{clock(duration)}</span>
      <select
        value={player.rate}
        onChange={(e) => player.setRate(Number(e.target.value))}
        aria-label="Playback speed"
        className="h-7 shrink-0 rounded-md border border-sv-border bg-sv-bg px-1.5 text-xs text-sv-muted outline-none focus:border-sv-accent"
      >
        {SPEEDS.map((r) => (
          <option key={r} value={r}>
            {r}×
          </option>
        ))}
      </select>
    </div>
  );
}

/** "People besides you": fixes how many Others the speaker labels use, when the
 *  model's own guess merged or split voices. Relabels in ~20 s, transcribes nothing. */
function PeopleCount({ t }: { t: Transcript }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  // New labels arrive as a transcript update.
  useEffect(() => setBusy(false), [t.lines]);
  // None comes if the speaker step failed (it logs why): don't spin forever.
  useEffect(() => {
    if (!busy) return;
    const timer = setTimeout(() => setBusy(false), 120_000);
    return () => clearTimeout(timer);
  }, [busy]);
  return (
    <>
      <span className="inline-flex items-center gap-1.5">
      <label className="inline-flex items-center gap-1.5">
        <PeopleIcon />
        <span className="sr-only">People besides you</span>
        <select
          value={t.others ?? ""}
          disabled={busy}
          onChange={(e) => {
            setBusy(true);
            setErr(null);
            meetingSetPeople(t.id, e.target.value ? Number(e.target.value) : null).catch((x) => {
              setBusy(false);
              setErr(String(x));
            });
          }}
          className="h-6 rounded-md border border-sv-border bg-sv-bg px-1 text-xs text-sv-text outline-none focus:border-sv-accent disabled:opacity-60"
        >
          <option value="">Others: auto</option>
          {[1, 2, 3, 4, 5, 6].map((n) => (
            <option key={n} value={n}>
              {n === 1 ? "1 other person" : `${n} other people`}
            </option>
          ))}
        </select>
      </label>
      <Info label="people count" text="How many voices to tell apart on the Others side. Auto lets the model guess; pick the number if you know it." />
      </span>
      {busy && <span role="status">Telling voices apart…</span>}
      {err && <span className="text-sv-warn">{err}</span>}
    </>
  );
}

function CalendarIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-3.5 w-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden>
      <rect x="4" y="5.5" width="16" height="14" rx="2.5" />
      <path d="M4 10h16M8.5 3.5v4M15.5 3.5v4" />
    </svg>
  );
}

function ClockIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-3.5 w-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </svg>
  );
}

function PeopleIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-3.5 w-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden>
      <circle cx="9" cy="8.5" r="3.5" />
      <path d="M3 19.5a6 6 0 0 1 12 0M16 5.2a3.5 3.5 0 0 1 0 6.6M18 14a6 6 0 0 1 3 5.5" />
    </svg>
  );
}

function RedoIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3" />
      <path d="M19.5 4.5v4.2h-4.2" />
    </svg>
  );
}
