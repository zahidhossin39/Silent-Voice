import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import ProviderLogo from "../shared/ProviderLogo";
import { Icon, ModelPicker, isChatModel, showModel, useProviders } from "./providers/shared";
import { useSettingsStore } from "../../stores/settingsStore";
import { LLM_MODELS } from "../../services/catalog";
import { listDownloadedLlm, meetingSummarize, type SummaryWith, type Transcript } from "../../services/tauriBridge";

// The Overview of a meeting: an AI-written summary, key points, decisions and
// action items (meeting_summary.rs), with the model the user picks here.

const WITH_KEY = "sv-notetaker-summary-with";
/** Per provider, the model picked for summaries: the provider's default may be a transcription model. */
const MODELS_KEY = "sv-notetaker-summary-models";

export interface Choice extends SummaryWith {
  key: string;
  /** Provider id, or the local model id. */
  id: string;
  /** Provider name, or the local model's name. */
  label: string;
  /** Short model id ("gemini-2.5-flash-lite"), empty for local. */
  detail: string;
}

/** Cloud providers set up for AI, then the local models on this PC. */
function useChoices(models: Record<string, string>): Choice[] {
  const providers = useSettingsStore((s) => s.providers);
  const [local, setLocal] = useState<string[]>([]);
  useEffect(() => {
    listDownloadedLlm().then(setLocal);
  }, []);
  return useMemo(
    () => [
      ...providers
        .filter((p) => p.uses.includes("llm") && p.base_url)
        .map((p) => {
          const model = models[p.id] || (isChatModel(p.model) ? p.model : "");
          return {
          key: `api:${p.id}`,
          id: p.id,
          label: p.name,
          detail: model ? (showModel(model).split("/").pop() ?? model) : "Choose a model",
          source: "api" as const,
          model,
          base_url: p.base_url,
          api_key: p.api_key,
          };
        }),
      ...local.map((id) => ({
        key: `local:${id}`,
        id,
        label: LLM_MODELS.find((m) => m.id === id)?.name ?? id,
        detail: "",
        source: "local" as const,
        model: id,
        base_url: "",
        api_key: "",
      })),
    ],
    [providers, local, models]
  );
}

function read(key: string): string {
  try {
    return localStorage.getItem(key) ?? "";
  } catch {
    return "";
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* remembered for this visit only */
  }
}

/** The summary model the user picked in Notetaker settings, shared by the settings panel and the Overview. */
export function useSummaryChoice() {
  const [models, setModels] = useState<Record<string, string>>(() => {
    try {
      return JSON.parse(read(MODELS_KEY) || "{}");
    } catch {
      return {};
    }
  });
  const choices = useChoices(models);
  const providerApi = useProviders();
  const [key, setKey] = useState(() => read(WITH_KEY));
  const chosen: Choice | undefined = choices.find((c) => c.key === key) ?? choices[0];

  const pick = (k: string) => {
    setKey(k);
    write(WITH_KEY, k);
  };
  const pickModel = (providerId: string, model: string) => {
    const next = { ...models, [providerId]: model };
    setModels(next);
    write(MODELS_KEY, JSON.stringify(next));
  };
  const provider = chosen?.source === "api" ? providerApi.providers.find((p) => p.id === chosen.id) : undefined;
  return { choices, chosen, pick, pickModel, provider, providerApi };
}

export type SummaryChoice = ReturnType<typeof useSummaryChoice>;

/** The summary model fields in Notetaker settings. */
export function SummarySettings({ s }: { s: SummaryChoice }) {
  if (!s.chosen) return <p className="text-xs text-sv-muted">Add a cloud provider in Cloud providers, or download a model in the Model Store.</p>;
  return (
    <div className="flex flex-col gap-2">
      <Picker choices={s.choices} chosen={s.chosen} disabled={false} onPick={s.pick} action="Summarize with" />
      {s.provider && (
        <ModelPicker p={s.provider} api={s.providerApi} value={s.chosen.model} onPick={(m) => s.pickModel(s.provider!.id, m)} />
      )}
    </div>
  );
}

