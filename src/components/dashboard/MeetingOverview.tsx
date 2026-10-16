import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { LLM_MODELS } from "../../services/catalog";
import { listDownloadedLlm, meetingSummarize, type SummaryWith, type Transcript } from "../../services/tauriBridge";

// The Overview of a meeting: an AI-written summary, key points, decisions and
// action items (meeting_summary.rs), with the model the user picks here.

const WITH_KEY = "sv-notetaker-summary-with";

interface Choice extends SummaryWith {
  key: string;
  label: string;
}

/** Cloud providers set up for AI, then the local models on this PC. */
function useChoices(): Choice[] {
  const providers = useSettingsStore((s) => s.providers);
  const [local, setLocal] = useState<string[]>([]);
  useEffect(() => {
    listDownloadedLlm().then(setLocal);
  }, []);
  return useMemo(
    () => [
      ...providers
        .filter((p) => p.uses.includes("llm") && p.base_url && p.model)
        .map((p) => ({
          key: `api:${p.id}`,
          label: `${p.name} · ${p.model}`,
          source: "api" as const,
          model: p.model,
          base_url: p.base_url,
          api_key: p.api_key,
        })),
      ...local.map((id) => ({
        key: `local:${id}`,
        label: `${LLM_MODELS.find((m) => m.id === id)?.name ?? id} (on this PC)`,
        source: "local" as const,
        model: id,
        base_url: "",
        api_key: "",
      })),
    ],
    [providers, local]
  );
}

function readKey(): string {
  try {
    return localStorage.getItem(WITH_KEY) ?? "";
  } catch {
    return "";
  }
}

export default function MeetingOverview({ t, onSummarized }: { t: Transcript; onSummarized: (t: Transcript) => void }) {
  const choices = useChoices();
  const [key, setKey] = useState(readKey);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const live = t.status === "running";
  const chosen = choices.find((c) => c.key === key) ?? choices[0];

  const pick = (k: string) => {
    setKey(k);
    try {
      localStorage.setItem(WITH_KEY, k);
    } catch {
      /* remembered for this visit only */
    }
  };

  const run = async () => {
    if (!chosen) return;
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

  return (
    <section aria-labelledby="overview" className="mb-6" aria-busy={busy}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 id="overview" className="text-xs font-semibold text-sv-muted">
          Overview
        </h3>
        {!live && hasText && choices.length > 0 && (
          <div className="flex items-center gap-1.5">
            <label className="sr-only" htmlFor="summary-with">
              Summarize with
            </label>
            <select
              id="summary-with"
              value={chosen?.key}
              disabled={busy}
              onChange={(e) => pick(e.target.value)}
              className="max-w-[16rem] truncate rounded-lg border border-sv-border bg-sv-bg px-2 py-1 text-xs text-sv-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
            >
              {choices.map((c) => (
                <option key={c.key} value={c.key}>
                  {c.label}
                </option>
              ))}
            </select>
            <button
              onClick={run}
              disabled={busy}
              className={`rounded-lg px-2.5 py-1 text-xs font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent disabled:cursor-not-allowed disabled:opacity-50 ${
                t.summary
                  ? "border border-sv-border text-sv-muted hover:border-sv-accent/60 hover:text-sv-text"
                  : "bg-sv-accent text-white hover:brightness-110"
              }`}
            >
              {busy ? "Writing…" : t.summary ? "Redo" : "Summarize"}
            </button>
          </div>
        )}
      </div>

      {error && (
        <p role="alert" className="mt-2 rounded-lg bg-sv-bad/10 px-3 py-2 text-xs text-sv-bad">
          {error}
        </p>
      )}

      {busy && !t.summary ? (
        <p className="mt-1.5 text-sm text-sv-muted">Writing the summary. A long meeting can take a minute or two.</p>
      ) : t.summary ? (
        <div className={`mt-2 max-w-[72ch] transition-opacity ${busy ? "opacity-50" : ""}`}>{renderNotes(t.summary)}</div>
      ) : (
        <p className="mt-1.5 text-sm text-sv-muted">
          {live
            ? "The transcript fills in as people talk. Summarize it when the meeting ends."
            : !hasText
              ? "There is nothing to summarize."
              : choices.length === 0
                ? "To summarize meetings, add an AI provider in Settings or download a local model."
                : "Summarize to get the main points, decisions and action items."}
        </p>
      )}
    </section>
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
