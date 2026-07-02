import { useState } from "react";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { apiGenerate, apiListModels, apiTestStt } from "../../services/tauriBridge";
import type { ApiProvider, ApiUse } from "../../types";

// Cloud providers only — local models are downloaded & run in the Model Store,
// not via API keys. `sttModel` is only set for providers this app knows how
// to actually call for speech-to-text: OpenAI and Groq (standard multipart
// Whisper endpoint) and OpenRouter (its own JSON+base64 audio endpoint, a
// different request shape handled separately in Rust). Providers without a
// known-working STT endpoint leave it blank; checking "STT" for them will
// fail at Test/dictation time.
const PRESETS: {
  name: string;
  base_url: string;
  model: string;
  sttModel?: string;
  note?: string;
}[] = [
  {
    name: "OpenAI",
    base_url: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    sttModel: "whisper-1",
  },
  {
    name: "Groq",
    base_url: "https://api.groq.com/openai/v1",
    model: "llama-3.3-70b-versatile",
    sttModel: "whisper-large-v3-turbo",
  },
  {
    name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
    model: "anthropic/claude-sonnet-4-6",
    sttModel: "openai/whisper-large-v3-turbo",
  },
  {
    name: "Together",
    base_url: "https://api.together.xyz/v1",
    model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
  },
  {
    name: "Mistral AI",
    base_url: "https://api.mistral.ai/v1",
    model: "",
  },
  {
    name: "DeepSeek",
    base_url: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  {
    name: "xAI (Grok)",
    base_url: "https://api.x.ai/v1",
    model: "",
  },
  {
    name: "Fireworks AI",
    base_url: "https://api.fireworks.ai/inference/v1",
    model: "",
  },
  {
    name: "DeepInfra",
    base_url: "https://api.deepinfra.com/v1/openai",
    model: "",
  },
  {
    name: "Perplexity",
    base_url: "https://api.perplexity.ai",
    model: "",
  },
  {
    name: "Google (Gemini)",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "",
    note: "Uses Google's official OpenAI-compatibility layer for chat. No working STT endpoint through this app.",
  },
  {
    name: "Anthropic (Claude)",
    base_url: "https://api.anthropic.com/v1",
    model: "",
    note: "Anthropic's own docs call this OpenAI-compat layer a testing aid, not production-ready — expect rough edges. It does not support audio, so never enable STT for it.",
  },
  { name: "Custom", base_url: "", model: "" },
];

export default function ApiKeys() {
  const providers = useSettingsStore((s) => s.providers);
  const addProvider = useSettingsStore((s) => s.addProvider);
  const updateProvider = useSettingsStore((s) => s.updateProvider);
  const deleteProvider = useSettingsStore((s) => s.deleteProvider);

  const [preset, setPreset] = useState(PRESETS[0].name);
  const [testResult, setTestResult] = useState<Record<string, string>>({});
  const [models, setModels] = useState<Record<string, string[]>>({});
  const [loadingModels, setLoadingModels] = useState<Record<string, boolean>>({});
  const [modelError, setModelError] = useState<Record<string, string>>({});

  async function loadModels(p: ApiProvider) {
    setLoadingModels((s) => ({ ...s, [p.id]: true }));
    setModelError((s) => ({ ...s, [p.id]: "" }));
    try {
      const list = await apiListModels(p.base_url, p.api_key);
      setModels((s) => ({ ...s, [p.id]: list }));
      if (list.length === 0)
        setModelError((s) => ({ ...s, [p.id]: "No models returned." }));
    } catch (e) {
      setModelError((s) => ({ ...s, [p.id]: String(e) }));
    } finally {
      setLoadingModels((s) => ({ ...s, [p.id]: false }));
    }
  }

  function add() {
    const p = PRESETS.find((x) => x.name === preset)!;
    addProvider({
      id: `prov_${Date.now()}`,
      name: p.name,
      api_key: "",
      base_url: p.base_url,
      model: p.model,
      stt_model: p.sttModel ?? "",
      uses: ["llm"],
    });
  }

  function toggleUse(p: ApiProvider, use: ApiUse) {
    const uses = p.uses.includes(use)
      ? p.uses.filter((u) => u !== use)
      : [...p.uses, use];
    updateProvider(p.id, { uses });
  }

  // Tests whichever capability is actually enabled for this provider — STT
  // and AI Processing hit completely different endpoints, so testing the
  // wrong one (e.g. chat/completions for an STT-only provider with no chat
  // model set) always fails even when the real, enabled endpoint works fine.
  async function test(p: ApiProvider) {
    setTestResult((r) => ({ ...r, [p.id]: "Testing…" }));
    const isLocal = p.base_url.includes("localhost") || p.base_url.includes("127.0.0.1");
    if (!p.api_key && !isLocal) {
      setTestResult((r) => ({ ...r, [p.id]: "✗ No API key set" }));
      return;
    }
    try {
      if (p.uses.includes("stt")) {
        const out = await apiTestStt(p.base_url, p.api_key, p.stt_model);
        setTestResult((r) => ({ ...r, [p.id]: `✓ ${out}` }));
      } else {
        const out = await apiGenerate(
          p.base_url,
          p.api_key,
          p.model,
          "You are a connection test. Reply with exactly: OK",
          "ping"
        );
        setTestResult((r) => ({
          ...r,
          [p.id]: `✓ Connected — model replied: "${out.slice(0, 40)}"`,
        }));
      }
    } catch (e) {
      setTestResult((r) => ({ ...r, [p.id]: `✗ ${e}` }));
    }
  }

  return (
    <Page
      title="API Keys"
      subtitle="Optional cloud providers for STT and AI processing. Keys are stored locally."
      actions={
        <div className="flex gap-2">
          <select
            value={preset}
            onChange={(e) => setPreset(e.target.value)}
            className="rounded-lg border border-sv-border bg-sv-surface px-3 py-1.5 text-sm"
          >
            {PRESETS.map((p) => (
              <option key={p.name}>{p.name}</option>
            ))}
          </select>
          <button
            onClick={add}
            className="rounded-lg bg-sv-accent px-3 py-1.5 text-sm font-medium text-white hover:bg-sv-accent-hover"
          >
            + Add provider
          </button>
        </div>
      }
    >
      {PRESETS.find((p) => p.name === preset)?.note && (
        <div className="mb-4 rounded-lg border border-sv-warn/30 bg-sv-warn/10 px-3 py-2 text-xs text-sv-warn">
          {PRESETS.find((p) => p.name === preset)?.note}
        </div>
      )}
      <div className="mb-4 rounded-lg border border-sv-border bg-sv-surface-2 px-3 py-2 text-[11px] text-sv-muted">
        Cloud STT is confirmed working with <strong>OpenAI</strong>,{" "}
        <strong>Groq</strong>, and <strong>OpenRouter</strong> (its own
        JSON+base64 audio format is handled separately from the other two).
        Everything else has no known-working transcription endpoint here —
        leave "STT" unchecked for them, or check it and use Test to find out.
      </div>
      {providers.length === 0 ? (
        <div className="rounded-xl border border-dashed border-sv-border bg-sv-surface p-8 text-center text-sm text-sv-muted">
          No providers yet. The app works fully offline with local models — add
          a provider only if you want to use a cloud API.
        </div>
      ) : (
        <div className="space-y-4">
          {providers.map((p) => (
            <div
              key={p.id}
              className="rounded-xl border border-sv-border bg-sv-surface p-4"
            >
              <div className="mb-3 flex items-center justify-between">
                <h3 className="font-medium">{p.name}</h3>
                <button
                  onClick={() => deleteProvider(p.id)}
                  className="text-xs text-sv-muted hover:text-sv-bad"
                >
                  Remove
                </button>
              </div>
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="API Key">
                  <input
                    type="password"
                    value={p.api_key}
                    onChange={(e) =>
                      updateProvider(p.id, { api_key: e.target.value })
                    }
                    placeholder="sk-…"
                    className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                  />
                </Field>
                <Field label="Model">
                  <div className="flex gap-2">
                    <input
                      value={p.model}
                      list={`models-${p.id}`}
                      placeholder="type to search…"
                      onChange={(e) =>
                        updateProvider(p.id, { model: e.target.value })
                      }
                      className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                    />
                    <button
                      onClick={() => loadModels(p)}
                      disabled={loadingModels[p.id]}
                      title="Fetch all available models"
                      className="shrink-0 rounded-lg border border-sv-border px-2 py-2 text-xs hover:bg-sv-surface-2 disabled:opacity-50"
                    >
                      {loadingModels[p.id] ? "…" : "↻ Load"}
                    </button>
                    <datalist id={`models-${p.id}`}>
                      {(models[p.id] ?? []).map((m) => (
                        <option key={m} value={m} />
                      ))}
                    </datalist>
                  </div>
                  {models[p.id]?.length > 0 && (
                    <p className="mt-1 text-[11px] text-sv-muted">
                      {models[p.id].length} models available — type in the box to
                      search
                    </p>
                  )}
                  {modelError[p.id] && (
                    <p className="mt-1 text-[11px] text-sv-bad">
                      {modelError[p.id]}
                    </p>
                  )}
                </Field>
                <Field label="Base URL">
                  <input
                    value={p.base_url}
                    onChange={(e) =>
                      updateProvider(p.id, { base_url: e.target.value })
                    }
                    className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                  />
                </Field>
                <Field label="Use for">
                  <div className="flex gap-4 pt-2 text-sm">
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={p.uses.includes("stt")}
                        onChange={() => toggleUse(p, "stt")}
                      />
                      STT (cloud Whisper)
                    </label>
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={p.uses.includes("llm")}
                        onChange={() => toggleUse(p, "llm")}
                      />
                      AI Processing
                    </label>
                  </div>
                  {p.uses.includes("stt") && (
                    <input
                      value={p.stt_model}
                      onChange={(e) =>
                        updateProvider(p.id, { stt_model: e.target.value })
                      }
                      placeholder="STT model id, e.g. whisper-1 or whisper-large-v3-turbo"
                      className="mt-2 w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                    />
                  )}
                  {p.uses.includes("stt") &&
                    !PRESETS.find((x) => x.name === p.name)?.sttModel && (
                      <p className="mt-1 text-[11px] text-sv-warn">
                        This provider isn't confirmed to support the
                        multipart Whisper endpoint this app uses — pick it as
                        the active source in Settings and Test to confirm.
                      </p>
                    )}
                </Field>
              </div>
              <div className="mt-3 flex items-center gap-3">
                <button
                  onClick={() => test(p)}
                  className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:bg-sv-surface-2"
                >
                  Test connection
                </button>
                {testResult[p.id] && (
                  <span className="text-xs text-sv-muted">
                    {testResult[p.id]}
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block text-sv-muted">{label}</span>
      {children}
    </label>
  );
}
