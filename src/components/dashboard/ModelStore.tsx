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
import { hfPiperVoices } from "../../services/tauriBridge";
import HfBrowser from "./hf/HfBrowser";

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
      subtitle="Pick a model to dictate with. Colored dots show what fits your device."
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
          <div className="grid grid-cols-1 items-start gap-2 lg:grid-cols-2">
            {sortedTts.map((v) => (
              <TtsCard
                key={v.id}
                voice={v}
                active={activeTts === v.id}
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
            tab. You can also use a cloud provider instead (API Keys).
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
  fast: { label: "Fast", cls: "bg-sv-good/15 text-sv-good" },
  balanced: { label: "Balanced", cls: "bg-sv-accent/15 text-sv-accent" },
  natural: { label: "Natural HD", cls: "bg-sv-warn/15 text-sv-warn" },
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
  const remove = useModelStore((s) => s.removeTts);

  const download = () => {
    // If it's a known catalog model, we could use downloadTts.
    // However, for merged custom Piper models from hfPiperVoices, we must use downloadCustomTts
    // with the explicit URLs (as they are not in the hardcoded TTS_MODELS array).
    downloadCustomTts(voice.id, voice.url_onnx, voice.url_json, voice.size_mb);
  };

  const isDownloading = progress?.status === "downloading";
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;
  const chip = TTS_QUALITY_CHIP[voice.quality];

  return (
    <div
      className={`flex items-center gap-3 rounded-xl border px-3.5 py-2.5 transition ${
        active
          ? "border-sv-accent bg-sv-accent/5 ring-1 ring-sv-accent/40"
          : "border-sv-border bg-sv-surface hover:border-sv-muted/40"
      }`}
    >
      {/* Speaker glyph */}
      <span className="flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-lg bg-sv-surface-2 text-sv-muted">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
          <path d="M11 5 6 9H3v6h3l5 4z" />
          <path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" />
        </svg>
      </span>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <h3 className="truncate text-sm font-medium">{voice.label}</h3>
          <span
            className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium ${chip.cls}`}
          >
            {chip.label}
          </span>
          {active && (
            <span className="shrink-0 rounded-full bg-sv-accent px-2 py-0.5 text-[10px] font-medium text-white">
              Active
            </span>
          )}
        </div>
        <p className="mt-0.5 truncate text-[11px] text-sv-muted">
          {voice.engine === "sherpa" ? "Sherpa" : "Piper"} · {voice.language} ·{" "}
          {formatMB(voice.size_mb)}
        </p>
      </div>

      <div className="flex shrink-0 items-center justify-end gap-2 min-w-[168px]">
        <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={pinned ? "mr-auto rounded-lg p-1.5 transition text-sv-accent" : "mr-auto rounded-lg p-1.5 transition text-sv-muted hover:text-sv-accent"}><StarIcon filled={pinned} /></button>
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
            {active ? (
              <span className="w-[84px] text-right text-[11px] text-sv-good">In use</span>
            ) : (
              <button
                onClick={onSelect}
                className="w-[84px] text-center rounded-lg bg-sv-surface-2 px-3 py-1.5 text-xs font-medium hover:bg-sv-accent hover:text-white"
              >
                Select
              </button>
            )}
            <button
              onClick={() => remove(voice.id)}
              title="Remove download"
              className="rounded-lg p-1.5 text-sv-muted transition hover:bg-sv-surface-2 hover:text-sv-bad"
            >
              <TrashIcon />
            </button>
          </>
        ) : (
          <button
            onClick={download}
            className="w-[84px] text-center rounded-lg border border-sv-border px-3 py-1.5 text-xs font-medium text-sv-text hover:border-sv-accent hover:text-sv-accent"
          >
            Download
          </button>
        )}
      </div>

      {progress?.status === "error" && (
        <p className="w-full text-[11px] text-sv-bad">{progress.error}</p>
      )}
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

function StarIcon({ filled }: { filled: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="15"
      height="15"
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth={filled ? undefined : "1.75"}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" />
    </svg>
  );
}
