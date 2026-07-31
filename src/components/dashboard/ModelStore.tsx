import { useMemo, useState, useEffect } from "react";
import Page from "../shared/Page";
import {
  STT_MODELS,
  TTS_MODELS,
  sttLanguage,
} from "../../services/catalog";
import { formatMB } from "../../services/format";
import { useModelStore } from "../../stores/modelStore";
import { useSettingsStore } from "../../stores/settingsStore";
import type {
  SttPreset,
  TtsModel,
  CompatibilityLevel,
  PiperVoice,
} from "../../types";
import { hfPiperVoices, ttsSpeakText } from "../../services/tauriBridge";
import HfBrowser, { MetricBar } from "./hf/HfBrowser";
import ProviderLogo from "../shared/ProviderLogo";
import { ttsNaturalnessScore, ttsNaturalnessLabel, ttsSpeedScore, ttsSpeedLabel } from "../../services/modelMetrics";

type Tab = "stt" | "llm" | "tts";

const CATEGORIES: { id: SttPreset | "all"; label: string }[] = [
  { id: "all", label: "All categories" },
  { id: "speed", label: "Speed" },
  { id: "balanced", label: "Balanced" },
  { id: "accuracy", label: "Accuracy" },
  { id: "multilingual", label: "Multilingual" },
];


const DOT: Record<CompatibilityLevel, string> = {
  good: "bg-sv-good",
  warn: "bg-sv-warn",
  bad: "bg-sv-bad",
};

