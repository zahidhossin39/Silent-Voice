import { useMemo, useState } from "react";
import Page from "../shared/Page";
import ModelCard from "../shared/ModelCard";
import ProviderLogo from "../shared/ProviderLogo";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { STT_MODELS, LLM_MODELS, sttLanguage } from "../../services/catalog";
import { llmCompatibility, sttCompatibility } from "../../services/recommend";
import { formatMB, formatGB } from "../../services/format";
import { useModelStore } from "../../stores/modelStore";
import { useSettingsStore } from "../../stores/settingsStore";
import type {
  SttPreset,
  LlmModel,
  HardwareInfo,
  CompatibilityLevel,
} from "../../types";

type Tab = "stt" | "llm";

const CATEGORIES: { id: SttPreset | "all"; label: string }[] = [
  { id: "all", label: "All categories" },
  { id: "speed", label: "Speed" },
  { id: "balanced", label: "Balanced" },
  { id: "accuracy", label: "Accuracy" },
  { id: "multilingual", label: "Multilingual" },
];

// Lower = shown first. Recommended leads, then compatible, then heavy.
const LEVEL_RANK: Record<CompatibilityLevel, number> = {
  good: 0,
  warn: 1,
  bad: 2,
};

const DOT: Record<CompatibilityLevel, string> = {
  good: "bg-sv-good",
  warn: "bg-sv-warn",
  bad: "bg-sv-bad",
};

export default function ModelStore() {
  const { hardware } = useHardwareInfo();
  const [tab, setTab] = useState<Tab>("stt");
  const [category, setCategory] = useState<SttPreset | "all">("all");
  const [language, setLanguage] = useState<string>("all");
  const downloadedStt = useModelStore((s) => s.downloaded);
  const downloadedLlm = useModelStore((s) => s.downloadedLlm);

  const activeStt = useSettingsStore((s) => s.settings.active_stt_model);
  const usingCloudStt = useSettingsStore((s) => s.settings.stt_cloud_provider_id);
  const setSettings = useSettingsStore((s) => s.setSettings);

  // Selecting a local model also switches the STT source back to local so it
  // actually takes effect (a cloud provider would otherwise override it).
  const selectStt = (id: string) =>
    setSettings({ active_stt_model: id, stt_cloud_provider_id: null });

  // All languages present in the catalog, for the language dropdown.
  const languages = useMemo(() => {
    const set = new Set(STT_MODELS.map(sttLanguage));
    return ["all", ...Array.from(set).sort()];
  }, []);

  const sttModels = useMemo(() => {
    let list = STT_MODELS.slice();
    if (category !== "all") list = list.filter((m) => m.preset === category);
    if (language !== "all")
      list = list.filter((m) => sttLanguage(m) === language);
    return list.sort((a, b) => {
      const aDown = downloadedStt.has(a.id) ? 0 : 1;
      const bDown = downloadedStt.has(b.id) ? 0 : 1;
      if (aDown !== bDown) return aDown - bDown;
      const aRank = LEVEL_RANK[sttCompatibility(a, hardware).level];
      const bRank = LEVEL_RANK[sttCompatibility(b, hardware).level];
      if (aRank !== bRank) return aRank - bRank;
      return a.size_mb - b.size_mb;
    });
  }, [category, language, downloadedStt, hardware]);

  const sortedLlm = useMemo(
    () =>
      [...LLM_MODELS].sort((a, b) => {
        const aDown = downloadedLlm.has(a.id) ? 0 : 1;
        const bDown = downloadedLlm.has(b.id) ? 0 : 1;
        if (aDown !== bDown) return aDown - bDown;
        const aRank = LEVEL_RANK[llmCompatibility(a, hardware).level];
        const bRank = LEVEL_RANK[llmCompatibility(b, hardware).level];
        if (aRank !== bRank) return aRank - bRank;
        return a.size_mb - b.size_mb;
      }),
    [downloadedLlm, hardware]
  );

  return (
    <Page
      title="Model Store"
      subtitle="Pick a model to dictate with. Colored dots show what fits your device."
    >
      {/* Tab switch */}
      <div className="mb-4 inline-flex rounded-lg border border-sv-border bg-sv-surface p-1 text-sm">
        <TabButton active={tab === "stt"} onClick={() => setTab("stt")}>
          Speech-to-Text
        </TabButton>
        <TabButton active={tab === "llm"} onClick={() => setTab("llm")}>
          AI Processing (LLM)
        </TabButton>
      </div>

      {/* Legend */}
      <div className="mb-4 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[11px] text-sv-muted">
        <LegendDot level="good" label="Recommended" />
        <LegendDot level="warn" label="Works, may be slow" />
        <LegendDot level="bad" label="Heavy for your device" />
      </div>

      {tab === "stt" ? (
        <>
          <div className="mb-4 flex flex-wrap items-center gap-2">
            <Select value={category} onChange={(v) => setCategory(v as SttPreset | "all")}>
              {CATEGORIES.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label}
                </option>
              ))}
            </Select>
            <Select value={language} onChange={setLanguage}>
              {languages.map((l) => (
                <option key={l} value={l}>
                  {l === "all" ? "All languages" : l}
                </option>
              ))}
            </Select>
            <span className="ml-auto text-[11px] text-sv-muted">
              {sttModels.length} model{sttModels.length === 1 ? "" : "s"}
            </span>
          </div>

          <div className="grid grid-cols-1 items-start gap-2 lg:grid-cols-2">
            {sttModels.map((m) => (
              <ModelCard
                key={m.id}
                model={m}
                hardware={hardware}
                active={!usingCloudStt && activeStt === m.id}
                onSelect={() => selectStt(m.id)}
              />
            ))}
            {sttModels.length === 0 && (
              <p className="rounded-xl border border-dashed border-sv-border bg-sv-surface p-6 text-center text-sm text-sv-muted lg:col-span-2">
                No models match these filters.
              </p>
            )}
          </div>
        </>
      ) : (
        <>
          <p className="mb-4 text-xs text-sv-muted">
            These run <strong>inside Silent Voice</strong> and power your AI
            modes (Clean Up, Formal, Email…). Assign one to a mode in the Modes
            tab. You can also use a cloud provider instead (API Keys).
          </p>
          <div className="grid grid-cols-1 items-start gap-2 lg:grid-cols-2">
            {sortedLlm.map((m) => (
              <LlmCard key={m.id} model={m} hardware={hardware} />
            ))}
          </div>
        </>
      )}
    </Page>
  );
}

