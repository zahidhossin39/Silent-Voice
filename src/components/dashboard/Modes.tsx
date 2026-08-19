import { useState, useEffect, useRef } from "react";
import { Link } from "react-router-dom";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { llmCompatibility } from "../../services/recommend";
import Page from "../shared/Page";
import Select from "../shared/Select";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { localLlmGenerate, apiGenerate } from "../../services/tauriBridge";
import { LLM_MODELS } from "../../services/catalog";
import { formatGB } from "../../services/format";
import { SAMPLE_INPUT } from "../../services/modes";
import type { HardwareInfo, LlmModel, Mode, ModelSource } from "../../types";

const EMPTY: Mode = {
  id: "",
  name: "",
  icon: "sparkles",
  system_prompt: "",
  model_source: "local",
  model_id: "llama-3.2-1b-instruct-q4",
  builtin: false,
};

export default function Modes() {
  const modes = useSettingsStore((s) => s.modes);
  const activeId = useSettingsStore((s) => s.settings.active_mode_id);
  const providers = useSettingsStore((s) => s.providers);
  const setActiveMode = useSettingsStore((s) => s.setActiveMode);
  const addMode = useSettingsStore((s) => s.addMode);
  const updateMode = useSettingsStore((s) => s.updateMode);
  const deleteMode = useSettingsStore((s) => s.deleteMode);
  const togglePinMode = useSettingsStore((s) => s.togglePinMode);
  const resetMode = useSettingsStore((s) => s.resetMode);
  const downloadedLlm = useModelStore((s) => s.downloadedLlm);
  const customLlm = useModelStore((s) => s.customLlm);

  const [editing, setEditing] = useState<Mode | null>(null);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [runningPreviews, setRunningPreviews] = useState<Record<string, boolean>>({});
  const localCount = downloadedLlm.size;

  async function runPreview(m: Mode) {
    if (previews[m.id] || runningPreviews[m.id]) return;
    setRunningPreviews((prev) => ({ ...prev, [m.id]: true }));
    try {
      let out: string;
      if (m.model_source === "api") {
        const p = providers.find((p) => p.id === m.provider_id);
        if (!p) throw new Error("No provider");
        out = await apiGenerate(p.base_url, p.api_key, p.model, m.system_prompt, SAMPLE_INPUT);
      } else {
        out = await localLlmGenerate(m.model_id, m.system_prompt, SAMPLE_INPUT);
      }
      setPreviews((prev) => ({ ...prev, [m.id]: out }));
    } catch (e) {
      setPreviews((prev) => ({ ...prev, [m.id]: `Error: ${e}` }));
    } finally {
      setRunningPreviews((prev) => ({ ...prev, [m.id]: false }));
    }
  }

  function save() {
    if (!editing) return;
    if (!editing.name.trim()) return;
    if (modes.some((m) => m.id === editing.id)) {
      // Editing a built-in marks it customized so a future prompt fix
      // shipped in BUILTIN_MODES doesn't silently overwrite the user's edit.
      updateMode(editing.id, editing.builtin ? { ...editing, customized: true } : editing);
    } else {
      addMode({ ...editing, id: editing.id || `custom_${Date.now()}` });
    }
    setEditing(null);
  }

  const sortedModes = [...modes].sort((a, b) => {
    // 1. Active mode always first
    if (a.id === activeId) return -1;
    if (b.id === activeId) return 1;

    // 2. Pinned modes next
    if (!!a.pinned !== !!b.pinned) return a.pinned ? -1 : 1;

    // 3. Custom modes (newest first)
    const getTimestamp = (id: string) => {
      if (id.startsWith("custom_")) {
        const tsStr = id.split("_")[1];
        const ts = parseInt(tsStr, 10);
        return isNaN(ts) ? 0 : ts;
      }
      return 0;
    };

    const tsA = getTimestamp(a.id);
    const tsB = getTimestamp(b.id);

    if (tsA !== tsB) {
      return tsB - tsA;
    }

    return 0;
  });

  return (
        <Page
          title="Writing styles"
          subtitle="Pick what the AI does to your words before they land at the cursor."
          actions={
            <button
              onClick={() => setEditing({ ...EMPTY })}
              className="rounded-lg bg-sv-accent px-3 py-1.5 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover"
            >
              + New mode
            </button>
          }
        >
          {/* Local engine status. */}
          <div
            className={`mb-5 flex items-center justify-between rounded-lg border px-4 py-3 text-xs ${
              localCount > 0
                ? "border-sv-good/30 bg-sv-good/10 text-sv-good"
                : "border-sv-warn/30 bg-sv-warn/10 text-sv-warn"
            }`}
          >
            <span>
              {localCount > 0 ? (
                <>
                  ✓ Built-in AI engine ready — {localCount} local model
                  {localCount === 1 ? "" : "s"} downloaded. AI modes run on your
                  device.
                </>
              ) : (
                <>
                  ⚠ No local AI model yet. AI modes fall back to raw transcription.{" "}
                  <Link to="/models" className="underline">
                    Download one in Model Store
                  </Link>{" "}
                  (AI Processing tab), or use a cloud provider in Cloud providers.
                </>
              )}
            </span>
          </div>

          <div className="grid gap-3 md:grid-cols-2">
            {sortedModes.map((m) => (
              <div
                key={m.id}
                className={`rounded-xl border p-4 ${
                  activeId === m.id
                    ? "border-sv-accent/40 bg-sv-accent/5"
                    : m.pinned
                    ? "border-sv-border bg-sv-surface ring-1 ring-sv-accent/15"
                    : "border-sv-border bg-sv-surface"
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <h3 className="font-medium">{m.name}</h3>
                    <p className="mt-0.5 line-clamp-2 text-xs text-sv-muted">
                      {m.description || m.system_prompt}
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-1.5">
                    {m.builtin && (
                      <span className="whitespace-nowrap rounded border border-sv-border/50 bg-sv-surface-2 px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wider text-sv-muted">
                        {m.customized ? "edited" : "built-in"}
                      </span>
                    )}
                    <button
                      onClick={() => togglePinMode(m.id)}
                      title={m.pinned ? "Unpin" : "Pin to top"}
                      className={`transition-colors duration-75 ${
                        m.pinned ? "text-sv-accent" : "text-sv-muted/70 hover:text-sv-accent"
                      }`}
                    >
                      <svg
                        viewBox="0 0 24 24"
                        width="15"
                        height="15"
                        fill={m.pinned ? "currentColor" : "none"}
                        stroke={m.pinned ? "none" : "currentColor"}
                        strokeWidth={m.pinned ? undefined : "1.75"}
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <path d="M12 2.5l2.9 6.2 6.6.6-5 4.6 1.4 6.6L12 17l-5.9 3.5L7.5 14l-5-4.6 6.6-.6L12 2.5z" />
                      </svg>
                    </button>
                  </div>
                </div>
                {previews[m.id] && (
                  <div className="mt-3 rounded-lg bg-sv-surface-2 p-3 text-xs">
                    <p className="mb-1 text-[11px] italic text-sv-muted">"{SAMPLE_INPUT}"</p>
                    <p>
                      <span className="text-sv-muted mr-1">Example:</span>
                      {previews[m.id]}
                    </p>
                  </div>
                )}
                <div className="mt-3 flex items-center gap-2 text-xs">
                  {activeId === m.id ? (
                    <span className="rounded bg-sv-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-sv-accent">
                      Active
                    </span>
                  ) : (
                    <button
                      onClick={() => setActiveMode(m.id)}
                      className="rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-1.5 text-xs font-medium text-sv-text transition-colors duration-75 hover:border-sv-accent hover:text-sv-accent"
                    >
                      Use
                    </button>
                  )}
                  <button
                    onClick={() => setEditing({ ...m })}
                    className="rounded-lg border border-transparent px-2.5 py-1.5 text-xs font-medium text-sv-muted transition-colors duration-75 hover:border-sv-border hover:text-sv-text"
                  >
                    Edit
                  </button>
                  {!m.builtin && (
                    <button
                      onClick={() => deleteMode(m.id)}
                      className="rounded-lg border border-transparent px-2.5 py-1.5 text-xs font-medium text-sv-muted transition-colors duration-75 hover:border-sv-bad/40 hover:bg-sv-bad/10 hover:text-sv-bad"
                    >
                      Delete
                    </button>
                  )}
                  {m.builtin && m.customized && (
                    <button
                      onClick={() => resetMode(m.id)}
                      title="Discard your edits and restore the original"
                      className="rounded-lg border border-transparent px-2.5 py-1.5 text-xs font-medium text-sv-muted transition-colors duration-75 hover:border-sv-border hover:text-sv-text"
                    >
                      Reset
                    </button>
                  )}
                  {m.model_source !== "none" &&
                   ((m.model_source === "local" && downloadedLlm.has(m.model_id)) ||
                    (m.model_source === "api" && providers.some(p => p.id === m.provider_id))) && (
                    <button
                      onClick={() => runPreview(m)}
                      className="ml-auto rounded-lg border border-transparent px-2.5 py-1.5 text-xs font-medium text-sv-muted transition-colors duration-75 hover:border-sv-border hover:text-sv-text"
                    >
                      {runningPreviews[m.id] ? "Running…" : "Preview"}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>

      {editing && (
        <Editor
          mode={editing}
          onChange={setEditing}
          onSave={save}
          onCancel={() => setEditing(null)}
          downloadedLlm={downloadedLlm}
          customLlm={customLlm}
        />
      )}
    </Page>
  );
}

function Editor({
  mode,
  onChange,
  onSave,
  onCancel,
  downloadedLlm,
  customLlm,
}: {
  mode: Mode;
  onChange: (m: Mode) => void;
  onSave: () => void;
  onCancel: () => void;
  downloadedLlm: Set<string>;
  customLlm: LlmModel[];
}) {
  const providers = useSettingsStore((s) => s.providers);
  const [testing, setTesting] = useState(false);
  const [testOut, setTestOut] = useState<string | null>(null);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onCancel();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  const provider =
    mode.model_source === "api"
      ? providers.find((p) => p.id === mode.provider_id)
      : undefined;

  // Local models the user has downloaded (joined with catalog for names).
  const localModels = [
    ...LLM_MODELS.filter((m) => downloadedLlm.has(m.id)),
    ...customLlm.filter((m) => downloadedLlm.has(m.id)),
  ];
  const localReady =
    mode.model_source === "local" && downloadedLlm.has(mode.model_id);

  async function runTest() {
    setTesting(true);
    setTestOut(null);
    try {
      let out: string;
      if (mode.model_source === "api") {
        if (!provider) throw new Error("Pick a provider first (Cloud providers tab)");
        out = await apiGenerate(
          provider.base_url,
          provider.api_key,
          provider.model,
          mode.system_prompt,
          SAMPLE_INPUT
        );
      } else {
        out = await localLlmGenerate(
          mode.model_id,
          mode.system_prompt,
          SAMPLE_INPUT
        );
      }
      setTestOut(out);
    } catch (e) {
      setTestOut(`Error: ${e}`);
    } finally {
      setTesting(false);
    }
  }

  return (
    // items-start + overflow-y-auto + my-auto: stays centred when it fits, and
    // becomes scrollable when it doesn't. With items-center the panel overflowed
    // equally out of both ends of the fixed box, putting Save out of reach at
    // the app's own minimum window height (560px).
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/60 p-4"
      onClick={onCancel}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="mode-editor-title"
        onClick={(e) => e.stopPropagation()}
        className="my-auto w-full max-w-lg rounded-xl border border-sv-border bg-sv-surface p-5"
      >
        <h2 id="mode-editor-title" className="mb-1 text-lg font-semibold">
          {mode.id ? "Edit mode" : "New mode"}
        </h2>
        {mode.builtin && (
          <p className="mb-4 text-[11px] text-sv-muted">
            This is a built-in mode. Edit it freely — your changes are kept even after
            app updates, and you can Reset it back to the original anytime.
          </p>
        )}
        <label className="mb-3 mt-4 block text-sm">
          <span className="mb-1 block text-sv-muted">Name</span>
          <input
            value={mode.name}
            onChange={(e) => onChange({ ...mode, name: e.target.value })}
            className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2"
          />
        </label>
        <label className="mb-3 block text-sm">
          <span className="mb-1 block text-sv-muted">Instructions sent to the AI</span>
          <textarea
            value={mode.system_prompt}
            rows={5}
            onChange={(e) =>
              onChange({ ...mode, system_prompt: e.target.value })
            }
            className="w-full resize-none rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
          />
        </label>
        <div className="mb-4 space-y-3 text-sm">
          <label className="block">
            <span className="mb-1 block text-sv-muted">Processing</span>
            <Select
              value={mode.model_source}
              onChange={(v) =>
                onChange({ ...mode, model_source: v as ModelSource })
              }
              className="w-full"
            >
              <option value="none">None (raw transcription)</option>
              <option value="local">Built-in (on-device model)</option>
              <option value="api">Cloud / API provider</option>
            </Select>
          </label>
          {mode.model_source === "api" ? (
            <label className="block">
              <span className="mb-1 block text-sv-muted">Provider</span>
              <Select
                value={mode.provider_id ?? ""}
                onChange={(v) => onChange({ ...mode, provider_id: v })}
                className="w-full"
              >
                <option value="">Select provider…</option>
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} ({p.model})
                  </option>
                ))}
              </Select>
            </label>
          ) : mode.model_source === "local" ? (
            <div>
              <span className="mb-1 block text-sv-muted">Local model</span>
              <ModelPicker
                value={mode.model_id}
                onChange={(id) => onChange({ ...mode, model_id: id })}
                models={localModels}
                hasUndownloaded={LLM_MODELS.some((m) => !downloadedLlm.has(m.id))}
              />
            </div>
          ) : null}
        </div>

        {mode.model_source === "local" && !localReady && (
          <p className="mb-3 text-[11px] text-sv-warn">
            This model isn’t downloaded. Get it in Model Store → AI
            Processing, or this mode will paste raw text.
          </p>
        )}
        {mode.model_source === "api" && providers.length === 0 && (
          <p className="mb-3 text-[11px] text-sv-warn">
            No providers yet — add one in the Cloud providers tab.
          </p>
        )}

        {/* Live test on a noisy sample sentence. */}
        {(mode.model_source === "local" || mode.model_source === "api") && (
          <div className="mb-4 rounded-lg border border-sv-border bg-sv-bg p-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="text-xs text-sv-muted">Test on sample speech</span>
              <button
                onClick={runTest}
                disabled={
                  testing ||
                  (mode.model_source === "local" && !localReady) ||
                  (mode.model_source === "api" && !provider)
                }
                className="rounded-lg bg-sv-accent px-3 py-1 text-xs font-medium text-sv-on-accent hover:bg-sv-accent-hover disabled:opacity-50"
              >
                {testing ? "Running…" : "▶ Test"}
              </button>
            </div>
            <p className="mb-2 text-[11px] italic text-sv-muted">“{SAMPLE_INPUT}”</p>
            {testing && mode.model_source === "local" && (
              <p className="text-[11px] text-sv-muted">
                First run loads the model into memory — can take a moment.
              </p>
            )}
            {testOut !== null && (
              <div className="rounded-lg bg-sv-surface-2 p-2 text-xs">
                {testOut}
              </div>
            )}
          </div>
        )}

        <div className="flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="rounded-lg border border-sv-border px-3 py-1.5 text-sm text-sv-muted hover:text-sv-text"
          >
            Close
          </button>
          <button
            onClick={onSave}
            disabled={!mode.name.trim()}
            title={mode.name.trim() ? undefined : "Give this mode a name first"}
            className="rounded-lg bg-sv-accent px-3 py-1.5 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}

const FIT_DOT: Record<string, string> = {
  good: "bg-sv-good",
  warn: "bg-sv-warn",
  bad: "bg-sv-bad",
};

// Picks which downloaded model powers a mode. Only downloaded models are
// listed: the old control padded the list with every un-downloaded catalog
// entry as a disabled row, so most of what it showed could not be chosen.
// Each row carries the same device-fit dot and RAM figure the Model Store
// uses, because "will this run well on my machine" is the actual decision.
function ModelPicker({
  value,
  onChange,
  models,
  hasUndownloaded,
}: {
  value: string;
  onChange: (id: string) => void;
  models: LlmModel[];
  hasUndownloaded: boolean;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const { hardware } = useHardwareInfo();

  useEffect(() => {
    if (!open) return;
    function onDocDown(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    window.addEventListener("mousedown", onDocDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDocDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  // A dropdown whose only entry is "(none downloaded yet)" is a dead control.
  // Send the user where the problem is actually solved instead.
  if (models.length === 0) {
    return (
      <div className="rounded-lg border border-sv-warn/30 bg-sv-warn/10 px-3 py-2.5 text-xs text-sv-warn">
        No on-device models yet.{" "}
        <Link to="/models" className="font-medium underline">
          Download one in Model Store
        </Link>
        , or switch Processing to a cloud provider.
      </div>
    );
  }

  const selected = models.find((m) => m.id === value);

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        className={`flex w-full items-center gap-2.5 rounded-lg border bg-sv-bg px-3 py-2 text-left transition-colors ${
          open ? "border-sv-accent" : "border-sv-border"
        }`}
      >
        {selected ? (
          <>
            <FitDot model={selected} hardware={hardware} />
            <span className="min-w-0 flex-1 truncate text-sv-text">
              {selected.name}
            </span>
            <span className="shrink-0 tabular-nums text-[11px] text-sv-muted">
              {selected.params} · {formatGB(selected.ram_gb)} RAM
            </span>
          </>
        ) : (
          <span className="flex-1 text-sv-muted">Select a model…</span>
        )}
        <svg
          viewBox="0 0 24 24"
          width="12"
          height="12"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={`shrink-0 text-sv-muted transition-transform ${open ? "rotate-180" : ""}`}
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      {open && (
        <div
          role="listbox"
          className="absolute left-0 right-0 top-[calc(100%+4px)] z-50 max-h-64 overflow-y-auto rounded-lg border border-sv-border bg-sv-surface p-1 shadow-xl"
        >
          {models.map((m) => {
            const isSelected = m.id === value;
            return (
              <button
                key={m.id}
                type="button"
                role="option"
                aria-selected={isSelected}
                onClick={() => {
                  onChange(m.id);
                  setOpen(false);
                }}
                className={`flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left transition-colors ${
                  isSelected ? "bg-sv-accent/15" : "hover:bg-sv-surface-2"
                }`}
              >
                <FitDot model={m} hardware={hardware} />
                <span
                  className={`min-w-0 flex-1 truncate ${isSelected ? "text-sv-accent" : "text-sv-text"}`}
                >
                  {m.name}
                </span>
                <span className="shrink-0 tabular-nums text-[11px] text-sv-muted">
                  {m.params} · {formatGB(m.ram_gb)} RAM
                </span>
              </button>
            );
          })}
          {hasUndownloaded && (
            <>
              <div className="my-1 border-t border-sv-border" />
              <Link
                to="/models"
                className="block rounded-md px-2.5 py-2 text-xs text-sv-muted transition-colors hover:bg-sv-surface-2 hover:text-sv-text"
              >
                Get more models in Model Store →
              </Link>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function FitDot({
  model,
  hardware,
}: {
  model: LlmModel;
  hardware: HardwareInfo | null;
}) {
  const fit = llmCompatibility(model, hardware);
  return (
    <span
      title={fit.reason}
      className={`h-2 w-2 shrink-0 rounded-full ${FIT_DOT[fit.level]}`}
    />
  );
}
