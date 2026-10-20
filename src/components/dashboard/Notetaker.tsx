import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import Page from "../shared/Page";
import Select from "../shared/Select";
import { useNotetakerSettings, useSettingsStore } from "../../stores/settingsStore";
import { DownloadButton, RemoveButton } from "./settings/downloads";
import { Toggle } from "./settings/parts";
import { LANGUAGES, STT_MODELS, honorsLanguage } from "../../services/catalog";
import TranscriptHeader, { clock } from "../shared/TranscriptHeader";
import { TrashIcon } from "../shared/TranscriptCard";
import MeetingOverview, { GearIcon, Picker, SummarySettings, useSummaryChoice, type Choice, type SummaryChoice } from "./MeetingOverview";
import { ModelPicker } from "./providers/shared";
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

  const sttProviders = useSettingsStore((s) => s.providers).filter((p) => p.uses.includes("stt") && p.base_url);
  const pickSttModel = (providerId: string, model: string) => setNt({ stt_models: { ...nt.stt_models, [providerId]: model } });
  const sttModelOf = (p: { id: string; stt_model: string }) => nt.stt_models[p.id] || p.stt_model;
  const sttKey = nt.stt;
  const cloudProvider = sttProviders.find((p) => p.id === sttKey);
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
      const t = await meetingStart(cloudArg, lang || undefined, nt.speakers);
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
        <h3 className="text-sm font-semibold">Language spoken</h3>
        <p className="mb-2 mt-0.5 text-xs text-sv-muted">What people speak in your meetings.</p>
        {language}
        <p className="mt-1.5 text-xs text-sv-muted">{languageNote}</p>
      </section>
      <section className="border-t border-sv-border pt-4">
        <h3 className="text-sm font-semibold">Transcription</h3>
        <p className="mb-2 mt-0.5 text-xs text-sv-muted">Turns the meeting's audio into text as people talk.</p>
        {stt}
        <p className="mt-1.5 text-xs text-sv-muted">{sttNote}</p>
      </section>
      <SpeakersSetting />
      <section className="border-t border-sv-border pt-4">
        <h3 className="text-sm font-semibold">Summary</h3>
        <p className="mb-2 mt-0.5 text-xs text-sv-muted">Writes the Overview: key points, decisions and action items.</p>
        <SummarySettings s={summary} />
      </section>
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
    <section className="border-t border-sv-border pt-4">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold">Tell speakers apart</h3>
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
      </div>
      <p className="mt-0.5 text-xs text-sv-muted">
        After you stop, Others is split into Speaker 1, Speaker 2… by voice, on this PC. Takes about 15 s per minute of meeting; the text shows
        meanwhile.
      </p>
      {error && <p className="mt-1 text-xs text-sv-warn">Download failed: {error}</p>}
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
        <MeetingOverview t={t} onSummarized={onSummarized} chosen={summaryWith} onSettings={onSettings} />

        {paras.length === 0 && !live ? (
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
      {!live && paras.some((p) => p.speaker) && duration > 0 && <PlayerBar player={player} duration={duration} />}
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