export default function ModelStore() {
  const [tab, setTab] = useState<Tab>("stt");
  const [category, setCategory] = useState<SttPreset | "all">("all");
  const [language, setLanguage] = useState<string>("all");
  const downloadedTts = useModelStore((s) => s.downloadedTts);

  const activeTts = useSettingsStore((s) => s.settings.active_tts_voice);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const pinnedTtsArr = useSettingsStore((s) => s.settings.pinned_tts);
  const togglePinnedTts = useSettingsStore((s) => s.togglePinnedTts);

  const pinnedTts = useMemo(() => new Set(pinnedTtsArr || []), [pinnedTtsArr]);

  // All languages present in the catalog, for the language dropdown.
  const languages = useMemo(() => {
    const set = new Set(STT_MODELS.map(sttLanguage));
    return ["all", ...Array.from(set).sort()];
  }, []);


  // TTS search + language filter.
  const [ttsSearch, setTtsSearch] = useState("");
  const [ttsLanguage, setTtsLanguage] = useState<string>("all");
  const [piperVoices, setPiperVoices] = useState<PiperVoice[]>([]);

  useEffect(() => {
    let active = true;
    hfPiperVoices().then(res => {
      if (active) setPiperVoices(res);
    }).catch(e => console.error("Piper voices error:", e));
    return () => { active = false; };
  }, []);

  const allTtsModels = useMemo(() => {
    const fromPiper = piperVoices.map((v) => {
      const urlBase = `https://huggingface.co/rhasspy/piper-voices/resolve/main/${v.onnx_path.replace('.onnx', '')}`;
      // Piper qualities are x_low/low/medium/high — map onto the app's tiers.
      const quality =
        v.quality === "high" ? ("natural" as const)
        : v.quality === "medium" ? ("balanced" as const)
        : ("fast" as const);
      return {
        id: v.key,
        label: `${v.name} (${v.language_english}, ${v.country_english})`,
        gender: "unknown" as const,
        accent: "US" as const,
        language: v.language_english,
        quality,
        size_mb: Math.round(v.onnx_size_bytes / (1024 * 1024)),
        engine: "piper" as const,
        url_onnx: `${urlBase}.onnx?download=true`,
        url_json: `${urlBase}.onnx.json?download=true`
      };
    });
    const merged = [...TTS_MODELS];
    for (const pv of fromPiper) {
      if (!merged.some(m => m.id === pv.id)) {
        merged.push(pv);
      }
    }
    return merged;
  }, [piperVoices]);

  const ttsLanguages = useMemo(() => {
    const set = new Set(allTtsModels.map((v) => v.language));
    return ["all", ...Array.from(set).sort()];
  }, [allTtsModels]);

  // Voices: ACTIVE voice first, then downloaded, then fast → natural (fast
  // tiers suit CPU-only machines).
  const QUALITY_RANK = { fast: 0, balanced: 1, natural: 2 } as const;
  const sortedTts = useMemo(() => {
    const q = ttsSearch.trim().toLowerCase();
    let list = allTtsModels.filter(
      (v) =>
        (ttsLanguage === "all" || v.language === ttsLanguage) &&
        (!q ||
          v.label.toLowerCase().includes(q) ||
          v.language.toLowerCase().includes(q) ||
          v.id.toLowerCase().includes(q))
    );
    return list.sort((a, b) => {
      const aActive = a.id === activeTts ? 0 : 1;
      const bActive = b.id === activeTts ? 0 : 1;
      if (aActive !== bActive) return aActive - bActive;
      const aPinned = pinnedTts.has(a.id) ? 0 : 1;
      const bPinned = pinnedTts.has(b.id) ? 0 : 1;
      if (aPinned !== bPinned) return aPinned - bPinned;
      const aDown = downloadedTts.has(a.id) ? 0 : 1;
      const bDown = downloadedTts.has(b.id) ? 0 : 1;
      if (aDown !== bDown) return aDown - bDown;
      return QUALITY_RANK[a.quality] - QUALITY_RANK[b.quality];
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [downloadedTts, activeTts, ttsSearch, ttsLanguage, pinnedTts, allTtsModels]);

  return (
    <Page
      title="Model Store"
      subtitle="Pick what listens, speaks, and rewrites. Coloured dots show what fits your device."
    >
      {/* Tab switch */}
      <div className="mb-4 inline-flex flex-wrap gap-1 rounded-lg border border-sv-border bg-sv-surface p-1 text-sm">
        <TabButton active={tab === "stt"} onClick={() => setTab("stt")}>
          <div className="flex items-center gap-2">
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/></svg>
            Speech-to-Text (Mic)
          </div>
        </TabButton>
        <TabButton active={tab === "tts"} onClick={() => setTab("tts")}>
          <div className="flex items-center gap-2">
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13"/></svg>
            Text-to-Speech (Speaker)
          </div>
        </TabButton>
        <TabButton active={tab === "llm"} onClick={() => setTab("llm")}>
          <div className="flex items-center gap-2">
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round"><rect width="16" height="16" x="4" y="4" rx="2"/><rect width="6" height="6" x="9" y="9" rx="1"/><path d="M15 2v2"/><path d="M15 20v2"/><path d="M2 15h2"/><path d="M2 9h2"/><path d="M20 15h2"/><path d="M20 9h2"/><path d="M9 2v2"/><path d="M9 20v2"/></svg>
            AI Processing (LLM)
          </div>
        </TabButton>
      </div>

      {/* Legend (compatibility dots apply to STT/LLM; voices all run on CPU) */}
      {tab !== "tts" && (
        <div className="mb-4 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[11px] text-sv-muted">
          <LegendDot level="good" label="Recommended" />
          <LegendDot level="warn" label="Works, may be slow" />
          <LegendDot level="bad" label="Heavy for your device" />
        </div>
      )}

      {tab === "tts" ? (
        <>
          <p className="mb-4 text-xs text-sv-muted">
            Voices for <strong>read-aloud</strong>: select text in any app and
            press the read-aloud hotkey (Settings → Read aloud) to hear it.
            All voices run on your CPU — "fast" tiers respond quickest;
            "natural" tiers sound best but take a moment longer.{" "}
            <strong>Tip:</strong> a voice can only pronounce its own language —
            pick an English voice for English text, a Bangla voice for Bangla.
          </p>
          <div className="mb-4 flex flex-wrap items-center gap-2">
            <input
              type="text"
              value={ttsSearch}
              onChange={(e) => setTtsSearch(e.target.value)}
              placeholder="Search voices…"
              className="w-52 rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm text-sv-text placeholder:text-sv-muted focus:border-sv-accent focus:outline-none"
            />
            <Select value={ttsLanguage} onChange={setTtsLanguage}>
              {ttsLanguages.map((l) => (
                <option key={l} value={l}>
                  {l === "all" ? "All languages" : l}
                </option>
              ))}
            </Select>
            {(ttsSearch || ttsLanguage !== "all") && (
              <span className="text-[11px] text-sv-muted">
                {sortedTts.length} voice{sortedTts.length === 1 ? "" : "s"}
              </span>
            )}
          </div>
          <div className="flex flex-col gap-2 max-w-[1180px]">
            {sortedTts.map((v) => (
              <TtsCard
                key={v.id}
                voice={v}
                active={Boolean(activeTts) && activeTts === v.id && downloadedTts.has(v.id)}
                onSelect={() => setSettings({ active_tts_voice: v.id })}
                pinned={pinnedTts.has(v.id)}
                onTogglePin={() => togglePinnedTts(v.id)}
              />
            ))}
          </div>
        </>
      ) : tab === "stt" ? (
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
          </div>

          <div className="flex-1 overflow-hidden">
            <HfBrowser track="stt" categoryFilter={category} languageFilter={language} />
          </div>
        </>
      ) : (
        <>
          <p className="mb-4 text-xs text-sv-muted">
            These run <strong>inside Silent Voice</strong> and power your AI
            modes (Clean Up, Formal, Email…). Assign one to a mode in the Modes
            tab. You can also use a cloud provider instead (Cloud providers).
          </p>
          <HfBrowser track="llm" />
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

const TTS_QUALITY_CHIP: Record<string, { label: string; cls: string }> = {
  fast: { label: "Fast", cls: "bg-sv-surface-2 text-sv-muted" },
  balanced: { label: "Balanced", cls: "bg-sv-surface-2 text-sv-muted" },
  natural: { label: "Natural", cls: "bg-sv-surface-2 text-sv-muted" },
};

function TtsCard({
  voice,
  active,
  onSelect,
  pinned,
  onTogglePin,
}: {
  voice: TtsModel;
  active: boolean;
  onSelect: () => void;
  pinned: boolean;
  onTogglePin: () => void;
}) {
  const downloaded = useModelStore((s) => s.downloadedTts.has(voice.id));
  const progress = useModelStore((s) => s.progress[voice.id]);
  const downloadCustomTts = useModelStore((s) => s.downloadCustomTts);
  const pauseStore = useModelStore((s) => s.pause);
  const cancelStore = useModelStore((s) => s.cancel);
  const remove = useModelStore((s) => s.removeTts);
  const [playing, setPlaying] = useState(false);

  const download = () => {
    downloadCustomTts(voice.id, voice.url_onnx, voice.url_json, voice.size_mb);
  };

  const handlePreview = async () => {
    setPlaying(true);
    try {
      await ttsSpeakText("This is a preview of the voice.");
    } finally {
      setPlaying(false);
    }
  };

  const isDownloading = progress?.status === "downloading";
  const isPaused = progress?.status === "paused";
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;
  const chip = TTS_QUALITY_CHIP[voice.quality];

  return (
    <div
      className={`group relative rounded-xl border ${
        active
          ? "border-sv-accent/40"
          : "border-sv-border"
      } bg-sv-surface transition-colors duration-75 hover:bg-sv-surface-2/40 flex flex-wrap items-center gap-x-5 gap-y-2 px-4 py-3`}
    >
      <div className="flex min-w-[230px] max-w-[420px] flex-1 items-center gap-3">
        <ProviderLogo provider={voice.engine} size={32} />
        <div className="min-w-0 flex-1 flex flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <span
              className="h-1.5 w-1.5 shrink-0 rounded-full bg-sv-good"
              title="Fits well"
            />
            <span className="truncate text-[13px] font-semibold text-sv-text">{voice.label}</span>
            <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium ${chip.cls}`}>
              {chip.label}
            </span>
            {active && <span className="shrink-0 rounded bg-sv-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-sv-accent">In use</span>}
            {pinned && <span className="shrink-0 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px] text-sv-muted">Pinned</span>}
          </div>
          <div className="truncate tabular-nums text-[11px] text-sv-muted">
            {voice.engine} · {voice.language} · {formatMB(voice.size_mb)}
          </div>
          {progress?.status === "error" && (
            <div className="truncate text-[11px] text-sv-bad">{progress.error}</div>
          )}
        </div>
      </div>

      <div className="flex w-[300px] shrink-0 flex-col gap-1">
        <MetricBar label="naturalness" value={ttsNaturalnessScore(voice.quality)} caption={ttsNaturalnessLabel(voice.quality)} />
        <MetricBar label="speed" value={ttsSpeedScore(voice.quality)} caption={ttsSpeedLabel(voice.quality)} />
      </div>

      <div className="ml-auto flex shrink-0 items-center gap-2">
        <div className="flex shrink-0 items-center">
          {downloaded && !active && (
            <button
              onClick={handlePreview}
              disabled={playing}
              className="mr-2 rounded-lg px-2.5 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:bg-sv-surface-2 disabled:opacity-50"
            >
              {playing ? "Playing…" : "Preview"}
            </button>
          )}

          {isDownloading || isPaused ? (
            <div className="flex items-center gap-1.5">
              <div className="flex w-28 items-center gap-2">
                {!progress || progress.total_bytes === 0 ? (
                  <>
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full border border-sv-border bg-sv-surface-2">
                      <div className="h-full w-1/3 rounded-full bg-sv-accent animate-[sv-indeterminate_1.1s_ease-in-out_infinite]" />
                    </div>
                    <span className="w-8 text-right tabular-nums text-[11px] text-sv-muted">
                      {isPaused ? "Paused" : "Starting…"}
                    </span>
                  </>
                ) : (
                  <>
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full border border-sv-border bg-sv-surface-2">
                      <div className={`h-full ${isPaused ? "bg-sv-muted" : "bg-sv-accent"} transition-all duration-75`} style={{ width: `${pct}%` }} />
                    </div>
                    <span className="w-8 text-right tabular-nums text-[11px] text-sv-muted">{pct}%</span>
                  </>
                )}
              </div>
              {isDownloading ? (
                <button
                  onClick={() => pauseStore(voice.id)}
                  title="Pause download"
                  className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                >
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                </button>
              ) : (
                <button
                  onClick={download}
                  title="Resume download"
                  className="p-1 text-sv-accent transition-colors duration-75 hover:text-sv-accent/80"
                >
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><polygon points="5,3 19,12 5,21" /></svg>
                </button>
              )}
              <button
                onClick={() => cancelStore(voice.id)}
                title="Cancel download"
                className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-bad"
              >
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
              </button>
            </div>
          ) : downloaded ? (
            <div className="flex items-center gap-2">
              {!active && (
                <button
                  onClick={onSelect}
                  className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent"
                >
                  Select
                </button>
              )}
              <button
                onClick={() => remove(voice.id)}
                className="rounded-lg border border-sv-border px-2.5 py-1.5 text-xs font-medium text-sv-muted transition-colors duration-75 hover:border-sv-bad/40 hover:bg-sv-bad/10 hover:text-sv-bad"
              >
                Remove
              </button>
            </div>
          ) : (
            <button
              onClick={download}
              disabled={isDownloading}
              className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent"
            >
              Download
            </button>
          )}
        </div>

        <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={`transition-colors duration-75 ${pinned ? "text-sv-accent" : "text-sv-muted/40 hover:text-sv-accent"}`}>
          <svg viewBox="0 0 24 24" width="16" height="16" fill={pinned ? "currentColor" : "none"} stroke={pinned ? "none" : "currentColor"} strokeWidth={pinned ? undefined : "1.75"} strokeLinecap="round" strokeLinejoin="round"><path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" /></svg>
        </button>
      </div>
    </div>
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

