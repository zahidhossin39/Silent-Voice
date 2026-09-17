// How well a model runs on this PC, shown two ways: the staff-pick lists are
// grouped under plain headings (no coloured dots to decode), and downloading
// anything outside "runs well" stops for a short check first.
import { useEffect, useRef } from "react";
import type { HardwareInfo } from "../../../types";

export type Fit = "good" | "warn" | "bad";

// macOS and Linux report most RAM as "used" because page cache counts as used,
// even though the OS hands it back on demand. Judging fit on available memory
// alone would mark every model too heavy on those platforms.
export function usableRamGb(hw: HardwareInfo): number {
  return Math.max(hw.available_ram_gb, hw.total_ram_gb * 0.5);
}

export function fitForRamGb(needGb: number, hw: HardwareInfo | null): Fit {
  if (!hw) return "good";
  if (needGb > usableRamGb(hw)) return "bad";
  if (needGb > usableRamGb(hw) * 0.8) return "warn";
  return "good";
}

export const FIT_ORDER: Fit[] = ["good", "warn", "bad"];

export function formatGb(n: number) {
  return `${n >= 10 ? Math.round(n) : Math.round(n * 10) / 10} GB`;
}

// ─── Group heading ──────────────────────────────────────────────────────────

const GROUP_TITLE: Record<Fit, string> = {
  good: "Runs well on this PC",
  warn: "Runs, but may be slow",
  bad: "Too heavy for this PC",
};

export function FitGroupHeading({ fit, count, why }: { fit: Fit; count: number; why?: string }) {
  return (
    <div className="mt-5 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-0.5 first:mt-0">
      <h4 className="text-[13.5px] font-semibold text-sv-text">
        {GROUP_TITLE[fit]}
        <span className="ml-1.5 font-normal tabular-nums text-sv-muted">{count}</span>
      </h4>
      {why && <span className="text-[12px] text-sv-muted">{why}</span>}
    </div>
  );
}

export function memoryWhy(fit: Fit, hw: HardwareInfo | null) {
  if (!hw) return undefined;
  const spare = formatGb(usableRamGb(hw));
  return fit === "good"
    ? `Comfortably inside the ${spare} of memory this PC can spare`
    : fit === "warn"
    ? `Uses most of the ${spare} this PC can spare`
    : `Needs more than the ${spare} this PC can spare`;
}

// ─── Download check ─────────────────────────────────────────────────────────

export interface FitCheck {
  fit: Exclude<Fit, "good">;
  modelName: string;
  // Memory comparison, when memory is the reason. Omit when the reason is
  // something else (e.g. an AI model that needs a graphics card).
  needGb?: number;
  spareGb?: number;
  // Plain-language reason used when there's no memory comparison.
  reason?: string;
  // What happens if you go ahead. Defaults to the voice-model wording.
  consequence?: string;
  // A lighter pick that runs well here.
  alternative?: { name: string; detail: string; actionLabel: string; onChoose: () => void };
}