function LegendDot({
  level,
  label,
}: {
  level: CompatibilityLevel;
  label: string;
}) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className={`h-2 w-2 rounded-full ${DOT[level]}`} />
      {label}
    </span>
  );
}

function Select({
  value,
  onChange,
  children,
}: {
  value: string;
  onChange: (v: string) => void;
  children: React.ReactNode;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="rounded-lg border border-sv-border bg-sv-surface px-3 py-1.5 text-xs text-sv-text"
    >
      {children}
    </select>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-md px-4 py-1.5 transition ${
        active ? "bg-sv-accent text-white" : "text-sv-muted hover:text-sv-text"
      }`}
    >
      {children}
    </button>
  );
}

function LlmCard({
  model,
  hardware,
}: {
  model: LlmModel;
  hardware: HardwareInfo | null;
}) {
  const downloaded = useModelStore((s) => s.downloadedLlm.has(model.id));
  const progress = useModelStore((s) => s.progress[model.id]);
  const download = useModelStore((s) => s.downloadLlm);
  const remove = useModelStore((s) => s.removeLlm);
  const [open, setOpen] = useState(false);

  const level = llmCompatibility(model, hardware).level;
  const isDownloading = progress?.status === "downloading";
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  return (
    <div className="overflow-hidden rounded-xl border border-sv-border bg-sv-surface transition hover:border-sv-muted/40">
      <div className="flex items-center gap-3 px-3.5 py-2.5">
        <button
          onClick={() => setOpen((v) => !v)}
          className="flex min-w-0 flex-1 items-center gap-3 text-left"
        >
          <ProviderLogo provider={model.provider} size={30} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span
                className={`h-2 w-2 shrink-0 rounded-full ${DOT[level]}`}
                title={level}
              />
              <h3 className="truncate text-sm font-medium">{model.name}</h3>
            </div>
            <p className="mt-0.5 truncate text-[11px] text-sv-muted">
              {model.provider} · {model.params} · {formatMB(model.size_mb)}
            </p>
          </div>
          <Chevron open={open} />
        </button>
        <div className="flex shrink-0 items-center gap-2">
          {isDownloading ? (
            <div className="flex items-center gap-2">
              <div className="h-1.5 w-20 overflow-hidden rounded-full bg-sv-surface-2">
                <div
                  className="h-full bg-sv-accent transition-all"
                  style={{ width: `${pct}%` }}
                />
              </div>
              <span className="w-8 text-right text-[11px] text-sv-muted">
                {pct}%
              </span>
            </div>
          ) : downloaded ? (
            <>
              <span className="text-[11px] text-sv-good">Installed</span>
              <button
                onClick={() => remove(model.id)}
                title="Remove download"
                className="rounded-lg p-1.5 text-sv-muted transition hover:bg-sv-surface-2 hover:text-sv-bad"
              >
                <TrashIcon />
              </button>
            </>
          ) : (
            <button
              onClick={() => download(model.id)}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs font-medium text-sv-text hover:border-sv-accent hover:text-sv-accent"
            >
              Download
            </button>
          )}
        </div>
      </div>

      {open && (
        <div className="border-t border-sv-border/70 px-3.5 py-3">
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <LlmStat label="Size (params)" value={model.params} />
            <LlmStat label="Speed" value={model.speed_label.replace("~", "")} />
            <LlmStat label="Download" value={formatMB(model.size_mb)} />
            <LlmStat label="Memory use" value={formatGB(model.ram_gb)} />
          </div>
          <p className="mt-2.5 text-[11px] text-sv-muted">
            {model.best_for} · {model.languages} · {model.license}
          </p>
        </div>
      )}

      {progress?.status === "error" && (
        <p className="px-3.5 pb-2 text-[11px] text-sv-bad">{progress.error}</p>
      )}
    </div>
  );
}

function LlmStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-sv-surface-2 px-2.5 py-1.5">
      <div className="text-[10px] uppercase tracking-wide text-sv-muted">
        {label}
      </div>
      <div className="text-xs font-medium">{value}</div>
    </div>
  );
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={`shrink-0 text-sv-muted transition-transform ${
        open ? "rotate-180" : ""
      }`}
    >
      <path d="M6 9l6 6 6-6" />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="15"
      height="15"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-12" />
    </svg>
  );
}
