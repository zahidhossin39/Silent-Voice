import { useState, type ReactNode } from "react";
import ProviderLogo from "../../shared/ProviderLogo";
import type { ApiProvider } from "../../../types";
import {
  Btn,
  Icon,
  KeyInput,
  ModelPicker,
  PRESETS,
  STATUS_LABEL,
  STATUS_TONE,
  SttWarning,
  StatusDot,
  TestLine,
  TextInput,
  presetOf,
  showModel,
  statusOf,
  type ProvidersApi,
} from "./shared";

// Design B, "Guided setup": each provider is a row that opens into three
// steps, key, model, test, so it is obvious what is left to do.

export default function VariantB({ api, onRemove }: { api: ProvidersApi; onRemove: (p: ApiProvider) => void }) {
  const [open, setOpen] = useState<string | null>(api.providers.find((p) => statusOf(p) !== "untested")?.id ?? null);
  const [adding, setAdding] = useState(api.providers.length === 0);

  return (
    <div className="space-y-6">
      {api.providers.length > 0 && (
        <ul className="divide-y divide-sv-border overflow-hidden rounded-2xl border border-sv-border bg-sv-surface">
          {api.providers.map((p) => (
            <ProviderRow key={p.id} p={p} api={api} open={open === p.id} onToggle={() => setOpen(open === p.id ? null : p.id)} onRemove={() => onRemove(p)} />
          ))}
        </ul>
      )}

      <section>
        {adding ? (
          <>
            <div className="mb-3 flex items-baseline justify-between gap-3">
              <h2 className="text-base font-semibold">{api.providers.length ? "Add another provider" : "Connect a provider"}</h2>
              {api.providers.length > 0 && (
                <button onClick={() => setAdding(false)} className="text-sm text-sv-muted hover:text-sv-text">
                  Cancel
                </button>
              )}
            </div>
            {api.providers.length === 0 && (
              <p className="-mt-1 mb-4 max-w-[62ch] text-sm text-sv-muted">
                Silent Voice runs fully on this PC. Connect a cloud account only to use its models for AI writing, meeting summaries or speech-to-text.
              </p>
            )}
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
              {PRESETS.map((pr) => (
                <button
                  key={pr.name}
                  onClick={() => {
                    const id = api.add(pr.name);
                    setOpen(id);
                    setAdding(false);
                  }}
                  className="flex flex-col items-start gap-2.5 rounded-xl border border-sv-border bg-sv-surface p-3.5 text-left transition hover:-translate-y-px hover:border-sv-accent/60 hover:shadow-[0_6px_16px_-8px_rgb(0_0_0/0.4)]"
                >
                  <ProviderLogo provider={pr.name} size={30} />
                  <span>
                    <span className="block text-sm font-medium">{pr.name}</span>
                    <span className="mt-0.5 block text-xs text-sv-muted">{pr.blurb}</span>
                  </span>
                </button>
              ))}
            </div>
          </>
        ) : (
          <Btn onClick={() => setAdding(true)}>
            <Icon.plus className="h-4 w-4" />
            Add a provider
          </Btn>
        )}
      </section>
    </div>
  );
}

