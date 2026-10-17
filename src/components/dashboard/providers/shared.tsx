import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useSettingsStore } from "../../../stores/settingsStore";
import { apiGenerate, apiListModels, apiTestStt } from "../../../services/tauriBridge";
import type { ApiProvider, ApiUse } from "../../../types";

// Shared by the Cloud providers page designs: the presets, the actions
// (load models, test, toggle a use) and the controls they are built from.

// `sttModel` is only set for providers this app knows how to call for
// speech-to-text: OpenAI and Groq (multipart Whisper) and OpenRouter (its own
// JSON + base64 audio shape, handled in Rust). The rest leave it blank.
export interface Preset {
  name: string;
  base_url: string;
  model: string;
  sttModel?: string;
  note?: string;
  blurb: string;
}

export const PRESETS: Preset[] = [
  { name: "OpenAI", base_url: "https://api.openai.com/v1", model: "gpt-4o-mini", sttModel: "whisper-1", blurb: "GPT models and Whisper" },
  { name: "Groq", base_url: "https://api.groq.com/openai/v1", model: "llama-3.3-70b-versatile", sttModel: "whisper-large-v3-turbo", blurb: "Very fast open models and Whisper" },
  { name: "OpenRouter", base_url: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-4-6", sttModel: "openai/whisper-large-v3-turbo", blurb: "Hundreds of models with one key" },
  {
    name: "Google (Gemini)",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "",
    blurb: "Gemini models",
    note: "Uses Google's OpenAI-compatible layer for chat. It has no speech-to-text this app can use.",
  },
  {
    name: "Anthropic (Claude)",
    base_url: "https://api.anthropic.com/v1",
    model: "",
    blurb: "Claude models",
    note: "Anthropic calls its OpenAI-compatible layer a testing aid, so expect rough edges. No speech-to-text.",
  },
  { name: "Together", base_url: "https://api.together.xyz/v1", model: "meta-llama/Llama-3.3-70B-Instruct-Turbo", blurb: "Open models" },
  { name: "Mistral AI", base_url: "https://api.mistral.ai/v1", model: "", blurb: "Mistral models" },
  { name: "DeepSeek", base_url: "https://api.deepseek.com/v1", model: "deepseek-chat", blurb: "DeepSeek chat and reasoning" },
  { name: "xAI (Grok)", base_url: "https://api.x.ai/v1", model: "", blurb: "Grok models" },
  { name: "Fireworks AI", base_url: "https://api.fireworks.ai/inference/v1", model: "", blurb: "Open models" },
  { name: "DeepInfra", base_url: "https://api.deepinfra.com/v1/openai", model: "", blurb: "Open models" },
  { name: "Perplexity", base_url: "https://api.perplexity.ai", model: "", blurb: "Search-backed models" },
  {
    name: "Custom",
    base_url: "",
    model: "",
    blurb: "Any OpenAI-compatible server",
    note: "Fill in a base URL (usually ending in /v1) and a model id.",
  },
];

export const presetOf = (p: ApiProvider) => PRESETS.find((x) => x.name === p.name);

export type TestState = { kind: "running" } | { kind: "ok"; msg: string } | { kind: "fail"; msg: string };

export type Status = "needs-key" | "needs-model" | "untested" | "testing" | "connected" | "failed";

const isLocalUrl = (u: string) => u.includes("localhost") || u.includes("127.0.0.1");

export function statusOf(p: ApiProvider, test?: TestState): Status {
  if (!p.api_key && !isLocalUrl(p.base_url)) return "needs-key";
  if (p.uses.includes("llm") && !p.model.trim()) return "needs-model";
  if (!test) return "untested";
  return test.kind === "running" ? "testing" : test.kind === "ok" ? "connected" : "failed";
}

export const STATUS_LABEL: Record<Status, string> = {
  "needs-key": "Needs a key",
  "needs-model": "Pick a model",
  untested: "Not tested",
  testing: "Testing…",
  connected: "Connected",
  failed: "Failed",
};

export const STATUS_TONE: Record<Status, string> = {
  "needs-key": "text-sv-warn",
  "needs-model": "text-sv-warn",
  untested: "text-sv-muted",
  testing: "text-sv-muted",
  connected: "text-sv-good",
  failed: "text-sv-bad",
};

/** Everything the designs do to providers, in one place. */
export function useProviders() {
  const providers = useSettingsStore((s) => s.providers);
  const addProvider = useSettingsStore((s) => s.addProvider);
  const update = useSettingsStore((s) => s.updateProvider);
  const remove = useSettingsStore((s) => s.deleteProvider);
  const [models, setModels] = useState<Record<string, string[]>>({});
  const [loading, setLoading] = useState<Record<string, boolean>>({});
  const [modelError, setModelError] = useState<Record<string, string>>({});
  const [tests, setTests] = useState<Record<string, TestState>>({});

  async function loadModels(p: ApiProvider) {
    setLoading((s) => ({ ...s, [p.id]: true }));
    setModelError((s) => ({ ...s, [p.id]: "" }));
    try {
      const list = await apiListModels(p.base_url, p.api_key);
      setModels((s) => ({ ...s, [p.id]: list }));
      if (list.length === 0) setModelError((s) => ({ ...s, [p.id]: "The provider returned no models." }));
    } catch (e) {
      setModelError((s) => ({ ...s, [p.id]: String(e) }));
    } finally {
      setLoading((s) => ({ ...s, [p.id]: false }));
    }
  }

  function add(name: string): string {
    const p = PRESETS.find((x) => x.name === name)!;
    const id = `prov_${Date.now()}`;
    addProvider({ id, name: p.name, api_key: "", base_url: p.base_url, model: p.model, stt_model: p.sttModel ?? "", uses: ["llm"] });
    return id;
  }

  function toggleUse(p: ApiProvider, use: ApiUse) {
    update(p.id, { uses: p.uses.includes(use) ? p.uses.filter((u) => u !== use) : [...p.uses, use] });
  }

  // Tests the use that is switched on: speech-to-text and AI hit different
  // endpoints, so testing chat for an STT-only provider would always fail.
  async function test(p: ApiProvider) {
    const set = (t: TestState) => setTests((s) => ({ ...s, [p.id]: t }));
    if (!p.api_key && !isLocalUrl(p.base_url)) return set({ kind: "fail", msg: "Paste an API key first." });
    set({ kind: "running" });
    try {
      if (p.uses.includes("stt") && !p.uses.includes("llm")) {
        set({ kind: "ok", msg: await apiTestStt(p.base_url, p.api_key, p.stt_model) });
      } else {
        const out = await apiGenerate(p.base_url, p.api_key, p.model, "You are a connection test. Reply with exactly: OK", "ping");
        set({ kind: "ok", msg: `${p.model.replace(/^models\//, "")} replied "${out.slice(0, 40)}"` });
      }
    } catch (e) {
      set({ kind: "fail", msg: String(e) });
    }
  }

  return { providers, add, update, remove, toggleUse, test, tests, models, loading, modelError, loadModels };
}

export type ProvidersApi = ReturnType<typeof useProviders>;

// ---------------- Model picker ----------------

/** Model ids that are not chat models (images, video, audio, embeddings…). */
const NOT_CHAT = /(embed|veo|imagen|lyria|tts|whisper|dall-e|moderation|banana|image|audio|realtime|transcribe|aqa|learnlm)/i;

export const showModel = (m: string) => m.replace(/^models\//, "");

/** Splits "openai/gpt-4o" into a family to group by; ids without a slash share one group. */
function family(m: string): string {
  const id = showModel(m);
  return id.includes("/") ? id.split("/")[0] : "";
}

/**
 * A searchable model list. `inline` renders the list in place (for a panel
 * that is already open); otherwise it is a field that opens a popover.
 */
export function ModelPicker({
  p,
  api,
  inline = false,
  onPicked,
}: {
  p: ApiProvider;
  api: ProvidersApi;
  inline?: boolean;
  onPicked?: () => void;
}) {
  const [open, setOpen] = useState(inline);
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const [showOther, setShowOther] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const list = api.models[p.id];
  const loading = api.loading[p.id];
  const error = api.modelError[p.id];
  const canLoad = !!p.base_url.trim() && (!!p.api_key || isLocalUrl(p.base_url));

  // Opening the picker loads the list once, when the provider can be asked.
  useEffect(() => {
    if (open && !list && !loading && !error && canLoad) api.loadModels(p);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, canLoad]);

  useEffect(() => {
    if (inline || !open) return;
    const close = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open, inline]);

  const { chat, other } = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const hits = (list ?? []).filter((m) => !needle || m.toLowerCase().includes(needle));
    return { chat: hits.filter((m) => !NOT_CHAT.test(m)), other: hits.filter((m) => NOT_CHAT.test(m)) };
  }, [list, q]);
  const visible = showOther || q ? [...chat, ...other] : chat;
  const custom = q.trim() && !(list ?? []).some((m) => showModel(m) === q.trim() || m === q.trim()) ? q.trim() : null;
  const rows = custom ? [custom, ...visible] : visible;

  useEffect(() => setActive(0), [q, showOther]);
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-i="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const pick = (m: string) => {
    api.update(p.id, { model: m });
    setQ("");
    if (!inline) setOpen(false);
    onPicked?.();
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, rows.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter" && rows[active]) {
      e.preventDefault();
      pick(rows[active]);
    } else if (e.key === "Escape" && !inline) {
      setOpen(false);
    }
  };

  let lastFamily: string | null = null;
  const panel = (
    <div
      className={
        inline
          ? "overflow-hidden rounded-xl border border-sv-border bg-sv-bg"
          : "absolute left-0 right-0 top-[calc(100%+6px)] z-30 overflow-hidden rounded-xl border border-sv-border bg-sv-surface shadow-[0_12px_32px_-8px_rgb(0_0_0/0.45)]"
      }
    >
      <div className="flex items-center gap-2 border-b border-sv-border px-3">
        <Icon.search className="h-4 w-4 shrink-0 text-sv-muted" />
        <input
          autoFocus={!inline}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={onKey}
          placeholder={list ? `Search ${list.length} models, or type a model id` : "Type a model id"}
          aria-label="Search models"
          className="h-10 w-full bg-transparent text-sm outline-none placeholder:text-sv-muted"
          style={{ outline: "none" }}
        />
        <button
          onClick={() => api.loadModels(p)}
          disabled={!canLoad || loading}
          title={canLoad ? "Reload the model list" : "Add the key and base URL first"}
          aria-label="Reload the model list"
          className="rounded-md p-1.5 text-sv-muted transition hover:bg-sv-surface-2 hover:text-sv-text disabled:opacity-40"
        >
          <Icon.refresh className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
        </button>
      </div>
      <ul ref={listRef} role="listbox" aria-label="Models" className="max-h-72 overflow-y-auto py-1">
        {loading && !list && <Skeleton />}
        {!loading && !list && (
          <li className="px-3 py-3 text-xs text-sv-muted">
            {error ? <span className="text-sv-bad">{error}</span> : canLoad ? "No list yet." : "Paste the API key to see this provider's models. You can also type a model id."}
          </li>
        )}
        {rows.map((m, i) => {
          const fam = m === custom ? null : family(m);
          const header = fam !== null && fam !== lastFamily && fam !== "" ? fam : null;
          if (fam !== null) lastFamily = fam;
          const selected = p.model === m;
          return (
            <li key={`${m}-${i}`}>
              {header && <div className="px-3 pb-1 pt-2.5 text-[11px] font-medium text-sv-muted">{header}</div>}
              <button
                data-i={i}
                role="option"
                aria-selected={selected}
                onMouseEnter={() => setActive(i)}
                onClick={() => pick(m)}
                className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm ${i === active ? "bg-sv-surface-2" : ""}`}
              >
                <span className="min-w-0 flex-1 truncate">
                  {m === custom ? (
                    <>
                      Use <span className="font-medium">{m}</span>
                    </>
                  ) : (
                    <Highlight text={fam ? showModel(m).slice(fam.length + 1) : showModel(m)} q={q} />
                  )}
                </span>
                {selected && <Icon.check className="h-4 w-4 shrink-0 text-sv-accent" />}
              </button>
            </li>
          );
        })}
        {list && rows.length === 0 && <li className="px-3 py-3 text-xs text-sv-muted">No model matches “{q}”.</li>}
      </ul>
      {list && !q && other.length > 0 && (
        <button
          onClick={() => setShowOther((s) => !s)}
          className="w-full border-t border-sv-border px-3 py-2 text-left text-xs text-sv-muted hover:text-sv-text"
        >
          {showOther ? "Hide" : "Show"} {other.length} models that can't chat (image, video, audio, embeddings)
        </button>
      )}
    </div>
  );

  if (inline) return panel;
  return (
    <div ref={box} className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        className={`flex h-10 w-full items-center gap-2 rounded-lg border bg-sv-bg px-3 text-left text-sm transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
          open ? "border-sv-accent" : "border-sv-border hover:border-sv-muted/60"
        }`}
      >
        <span className={`min-w-0 flex-1 truncate ${p.model ? "" : "text-sv-muted"}`}>{p.model ? showModel(p.model) : "Choose a model"}</span>
        <Icon.chevron className={`h-4 w-4 shrink-0 text-sv-muted transition ${open ? "rotate-180" : ""}`} />
      </button>
      {open && panel}
    </div>
  );
}

function Highlight({ text, q }: { text: string; q: string }) {
  const i = q ? text.toLowerCase().indexOf(q.trim().toLowerCase()) : -1;
  if (i < 0) return <>{text}</>;
  const n = q.trim().length;
  return (
    <>
      {text.slice(0, i)}
      <mark className="rounded-sm bg-sv-accent/20 text-sv-text">{text.slice(i, i + n)}</mark>
      {text.slice(i + n)}
    </>
  );
}

function Skeleton() {
  return (
    <>
      {[70, 52, 84, 60, 44].map((w, i) => (
        <li key={i} className="px-3 py-2">
          <div className="h-3 animate-pulse rounded bg-sv-surface-2" style={{ width: `${w}%` }} />
        </li>
      ))}
    </>
  );
}

// ---------------- Small controls ----------------

/** API key field with show/hide. */
export function KeyInput({ p, api, id }: { p: ApiProvider; api: ProvidersApi; id?: string }) {
  const [shown, setShown] = useState(false);
  return (
    <div className="relative">
      <input
        id={id}
        type={shown ? "text" : "password"}
        value={p.api_key}
        onChange={(e) => api.update(p.id, { api_key: e.target.value.trim() })}
        placeholder="Paste your API key"
        spellCheck={false}
        autoComplete="off"
        style={{ outline: "none" }}
        className="h-10 w-full rounded-lg border border-sv-border bg-sv-bg pl-3 pr-10 font-mono text-[13px] outline-none transition placeholder:font-sans placeholder:text-sv-muted focus:border-sv-accent"
      />
      <button
        onClick={() => setShown((s) => !s)}
        aria-label={shown ? "Hide key" : "Show key"}
        title={shown ? "Hide key" : "Show key"}
        className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-md p-1.5 text-sv-muted hover:text-sv-text"
      >
        {shown ? <Icon.eyeOff className="h-4 w-4" /> : <Icon.eye className="h-4 w-4" />}
      </button>
    </div>
  );
}

export function TextInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      spellCheck={false}
      style={{ outline: "none" }}
      {...props}
      className={`h-10 w-full rounded-lg border border-sv-border bg-sv-bg px-3 text-sm outline-none transition placeholder:text-sv-muted focus:border-sv-accent ${props.className ?? ""}`}
    />
  );
}

/** A switch row: what the provider is used for. */
export function UseSwitch({
  on,
  onChange,
  title,
  hint,
  disabled,
}: {
  on: boolean;
  onChange: () => void;
  title: string;
  hint: string;
  disabled?: boolean;
}) {
  return (
    <button
      role="switch"
      aria-checked={on}
      onClick={onChange}
      disabled={disabled}
      className="flex w-full items-center gap-3 rounded-lg px-1 py-1.5 text-left disabled:cursor-not-allowed disabled:opacity-50"
    >
      <span className="min-w-0 flex-1">
        <span className="block text-sm">{title}</span>
        <span className="block text-xs text-sv-muted">{hint}</span>
      </span>
      <span className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${on ? "bg-sv-accent" : "bg-sv-surface-2 ring-1 ring-inset ring-sv-border"}`}>
        <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-[left] duration-200 ${on ? "left-[18px]" : "left-0.5"}`} />
      </span>
    </button>
  );
}

export function StatusDot({ s }: { s: Status }) {
  const bg =
    s === "connected" ? "bg-sv-good" : s === "failed" ? "bg-sv-bad" : s === "needs-key" || s === "needs-model" ? "bg-sv-warn" : "bg-sv-muted/60";
  return <span className={`inline-block h-2 w-2 shrink-0 rounded-full ${bg} ${s === "testing" ? "animate-pulse" : ""}`} aria-hidden />;
}

export function Btn({
  children,
  onClick,
  disabled,
  kind = "quiet",
  className = "",
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  kind?: "primary" | "quiet" | "danger";
  className?: string;
  title?: string;
}) {
  const look =
    kind === "primary"
      ? "bg-sv-accent text-sv-on-accent hover:bg-sv-accent-hover"
      : kind === "danger"
        ? "text-sv-muted hover:bg-sv-bad/10 hover:text-sv-bad"
        : "border border-sv-border text-sv-text hover:bg-sv-surface-2";
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`inline-flex h-9 items-center justify-center gap-1.5 rounded-lg px-3 text-sm font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent disabled:cursor-not-allowed disabled:opacity-50 ${look} ${className}`}
    >
      {children}
    </button>
  );
}

/** One-line result of the last test. */
export function TestLine({ t }: { t?: TestState }) {
  if (!t) return null;
  if (t.kind === "running") return <p className="text-xs text-sv-muted">Sending a test message…</p>;
  return (
    <p role="status" className={`flex items-start gap-1.5 text-xs ${t.kind === "ok" ? "text-sv-good" : "text-sv-bad"}`}>
      {t.kind === "ok" ? <Icon.check className="mt-px h-3.5 w-3.5 shrink-0" /> : <Icon.x className="mt-px h-3.5 w-3.5 shrink-0" />}
      <span className="min-w-0 break-words">{t.msg}</span>
    </p>
  );
}

/** Warning when speech-to-text is on for a provider this app can't transcribe with. */
export function SttWarning({ p }: { p: ApiProvider }) {
  if (!p.uses.includes("stt") || presetOf(p)?.sttModel) return null;
  return <p className="text-xs text-sv-warn">This provider may not offer speech-to-text in the shape this app uses. Test it to check.</p>;
}

// ---------------- Icons (one 1.75 stroke set) ----------------

const svg = (d: ReactNode) =>
  function I({ className }: { className?: string }) {
    return (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round" className={className} aria-hidden>
        {d}
      </svg>
    );
  };

export const Icon = {
  search: svg(
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="m20 20-4.2-4.2" />
    </>
  ),
  refresh: svg(<path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" />),
  chevron: svg(<path d="m6 9 6 6 6-6" />),
  check: svg(<path d="m5 12.5 4.5 4.5L19 7.5" />),
  x: svg(<path d="M6 6l12 12M18 6 6 18" />),
  plus: svg(<path d="M12 5v14M5 12h14" />),
  eye: svg(
    <>
      <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z" />
      <circle cx="12" cy="12" r="2.75" />
    </>
  ),
  eyeOff: svg(<path d="M3 3l18 18M10.6 5.6A9.7 9.7 0 0 1 12 5.5C18 5.5 21.5 12 21.5 12a17 17 0 0 1-3 3.8M6.6 6.6C3.9 8.3 2.5 12 2.5 12S6 18.5 12 18.5a9 9 0 0 0 4.4-1.1M9.9 9.9a2.75 2.75 0 0 0 4.2 4.2" />),
  trash: svg(<path d="M4.5 7h15M9.5 7V4.5h5V7M6.5 7l.8 12.5h9.4L17.5 7" />),
  bolt: svg(<path d="M13 3 5 13.5h6L10 21l8-10.5h-6L13 3Z" />),
  settings: svg(
    <>
      <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
      <circle cx="16" cy="7" r="2" />
      <circle cx="10" cy="17" r="2" />
    </>
  ),
};
