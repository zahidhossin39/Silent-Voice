import { useState } from "react";
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
  UseSwitch,
  presetOf,
  showModel,
  statusOf,
  type ProvidersApi,
} from "./shared";

// Design A, "Inspector": connected providers listed on the left, the picked
// one edited on the right, like a mail account settings window.

export default function VariantA({ api, onRemove }: { api: ProvidersApi; onRemove: (p: ApiProvider) => void }) {
  const [sel, setSel] = useState<string | "add" | null>(api.providers[0]?.id ?? "add");
  const current = api.providers.find((p) => p.id === sel);
  const view = current ? "edit" : "add";

  return (
    <div className="grid min-h-[520px] overflow-hidden rounded-2xl border border-sv-border bg-sv-surface md:grid-cols-[15rem_minmax(0,1fr)]">
      <nav aria-label="Your providers" className="flex flex-col border-b border-sv-border bg-sv-bg/40 md:border-b-0 md:border-r">
        <div className="px-4 pb-2 pt-4 text-xs font-medium text-sv-muted">Your providers</div>
        <ul className="flex-1 space-y-0.5 px-2">
          {api.providers.map((p) => {
            const s = statusOf(p, api.tests[p.id]);
            const active = p.id === sel;
            return (
              <li key={p.id}>
                <button
                  onClick={() => setSel(p.id)}
                  aria-current={active}
                  className={`flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left transition ${
                    active ? "bg-sv-surface-2" : "hover:bg-sv-surface-2/60"
                  }`}
                >
                  <ProviderLogo provider={p.name} size={28} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">{p.name}</span>
                    <span className={`flex items-center gap-1.5 text-xs ${STATUS_TONE[s]}`}>
                      <StatusDot s={s} />
                      {STATUS_LABEL[s]}
                    </span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
        <div className="p-2">
          <button
            onClick={() => setSel("add")}
            className={`flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-sm transition ${
              view === "add" ? "bg-sv-surface-2 text-sv-text" : "text-sv-muted hover:bg-sv-surface-2/60 hover:text-sv-text"
            }`}
          >
            <span className="flex h-7 w-7 items-center justify-center rounded-md border border-dashed border-sv-border">
              <Icon.plus className="h-4 w-4" />
            </span>
            Add a provider
          </button>
        </div>
      </nav>

      <div className="min-w-0 p-6">
        {view === "add" ? (
          <AddGallery
            empty={api.providers.length === 0}
            onAdd={(name) => setSel(api.add(name))}
          />
        ) : (
          current && <Detail key={current.id} p={current} api={api} onRemove={() => onRemove(current)} />
        )}
      </div>
    </div>
  );
}

function AddGallery({ empty, onAdd }: { empty: boolean; onAdd: (name: string) => void }) {
  return (
    <div>
      <h2 className="text-lg font-semibold">Add a provider</h2>
      <p className="mt-1 max-w-[60ch] text-sm text-sv-muted">
        {empty
          ? "Everything works on this PC without one. Add a cloud provider to use its models for AI writing, meeting summaries or speech-to-text."
          : "Pick who you have an account with. You paste the key next."}
      </p>
      <ul className="mt-5 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {PRESETS.map((p) => (
          <li key={p.name}>
            <button
              onClick={() => onAdd(p.name)}
              className="group flex w-full items-center gap-3 rounded-xl border border-sv-border px-3 py-3 text-left transition hover:border-sv-accent/60 hover:bg-sv-surface-2/50"
            >
              <ProviderLogo provider={p.name} size={32} />
              <span className="min-w-0 flex-1">
                <span className="block text-sm font-medium">{p.name}</span>
                <span className="block truncate text-xs text-sv-muted">{p.blurb}</span>
              </span>
              {p.sttModel && (
                <span className="shrink-0 rounded-md bg-sv-surface-2 px-1.5 py-0.5 text-[10px] font-medium text-sv-muted" title="Can also do speech-to-text">
                  STT
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Detail({ p, api, onRemove }: { p: ApiProvider; api: ProvidersApi; onRemove: () => void }) {
  const [advanced, setAdvanced] = useState(!p.base_url);
  const s = statusOf(p, api.tests[p.id]);
  const preset = presetOf(p);
  const canStt = !!preset?.sttModel || p.uses.includes("stt");

  return (
    <div className="max-w-xl">
      <header className="flex items-center gap-3">
        <ProviderLogo provider={p.name} size={40} />
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-lg font-semibold">{p.name}</h2>
          <p className={`flex items-center gap-1.5 text-xs ${STATUS_TONE[s]}`}>
            <StatusDot s={s} />
            {STATUS_LABEL[s]}
            {p.model && s !== "needs-key" && <span className="truncate text-sv-muted">· {showModel(p.model)}</span>}
          </p>
        </div>
        <Btn onClick={() => api.test(p)} disabled={s === "testing"} kind={s === "untested" ? "primary" : "quiet"}>
          <Icon.bolt className="h-4 w-4" />
          Test
        </Btn>
      </header>
      <div className="mt-2 min-h-[1rem]">
        <TestLine t={api.tests[p.id]} />
      </div>

      {preset?.note && <p className="mt-4 text-xs text-sv-muted">{preset.note}</p>}

      <section className="mt-6 space-y-5">
        <Row label="API key" htmlFor={`key-${p.id}`}>
          <KeyInput p={p} api={api} id={`key-${p.id}`} />
        </Row>
        <Row label="Model" hint="Used for AI writing styles and meeting summaries.">
          <ModelPicker p={p} api={api} />
        </Row>
      </section>

      <section className="mt-7 border-t border-sv-border pt-5">
        <h3 className="mb-1 text-sm font-medium">Use it for</h3>
        <UseSwitch
          on={p.uses.includes("llm")}
          onChange={() => api.toggleUse(p, "llm")}
          title="AI writing and summaries"
          hint="Writing styles rewrite your dictation; Notetaker summarizes meetings."
        />
        <UseSwitch
          on={p.uses.includes("stt")}
          onChange={() => api.toggleUse(p, "stt")}
          title="Speech-to-text"
          hint={canStt ? "Transcribe dictation in the cloud instead of on this PC." : "Not offered by this provider."}
          disabled={!canStt}
        />
        <SttWarning p={p} />
      </section>

      <section className="mt-5 border-t border-sv-border pt-4">
        <button
          onClick={() => setAdvanced((a) => !a)}
          aria-expanded={advanced}
          className="flex items-center gap-1.5 text-sm text-sv-muted hover:text-sv-text"
        >
          <Icon.chevron className={`h-4 w-4 transition ${advanced ? "" : "-rotate-90"}`} />
          Connection details
        </button>
        {advanced && (
          <div className="mt-3 space-y-4">
            <Row label="Base URL" htmlFor={`url-${p.id}`}>
              <TextInput id={`url-${p.id}`} value={p.base_url} placeholder="https://…/v1" onChange={(e) => api.update(p.id, { base_url: e.target.value })} />
            </Row>
            {p.uses.includes("stt") && (
              <Row label="Speech-to-text model" htmlFor={`stt-${p.id}`}>
                <TextInput id={`stt-${p.id}`} value={p.stt_model} placeholder="whisper-1" onChange={(e) => api.update(p.id, { stt_model: e.target.value })} />
              </Row>
            )}
          </div>
        )}
      </section>

      <div className="mt-8">
        <Btn kind="danger" onClick={onRemove} className="-ml-3">
          <Icon.trash className="h-4 w-4" />
          Remove {p.name}
        </Btn>
      </div>
    </div>
  );
}

function Row({ label, hint, htmlFor, children }: { label: string; hint?: string; htmlFor?: string; children: React.ReactNode }) {
  return (
    <div>
      <label htmlFor={htmlFor} className="mb-1.5 block text-sm font-medium">
        {label}
      </label>
      {children}
      {hint && <p className="mt-1.5 text-xs text-sv-muted">{hint}</p>}
    </div>
  );
}
