import { useState, useEffect, useMemo } from "react";
import { Link } from "react-router-dom";
import ProviderLogo from "../../shared/ProviderLogo";
import ConfirmDialog from "../../shared/ConfirmDialog";
import { useHardwareInfo } from "../../../hooks/useHardwareInfo";
import { LLM_MODELS } from "../../../services/catalog";
import { llmCompatibility } from "../../../services/recommend";
import { hfSearchModels, hfModelDetails } from "../../../services/tauriBridge";
import { useModelStore } from "../../../stores/modelStore";
import { useSettingsStore } from "../../../stores/settingsStore";
import type { HfSearchItem, HfModelDetails, LlmModel, HardwareInfo, HfFile, SttModel } from "../../../types";
import { formatMB, formatGB } from "../../../services/format";
import SimpleMarkdown from "./SimpleMarkdown";
import { STT_MODELS, sttLanguage } from "../../../services/catalog";
import { accuracyScore, speedScore, deviceRealtimeLabel, llmQualityScore, llmSpeedScore, llmSpeedLabel } from "../../../services/modelMetrics";

// Fixed-width action buttons so Select / Remove / Download stay the same size
// and line up in a column across rows regardless of which state each row is in.
export const ROW_ACTION = "w-[84px] shrink-0 rounded-lg border px-3 py-1.5 text-center text-xs font-medium transition-colors duration-75";
export const ROW_ACTION_PRIMARY = `${ROW_ACTION} border-sv-border bg-sv-surface-2 text-sv-text hover:border-sv-accent hover:text-sv-accent`;
export const ROW_ACTION_DANGER = `${ROW_ACTION} border-sv-border text-sv-muted hover:border-sv-bad/40 hover:bg-sv-bad/10 hover:text-sv-bad`;

// --- Helpers ---
function formatNdaysAgo(isoDate: string) {
  const d = new Date(isoDate);
  const now = new Date();
  const diffTime = Math.abs(now.getTime() - d.getTime());
  const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "1 day ago";
  return `${diffDays} days ago`;
}

function parseQuant(filename: string): string {
  const match = filename.match(/(I?Q\d[_A-Z0-9]*|F16|BF16|F32)(?=\.gguf$)/i);
  return match ? match[1].toUpperCase() : "GGUF";
}

function getFit(sizeBytes: number, hw: HardwareInfo | null) {
  if (!hw) return null;
  const estRamGb = (sizeBytes / (1024 * 1024 * 1024)) * 1.2;
  if (estRamGb < hw.available_ram_gb * 0.8) return "good";
  if (estRamGb < hw.available_ram_gb) return "warn";
  return "bad";
}

function estimateFitFromParams(params_b: number | null, hw: HardwareInfo | null, repoName?: string) {
  if (!hw) return null;
  let p = params_b;
  if (p === null && repoName) {
    const m = repoName.match(/(\d+(?:\.\d+)?)\s*[bB]\b/);
    if (m) p = parseFloat(m[1]);
  }
  if (p === null) return null;
  const estRamGb = p * 0.6 * 1.2;
  if (estRamGb < hw.available_ram_gb * 0.8) return "good";
  if (estRamGb < hw.available_ram_gb) return "warn";
  return "bad";
}

const FIT_DOT = {
  good: "bg-sv-good",
  warn: "bg-sv-warn",
  bad: "bg-sv-bad",
};

// Recommend Q4_K_M > Q5_K_M > Q4 variants > smallest that fits "good"
function getRecommendedIndex(files: HfFile[], hw: HardwareInfo | null): number {
  if (files.length === 0) return -1;
  const getScore = (f: HfFile) => {
    let score = 0;
    const quant = parseQuant(f.name);
    if (quant === "Q4_K_M") score += 1000;
    else if (quant === "Q5_K_M") score += 900;
    else if (quant.startsWith("Q4")) score += 800;
    
    const fit = getFit(f.size_bytes, hw);
    if (fit === "good") score += 100;
    else if (fit === "warn") score += 50;
    
    score -= (f.size_bytes / (1024 * 1024 * 1024)); // prefer smaller if tied
    return score;
  };
  
  let bestIdx = 0;
  let bestScore = getScore(files[0]);
  for (let i = 1; i < files.length; i++) {
    const s = getScore(files[i]);
    if (s > bestScore) {
      bestScore = s;
      bestIdx = i;
    }
  }
  return bestIdx;
}

function parseParams(params_b: number | null): string {
  if (!params_b) return "?B";
  return params_b % 1 === 0 ? `${params_b}B` : `${params_b.toFixed(1)}B`;
}

// --- Components ---

// A labeled 0..1 meter. Minimal on purpose: a single monochrome fill (no
// decorative color-coding between bars), a thin track, and the numeric
// caption doing the real talking. The label already tells the two apart.
export function MetricBar({
  label,
  value,
  caption,
}: {
  label: string;
  value: number;
  caption?: string;
}) {
  const pct = Math.round(Math.max(0, Math.min(1, value)) * 100);
  return (
    <div className="flex items-center gap-2.5">
      <div className="w-14 shrink-0 text-right text-[11px] lowercase text-sv-muted">{label}</div>
      <div className="h-1.5 w-[104px] shrink-0 overflow-hidden rounded-full bg-sv-surface-2">
        <div
          className="h-full rounded-full transition-all"
          style={{ width: `${pct}%`, backgroundColor: "color-mix(in srgb, var(--color-sv-text) 62%, transparent)" }}
        />
      </div>
      {caption && (
        <div className="text-[11px] tabular-nums text-sv-muted truncate">{caption}</div>
      )}
    </div>
  );
}