function ProviderRow({
  p,
  api,
  open,
  onToggle,
  onRemove,
}: {
  p: ApiProvider;
  api: ProvidersApi;
  open: boolean;
  onToggle: () => void;
  onRemove: () => void;
}) {
  const s = statusOf(p, api.tests[p.id]);
  const t = api.tests[p.id];
  const hasKey = s !== "needs-key";
  const hasModel = hasKey && s !== "needs-model";
  const tested = s === "connected";
  const preset = presetOf(p);
  const canStt = !!preset?.sttModel || p.uses.includes("stt");

  return (
    <li>
      <button onClick={onToggle} aria-expanded={open} className="flex w-full items-center gap-3.5 px-5 py-4 text-left transition hover:bg-sv-surface-2/40">
        <ProviderLogo provider={p.name} size={34} />
        <span className="min-w-0 flex-1">
          <span className="block text-sm font-semibold">{p.name}</span>
          <span className="block truncate text-xs text-sv-muted">
            {p.model ? showModel(p.model) : "No model yet"}
            {p.uses.length > 0 && ` · ${p.uses.map((u) => (u === "llm" ? "AI" : "Speech-to-text")).join(" + ")}`}
          </span>
        </span>
        <span className={`hidden items-center gap-1.5 text-xs sm:flex ${STATUS_TONE[s]}`}>
          <StatusDot s={s} />
          {STATUS_LABEL[s]}
        </span>
        <Icon.chevron className={`h-4 w-4 text-sv-muted transition ${open ? "rotate-180" : ""}`} />
      </button>

      {open && (
        <div className="border-t border-sv-border bg-sv-bg/30 px-5 pb-5 pt-2">
          {preset?.note && <p className="mt-3 text-xs text-sv-muted">{preset.note}</p>}
          <ol>
            <Step n={1} done={hasKey} title="Paste your API key" hint={`Find it in your ${p.name} account. It stays on this PC.`}>
              <KeyInput p={p} api={api} />
            </Step>
            <Step n={2} done={hasModel} title="Choose a model" hint="Used for AI writing styles and meeting summaries.">
              <ModelPicker p={p} api={api} />
            </Step>
            <Step n={3} done={tested} title="Test the connection" last>
              <div className="flex flex-wrap items-center gap-3">
                <Btn onClick={() => api.test(p)} disabled={s === "testing" || !hasKey} kind={hasModel && !tested ? "primary" : "quiet"}>
                  <Icon.bolt className="h-4 w-4" />
                  {t?.kind === "fail" ? "Try again" : "Send a test message"}
                </Btn>
                <div className="min-w-0 flex-1">
                  <TestLine t={t} />
                </div>
              </div>
            </Step>
          </ol>

          <div className="mt-5 grid gap-4 rounded-xl border border-sv-border bg-sv-surface p-4 sm:grid-cols-2">
            <div>
              <h4 className="text-xs font-medium text-sv-muted">Use it for</h4>
              <div className="mt-2 flex flex-wrap gap-2">
                <Chip on={p.uses.includes("llm")} onClick={() => api.toggleUse(p, "llm")}>
                  AI writing and summaries
                </Chip>
                <Chip on={p.uses.includes("stt")} onClick={() => api.toggleUse(p, "stt")} disabled={!canStt} title={canStt ? undefined : "Not offered by this provider"}>
                  Speech-to-text
                </Chip>
              </div>
              <div className="mt-2">
                <SttWarning p={p} />
              </div>
            </div>
            <div className="space-y-2">
              <label className="block">
                <span className="text-xs font-medium text-sv-muted">Base URL</span>
                <TextInput className="mt-1 h-9 text-[13px]" value={p.base_url} placeholder="https://…/v1" onChange={(e) => api.update(p.id, { base_url: e.target.value })} />
              </label>
              {p.uses.includes("stt") && (
                <label className="block">
                  <span className="text-xs font-medium text-sv-muted">Speech-to-text model</span>
                  <TextInput className="mt-1 h-9 text-[13px]" value={p.stt_model} placeholder="whisper-1" onChange={(e) => api.update(p.id, { stt_model: e.target.value })} />
                </label>
              )}
            </div>
          </div>

          <div className="mt-3 flex justify-end">
            <Btn kind="danger" onClick={onRemove}>
              <Icon.trash className="h-4 w-4" />
              Remove
            </Btn>
          </div>
        </div>
      )}
    </li>
  );
}

function Step({ n, done, title, hint, last, children }: { n: number; done: boolean; title: string; hint?: string; last?: boolean; children: ReactNode }) {
  return (
    <li className="relative grid grid-cols-[2rem_minmax(0,1fr)] gap-3 pt-4">
      {!last && <span className="absolute bottom-0 left-[15px] top-12 w-px bg-sv-border" aria-hidden />}
      <span
        className={`flex h-8 w-8 items-center justify-center rounded-full text-xs font-semibold transition-colors ${
          done ? "bg-sv-good/15 text-sv-good" : "border border-sv-border text-sv-muted"
        }`}
      >
        {done ? <Icon.check className="h-4 w-4" /> : n}
      </span>
      <div className="min-w-0 pb-1">
        <div className="pt-1.5 text-sm font-medium">{title}</div>
        {hint && <p className="mb-2 mt-0.5 text-xs text-sv-muted">{hint}</p>}
        <div className={hint ? "" : "mt-2"}>{children}</div>
      </div>
    </li>
  );
}

function Chip({ on, onClick, disabled, title, children }: { on: boolean; onClick: () => void; disabled?: boolean; title?: string; children: ReactNode }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-40 ${
        on ? "border-sv-accent/50 bg-sv-accent/10 text-sv-text" : "border-sv-border text-sv-muted hover:text-sv-text"
      }`}
    >
      {on && <Icon.check className="h-3.5 w-3.5 text-sv-accent" />}
      {children}
    </button>
  );
}
