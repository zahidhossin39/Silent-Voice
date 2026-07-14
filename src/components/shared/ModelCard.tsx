import { useState } from "react";
import type { SttModel, HardwareInfo, CompatibilityLevel } from "../../types";
import ProviderLogo from "./ProviderLogo";
import { sttCompatibility } from "../../services/recommend";
import { sttLanguage } from "../../services/catalog";
import { formatMB } from "../../services/format";
import { useModelStore } from "../../stores/modelStore";

const DOT: Record<CompatibilityLevel, string> = {
  good: "bg-sv-good",
  warn: "bg-sv-warn",
  bad: "bg-sv-bad",
};

const DOT_TITLE: Record<CompatibilityLevel, string> = {
  good: "Recommended for your device",
  warn: "Works, but may be slow on your device",
  bad: "Heavy for your device — better on a stronger machine",
};

// Plain-language capability labels derived from the raw catalog numbers.
function friendlySpeed(label: string): string {
  const m = label.match(/([\d.]+)x/);
  if (!m) return label;
  const x = parseFloat(m[1]);
  if (x >= 5) return "Very fast";
  if (x >= 2) return "Fast";
  if (x >= 1) return "Real-time";
  if (x >= 0.5) return "A bit slow";
  return "Slow on CPU";
}
function friendlyAccuracy(wer: string): string {
  const m = wer.match(/([\d.]+)/);
  if (!m) return wer;
  const w = parseFloat(m[1]);
  if (w <= 3) return "Excellent";
  if (w <= 4) return "Very good";
  if (w <= 6) return "Good";
  return "Basic";
}

// A minimal model row: logo · name · provider/lang/size · action. Click the
// left area to expand a small details panel. STT models can be selected as the
// active dictation model once downloaded.
export default function ModelCard({
  model,
  hardware,
  active,
  onSelect,
  pinned,
  onTogglePin,
}: {
  model: SttModel;
  hardware: HardwareInfo | null;
  active: boolean;
  onSelect: () => void;
  pinned: boolean;
  onTogglePin: () => void;
}) {
  const downloaded = useModelStore((s) => s.downloaded.has(model.id));
  const progress = useModelStore((s) => s.progress[model.id]);
  const download = useModelStore((s) => s.download);
  const remove = useModelStore((s) => s.remove);
  const [open, setOpen] = useState(false);

  const level = sttCompatibility(model, hardware).level;
  const isDownloading = progress?.status === "downloading";
  const pct =
    progress && progress.total_bytes > 0
      ? Math.round((progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  return (
    <div
      className={`overflow-hidden rounded-xl border transition ${
        active
          ? "border-sv-accent bg-sv-accent/5 ring-1 ring-sv-accent/40"
          : "border-sv-border bg-sv-surface hover:border-sv-muted/40"
      }`}
    >
      <div className="flex items-center gap-3 px-3.5 py-2.5">
        <button
          onClick={() => setOpen((v) => !v)}
          className="flex min-w-0 flex-1 items-center gap-3 text-left"
        >
          <ProviderLogo provider={model.provider} size={30} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span
                className={`h-2 w-2 shrink-0 rounded-full ${DOT[level]}`}
                title={DOT_TITLE[level]}
              />
              <h3 className="truncate text-sm font-medium">{model.label}</h3>
              {active && (
                <span className="shrink-0 rounded-full bg-sv-accent px-2 py-0.5 text-[10px] font-medium text-white">
                  Active
                </span>
              )}
            </div>
            <p className="mt-0.5 truncate text-[11px] text-sv-muted">
              {model.provider} · {sttLanguage(model)} · {formatMB(model.size_mb)}
            </p>
          </div>
          <Chevron open={open} />
        </button>

        <div className="flex shrink-0 items-center gap-2">
          <button onClick={onTogglePin} title={pinned ? "Unpin" : "Pin to top"} className={pinned ? "rounded-lg p-1.5 transition text-sv-accent" : "rounded-lg p-1.5 transition text-sv-muted hover:text-sv-accent"}><StarIcon filled={pinned} /></button>
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
                <span className="text-[11px] text-sv-good">In use</span>
              ) : (
                <button
                  onClick={onSelect}
                  className="rounded-lg bg-sv-surface-2 px-3 py-1.5 text-xs font-medium hover:bg-sv-accent hover:text-white"
                >
                  Select
                </button>
              )}
              <button
                onClick={() => remove(model.id)}
                title="Remove download"
                className="rounded-lg p-1.5 text-sv-muted transition hover:bg-sv-surface-2 hover:text-sv-bad"
              >
                <TrashIcon />
              </button>
            </>
          ) : (
            <button
              onClick={() => download(model.id)}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs font-medium text-sv-text hover:border-sv-accent hover:text-sv-accent"
            >
              Download
            </button>
          )}
        </div>
      </div>

      {open && (
        <div className="border-t border-sv-border/70 px-3.5 py-3">
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <Stat label="Speed" value={friendlySpeed(model.speed_label)} sub={model.speed_label.replace("~", "")} />
            <Stat label="Accuracy" value={friendlyAccuracy(model.wer)} sub={`${model.wer} errors`} />
            <Stat label="Download" value={formatMB(model.size_mb)} />
            <Stat label="Memory use" value={formatMB(model.ram_mb)} />
          </div>
          <p className="mt-2.5 text-[11px] text-sv-muted">{model.best_for}</p>
        </div>
      )}

      {progress?.status === "error" && (
        <p className="px-3.5 pb-2 text-[11px] text-sv-bad">{progress.error}</p>
      )}
    </div>
  );
}

function Stat({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="rounded-lg bg-sv-surface-2 px-2.5 py-1.5">
      <div className="text-[10px] uppercase tracking-wide text-sv-muted">
        {label}
      </div>
      <div className="text-xs font-medium">{value}</div>
      {sub && <div className="text-[10px] text-sv-muted">{sub}</div>}
    </div>
  );
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={`shrink-0 text-sv-muted transition-transform ${
        open ? "rotate-180" : ""
      }`}
    >
      <path d="M6 9l6 6 6-6" />
    </svg>
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