export default function HfBrowser({ track, categoryFilter, languageFilter }: { track: "llm" | "stt", categoryFilter?: string, languageFilter?: string }) {
  const { hardware } = useHardwareInfo();
  
  // Search state
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [sort, setSort] = useState("downloads");
  
  const [loadingSearch, setLoadingSearch] = useState(false);
  const [searchResults, setSearchResults] = useState<HfSearchItem[]>([]);
  const [searchError, setSearchError] = useState<string | null>(null);

  // Detail state
    const [selectedHfId, setSelectedHfId] = useState<string | null>(null);

  const [loadingDetails, setLoadingDetails] = useState(false);
  const [hfDetails, setHfDetails] = useState<HfModelDetails | null>(null);
  const [hfDetailsError, setHfDetailsError] = useState<string | null>(null);

  const pinnedArr = useSettingsStore((s) => track === "stt" ? s.settings.pinned_stt : s.settings.pinned_llm);
  const togglePinned = useSettingsStore((s) => track === "stt" ? s.togglePinnedStt : s.togglePinnedLlm);
  const pinnedSet = useMemo(() => new Set(pinnedArr || []), [pinnedArr]);

  const hfShowIncompatible = useSettingsStore((s) => s.settings.hf_show_incompatible);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const downloadedLlm = useModelStore((s) => s.downloadedLlm);
  const downloadedStt = useModelStore((s) => s.downloaded);
  const modes = useSettingsStore((s) => s.modes);
  const activeStt = useSettingsStore((s) => s.settings.active_stt_model);
  const usingCloudStt = useSettingsStore((s) => s.settings.stt_cloud_provider_id);

  const isModelInUse = (id: string, isHf: boolean = false) => {
    if (!isHf) {
      if (track === "llm") {
        return modes.some((m) => m.model_source === "local" && m.model_id === id);
      }
      return !usingCloudStt && Boolean(activeStt) && activeStt === id && downloadedStt.has(id);
    }
    // HF rows only know the repo id, not the downloaded file stem — loose match.
    const searchId = (id.split("/").pop() || id).toLowerCase();
    if (track === "llm") {
      return modes.some((m) => m.model_source === "local" && m.model_id.toLowerCase().includes(searchId)) && isModelDownloaded(id, true);
    }
    return !usingCloudStt && Boolean(activeStt) && activeStt.toLowerCase().includes(searchId) && isModelDownloaded(id, true);
  };

  const isModelDownloaded = (id: string, isHf: boolean = false) => {
    const searchId = isHf ? (id.split("/").pop() || id).toLowerCase() : id;
    const set = track === "llm" ? downloadedLlm : downloadedStt;
    if (!isHf) return set.has(id);
    for (const downloadedId of set) {
      if (downloadedId.toLowerCase().includes(searchId)) return true;
    }
    return false;
  };

  // Debounce query
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query.trim());
    }, 400);
    return () => clearTimeout(timer);
  }, [query]);

  // Fetch search results
  useEffect(() => {
    let active = true;
    setLoadingSearch(true);
    setSearchError(null);
    
    hfSearchModels(debouncedQuery, sort, 20, track)
      .then((res) => {
        if (!active) return;
        setSearchResults(res);
      })
      .catch(() => {
        if (!active) return;
        setSearchError("Hugging Face is unreachable.");
      })
      .finally(() => {
        if (active) setLoadingSearch(false);
      });
      
    return () => { active = false; };
  }, [debouncedQuery, sort, track]);

  // Fetch details
  useEffect(() => {
    if (selectedHfId) {
      let active = true;
      setLoadingDetails(true);
      setHfDetailsError(null);
      setHfDetails(null);
      
      hfModelDetails(selectedHfId, track)
        .then((res) => {
          if (!active) return;
          setHfDetails(res);
        })
        .catch(() => {
          if (!active) return;
          setHfDetailsError("Failed to load model details.");
        })
        .finally(() => {
          if (active) setLoadingDetails(false);
        });
        
      return () => { active = false; };
    }
  }, [selectedHfId]);

  const sortStaffPicks = (a: any, b: any) => {
    const getScore = (m: any) => {
      let score = 0;
      if (isModelInUse(m.id, false)) score += 10000;
      if (pinnedSet.has(m.id)) score += 1000;
      if (isModelDownloaded(m.id, false)) score += 100;
      return score;
    };
    return getScore(b) - getScore(a);
  };

  const sortHfResults = (a: HfSearchItem, b: HfSearchItem) => {
    const getScore = (m: HfSearchItem) => {
      let score = 0;
      if (isModelInUse(m.id, true)) score += 10000;
      if (pinnedSet.has(m.id)) score += 1000;
      if (isModelDownloaded(m.id, true)) score += 100;
      return score;
    };
    return getScore(b) - getScore(a);
  };

  const staffPicksRaw = track === "stt" ? STT_MODELS.filter(m => {
    if (categoryFilter && categoryFilter !== "all" && m.preset !== categoryFilter) return false;
    if (languageFilter && languageFilter !== "all" && sttLanguage(m) !== languageFilter) return false;
    return true;
  }) : LLM_MODELS;
  
  const staffPicksSorted = [...staffPicksRaw].sort(sortStaffPicks);
  
  const hfResultsVisible = searchResults.filter(item => {
    if (hfShowIncompatible) return true;
    const fit = estimateFitFromParams(item.params_b, hardware, item.id.split("/")[1]);
    return fit !== "bad";
  }).sort(sortHfResults);

  return (
    <div className="flex h-[calc(100vh-140px)] w-full flex-col gap-4">
      <div className="flex w-full max-w-[1180px] flex-wrap items-center gap-x-4 gap-y-2">
        <input
          type="text"
          placeholder="Search models on Hugging Face..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="min-w-[220px] flex-1 rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm placeholder:text-sv-muted focus:border-sv-accent focus:outline-none"
        />
        <select
          value={sort}
          onChange={(e) => setSort(e.target.value)}
          className="rounded-lg border border-sv-border bg-sv-bg px-2 py-1.5 text-sm text-sv-text"
        >
          <option value="downloads">Best Match / Downloads</option>
          <option value="likes">Most Likes</option>
          <option value="trending">Trending</option>
          <option value="lastModified">Recently Updated</option>
        </select>
        <label className="flex items-center gap-2 text-sm text-sv-text cursor-pointer whitespace-nowrap">
          <input 
            type="checkbox" 
            checked={hfShowIncompatible}
            onChange={(e) => setSettings({ hf_show_incompatible: e.target.checked })}
            className="rounded border-sv-border bg-sv-surface-2 text-sv-accent focus:ring-sv-accent"
          />
          Show incompatible
        </label>
      </div>

      <div className="flex-1 overflow-y-auto pr-2 flex flex-col gap-2 max-w-[1180px]">
        {!debouncedQuery && (
          <div className="flex flex-col gap-2">
            <h3 className="mb-2 text-[10px] font-medium uppercase tracking-wider text-sv-muted">Staff Picks</h3>
            {staffPicksSorted.map((m: any) => {
              if (track === "stt") {
                return (
                  <SttRow
                    key={m.id}
                    model={m}
                    hardware={hardware}
                    pinned={pinnedSet.has(m.id)}
                    onTogglePin={() => togglePinned(m.id)}
                  />
                );
              } else {
                return (
                  <LlmRow
                    key={m.id}
                    model={m}
                    hardware={hardware}
                    pinned={pinnedSet.has(m.id)}
                    onTogglePin={() => togglePinned(m.id)}
                  />
                );
              }
            })}
          </div>
        )}

        <div className="flex flex-col gap-2">
          {!debouncedQuery && <h3 className="mt-5 mb-2 text-[10px] font-medium uppercase tracking-wider text-sv-muted">Trending on Hugging Face</h3>}
          
          {loadingSearch ? (
            [1, 2, 3, 4].map((i) => (
              <div key={i} className="flex animate-pulse items-start gap-3 rounded-xl border border-sv-border bg-sv-surface px-4 py-3">
                <div className="h-9 w-9 shrink-0 rounded-full bg-sv-border"></div>
                <div className="flex-1 flex flex-col gap-2 pt-1">
                  <div className="h-3.5 w-1/4 rounded bg-sv-border"></div>
                  <div className="h-2.5 w-1/3 rounded bg-sv-border"></div>
                </div>
              </div>
            ))
          ) : searchError ? (
            <div className="p-4 text-center text-sm text-sv-bad">{!debouncedQuery ? "Hugging Face unreachable" : searchError}</div>
          ) : hfResultsVisible.length === 0 ? (
            <div className="p-4 text-center text-sm text-sv-muted">No models found.</div>
          ) : (
            hfResultsVisible.map((item) => {
              const isExpanded = selectedHfId === item.id;
              return (
                <HfRow
                  key={item.id}
                  item={item}
                  hardware={hardware}
                  pinned={pinnedSet.has(item.id)}
                  onTogglePin={() => togglePinned(item.id)}
                  isExpanded={isExpanded}
                  onToggleExpand={() => setSelectedHfId(isExpanded ? null : item.id)}
                  track={track}
                >
                  {isExpanded && (
                    <div className="mt-3 border-t border-sv-border pt-3">
                      {loadingDetails ? (
                        <div className="flex justify-center p-4">
                          <div className="h-6 w-6 animate-spin rounded-full border-2 border-sv-accent border-t-transparent"></div>
                        </div>
                      ) : hfDetailsError ? (
                        <div className="p-4 text-center text-sm text-sv-bad">{hfDetailsError}</div>
                      ) : hfDetails ? (
                        <HfDetail details={hfDetails} hardware={hardware} track={track} />
                      ) : null}
                    </div>
                  )}
                </HfRow>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}

// --- Detail Views ---



function SttRow({ 
  model, 
  hardware, 
  pinned,
  onTogglePin
}: { 
  model: SttModel; 
  hardware: HardwareInfo | null;
  pinned: boolean;
  onTogglePin: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [starting, setStarting] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const downloaded = useModelStore((s) => s.downloaded.has(model.id));
  const progress = useModelStore((s) => s.progress[model.id]);
  const download = useModelStore((s) => s.download);
  const pauseStore = useModelStore((s) => s.pause);
  const cancelStore = useModelStore((s) => s.cancel);
  const remove = useModelStore((s) => s.remove);

  const activeStt = useSettingsStore((s) => s.settings.active_stt_model);
  const usingCloudStt = useSettingsStore((s) => s.settings.stt_cloud_provider_id);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const isActive = !usingCloudStt && Boolean(activeStt) && activeStt === model.id && downloaded;

  const selectStt = (id: string) =>
    setSettings({ active_stt_model: id, stt_cloud_provider_id: null });

  const estRamGb = model.ram_mb / 1024;
  let level = "good";
  if (hardware) {
    if (estRamGb > hardware.available_ram_gb) level = "bad";
    else if (estRamGb > hardware.available_ram_gb * 0.8) level = "warn";
  }

  const isDownloading = progress?.status === "downloading";
  const isPaused = progress?.status === "paused";
  const isBusy = starting || isDownloading;
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  const handleDownload = async () => {
    setStarting(true);
    try {
      await download(model.id);
    } finally {
      setStarting(false);
    }
  };

  return (
    <div
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      className={`rounded-xl border ${isActive ? "border-sv-accent/40" : "border-sv-border"} bg-sv-surface transition-colors duration-75 hover:bg-sv-surface-2/40 px-4 py-3`}
    >
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        {/* Fit dot: the single at-a-glance "does this run well here" cue. The old
            "May be slow"/"Pinned"/redundant chips are gone — the dot encodes fit,
            the pin star encodes pinned, so at most one state chip rides the name. */}
        <span
          className={`h-2 w-2 shrink-0 rounded-full ${(FIT_DOT as any)[level]}`}
          title={level === "good" ? "Fits well on your device" : level === "warn" ? "Runs, may be slow on your device" : "Too heavy for your device"}
        />
        <ProviderLogo provider={model.provider} size={30} />
        <div className="flex min-w-[150px] max-w-[380px] flex-1 flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <span className="truncate text-[13px] font-semibold text-sv-text">{model.label}</span>
            {isActive ? (
              <span className="shrink-0 rounded bg-sv-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-sv-accent">In use</span>
            ) : (
              <span className="shrink-0 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px] text-sv-muted">Staff Pick</span>
            )}
          </div>
          <div className="truncate text-[11px] text-sv-muted">{model.provider}</div>
          {progress?.status === "error" && <div className="truncate text-[11px] text-sv-bad">{progress.error}</div>}
        </div>

        {/* Accuracy + speed — clean neutral bars by default */}
        <div className="hidden md:flex w-[172px] shrink-0 flex-col gap-1">
          <MetricBar label="accuracy" value={accuracyScore(model.wer)} />
          <MetricBar label="speed" value={speedScore(model.speed_label, hardware)} />
        </div>

        {/* Size / RAM — its own right-aligned column so it lines up down the list. */}
        <div className="hidden sm:block w-[86px] shrink-0 text-right tabular-nums">
          <div className="text-[12.5px] font-semibold text-sv-text">{formatMB(model.size_mb)}</div>
          <div className="text-[10.5px] text-sv-muted">{model.ram_mb} MB RAM</div>
        </div>

        <div className="ml-auto flex w-[190px] shrink-0 items-center justify-end gap-2">
          <div className="flex shrink-0 items-center">
            {isBusy || isPaused ? (
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
                    onClick={() => pauseStore(model.id)}
                    title="Pause download"
                    className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                  >
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                  </button>
                ) : (
                  <button
                    onClick={handleDownload}
                    title="Resume download"
                    className="p-1 text-sv-accent transition-colors duration-75 hover:text-sv-accent/80"
                  >
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><polygon points="5,3 19,12 5,21" /></svg>
                  </button>
                )}
                <button
                  onClick={() => cancelStore(model.id)}
                  title="Cancel download"
                  className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-bad"
                >
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                </button>
              </div>
            ) : downloaded ? (
              <div className="flex items-center gap-2">
                {!isActive && (
                  <button onClick={() => selectStt(model.id)} className={ROW_ACTION_PRIMARY}>
                    Select
                  </button>
                )}
                <button onClick={() => setConfirmRemove(true)} className={ROW_ACTION_DANGER}>
                  Remove
                </button>
              </div>
            ) : (
              <button disabled={isBusy} onClick={handleDownload} className={ROW_ACTION_PRIMARY}>
                Download
              </button>
            )}
          </div>

          <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={`transition-colors duration-75 ${pinned ? "text-sv-accent" : "text-sv-muted/40 hover:text-sv-accent"}`}>
            <svg viewBox="0 0 24 24" width="16" height="16" fill={pinned ? "currentColor" : "none"} stroke={pinned ? "none" : "currentColor"} strokeWidth={pinned ? undefined : "1.75"} strokeLinecap="round" strokeLinejoin="round"><path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" /></svg>
          </button>
        </div>
      </div>

      <div
        className="grid transition-[grid-template-rows] duration-200 ease-out"
        style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
      >
        <div className="overflow-hidden">
          <div className="flex items-center gap-6 border-t border-sv-border pt-2 mt-2 text-[11px] text-sv-muted tabular-nums">
            <div><span className="mr-1.5 text-sv-muted">accuracy</span>{model.wer.replace("~", "≈")} word error</div>
            <div><span className="mr-1.5 text-sv-muted">speed</span>{(deviceRealtimeLabel(model.speed_label, hardware) ?? model.speed_label).replace(" on your device", "")}</div>
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={confirmRemove}
        title="Remove this model?"
        message={
          <>
            This deletes <span className="text-sv-text">{model.label}</span> ({formatMB(model.size_mb)}) from
            disk. You can download it again later.
          </>
        }
        confirmLabel="Remove"
        onConfirm={() => {
          remove(model.id);
          setConfirmRemove(false);
        }}
        onCancel={() => setConfirmRemove(false)}
      />
    </div>
  );
}

function LlmRow({ 
  model, 
  hardware, 
  pinned,
  onTogglePin
}: { 
  model: LlmModel; 
  hardware: HardwareInfo | null;
  pinned: boolean;
  onTogglePin: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [starting, setStarting] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const downloaded = useModelStore((s) => s.downloadedLlm.has(model.id));
  const progress = useModelStore((s) => s.progress[model.id]);
  const download = useModelStore((s) => s.downloadLlm);
  const pauseStore = useModelStore((s) => s.pause);
  const cancelStore = useModelStore((s) => s.cancel);
  const remove = useModelStore((s) => s.removeLlm);

  const modes = useSettingsStore((s) => s.modes);
  const inUse = modes.some(m => m.model_source === "local" && m.model_id === model.id);

  const level = llmCompatibility(model, hardware).level;
  const isDownloading = progress?.status === "downloading";
  const isPaused = progress?.status === "paused";
  const isBusy = starting || isDownloading;
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  const handleDownload = async () => {
    setStarting(true);
    try {
      await download(model.id);
    } finally {
      setStarting(false);
    }
  };

  return (
    <div
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      className={`rounded-xl border ${inUse ? "border-sv-accent/40" : "border-sv-border"} bg-sv-surface transition-colors duration-75 hover:bg-sv-surface-2/40 px-4 py-3`}
    >
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <span
          className={`h-2 w-2 shrink-0 rounded-full ${(FIT_DOT as any)[level]}`}
          title={level === "good" ? "Fits well on your device" : level === "warn" ? "Runs, may be slow on your device" : "Too heavy for your device"}
        />
        <ProviderLogo provider={model.provider} size={30} />
        <div className="flex min-w-[150px] max-w-[380px] flex-1 flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <span className="truncate text-[13px] font-semibold text-sv-text">{model.name}</span>
            {inUse ? (
              <span className="shrink-0 rounded bg-sv-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-sv-accent">In use</span>
            ) : (
              <span className="shrink-0 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px] text-sv-muted">Staff Pick</span>
            )}
          </div>
          <div className="truncate text-[11px] text-sv-muted">{model.provider}</div>
          {progress?.status === "error" && <div className="truncate text-[11px] text-sv-bad">{progress.error}</div>}
        </div>

        <div className="hidden md:flex w-[172px] shrink-0 flex-col gap-1">
          <MetricBar label="quality" value={llmQualityScore(model.params)} />
          <MetricBar label="speed" value={llmSpeedScore(level)} />
        </div>

        <div className="hidden sm:block w-[86px] shrink-0 text-right tabular-nums">
          <div className="text-[12.5px] font-semibold text-sv-text">{model.params}</div>
          <div className="text-[10.5px] text-sv-muted">{formatGB(model.ram_gb)} RAM</div>
        </div>

        <div className="ml-auto flex w-[190px] shrink-0 items-center justify-end gap-2">
          <div className="flex shrink-0 items-center">
            {isBusy || isPaused ? (
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
                    onClick={() => pauseStore(model.id)}
                    title="Pause download"
                    className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                  >
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                  </button>
                ) : (
                  <button
                    onClick={handleDownload}
                    title="Resume download"
                    className="p-1 text-sv-accent transition-colors duration-75 hover:text-sv-accent/80"
                  >
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><polygon points="5,3 19,12 5,21" /></svg>
                  </button>
                )}
                <button
                  onClick={() => cancelStore(model.id)}
                  title="Cancel download"
                  className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-bad"
                >
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                </button>
              </div>
            ) : downloaded ? (
              <div className="flex items-center gap-2">
                {inUse ? (
                  <span className="text-[11px] font-medium text-sv-good">In use</span>
                ) : (
                  <Link to="/modes" className="whitespace-nowrap rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent">
                    Assign to a mode
                  </Link>
                )}
                <button onClick={() => setConfirmRemove(true)} className={ROW_ACTION_DANGER}>
                  Remove
                </button>
              </div>
            ) : (
              <button disabled={isBusy} onClick={handleDownload} className={ROW_ACTION_PRIMARY}>
                Download
              </button>
            )}
          </div>

          <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={`transition-colors duration-75 ${pinned ? "text-sv-accent" : "text-sv-muted/40 hover:text-sv-accent"}`}>
            <svg viewBox="0 0 24 24" width="16" height="16" fill={pinned ? "currentColor" : "none"} stroke={pinned ? "none" : "currentColor"} strokeWidth={pinned ? undefined : "1.75"} strokeLinecap="round" strokeLinejoin="round"><path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" /></svg>
          </button>
        </div>
      </div>

      <div
        className="grid transition-[grid-template-rows] duration-200 ease-out"
        style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
      >
        <div className="overflow-hidden">
          <div className="flex items-center gap-6 border-t border-sv-border pt-2 mt-2 text-[11px] text-sv-muted tabular-nums">
            <div><span className="mr-1.5 text-sv-muted">quality</span>{model.params} parameters</div>
            <div><span className="mr-1.5 text-sv-muted">speed</span>{llmSpeedLabel(level)}</div>
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={confirmRemove}
        title="Remove this model?"
        message={
          <>
            This deletes <span className="text-sv-text">{model.name}</span> from disk. You can
            download it again later.
          </>
        }
        confirmLabel="Remove"
        onConfirm={() => {
          remove(model.id);
          setConfirmRemove(false);
        }}
        onCancel={() => setConfirmRemove(false)}
      />
    </div>
  );
}

function HfRow({
  item,
  hardware,
  pinned,
  onTogglePin,
  isExpanded,
  onToggleExpand,
  track,
  children,
}: {
  item: HfSearchItem;
  hardware: HardwareInfo | null;
  pinned: boolean;
  onTogglePin: () => void;
  isExpanded: boolean;
  onToggleExpand: () => void;
  track: "llm" | "stt";
  children?: React.ReactNode;
}) {
  const [owner, name] = item.id.split("/");
  const isVision = item.tags?.includes("vision") || item.tags?.includes("multimodal") || item.pipeline_tag === "image-text-to-text";
  const isToolUse = item.tags?.includes("tool-use") || item.tags?.includes("function-calling");
  const isReasoning = item.tags?.includes("reasoning") || item.tags?.includes("thinking");

  const downloadedLlm = useModelStore((s) => s.downloadedLlm);
  const downloadedStt = useModelStore((s) => s.downloaded);
  const modes = useSettingsStore((s) => s.modes);
  const activeStt = useSettingsStore((s) => s.settings.active_stt_model);
  const usingCloudStt = useSettingsStore((s) => s.settings.stt_cloud_provider_id);
  
  const searchId = (item.id.split("/").pop() || item.id).toLowerCase();
  const downloadedSet = track === "llm" ? downloadedLlm : downloadedStt;
  const isDownloaded = Array.from(downloadedSet).some((id) => id.toLowerCase().includes(searchId));

  let inUse = false;
  if (track === "llm") {
    inUse = modes.some((m) => m.model_source === "local" && m.model_id.toLowerCase().includes(searchId)) && isDownloaded;
  } else {
    inUse = !usingCloudStt && Boolean(activeStt) && activeStt.toLowerCase().includes(searchId) && isDownloaded;
  }

  const fit = estimateFitFromParams(item.params_b, hardware, name);
  
  return (
    <div className="group relative rounded-xl border border-sv-border bg-sv-surface transition-colors duration-75 hover:bg-sv-surface-2/40 flex flex-col">
      <div className="flex flex-wrap items-center gap-x-5 gap-y-2 px-4 py-3">
        <div className="flex min-w-[230px] max-w-[420px] flex-1 items-center gap-3">
          <ProviderLogo provider={owner} size={32} />
          <div className="min-w-0 flex-1 flex flex-col gap-1.5">
            <div className="flex items-center gap-2">
              {fit && (
                <span
                  className={`h-1.5 w-1.5 shrink-0 rounded-full ${(FIT_DOT as any)[fit]}`}
                  title={fit === "good" ? "Fits well" : fit === "warn" ? "May be slow" : "Too heavy"}
                />
              )}
              <span className="truncate text-[13px] font-semibold text-sv-text">{name}</span>
              {inUse && <span className="shrink-0 rounded bg-sv-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-sv-accent">In use</span>}
              {pinned && <span className="shrink-0 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px] text-sv-muted">Pinned</span>}
              {fit && fit !== "good" && <span className="shrink-0 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px] text-sv-text">{fit === "warn" ? "May be slow" : "Too heavy"}</span>}
            </div>
            <div className="truncate tabular-nums text-[11px] text-sv-muted">
               {owner} · ↓{item.downloads.toLocaleString()} · {formatNdaysAgo(item.last_modified)} · {parseParams(item.params_b)}
               {(isVision || isToolUse || isReasoning) && " · "}
               {isVision && <span className="ml-1 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px]">Vision</span>}
               {isToolUse && <span className="ml-1 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px]">Tools</span>}
               {isReasoning && <span className="ml-1 rounded bg-sv-surface-2 px-1.5 py-0.5 text-[10px]">Think</span>}
            </div>
          </div>
        </div>

        <div className="flex w-[300px] shrink-0 flex-col gap-1"></div>

        <div className="ml-auto flex shrink-0 items-center gap-2">
          <div className="flex shrink-0 items-center">
            <button 
              onClick={onToggleExpand}
              className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent"
            >
              {isExpanded ? "Close" : "Choose version"}
            </button>
          </div>

          <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={`transition-colors duration-75 ${pinned ? "text-sv-accent" : "text-sv-muted/40 hover:text-sv-accent"}`}>
            <svg viewBox="0 0 24 24" width="16" height="16" fill={pinned ? "currentColor" : "none"} stroke={pinned ? "none" : "currentColor"} strokeWidth={pinned ? undefined : "1.75"} strokeLinecap="round" strokeLinejoin="round"><path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" /></svg>
          </button>
        </div>
      </div>
      {children}
    </div>
  );
}

function HfDetail({ details, hardware, track }: { details: HfModelDetails; hardware: HardwareInfo | null; track: "llm" | "stt" }) {
  const [owner, name] = details.id.split("/");

  // Filter and group files
  const ggufs = details.files.filter(f => track === "stt" ? (f.name.includes("ggml-") && f.name.endsWith(".bin")) : (f.name.toLowerCase().endsWith(".gguf") && !f.name.includes("mmproj")));
  const multiPartRegex = /-(000\d{2})-of-(000\d{2})\.gguf$/i;
  
  const fileGroups = new Map<string, { name: string, size: number, isMultiPart: boolean, originalFiles: HfFile[] }>();
  ggufs.forEach(f => {
    const match = f.name.match(multiPartRegex);
    if (match) {
      const baseName = f.name.replace(multiPartRegex, "");
      if (!fileGroups.has(baseName)) {
        fileGroups.set(baseName, { name: baseName + " (Multi-part)", size: 0, isMultiPart: true, originalFiles: [] });
      }
      const g = fileGroups.get(baseName)!;
      g.size += f.size_bytes;
      g.originalFiles.push(f);
    } else {
      fileGroups.set(f.name, { name: f.name, size: f.size_bytes, isMultiPart: false, originalFiles: [f] });
    }
  });

  const availableFiles = Array.from(fileGroups.values()).map(g => ({
    name: g.name,
    size_bytes: g.size,
    isMultiPart: g.isMultiPart,
    originalFile: g.originalFiles[0],
  }));

  const recommendedIndex = getRecommendedIndex(availableFiles, hardware);
  const [selectedIndex, setSelectedIndex] = useState(recommendedIndex >= 0 ? recommendedIndex : 0);

  const selectedFile = availableFiles[selectedIndex];
  
  // Custom download logic
  const downloadedLlm = useModelStore(s => s.downloadedLlm);
  const downloadedStt = useModelStore(s => s.downloaded);
  const progress = useModelStore(s => s.progress);
  const pauseStore = useModelStore(s => s.pause);
  const cancelStore = useModelStore(s => s.cancel);
  const downloadCustomLlm = useModelStore(s => s.downloadCustomLlm);
  const removeLlm = useModelStore(s => s.removeLlm);
  const downloadCustomStt = useModelStore(s => s.downloadCustomStt);
  const removeStt = useModelStore(s => s.remove);

  const activeStt = useSettingsStore((s) => s.settings.active_stt_model);
  const usingCloudStt = useSettingsStore((s) => s.settings.stt_cloud_provider_id);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const modes = useSettingsStore((s) => s.modes);
  const selectStt = (id: string) =>
    setSettings({ active_stt_model: id, stt_cloud_provider_id: null });

  // Derive model ID for storing
  const getModelId = (filename: string) => {
    let base = filename;
    if (base.includes('/')) {
      base = base.split('/').pop() || base;
    }
    if (track === "stt") {
      return base.replace(/^ggml-/i, "").replace(/\.bin$/i, "").toLowerCase();
    }
    return base.replace(/\.gguf$/i, "").toLowerCase();
  };

  const [readmeExpanded, setReadmeExpanded] = useState(false);

  return (
    <div className="flex flex-col gap-4">
      {details.gated ? (
        <div className="rounded-xl border border-sv-warn/30 bg-sv-warn/10 p-4 text-sm text-sv-warn">
          <p className="font-medium">This model is gated.</p>
          <p className="mt-1">You need to accept the license agreement on the Hugging Face website before downloading. Downloading via this app is not supported yet for gated repos.</p>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {availableFiles.map((f, i) => {
            const fit = getFit(f.size_bytes, hardware);
            const mId = getModelId(f.name);
            const isDownloaded = track === "stt" ? downloadedStt.has(mId) : downloadedLlm.has(mId);
            const isPaused = !isDownloaded && progress[mId]?.status === "paused";
            const parsedLabel = track === "stt" ? f.name.split('/').pop()?.replace(/^ggml-/i, "").replace(/\.bin$/i, "") : parseQuant(f.name);
            const extraTag = track === "stt" ? (f.name.includes(".en") ? "English-only" : "Multilingual") : "";
            const isSelected = selectedIndex === i;
            
            return (
              <button
                key={f.name}
                disabled={f.isMultiPart}
                onClick={() => setSelectedIndex(i)}
                className={`flex items-center justify-between rounded-lg border p-3 text-left transition ${
                  f.isMultiPart ? "opacity-50 cursor-not-allowed border-sv-border bg-sv-bg" :
                  isSelected ? "border-sv-accent bg-sv-accent/5 ring-1 ring-sv-accent/20" : "border-sv-border bg-sv-bg hover:bg-sv-surface-2"
                }`}
              >
                <div className="flex items-center gap-3">
                  {fit && <span className={`shrink-0 h-2 w-2 rounded-full ${(FIT_DOT as any)[fit]}`} />}
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-sm">{parsedLabel}</span>
                      {isDownloaded && <span className="text-[10px] font-medium text-sv-good">Downloaded</span>}
                      {isPaused && <span className="text-[10px] font-medium text-sv-warn">Paused</span>}
                    </div>
                    <div className="mt-0.5 text-xs text-sv-muted">
                      {formatMB(f.size_bytes / (1024 * 1024))}
                      {extraTag ? ` · ${extraTag}` : ""}
                      {fit === "good" ? " · Fits well" : fit === "warn" ? " · Tight fit" : fit === "bad" ? " · Too large" : ""}
                      {f.isMultiPart ? " · (Multi-part, not supported)" : ""}
                    </div>
                  </div>
                </div>
                {isSelected && !f.isMultiPart && (
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-sv-accent">
                    <polyline points="20 6 9 17 4 12"></polyline>
                  </svg>
                )}
              </button>
            );
          })}

          {selectedFile && !selectedFile.isMultiPart && (() => {
            const fit = getFit(selectedFile.size_bytes, hardware);
            const estSpeed = details.params_b ? (details.params_b <= 4 ? "Fast on this device" : details.params_b <= 9 ? "Usable" : "May be slow") : null;
            const mId = getModelId(selectedFile.name);
            const isDl = track === "stt" ? downloadedStt.has(mId) : downloadedLlm.has(mId);
            const isActiveStt = track === "stt" && !usingCloudStt && Boolean(activeStt) && activeStt === mId && downloadedStt.has(mId);
            const isActiveLlm = track === "llm" && modes.some(m => m.model_source === "local" && m.model_id === mId);
            const isActive = track === "stt" ? isActiveStt : isActiveLlm;
            const prog = progress[mId];
            const isDownloading = prog?.status === "downloading";
            const isPaused = prog?.status === "paused";
            const pct = prog && prog.total_bytes > 0 ? Math.round((prog.downloaded_bytes / prog.total_bytes) * 100) : 0;

            const doDownload = () => {
              const modelUrl = `https://huggingface.co/${details.id}/resolve/main/${selectedFile.originalFile.name}?download=true`;
              if (track === "stt") {
                const filename = selectedFile.originalFile.name.split('/').pop() || selectedFile.originalFile.name;
                downloadCustomStt(mId, modelUrl, filename, selectedFile.size_bytes / (1024*1024));
              } else {
                const customLlm: LlmModel = {
                  id: mId,
                  name: name + " (" + parseQuant(selectedFile.name) + ")",
                  provider: owner,
                  url: modelUrl,
                  params: details.params_b ? parseParams(details.params_b) : "?B",
                  size_mb: selectedFile.size_bytes / (1024*1024),
                  ram_gb: (selectedFile.size_bytes / (1024*1024*1024)) * 1.2,
                  tier: details.params_b ? (details.params_b <= 4 ? "tiny" : details.params_b <= 9 ? "small" : details.params_b <= 14 ? "medium" : "large") : "medium",
                  speed_label: estSpeed || "Unknown",
                  languages: "Multi",
                  license: "HF",
                  best_for: "General",
                };
                downloadCustomLlm(customLlm);
              }
            };

            return (
              <div className="mt-2 flex items-center justify-between">
                <div className="flex flex-col gap-1 text-[11px]">
                  {fit && (
                    <div className="flex items-center gap-1.5">
                      <span className={`h-2 w-2 rounded-full ${(FIT_DOT as any)[fit]}`} />
                      <span className="text-sv-text">{fit === "good" ? "Recommended" : fit === "warn" ? "Works, may be slow" : "Heavy for your device"}</span>
                    </div>
                  )}
                  {estSpeed && fit !== "bad" && <span className="text-sv-muted">{estSpeed}</span>}
                </div>
                
                <div>
                  {isDownloading || isPaused ? (
                    <div className="flex items-center gap-1.5">
                      <div className="flex w-28 items-center gap-2">
                        <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-sv-surface-2 border border-sv-border">
                          <div className={`h-full ${isPaused ? "bg-sv-muted" : "bg-sv-accent"} transition-all`} style={{ width: `${pct}%` }} />
                        </div>
                        <span className="w-8 text-right text-xs text-sv-muted">{isPaused ? "Paused" : `${pct}%`}</span>
                      </div>
                      {isDownloading ? (
                        <button
                          onClick={() => pauseStore(mId)}
                          title="Pause download"
                          className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                        >
                          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                        </button>
                      ) : (
                        <button
                          onClick={doDownload}
                          title="Resume download"
                          className="p-1 text-sv-accent transition-colors duration-75 hover:text-sv-accent/80"
                        >
                          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><polygon points="5,3 19,12 5,21" /></svg>
                        </button>
                      )}
                      <button
                        onClick={() => cancelStore(mId)}
                        title="Cancel download"
                        className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-bad"
                      >
                        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                      </button>
                    </div>
                  ) : isDl ? (
                    <div className="flex flex-col items-end gap-1">
                      <div className="flex items-center gap-3">
                        {isActive ? (
                          <span className="text-xs font-medium text-sv-good">In use</span>
                        ) : (
                          track === "stt" ? (
                            <button onClick={() => selectStt(mId)} className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent">
                              Select
                            </button>
                          ) : (
                            <Link to="/modes" className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent">
                              Assign to a mode
                            </Link>
                          )
                        )}
                        <button onClick={() => track === "stt" ? removeStt(mId) : removeLlm(mId)} className="rounded-lg border border-sv-border px-2.5 py-1.5 text-xs font-medium text-sv-muted transition-colors duration-75 hover:border-sv-bad/40 hover:bg-sv-bad/10 hover:text-sv-bad">
                          Remove
                        </button>
                      </div>
                    </div>
                  ) : (
                    <button onClick={doDownload} className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent">
                      Download
                    </button>
                  )}
                </div>
              </div>
            );
          })()}
        </div>
      )}

      {details.readme && (
        <div className="rounded-xl border border-sv-border bg-sv-surface">
          <button 
            onClick={() => setReadmeExpanded(!readmeExpanded)}
            className="flex w-full items-center justify-between p-4 text-left font-medium hover:bg-sv-surface-2/50"
          >
            <span>About this model</span>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={`transition-transform ${readmeExpanded ? "rotate-180" : ""}`}><path d="M6 9l6 6 6-6" /></svg>
          </button>
          {readmeExpanded && (
            <div className="border-t border-sv-border p-4 text-sm text-sv-text max-w-full overflow-hidden">
              <SimpleMarkdown content={details.readme} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