export default function MeetingOverview({
  t,
  onSummarized,
  chosen,
  onSettings,
}: {
  t: Transcript;
  onSummarized: (t: Transcript) => void;
  chosen?: Choice;
  onSettings: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const live = t.status === "running";

  const run = async () => {
    if (!chosen?.model) return onSettings();
    setBusy(true);
    setError(null);
    try {
      onSummarized(await meetingSummarize(t.id, chosen));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const hasText = t.lines.length > 0;

  const ready = !!chosen?.model;
  const controls = !live && hasText && (
    <div className="flex flex-wrap items-center gap-2">
      {!ready ? (
        <button
          onClick={onSettings}
          className="inline-flex h-10 items-center gap-1.5 rounded-lg bg-sv-accent px-3 text-sm font-medium text-white transition hover:brightness-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
        >
          <GearIcon className="h-4 w-4" />
          Choose a summary model
        </button>
      ) : (
      <button
        onClick={run}
        disabled={busy}
        className={`inline-flex h-10 shrink-0 items-center gap-1.5 rounded-lg px-3 text-sm font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent disabled:cursor-not-allowed disabled:opacity-60 ${
          t.summary
            ? "border border-sv-border text-sv-text hover:bg-sv-surface-2"
            : "bg-sv-accent text-white hover:brightness-110"
        }`}
      >
        {busy ? <Spinner /> : t.summary ? <Icon.refresh className="h-4 w-4" /> : <Icon.bolt className="h-4 w-4" />}
        {busy ? "Writing" : t.summary ? "Redo" : "Summarize"}
      </button>
      )}
    </div>
  );

  const [head, sub] = live
    ? ["The transcript fills in as people talk.", "Summarize it when the meeting ends."]
    : !hasText
      ? ["There is nothing to summarize.", "This meeting has no transcript."]
      : !ready
        ? ["Get the main points, decisions and action items.", "First choose the model that writes summaries."]
        : busy
          ? ["Writing the summary…", "A long meeting can take a few minutes, longer with a model on this PC."]
          : ["Get the main points, decisions and action items.", "Written with the summary model from Notetaker settings."];

  return (
    <section aria-labelledby="overview" className="mb-6" aria-busy={busy}>
      <h3 id="overview" className="text-xs font-semibold text-sv-muted">
        Overview
      </h3>

      {t.summary ? (
        <>
          <div className={`mt-2 max-w-[72ch] transition-opacity ${busy ? "opacity-50" : ""}`}>{renderNotes(t.summary)}</div>
          {controls && <div className="mt-4 border-t border-sv-border pt-3">{controls}</div>}
        </>
      ) : (
        <div className="mt-2 rounded-xl border border-sv-border bg-sv-surface-2/40 p-4">
          <p className="text-sm text-sv-text">{head}</p>
          <p className="mt-1 text-xs text-sv-muted">{sub}</p>
          {controls && <div className="mt-3">{controls}</div>}
        </div>
      )}

      {error && (
        <p role="alert" className="mt-3 rounded-lg bg-sv-bad/10 px-3 py-2 text-xs text-sv-bad">
          {error}
        </p>
      )}
    </section>
  );
}

export function GearIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" className={className} aria-hidden>
      <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" />
    </svg>
  );
}

function Spinner() {
  return <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" aria-hidden />;
}

/** Model picker: cloud providers, then models on this PC, with logos. */
/** A provider/local-model dropdown with logos, grouped Cloud / On this PC. */
export function Picker({
  choices,
  chosen,
  disabled,
  onPick,
  action,
  localTitle = "On this PC · offline, slower",
}: {
  choices: Choice[];
  chosen: Choice;
  disabled: boolean;
  onPick: (key: string) => void;
  /** "Summarize with", for the button's accessible name. */
  action: string;
  localTitle?: string;
}) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    list.current?.querySelector<HTMLElement>("[aria-selected=true]")?.focus();
    const close = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const move = (e: KeyboardEvent) => {
    const items = [...(list.current?.querySelectorAll<HTMLElement>("[role=option]") ?? [])];
    const i = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") items[Math.min(i + 1, items.length - 1)]?.focus();
    else if (e.key === "ArrowUp") items[Math.max(i - 1, 0)]?.focus();
    else if (e.key === "Escape" || e.key === "Tab") setOpen(false);
    else return;
    e.preventDefault();
  };

  const groups = [
    { title: "Cloud", items: choices.filter((c) => c.source === "api") },
    { title: localTitle, items: choices.filter((c) => c.source === "local") },
  ].filter((g) => g.items.length);

  return (
    <div ref={box} className="relative min-w-[11rem] flex-1">
      <button
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`${action} ${chosen.label}`}
        onClick={() => setOpen((o) => !o)}
        className="flex h-10 w-full min-w-0 items-center gap-2 rounded-lg border border-sv-border bg-sv-bg px-2 text-left transition hover:border-sv-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent disabled:opacity-60"
      >
        <Logo c={chosen} size={22} />
        <span className="min-w-0 flex-1 truncate text-sm">
          <span className="text-sv-text">{chosen.label}</span>
          {chosen.source === "local" && chosen.detail && <span className="text-sv-muted"> · {chosen.detail}</span>}
        </span>
        <Icon.chevron className={`h-4 w-4 shrink-0 text-sv-muted transition ${open ? "rotate-180" : ""}`} />
      </button>
      {open && (
        <div
          ref={list}
          role="listbox"
          onKeyDown={move}
          className="absolute left-0 top-[calc(100%+6px)] z-30 w-full min-w-[16rem] overflow-hidden rounded-xl border border-sv-border bg-sv-surface py-1 shadow-[0_12px_32px_-8px_rgb(0_0_0/0.45)]"
        >
          {groups.map((g) => (
            <div key={g.title} role="group" aria-label={g.title}>
              <div className="px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-wide text-sv-muted">{g.title}</div>
              {g.items.map((c) => (
                <button
                  key={c.key}
                  role="option"
                  aria-selected={c.key === chosen.key}
                  onClick={() => {
                    onPick(c.key);
                    setOpen(false);
                  }}
                  className="flex w-full items-center gap-2.5 px-3 py-2 text-left hover:bg-sv-surface-2 focus:bg-sv-surface-2 focus:outline-none"
                >
                  <Logo c={c} size={26} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-sv-text">{c.label}</span>
                    {c.detail && <span className="block truncate text-xs text-sv-muted">{c.detail}</span>}
                  </span>
                  {c.key === chosen.key && <Icon.check className="h-4 w-4 shrink-0 text-sv-accent" />}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Logo({ c, size }: { c: Choice; size: number }) {
  if (c.source === "api") return <ProviderLogo provider={c.label} size={size} />;
  return (
    <span
      className="flex shrink-0 items-center justify-center rounded-md bg-sv-surface-2 text-sv-muted"
      style={{ width: size, height: size }}
      aria-hidden
    >
      <svg viewBox="0 0 24 24" width={size * 0.6} height={size * 0.6} fill="none" stroke="currentColor" strokeWidth={1.8}>
        <rect x="6" y="6" width="12" height="12" rx="2" />
        <path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4" />
      </svg>
    </span>
  );
}

/** The few Markdown shapes the prompt asks for: ## headings, bullets, text. */
function renderNotes(md: string): ReactNode[] {
  const out: ReactNode[] = [];
  let bullets: string[] = [];
  const flush = () => {
    if (bullets.length) {
      out.push(
        <ul key={out.length} className="mt-1 list-disc space-y-1 pl-5 text-[15px] leading-relaxed text-sv-text">
          {bullets.map((b, i) => (
            <li key={i}>{b}</li>
          ))}
        </ul>
      );
      bullets = [];
    }
  };
  for (const raw of md.split("\n")) {
    const line = raw.trim().replace(/\*\*(.+?)\*\*/g, "$1");
    if (!line) continue;
    const bullet = line.match(/^[-*•]\s+(.*)$/) ?? line.match(/^\d+[.)]\s+(.*)$/);
    if (bullet) {
      bullets.push(bullet[1]);
      continue;
    }
    flush();
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    out.push(
      heading ? (
        <h4 key={out.length} className="mt-4 text-sm font-semibold text-sv-text first:mt-0">
          {heading[1]}
        </h4>
      ) : (
        <p key={out.length} className="mt-1 text-[15px] leading-relaxed text-sv-text">
          {line}
        </p>
      )
    );
  }
  flush();
  return out;
}
