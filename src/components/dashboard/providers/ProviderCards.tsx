import { useEffect, useRef, useState } from "react";
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
  presetOf,
  showModel,
  statusOf,
  type ProvidersApi,
} from "./shared";

// Cloud providers as cards: one compact card per provider with everything
// on it at once; the model list opens wide inside the card, not as a dropdown.

export default function ProviderCards({ api, onRemove }: { api: ProvidersApi; onRemove: (p: ApiProvider) => void }) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-sv-muted">
          {api.providers.length === 0
            ? "None connected. Silent Voice works fully on this PC without one."
            : `${api.providers.length} connected`}
        </p>
        <AddMenu onAdd={(n) => api.add(n)} />
      </div>
      {api.providers.map((p) => (
        <Card key={p.id} p={p} api={api} onRemove={() => onRemove(p)} />
      ))}
    </div>
  );
}

function AddMenu({ onAdd }: { onAdd: (name: string) => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);
  const hits = PRESETS.filter((p) => p.name.toLowerCase().includes(q.toLowerCase()));

  return (
    <div ref={box} className="relative">
      <Btn kind="primary" onClick={() => setOpen((o) => !o)}>
        <Icon.plus className="h-4 w-4" />
        Add provider
      </Btn>
      {open && (
        <div className="absolute right-0 top-[calc(100%+6px)] z-30 w-80 overflow-hidden rounded-xl border border-sv-border bg-sv-surface shadow-[0_12px_32px_-8px_rgb(0_0_0/0.45)]">
          <div className="flex items-center gap-2 border-b border-sv-border px-3">
            <Icon.search className="h-4 w-4 text-sv-muted" />
            <input
              autoFocus
              value={q}
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setOpen(false);
                if (e.key === "Enter" && hits[0]) {
                  onAdd(hits[0].name);
                  setOpen(false);
                }
              }}
              placeholder="Find a provider"
              aria-label="Find a provider"
              className="h-10 w-full bg-transparent text-sm outline-none placeholder:text-sv-muted"
          style={{ outline: "none" }}
            />
          </div>
          <ul className="max-h-80 overflow-y-auto py-1">
            {hits.map((p) => (
              <li key={p.name}>
                <button
                  onClick={() => {
                    onAdd(p.name);
                    setOpen(false);
                    setQ("");
                  }}
                  className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-sv-surface-2"
                >
                  <ProviderLogo provider={p.name} size={26} />
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm">{p.name}</span>
                    <span className="block truncate text-xs text-sv-muted">{p.blurb}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function Card({ p, api, onRemove }: { p: ApiProvider; api: ProvidersApi; onRemove: () => void }) {
  const [browsing, setBrowsing] = useState(false);
  const [editUrl, setEditUrl] = useState(!p.base_url);
  const s = statusOf(p, api.tests[p.id]);
  const preset = presetOf(p);
  const canStt = !!preset?.sttModel || p.uses.includes("stt");

  return (
    <article className="rounded-2xl border border-sv-border bg-sv-surface">
      <header className="flex flex-wrap items-center gap-3 px-5 pt-4">
        <ProviderLogo provider={p.name} size={36} />
        <div className="min-w-0 flex-1">
          <h3 className="text-[15px] font-semibold">{p.name}</h3>
          {editUrl ? (
            <input
              autoFocus={!!p.base_url}
              value={p.base_url}
              onChange={(e) => api.update(p.id, { base_url: e.target.value })}
              onBlur={() => p.base_url && setEditUrl(false)}
              onKeyDown={(e) => e.key === "Enter" && p.base_url && setEditUrl(false)}
              placeholder="Base URL, e.g. https://example.com/v1"
              aria-label="Base URL"
              spellCheck={false}
              className="mt-0.5 w-full max-w-md rounded border-b border-sv-accent bg-transparent text-xs outline-none placeholder:text-sv-muted"
            />
          ) : (
            <button onClick={() => setEditUrl(true)} title="Edit the base URL" className="group flex max-w-full items-center gap-1 text-xs text-sv-muted hover:text-sv-text">
              <span className="truncate">{p.base_url.replace(/^https?:\/\//, "")}</span>
              <span className="opacity-0 transition group-hover:opacity-100">· edit</span>
            </button>
          )}
        </div>
        <span className={`flex items-center gap-1.5 rounded-full bg-sv-bg px-2.5 py-1 text-xs ${STATUS_TONE[s]}`}>
          <StatusDot s={s} />
          {STATUS_LABEL[s]}
        </span>
        <button
          onClick={onRemove}
          title={`Remove ${p.name}`}
          aria-label={`Remove ${p.name}`}
          className="rounded-lg p-2 text-sv-muted transition hover:bg-sv-bad/10 hover:text-sv-bad"
        >
          <Icon.trash className="h-4 w-4" />
        </button>
      </header>

      <div className="grid gap-3 px-5 pt-4 md:grid-cols-2">
        <div>
          <div className="mb-1.5 text-xs font-medium text-sv-muted">API key</div>
          <KeyInput p={p} api={api} />
        </div>
        <div>
          <div className="mb-1.5 text-xs font-medium text-sv-muted">Model</div>
          <button
            onClick={() => setBrowsing((b) => !b)}
            aria-expanded={browsing}
            className={`flex h-10 w-full items-center gap-2 rounded-lg border bg-sv-bg px-3 text-left text-sm transition ${
              browsing ? "border-sv-accent" : "border-sv-border hover:border-sv-muted/60"
            }`}
          >
            <span className={`min-w-0 flex-1 truncate ${p.model ? "" : "text-sv-muted"}`}>{p.model ? showModel(p.model) : "Choose a model"}</span>
            <span className="shrink-0 text-xs text-sv-accent">{browsing ? "Done" : "Browse"}</span>
          </button>
        </div>
      </div>

      {browsing && (
        <div className="px-5 pt-3">
          <ModelPicker p={p} api={api} inline onPicked={() => setBrowsing(false)} />
        </div>
      )}

      {preset?.note && <p className="px-5 pt-3 text-xs text-sv-muted">{preset.note}</p>}

      <footer className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-sv-border px-5 py-3">
        <div role="group" aria-label="Use it for" className="flex rounded-lg bg-sv-bg p-0.5">
          <Seg on={p.uses.includes("llm")} onClick={() => api.toggleUse(p, "llm")}>
            AI writing
          </Seg>
          <Seg on={p.uses.includes("stt")} onClick={() => api.toggleUse(p, "stt")} disabled={!canStt} title={canStt ? "Transcribe in the cloud" : "Not offered by this provider"}>
            Speech-to-text
          </Seg>
        </div>
        {p.uses.includes("stt") && (
          <input
            value={p.stt_model}
            onChange={(e) => api.update(p.id, { stt_model: e.target.value })}
            placeholder="Speech-to-text model"
            aria-label="Speech-to-text model"
            spellCheck={false}
            className="h-8 w-48 rounded-md border border-sv-border bg-sv-bg px-2 text-xs outline-none focus:border-sv-accent"
          />
        )}
        <div className="min-w-0 flex-1">
          <TestLine t={api.tests[p.id]} />
          <SttWarning p={p} />
        </div>
        <Btn onClick={() => api.test(p)} disabled={s === "testing"} kind={s === "untested" ? "primary" : "quiet"} className="h-8">
          <Icon.bolt className="h-3.5 w-3.5" />
          Test
        </Btn>
      </footer>
    </article>
  );
}

function Seg({ on, onClick, disabled, title, children }: { on: boolean; onClick: () => void; disabled?: boolean; title?: string; children: React.ReactNode }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`h-7 rounded-md px-3 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-40 ${
        on ? "bg-sv-surface-2 text-sv-text shadow-[0_1px_2px_rgb(0_0_0/0.25)]" : "text-sv-muted hover:text-sv-text"
      }`}
    >
      {children}
    </button>
  );
}
