import { useState } from "react";
import { Link } from "react-router-dom";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { localLlmGenerate, apiGenerate } from "../../services/tauriBridge";
import { LLM_MODELS } from "../../services/catalog";
import type { Mode, ModelSource } from "../../types";

const SAMPLE_TEXT =
  "um so i was thinking like maybe we could uh meet tomorrow you know to go over the the budget stuff";

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
  const setActiveMode = useSettingsStore((s) => s.setActiveMode);
  const addMode = useSettingsStore((s) => s.addMode);
  const updateMode = useSettingsStore((s) => s.updateMode);
  const deleteMode = useSettingsStore((s) => s.deleteMode);
  const downloadedLlm = useModelStore((s) => s.downloadedLlm);

  const [editing, setEditing] = useState<Mode | null>(null);
  const localCount = downloadedLlm.size;

  function save() {
    if (!editing) return;
    if (!editing.name.trim()) return;
    if (modes.some((m) => m.id === editing.id)) {
      updateMode(editing.id, editing);
    } else {
      addMode({ ...editing, id: editing.id || `custom_${Date.now()}` });
    }
    setEditing(null);
  }

  const sortedModes = [...modes].sort((a, b) => {
    // 1. Active mode always first
    if (a.id === activeId) return -1;
    if (b.id === activeId) return 1;

    // 2. Custom modes (newest first)
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
          title="Modes"
          subtitle="How transcribed text is processed before pasting"
          actions={
            <button
              onClick={() => setEditing({ ...EMPTY })}
              className="rounded-lg bg-sv-accent px-3 py-1.5 text-sm font-medium text-white hover:bg-sv-accent-hover"
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
                    Download one in the Model Store
                  </Link>{" "}
                  (AI Processing tab), or use a cloud provider in API Keys.
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
                    ? "border-sv-accent bg-sv-accent/5"
                    : "border-sv-border bg-sv-surface"
                }`}
              >
                <div className="flex items-start justify-between">
                  <div>
                    <h3 className="font-medium">{m.name}</h3>
                    <p className="mt-0.5 line-clamp-2 text-xs text-sv-muted">
                      {m.model_source === "none"
                        ? "Pastes exactly what was said."
                        : m.system_prompt}
                    </p>
                  </div>
                  {m.builtin && (
                    <span className="whitespace-nowrap shrink-0 rounded border border-sv-border/50 bg-sv-surface-2 px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wider text-sv-muted">
                      built-in
                    </span>
                  )}
                </div>
                <div className="mt-3 flex items-center gap-2 text-xs">
                  <button
                    onClick={() => setActiveMode(m.id)}
                    disabled={activeId === m.id}
                    className={`rounded-lg px-2.5 py-1 ${
                      activeId === m.id
                        ? "bg-sv-good/15 text-sv-good"
                        : "bg-sv-accent text-white hover:bg-sv-accent-hover"
                    }`}
                  >
                    {activeId === m.id ? "✓ Active" : "Set active"}
                  </button>
                  <button
                    onClick={() => setEditing({ ...m })}
                    className="rounded-lg border border-sv-border px-2.5 py-1 text-sv-muted hover:text-sv-text"
                  >
                    Edit
                  </button>
                  <button
                    onClick={() => deleteMode(m.id)}
                    className="rounded-lg border border-sv-border px-2.5 py-1 text-sv-muted hover:text-sv-bad"
                  >
                    Delete
                  </button>
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
}: {
  mode: Mode;
  onChange: (m: Mode) => void;
  onSave: () => void;
  onCancel: () => void;
  downloadedLlm: Set<string>;
}) {
  const readonly = false;
  const providers = useSettingsStore((s) => s.providers);
  const [testing, setTesting] = useState(false);
  const [testOut, setTestOut] = useState<string | null>(null);

  const provider =
    mode.model_source === "api"
      ? providers.find((p) => p.id === mode.provider_id)
      : undefined;

  // Local models the user has downloaded (joined with catalog for names).
  const localModels = LLM_MODELS.filter((m) => downloadedLlm.has(m.id));
  const localReady =
    mode.model_source === "local" && downloadedLlm.has(mode.model_id);

  async function runTest() {
    setTesting(true);
    setTestOut(null);
    try {
      let out: string;
      if (mode.model_source === "api") {
        if (!provider) throw new Error("Pick a provider first (API Keys tab)");
        out = await apiGenerate(
          provider.base_url,
          provider.api_key,
          provider.model,
          mode.system_prompt,
          SAMPLE_TEXT
        );
      } else {
        out = await localLlmGenerate(
          mode.model_id,
          mode.system_prompt,
          SAMPLE_TEXT
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-lg rounded-xl border border-sv-border bg-sv-surface p-5">
        <h2 className="mb-4 text-lg font-semibold">
          {readonly ? mode.name : mode.id ? "Edit mode" : "New mode"}
        </h2>
        <label className="mb-3 block text-sm">
          <span className="mb-1 block text-sv-muted">Name</span>
          <input
            value={mode.name}
            disabled={readonly}
            onChange={(e) => onChange({ ...mode, name: e.target.value })}
            className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 disabled:opacity-60"
          />
        </label>
        <label className="mb-3 block text-sm">
          <span className="mb-1 block text-sv-muted">System prompt</span>
          <textarea
            value={mode.system_prompt}
            disabled={readonly}
            rows={5}
            onChange={(e) =>
              onChange({ ...mode, system_prompt: e.target.value })
            }
            className="w-full resize-none rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm disabled:opacity-60"
          />
        </label>
        <div className="mb-4 grid grid-cols-2 gap-3 text-sm">
          <label>
            <span className="mb-1 block text-sv-muted">Processing</span>
            <select
              value={mode.model_source}
              disabled={readonly}
              onChange={(e) =>
                onChange({ ...mode, model_source: e.target.value as ModelSource })
              }
              className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 disabled:opacity-60"
            >
              <option value="none">None (raw transcription)</option>
              <option value="local">Built-in (on-device model)</option>
              <option value="api">Cloud / API provider</option>
            </select>
          </label>
          {mode.model_source === "api" ? (
            <label>
              <span className="mb-1 block text-sv-muted">Provider</span>
              <select
                value={mode.provider_id ?? ""}
                disabled={readonly}
                onChange={(e) =>
                  onChange({ ...mode, provider_id: e.target.value })
                }
                className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 disabled:opacity-60"
              >
                <option value="">Select provider…</option>
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} ({p.model})
                  </option>
                ))}
              </select>
            </label>
          ) : mode.model_source === "local" ? (
            <label>
              <span className="mb-1 block text-sv-muted">Local model</span>
              <select
                value={mode.model_id}
                disabled={readonly}
                onChange={(e) => onChange({ ...mode, model_id: e.target.value })}
                className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 disabled:opacity-60"
              >
                {localModels.length === 0 && (
                  <option value={mode.model_id}>
                    (none downloaded yet)
                  </option>
                )}
                {localModels.map((m) => (
                  <option key={m.id} value={m.id}>
                    ✓ {m.name} ({m.params})
                  </option>
                ))}
                {LLM_MODELS.filter((m) => !downloadedLlm.has(m.id)).map((m) => (
                  <option key={m.id} value={m.id} disabled>
                    {m.name} ({m.params}) — not downloaded
                  </option>
                ))}
              </select>
            </label>
          ) : (
            <div />
          )}
        </div>

        {mode.model_source === "local" && !localReady && (
          <p className="mb-3 text-[11px] text-sv-warn">
            This model isn’t downloaded. Get it in the Model Store → AI
            Processing, or this mode will paste raw text.
          </p>
        )}
        {mode.model_source === "api" && providers.length === 0 && (
          <p className="mb-3 text-[11px] text-sv-warn">
            No providers yet — add one in the API Keys tab.
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
                className="rounded-lg bg-sv-accent px-3 py-1 text-xs font-medium text-white hover:bg-sv-accent-hover disabled:opacity-50"
              >
                {testing ? "Running…" : "▶ Test"}
              </button>
            </div>
            <p className="mb-2 text-[11px] italic text-sv-muted">“{SAMPLE_TEXT}”</p>
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
          {!readonly && (
            <button
              onClick={onSave}
              className="rounded-lg bg-sv-accent px-3 py-1.5 text-sm font-medium text-white hover:bg-sv-accent-hover"
            >
              Save
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