export function FitCheckDialog({
  check,
  onDownloadAnyway,
  onCancel,
}: {
  check: FitCheck | null;
  onDownloadAnyway: () => void;
  onCancel: () => void;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const anywayRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!check) return;
    const returnTo = document.activeElement as HTMLElement | null;
    // The safe choice gets focus when the model is too heavy; for a model that
    // merely runs slowly, going ahead is reasonable.
    (check.fit === "bad" ? cancelRef : anywayRef).current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      } else if (e.key === "Tab" && dialogRef.current) {
        const items = dialogRef.current.querySelectorAll<HTMLElement>("button");
        const first = items[0];
        const last = items[items.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      returnTo?.focus?.();
    };
  }, [check, onCancel]);

  if (!check) return null;
  const heavy = check.fit === "bad";
  const title = heavy ? `${check.modelName} is too heavy for this PC` : `${check.modelName} may run slowly on this PC`;
  const consequence =
    check.consequence ??
    (heavy
      ? "It will probably fail to load, or slow your whole PC down while it runs."
      : "It will still work, but each transcription takes longer and other apps may slow down while it runs.");

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 animate-[sv-fade-in_160ms_ease-out] motion-reduce:animate-none"
      onMouseDown={onCancel}
    >
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="fit-check-title"
        aria-describedby="fit-check-body"
        onMouseDown={(e) => e.stopPropagation()}
        className="w-full max-w-[440px] rounded-2xl border border-sv-border bg-sv-surface shadow-[0_24px_60px_-16px_rgba(0,0,0,0.6)] animate-[sv-dialog-in_200ms_cubic-bezier(0.22,1,0.36,1)] motion-reduce:animate-none"
      >
        <div className="px-6 pb-5 pt-6">
          <h2 id="fit-check-title" className="text-[15px] font-semibold leading-snug tracking-[-0.01em] text-sv-text">
            {title}
          </h2>

          <div id="fit-check-body">
            {check.needGb !== undefined && check.spareGb !== undefined ? (
              <MemoryComparison needGb={check.needGb} spareGb={check.spareGb} heavy={heavy} />
            ) : (
              check.reason && <p className="mt-3 text-[13px] leading-relaxed text-sv-text">{check.reason}</p>
            )}
            <p className="mt-3 text-[13px] leading-relaxed text-sv-muted">{consequence}</p>
          </div>

          {check.alternative && (
            <div className="mt-5 flex items-center justify-between gap-4 rounded-xl bg-sv-bg/60 px-4 py-3 ring-1 ring-inset ring-sv-border">
              <div className="min-w-0">
                <div className="text-[12px] text-sv-muted">Runs well on this PC</div>
                <div className="truncate text-[13.5px] font-medium text-sv-text">{check.alternative.name}</div>
                <div className="text-[12px] text-sv-muted">{check.alternative.detail}</div>
              </div>
              <button
                type="button"
                onClick={check.alternative.onChoose}
                className="shrink-0 rounded-lg bg-sv-accent px-3 py-1.5 text-[12.5px] font-medium text-sv-on-accent transition-colors duration-150 hover:bg-sv-accent-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface"
              >
                {check.alternative.actionLabel}
              </button>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-sv-border px-6 py-4">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            className="rounded-lg border border-sv-border px-3.5 py-1.5 text-[13px] text-sv-text transition-colors duration-150 hover:bg-sv-surface-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
          >
            Cancel
          </button>
          <button
            ref={anywayRef}
            type="button"
            onClick={onDownloadAnyway}
            className={
              heavy || check.alternative
                ? "rounded-lg border border-sv-border px-3.5 py-1.5 text-[13px] text-sv-muted transition-colors duration-150 hover:border-sv-bad/40 hover:bg-sv-bad/10 hover:text-sv-bad focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
                : "rounded-lg bg-sv-accent px-3.5 py-1.5 text-[13px] font-medium text-sv-on-accent transition-colors duration-150 hover:bg-sv-accent-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface"
            }
          >
            Download anyway
          </button>
        </div>
      </div>
    </div>
  );
}

// One track, drawn to scale: the part this PC can spare, the model's need laid
// over it, and — when the need is larger — the overflow past the limit.
function MemoryComparison({ needGb, spareGb, heavy }: { needGb: number; spareGb: number; heavy: boolean }) {
  const scale = Math.max(needGb, spareGb) * 1.06;
  const pct = (n: number) => `${(n / scale) * 100}%`;
  const within = Math.min(needGb, spareGb);
  return (
    <div className="mt-4">
      <div className="flex items-baseline justify-between text-[12.5px]">
        <span className="text-sv-text">
          Needs <span className="font-semibold tabular-nums">{formatGb(needGb)}</span>
        </span>
        <span className="text-sv-muted">
          This PC can spare <span className="tabular-nums text-sv-text">{formatGb(spareGb)}</span>
        </span>
      </div>
      <div className="relative mt-2 h-2.5 rounded-full bg-sv-bg ring-1 ring-inset ring-sv-border">
        <div className="absolute inset-y-0 left-0 rounded-full bg-sv-surface-2" style={{ width: pct(spareGb) }} />
        <div
          className="absolute inset-y-0 left-0 rounded-full"
          style={{ width: pct(within), backgroundColor: "color-mix(in srgb, var(--color-sv-text) 58%, transparent)" }}
        />
        {needGb > spareGb && (
          <div
            className="absolute inset-y-0 rounded-r-full bg-sv-bad"
            style={{ left: pct(spareGb), width: `calc(${pct(needGb - spareGb)})` }}
          />
        )}
        <div
          aria-hidden
          className="absolute -bottom-1 -top-1 w-px bg-sv-text/70"
          style={{ left: pct(spareGb) }}
        />
      </div>
      <p className="mt-2 text-[12px] text-sv-muted">
        {heavy
          ? `${formatGb(needGb - spareGb)} more than this PC has free.`
          : `That leaves about ${formatGb(Math.max(0, spareGb - needGb))} for everything else.`}
      </p>
    </div>
  );
}
