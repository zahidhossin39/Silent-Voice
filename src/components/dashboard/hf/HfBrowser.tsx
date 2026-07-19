import { useState, useEffect, useMemo } from "react";
import ProviderLogo from "../../shared/ProviderLogo";
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
  const [selectedType, setSelectedType] = useState<"catalog" | "hf" | null>(null);
  const [selectedCatalogId, setSelectedCatalogId] = useState<string | null>(null);
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
      return !usingCloudStt && activeStt === id;
    }
    // HF rows only know the repo id, not the downloaded file stem — loose match.
    const searchId = (id.split("/").pop() || id).toLowerCase();
    if (track === "llm") {
      return modes.some((m) => m.model_source === "local" && m.model_id.toLowerCase().includes(searchId));
    }
    return !usingCloudStt && !!activeStt?.toLowerCase().includes(searchId);
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
    if (selectedType === "hf" && selectedHfId) {
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
  }, [selectedType, selectedHfId]);

  const sortStaffPicks = (a: any, b: any) => {
    const getScore = (m: any) => {
      let score = 0;
      if (isModelInUse(m.id, false)) score += 1000;
      if (pinnedSet.has(m.id)) score += 100;
      if (isModelDownloaded(m.id, false)) score += 10;
      return score;
    };
    return getScore(b) - getScore(a);
  };

  const sortHfResults = (a: HfSearchItem, b: HfSearchItem) => {
    const getScore = (m: HfSearchItem) => {
      let score = 0;
      if (isModelInUse(m.id, true)) score += 1000;
      if (pinnedSet.has(m.id)) score += 100;
      if (isModelDownloaded(m.id, true)) score += 10;
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
    <div className="flex h-[calc(100vh-140px)] w-full gap-4">
      {/* Left Pane - Search & List */}
      <div className="flex w-1/3 min-w-[320px] flex-col overflow-hidden rounded-xl border border-sv-border bg-sv-surface">
        <div className="flex flex-col gap-2 border-b border-sv-border p-3">
          <input
            type="text"
            placeholder="Search models on Hugging Face..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm placeholder:text-sv-muted focus:border-sv-accent focus:outline-none"
          />
          <div className="flex items-center justify-between">
            <select
              value={sort}
              onChange={(e) => setSort(e.target.value)}
              className="rounded-lg border border-sv-border bg-sv-bg px-2 py-1 text-xs text-sv-text"
            >
              <option value="downloads">Best Match / Downloads</option>
              <option value="likes">Most Likes</option>
              <option value="trending">Trending</option>
              <option value="lastModified">Recently Updated</option>
            </select>
            <label className="flex items-center gap-1.5 text-xs text-sv-muted cursor-pointer">
              <input 
                type="checkbox" 
                checked={hfShowIncompatible}
                onChange={(e) => setSettings({ hf_show_incompatible: e.target.checked })}
                className="rounded border-sv-border bg-sv-surface-2 text-sv-accent focus:ring-sv-accent"
              />
              Show incompatible
            </label>
          </div>
        </div>
        
        <div className="flex-1 overflow-y-auto p-2">
          {!debouncedQuery && (
            <div className="mb-4 flex flex-col gap-1">
              <h3 className="px-2 pb-1 text-xs font-medium uppercase tracking-wide text-sv-muted">Staff Picks</h3>
              {staffPicksSorted.map((m: any) => {
                const isSelected = selectedType === "catalog" && selectedCatalogId === m.id;
                const inUse = isModelInUse(m.id, false);
                const fit = track === "llm" ? llmCompatibility(m, hardware).level : getFit(m.ram_mb * 1024 * 1024, hardware);
                return (
                  <button
                    key={m.id}
                    onClick={() => { setSelectedType("catalog"); setSelectedCatalogId(m.id); }}
                    className={`flex items-start gap-3 rounded-lg p-2.5 text-left transition ${
                      isSelected ? "bg-sv-accent/10 ring-1 ring-sv-accent/40" : "hover:bg-sv-surface-2"
                    }`}
                  >
                    <ProviderLogo provider={m.provider} size={32} />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        {fit && <span className={`shrink-0 h-2 w-2 rounded-full ${(FIT_DOT as any)[fit]}`} />}
                        <h4 className="truncate text-sm font-medium">{m.name || m.label}</h4>
                      </div>
                      <p className="mt-0.5 truncate text-[11px] text-sv-muted">
                        {m.provider} · {m.params || m.speed_label} · {formatMB(m.size_mb)}
                      </p>
                      <div className="mt-1 flex flex-wrap items-center gap-1.5">
                        <span className="rounded-full bg-sv-accent/10 px-1.5 py-0.5 text-[9px] font-medium text-sv-accent">
                          Staff Pick
                        </span>
                        {inUse && <span className="rounded-full bg-sv-good/10 px-1.5 py-0.5 text-[9px] font-medium text-sv-good">In use</span>}
                        {pinnedSet.has(m.id) && <span className="text-[10px] text-sv-accent">★ Pinned</span>}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          )}

          <div className="flex flex-col gap-1">
            {!debouncedQuery && <h3 className="px-2 pb-1 pt-2 text-xs font-medium uppercase tracking-wide text-sv-muted">Trending on Hugging Face</h3>}
            
            {loadingSearch ? (
              <div className="flex flex-col gap-2 p-2">
                {[1, 2, 3, 4].map((i) => (
                  <div key={i} className="flex h-[72px] animate-pulse items-center gap-3 rounded-lg bg-sv-surface-2 p-2.5">
                    <div className="h-8 w-8 rounded-full bg-sv-border"></div>
                    <div className="flex-1 space-y-2">
                      <div className="h-3 w-3/4 rounded bg-sv-border"></div>
                      <div className="h-2 w-1/2 rounded bg-sv-border"></div>
                    </div>
                  </div>
                ))}
              </div>
            ) : searchError ? (
              <div className="p-2 text-sm text-sv-muted text-center">{!debouncedQuery ? "Hugging Face unreachable" : searchError}</div>
            ) : hfResultsVisible.length === 0 ? (
              <div className="p-4 text-center text-sm text-sv-muted">No models found.</div>
            ) : (
              hfResultsVisible.map((item) => {
                const isSelected = selectedType === "hf" && selectedHfId === item.id;
                const [owner, name] = item.id.split("/");
                const isVision = item.tags?.includes("vision") || item.tags?.includes("multimodal") || item.pipeline_tag === "image-text-to-text";
                const isToolUse = item.tags?.includes("tool-use") || item.tags?.includes("function-calling");
                const isReasoning = item.tags?.includes("reasoning") || item.tags?.includes("thinking");
                const inUse = isModelInUse(item.id, true);
                const fit = estimateFitFromParams(item.params_b, hardware, name);

                return (
                  <button
                    key={item.id}
                    onClick={() => { setSelectedType("hf"); setSelectedHfId(item.id); }}
                    className={`flex items-start gap-3 rounded-lg p-2.5 text-left transition ${
                      isSelected ? "bg-sv-accent/10 ring-1 ring-sv-accent/40" : "hover:bg-sv-surface-2"
                    }`}
                  >
                    <ProviderLogo provider={owner} size={32} />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        {fit && <span className={`shrink-0 h-2 w-2 rounded-full ${(FIT_DOT as any)[fit]}`} />}
                        <h4 className="truncate text-sm font-medium" title={name}>{name}</h4>
                      </div>
                      <p className="mt-0.5 truncate text-[11px] text-sv-muted" title={owner}>
                        {owner} · ↓{item.downloads.toLocaleString()} · {formatNdaysAgo(item.last_modified)}
                      </p>
                      <div className="mt-1.5 flex flex-wrap gap-1 items-center">
                        {inUse && <span className="rounded bg-sv-good/10 px-1 py-0.5 text-[9px] text-sv-good">In use</span>}
                        {isVision && <span className="rounded bg-sv-surface-2 px-1 py-0.5 text-[9px] text-sv-muted">Vision</span>}
                        {isToolUse && <span className="rounded bg-sv-surface-2 px-1 py-0.5 text-[9px] text-sv-muted">Tools</span>}
                        {isReasoning && <span className="rounded bg-sv-surface-2 px-1 py-0.5 text-[9px] text-sv-muted">Think</span>}
                      </div>
                    </div>
                  </button>
                );
              })
            )}
          </div>
        </div>
      </div>

      {/* Right Pane - Details */}
      <div className="flex flex-1 flex-col overflow-hidden rounded-xl border border-sv-border bg-sv-surface">
        {!selectedType ? (
          <div className="flex h-full items-center justify-center p-6 text-center text-sm text-sv-muted">
            Select a model from the list to view details.
          </div>
        ) : selectedType === "catalog" && selectedCatalogId ? (
          track === "stt" ? (
            <SttCatalogDetail 
              model={STT_MODELS.find(m => m.id === selectedCatalogId)!} 
              hardware={hardware} 
              pinned={pinnedSet.has(selectedCatalogId)}
              onTogglePin={() => togglePinned(selectedCatalogId)}
            />
          ) : (
            <CatalogDetail 
              model={LLM_MODELS.find(m => m.id === selectedCatalogId)!} 
              hardware={hardware} 
              pinned={pinnedSet.has(selectedCatalogId)}
              onTogglePin={() => togglePinned(selectedCatalogId)}
            />
          )
        ) : selectedType === "hf" && selectedHfId ? (
          loadingDetails ? (
            <div className="flex h-full items-center justify-center">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-sv-accent border-t-transparent"></div>
            </div>
          ) : hfDetailsError ? (
            <div className="flex h-full items-center justify-center p-6 text-center text-sm text-sv-bad">
              {hfDetailsError}
            </div>
          ) : hfDetails ? (
            <HfDetail 
              details={hfDetails} 
              hardware={hardware} 
              track={track}
            />
          ) : null
        ) : null}
      </div>
    </div>
  );
}

// --- Detail Views ---

function SttCatalogDetail({ 
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
  const downloaded = useModelStore((s) => s.downloaded.has(model.id));
  const progress = useModelStore((s) => s.progress[model.id]);
  const download = useModelStore((s) => s.download);
  const remove = useModelStore((s) => s.remove);

  const activeStt = useSettingsStore((s) => s.settings.active_stt_model);
  const usingCloudStt = useSettingsStore((s) => s.settings.stt_cloud_provider_id);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const isActive = !usingCloudStt && activeStt === model.id;

  const selectStt = (id: string) =>
    setSettings({ active_stt_model: id, stt_cloud_provider_id: null });

  // Use the same fit heuristic as ModelStore uses
  const estRamGb = model.ram_mb / 1024;
  let level = "good";
  if (hardware) {
    if (estRamGb > hardware.available_ram_gb) level = "bad";
    else if (estRamGb > hardware.available_ram_gb * 0.8) level = "warn";
  }

  const isDownloading = progress?.status === "downloading";
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  return (
    <div className="flex h-full flex-col overflow-y-auto p-5">
      <div className="flex items-start gap-4">
        <ProviderLogo provider={model.provider} size={48} />
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-semibold">{model.label}</h2>
            <span className="rounded-full bg-sv-accent/10 px-2 py-0.5 text-[10px] font-medium text-sv-accent">
              Staff Pick
            </span>
            {isActive && (
              <span className="rounded-full bg-sv-accent px-2 py-0.5 text-[10px] font-medium text-white">
                Active
              </span>
            )}
          </div>
          <p className="mt-1 text-sm text-sv-muted">
            {model.provider} · {model.best_for}
          </p>
        </div>
        <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={pinned ? "rounded-lg p-2 transition text-sv-accent" : "rounded-lg p-2 transition text-sv-muted hover:text-sv-accent"}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill={pinned ? "currentColor" : "none"} stroke={pinned ? "none" : "currentColor"} strokeWidth={pinned ? undefined : "1.75"} strokeLinecap="round" strokeLinejoin="round"><path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" /></svg>
        </button>
      </div>

      <div className="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Size</div>
          <div className="mt-1 font-medium">{formatMB(model.size_mb)}</div>
        </div>
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Speed</div>
          <div className="mt-1 font-medium">{model.speed_label.replace("~", "")}</div>
        </div>
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Memory</div>
          <div className="mt-1 font-medium">{model.ram_mb} MB</div>
        </div>
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Languages</div>
          <div className="mt-1 font-medium">{model.multilingual ? "Multi" : "English"}</div>
        </div>
      </div>

      <div className="mt-6 flex flex-col gap-3 rounded-xl border border-sv-border bg-sv-surface-2/50 p-4">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2">
              <span className={`h-2 w-2 rounded-full ${(FIT_DOT as any)[level]}`} />
              <span className="font-medium">Download Model</span>
            </div>
            <p className="mt-1 text-xs text-sv-muted">
              {formatMB(model.size_mb)}
            </p>
          </div>
          <div>
            {isDownloading ? (
              <div className="flex w-32 items-center gap-2">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-sv-surface-2 border border-sv-border">
                  <div className="h-full bg-sv-accent transition-all" style={{ width: `${pct}%` }} />
                </div>
                <span className="w-8 text-right text-xs text-sv-muted">{pct}%</span>
              </div>
            ) : downloaded ? (
              <div className="flex items-center gap-3">
                {isActive ? (
                  <span className="text-xs font-medium text-sv-good">In use</span>
                ) : (
                  <button
                    onClick={() => selectStt(model.id)}
                    className="rounded-lg bg-sv-surface-2 px-3 py-1.5 text-xs font-medium hover:bg-sv-accent hover:text-white"
                  >
                    Select
                  </button>
                )}
                <button
                  onClick={() => remove(model.id)}
                  className="rounded-lg border border-sv-border bg-sv-surface px-3 py-1.5 text-xs text-sv-bad hover:bg-sv-surface-2"
                >
                  Remove
                </button>
              </div>
            ) : (
              <button
                onClick={() => download(model.id)}
                className="rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-white hover:bg-sv-accent-hover"
              >
                Download
              </button>
            )}
          </div>
        </div>
        {progress?.status === "error" && (
          <p className="text-xs text-sv-bad">{progress.error}</p>
        )}
      </div>
    </div>
  );
}

function CatalogDetail({ 
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
  const downloaded = useModelStore((s) => s.downloadedLlm.has(model.id));
  const progress = useModelStore((s) => s.progress[model.id]);
  const download = useModelStore((s) => s.downloadLlm);
  const remove = useModelStore((s) => s.removeLlm);

  const modes = useSettingsStore((s) => s.modes);
  const inUse = modes.some(m => m.model_source === "local" && m.model_id === model.id);

  const level = llmCompatibility(model, hardware).level;
  const isDownloading = progress?.status === "downloading";
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  return (
    <div className="flex h-full flex-col overflow-y-auto p-5">
      <div className="flex items-start gap-4">
        <ProviderLogo provider={model.provider} size={48} />
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-semibold">{model.name}</h2>
            <span className="rounded-full bg-sv-accent/10 px-2 py-0.5 text-[10px] font-medium text-sv-accent">
              Staff Pick
            </span>
          </div>
          <p className="mt-1 text-sm text-sv-muted">
            {model.provider} · {model.best_for}
          </p>
        </div>
        <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={pinned ? "rounded-lg p-2 transition text-sv-accent" : "rounded-lg p-2 transition text-sv-muted hover:text-sv-accent"}>
          <svg viewBox="0 0 24 24" width="20" height="20" fill={pinned ? "currentColor" : "none"} stroke={pinned ? "none" : "currentColor"} strokeWidth={pinned ? undefined : "1.75"} strokeLinecap="round" strokeLinejoin="round"><path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" /></svg>
        </button>
      </div>

      <div className="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Size</div>
          <div className="mt-1 font-medium">{model.params}</div>
        </div>
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Speed</div>
          <div className="mt-1 font-medium">{model.speed_label.replace("~", "")}</div>
        </div>
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Memory</div>
          <div className="mt-1 font-medium">{formatGB(model.ram_gb)}</div>
        </div>
        <div className="rounded-lg bg-sv-surface-2 p-3 text-center">
          <div className="text-[10px] uppercase tracking-wide text-sv-muted">Languages</div>
          <div className="mt-1 font-medium">{model.languages}</div>
        </div>
      </div>

      <div className="mt-6 flex flex-col gap-3 rounded-xl border border-sv-border bg-sv-surface-2/50 p-4">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2">
              <span className={`h-2 w-2 rounded-full ${FIT_DOT[level]}`} />
              <span className="font-medium">Download Model</span>
            </div>
            <p className="mt-1 text-xs text-sv-muted">
              {formatMB(model.size_mb)}
            </p>
          </div>
          <div>
            {isDownloading ? (
              <div className="flex w-32 items-center gap-2">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-sv-surface-2 border border-sv-border">
                  <div className="h-full bg-sv-accent transition-all" style={{ width: `${pct}%` }} />
                </div>
                <span className="w-8 text-right text-xs text-sv-muted">{pct}%</span>
              </div>
            ) : downloaded ? (
              <div className="flex flex-col items-end gap-1">
                <div className="flex items-center gap-3">
                  {inUse ? (
                    <span className="text-xs font-medium text-sv-good">In use</span>
                  ) : (
                    <span className="text-xs font-medium text-sv-good">Installed</span>
                  )}
                  <button
                    onClick={() => remove(model.id)}
                    className="rounded-lg border border-sv-border bg-sv-surface px-3 py-1.5 text-xs text-sv-bad hover:bg-sv-surface-2"
                  >
                    Remove
                  </button>
                </div>
                {!inUse && <span className="text-[10px] text-sv-muted">Assign it to a mode in the Modes tab</span>}
              </div>
            ) : (
              <button
                onClick={() => download(model.id)}
                className="rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-white hover:bg-sv-accent-hover"
              >
                Download
              </button>
            )}
          </div>
        </div>
        {progress?.status === "error" && (
          <p className="text-xs text-sv-bad">{progress.error}</p>
        )}
      </div>
    </div>
  );
}

function HfDetail({ details, hardware, track }: { details: HfModelDetails; hardware: HardwareInfo | null; track: "llm" | "stt" }) {
  const [owner, name] = details.id.split("/");
  const isVision = details.files.some(f => f.name.includes("mmproj")) || details.tags?.includes("vision") || details.tags?.includes("multimodal") || details.pipeline_tag === "image-text-to-text";
  const isToolUse = details.has_tools || details.tags?.includes("tool-use") || details.tags?.includes("function-calling");
  const isReasoning = details.tags?.includes("reasoning") || details.tags?.includes("thinking");

  // Filter and group files
  // Exclude mmproj and non-gguf. Group multi-part into one.
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

  // Derive model ID for storing (use stem of the selected file)
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

  const handleCopyId = () => {
    navigator.clipboard.writeText(details.id);
  };

  const [readmeExpanded, setReadmeExpanded] = useState(false);

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <div className="border-b border-sv-border p-5">
        <div className="flex items-start gap-4">
          <ProviderLogo provider={owner} size={48} />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-3">
              <h2 className="truncate text-xl font-semibold">{name}</h2>
              <button onClick={handleCopyId} className="shrink-0 text-sv-muted hover:text-sv-text" title="Copy Repo ID">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
              </button>
            </div>
            <p className="mt-1 flex items-center gap-3 text-sm text-sv-muted">
              <span>{owner}</span>
              <span>↓ {details.downloads.toLocaleString()}</span>
              <span>♥ {details.likes.toLocaleString()}</span>
              <span>{formatNdaysAgo(details.last_modified)}</span>
            </p>
          </div>
        </div>

        <div className="mt-4 flex flex-wrap gap-2">
          {details.params_b && (
            <span className="rounded bg-sv-surface-2 px-2 py-1 text-[10px] font-medium tracking-wide text-sv-text border border-sv-border">
              PARAMS {parseParams(details.params_b)}
            </span>
          )}
          {details.arch && (
            <span className="rounded bg-sv-surface-2 px-2 py-1 text-[10px] font-medium tracking-wide text-sv-text border border-sv-border">
              ARCH {details.arch.toUpperCase()}
            </span>
          )}
          <span className="rounded bg-sv-surface-2 px-2 py-1 text-[10px] font-medium tracking-wide text-sv-text border border-sv-border">
            {track === "stt" ? "GGML" : "GGUF"}
          </span>
          {details.context_length && (
            <span className="rounded bg-sv-surface-2 px-2 py-1 text-[10px] font-medium tracking-wide text-sv-text border border-sv-border">
              CTX {details.context_length}
            </span>
          )}
          {isVision && (
            <span className="rounded bg-indigo-500/10 px-2 py-1 text-[10px] font-medium tracking-wide text-indigo-500 border border-indigo-500/20">
              VISION
            </span>
          )}
          {isToolUse && (
            <span className="rounded bg-emerald-500/10 px-2 py-1 text-[10px] font-medium tracking-wide text-emerald-500 border border-emerald-500/20">
              TOOL USE
            </span>
          )}
          {isReasoning && (
            <span className="rounded bg-amber-500/10 px-2 py-1 text-[10px] font-medium tracking-wide text-amber-500 border border-amber-500/20">
              REASONING
            </span>
          )}
          {details.gated && (
            <span className="rounded bg-sv-bad/10 px-2 py-1 text-[10px] font-medium tracking-wide text-sv-bad border border-sv-bad/20">
              GATED
            </span>
          )}
        </div>
      </div>

      <div className="p-5">
        {details.gated ? (
          <div className="rounded-xl border border-sv-warn/30 bg-sv-warn/10 p-4 text-sm text-sv-warn">
            <p className="font-medium">This model is gated.</p>
            <p className="mt-1">You need to accept the license agreement on the Hugging Face website before downloading. Downloading via this app is not supported yet for gated repos.</p>
          </div>
        ) : (
          <div className="rounded-xl border border-sv-border bg-sv-surface-2/50 p-4">
            <h3 className="mb-3 text-sm font-medium">Download Options</h3>
            <div className="flex flex-col gap-2">
              <div className="flex flex-col gap-2">
                {availableFiles.map((f, i) => {
                  const fit = getFit(f.size_bytes, hardware);
                  const mId = getModelId(f.name);
                  const isDownloaded = track === "stt" ? downloadedStt.has(mId) : downloadedLlm.has(mId);
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
              </div>

              {selectedFile && !selectedFile.isMultiPart && (() => {
                const fit = getFit(selectedFile.size_bytes, hardware);
                const estSpeed = details.params_b ? (details.params_b <= 4 ? "Fast on this device" : details.params_b <= 9 ? "Usable" : "May be slow") : null;
                const mId = getModelId(selectedFile.name);
                const isDl = track === "stt" ? downloadedStt.has(mId) : downloadedLlm.has(mId);
                const isActiveStt = track === "stt" && !usingCloudStt && activeStt === mId;
                const isActiveLlm = track === "llm" && modes.some(m => m.model_source === "local" && m.model_id === mId);
                const isActive = track === "stt" ? isActiveStt : isActiveLlm;
                const prog = progress[mId];
                const isDownloading = prog?.status === "downloading";
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
                      {isDownloading ? (
                        <div className="flex w-32 items-center gap-2">
                          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-sv-surface-2 border border-sv-border">
                            <div className="h-full bg-sv-accent transition-all" style={{ width: `${pct}%` }} />
                          </div>
                          <span className="w-8 text-right text-xs text-sv-muted">{pct}%</span>
                        </div>
                      ) : isDl ? (
                        <div className="flex flex-col items-end gap-1">
                          <div className="flex items-center gap-3">
                            {isActive ? (
                              <span className="text-xs font-medium text-sv-good">In use</span>
                            ) : (
                              track === "stt" ? (
                                <button onClick={() => selectStt(mId)} className="rounded-lg bg-sv-surface-2 px-3 py-1.5 text-xs font-medium hover:bg-sv-accent hover:text-white">
                                  Select
                                </button>
                              ) : (
                                <span className="text-xs font-medium text-sv-good">Installed</span>
                              )
                            )}
                            <button onClick={() => track === "stt" ? removeStt(mId) : removeLlm(mId)} className="rounded-lg border border-sv-border bg-sv-surface px-3 py-1.5 text-xs text-sv-bad hover:bg-sv-surface-2">
                              Remove
                            </button>
                          </div>
                          {!isActive && track === "llm" && <span className="text-[10px] text-sv-muted">Assign it to a mode in the Modes tab</span>}
                        </div>
                      ) : (
                        <button onClick={doDownload} className="rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-white hover:bg-sv-accent-hover">
                          Download
                        </button>
                      )}
                    </div>
                  </div>
                );
              })()}
            </div>
          </div>
        )}
      </div>

      {details.readme && (
        <div className="px-5 pb-5">
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
        </div>
      )}
    </div>
  );
}
