// Building blocks for the Settings page: rail, topic heading, setting rows,
// the fold-open advanced card, the info popover, search results and small
// controls. Layout decisions came from the prototype rounds on branch
// prototype/settings-redesign (split rail → fold-open advanced → preview card
// with a left disclosure arrow).
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Ctx, SettingDef } from "./catalog";
import { TOPICS, matches, type TopicId } from "./topics";
import { SearchIcon } from "./icons";

// ─── Rail ───────────────────────────────────────────────────────────────────

export function Rail({
  query,
  onQuery,
  current,
  onSelect,
}: {
  query: string;
  onQuery: (q: string) => void;
  current: TopicId;
  onSelect: (id: TopicId) => void;
}) {
  return (
    <nav aria-label="Settings sections" className="sticky top-0 flex w-52 shrink-0 flex-col self-start xl:w-56">
      <label className="relative mb-3 block">
        <span className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-sv-muted">
          <SearchIcon />
        </span>
        <input
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onQuery("");
          }}
          placeholder="Search settings"
          aria-label="Search settings"
          className={inputCls + " w-full py-2 pl-8 text-[13px]"}
        />
      </label>
      {TOPICS.map((t) => {
        const active = !query && current === t.id;
        return (
          <button
            key={t.id}
            type="button"
            onClick={() => {
              onQuery("");
              onSelect(t.id);
            }}
            aria-current={active ? "page" : undefined}
            className={`mb-0.5 flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-[13.5px] transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
              active ? "bg-sv-surface-2 font-medium text-sv-text" : "text-sv-muted hover:bg-sv-surface-2/60 hover:text-sv-text"
            }`}
          >
            <span className={active ? "text-sv-text" : "text-sv-muted"}>
              <t.Icon />
            </span>
            {t.label}
          </button>
        );
      })}
    </nav>
  );
}

// ─── Headings ───────────────────────────────────────────────────────────────

export function TopicHeading({ id, size = "page" }: { id: TopicId; size?: "page" | "section" }) {
  const t = TOPICS.find((x) => x.id === id)!;
  if (size === "section")
    return (
      <h3 className="flex items-center gap-2.5 text-[14px] font-semibold">
        <span className="grid h-7 w-7 place-items-center rounded-lg bg-sv-surface-2 text-sv-text">
          <t.Icon />
        </span>
        {t.label}
      </h3>
    );
  return (
    <div className="flex items-center gap-3">
      <span className="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-sv-surface-2 text-sv-text">
        <t.Icon />
      </span>
      <div>
        <h2 className="text-lg font-semibold leading-tight tracking-[-0.01em]">{t.label}</h2>
        <p className="mt-0.5 text-[13px] text-sv-muted">{t.blurb}</p>
      </div>
    </div>
  );
}

// ─── Rows ───────────────────────────────────────────────────────────────────
// No overflow-hidden: the shared Select draws its menu absolutely inside the
// row, and a clipping ancestor would cut it off. Rows round their own corners.

export function Rows({
  ctx,
  items,
  flash,
  bare = false,
}: {
  ctx: Ctx;
  items: SettingDef[];
  flash: string | null;
  // bare = the caller already provides the card surface.
  bare?: boolean;
}) {
  if (items.length === 0) return null;
  return (
    <div className={`divide-y divide-sv-border/70 ${bare ? "" : "rounded-xl bg-sv-surface"}`}>
      {items.map((x) => (
        <div
          key={x.id}
          id={`set-${x.id}`}
          className={`scroll-mt-24 px-5 py-3.5 transition-colors duration-700 ${bare ? "" : "first:rounded-t-xl last:rounded-b-xl"} ${
            flash === x.id ? "bg-sv-accent/12" : ""
          }`}
        >
          <div className={x.wide ? "" : "flex items-center justify-between gap-6"}>
            <div className="flex min-w-0 items-center gap-1.5">
              <span className="text-[13.5px] text-sv-text">{x.label}</span>
              {x.info && <Info text={typeof x.info === "function" ? x.info(ctx) : x.info} label={x.label} />}
            </div>
            <div className={x.wide ? "mt-3" : "shrink-0"}>{x.render(ctx, x.label)}</div>
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Advanced card: preview when closed, settings when open ─────────────────

function previewText(labels: string[]) {
  const shown = labels.slice(0, 3);
  const rest = labels.length - shown.length;
  return rest > 0 ? `${shown.join(", ")} and ${rest} more` : shown.join(", ");
}

export function AdvancedCard({
  ctx,
  topicLabel,
  items,
  open,
  onToggle,
  flash,
  panelId,
}: {
  ctx: Ctx;
  topicLabel: string;
  items: SettingDef[];
  open: boolean;
  onToggle: () => void;
  flash: string | null;
  panelId: string;
}) {
  if (items.length === 0) return null;
  return (
    <div className="mt-4 rounded-xl bg-sv-surface">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={onToggle}
        className="group flex w-full items-center gap-2.5 rounded-xl px-5 py-4 text-left transition-colors duration-150 hover:bg-sv-surface-2/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
      >
        {/* -ml-1 cancels the arrow glyph's side padding so its visible edge
            lines up with the setting labels below. */}
        <span className="-ml-1 grid h-6 w-6 shrink-0 place-items-center text-sv-muted transition-colors duration-150 group-hover:text-sv-text">
          <svg
            viewBox="0 0 24 24"
            width="18"
            height="18"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.9"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden
            className={`transition-transform duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none ${
              open ? "rotate-0" : "-rotate-90"
            }`}
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-baseline gap-2">
            <span className="text-[14px] font-medium text-sv-text">Advanced {topicLabel.toLowerCase()}</span>
            <span className="text-[12.5px] tabular-nums text-sv-muted">
              {items.length} {items.length === 1 ? "setting" : "settings"}
            </span>
          </span>
          <Collapse open={!open} inline>
            <span className="mt-1 block truncate text-[12.5px] text-sv-muted">
              {previewText(items.map((x) => x.label))}
            </span>
          </Collapse>
        </span>
      </button>
      <Collapse open={open} id={panelId}>
        <div className="mx-5 border-t border-sv-border/70" />
        <Rows ctx={ctx} items={items} flash={flash} bare />
      </Collapse>
    </div>
  );
}

// Animates real height (grid 0fr → 1fr), then stops clipping once open so
// dropdown menus inside can escape. Closed content is inert (not tabbable).
export function Collapse({
  open,
  id,
  inline = false,
  children,
}: {
  open: boolean;
  id?: string;
  // inline = render spans, for use inside a <button>.
  inline?: boolean;
  children: ReactNode;
}) {
  const [clip, setClip] = useState(!open);
  useEffect(() => {
    if (!open) {
      setClip(true);
      return;
    }
    // Timer instead of transitionend: reduced motion fires no transition.
    const t = setTimeout(() => setClip(false), 260);
    return () => clearTimeout(t);
  }, [open]);
  const Outer = inline ? "span" : "div";
  const Inner = inline ? "span" : "div";
  return (
    <Outer
      id={id}
      inert={!open}
      className={`grid transition-[grid-template-rows] duration-[220ms] ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none ${
        open ? "grid-rows-[1fr]" : "grid-rows-[0fr]"
      }`}
    >
      <Inner className={`block min-h-0 ${clip ? "overflow-hidden" : ""}`}>{children}</Inner>
    </Outer>
  );
}

// ─── Search results ─────────────────────────────────────────────────────────

export function SearchResults({
  ctx,
  list,
  query,
  onPick,
}: {
  ctx: Ctx;
  list: SettingDef[];
  query: string;
  onPick: (def: SettingDef) => void;
}) {
  const hits = searchSettings(list, ctx, query);
  const grouped = TOPICS.map((t) => ({ t, items: hits.filter((x) => x.topic === t.id) })).filter((g) => g.items.length);

  if (grouped.length === 0)
    return (
      <div className="pt-16 text-center">
        <p className="text-[14px] text-sv-text">No settings match “{query}”.</p>
        <p className="mt-1 text-[13px] text-sv-muted">Try a plainer word, like “key”, “voice”, “memory” or “grammar”.</p>
      </div>
    );

  return (
    <div>
      <h2 className="text-lg font-semibold tracking-[-0.01em]" aria-live="polite">
        {hits.length} {hits.length === 1 ? "setting" : "settings"} for “{query}”
      </h2>
      {grouped.map(({ t, items }) => (
        <section key={t.id} className="mt-6">
          <TopicHeading id={t.id} size="section" />
          <ul className="mt-2.5 divide-y divide-sv-border/70 rounded-xl bg-sv-surface">
            {items.map((x) => {
              const hidden = !!x.visible && !x.visible(ctx);
              const info = typeof x.info === "function" ? x.info(ctx) : x.info;
              return (
                <li key={x.id}>
                  <button
                    type="button"
                    onClick={() => onPick(x)}
                    className="flex w-full items-center gap-4 px-5 py-3 text-left transition-colors duration-150 first:rounded-t-xl last:rounded-b-xl hover:bg-sv-surface-2/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-sv-accent"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block text-[13.5px] text-sv-text">{x.label}</span>
                      <span className="mt-0.5 block truncate text-[12px] text-sv-muted">
                        {hidden && x.requiresLabel ? `Turn on “${x.requiresLabel}” to use this` : info}
                      </span>
                    </span>
                    {x.tier === "advanced" && (
                      <span className="shrink-0 rounded px-1.5 py-px text-[11px] text-sv-muted ring-1 ring-sv-border">
                        Advanced
                      </span>
                    )}
                    <span className="shrink-0 text-sv-muted" aria-hidden>
                      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
                        <path d="m9 6 6 6-6 6" />
                      </svg>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}

// Settings matching the query. A setting that's hidden until another is turned
// on (e.g. Oxford comma needs underlines) still appears, and picking it jumps
// to the setting it depends on rather than silently changing anything.
// Where a jump should land: the setting itself, or — while it's hidden — the
// nearest parent setting that has to be turned on first.
export function resolveJumpTarget(def: SettingDef, list: SettingDef[], ctx: Ctx): SettingDef {
  let target = def;
  const seen = new Set<string>();
  while (target.visible && !target.visible(ctx) && target.requires && !seen.has(target.id)) {
    seen.add(target.id);
    target = list.find((x) => x.id === target.requires) ?? target;
  }
  return target;
}

export function searchSettings(list: SettingDef[], ctx: Ctx, query: string) {
  return list.filter((x) => matches(x, query) && (!x.visible || x.visible(ctx) || !!x.requires));
}

// Scroll a setting into view and briefly highlight it.
export function useJump() {
  const [flash, setFlash] = useState<string | null>(null);
  useEffect(() => {
    if (!flash) return;
    const raf = requestAnimationFrame(() =>
      document.getElementById(`set-${flash}`)?.scrollIntoView({ block: "center", behavior: "smooth" })
    );
    const t = setTimeout(() => setFlash(null), 1800);
    return () => {
      cancelAnimationFrame(raf);
      clearTimeout(t);
    };
  }, [flash]);
  return [flash, setFlash] as const;
}

// ─── Info popover ───────────────────────────────────────────────────────────
// Click to open. Fixed-position so a scrolling or clipped parent can't cut it
// off; closes on outside click, Escape, scroll or resize.

export function Info({ text, label }: { text: string; label: string }) {
  const btn = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number; up: boolean } | null>(null);

  useEffect(() => {
    if (!pos) return;
    const close = () => setPos(null);
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!btn.current?.contains(t) && !panel.current?.contains(t)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        close();
        btn.current?.focus();
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [pos]);

  return (
    <>
      <button
        ref={btn}
        type="button"
        aria-label={`About ${label}`}
        aria-expanded={!!pos}
        onClick={() => {
          if (pos) return setPos(null);
          const r = btn.current!.getBoundingClientRect();
          const up = r.bottom + 160 > window.innerHeight;
          setPos({
            x: Math.max(12, Math.min(r.left - 12, window.innerWidth - 300)),
            y: up ? r.top - 8 : r.bottom + 8,
            up,
          });
        }}
        className={`grid h-[18px] w-[18px] shrink-0 place-items-center rounded-full transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
          pos ? "bg-sv-surface-2 text-sv-text" : "text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text"
        }`}
      >
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" aria-hidden>
          <circle cx="12" cy="12" r="8.5" />
          <path d="M12 11v5M12 8h.01" />
        </svg>
      </button>
      {pos && (
        <div
          ref={panel}
          role="note"
          style={{ left: pos.x, top: pos.y, transform: pos.up ? "translateY(-100%)" : undefined }}
          className="fixed z-50 w-[288px] rounded-xl bg-sv-surface-2 px-3.5 py-3 text-[12.5px] leading-relaxed text-sv-text shadow-[0_12px_32px_-8px_rgba(0,0,0,0.55)]"
        >
          {text}
        </div>
      )}
    </>
  );
}

// ─── Controls ───────────────────────────────────────────────────────────────

export const inputCls =
  "rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm text-sv-text placeholder:text-sv-muted/70 transition-colors focus:border-sv-accent focus:outline-none";

export function Toggle({
  checked,
  onChange,
  label,
  disabled = false,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full border transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface disabled:opacity-40 ${
        checked ? "border-transparent bg-sv-accent" : "border-sv-border bg-sv-surface-2 hover:border-sv-muted/60"
      }`}
    >
      <span
        className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.35)] transition-[left] duration-150 ${
          checked ? "left-[22px]" : "left-0.5"
        }`}
      />
    </button>
  );
}

export function Slider({
  value,
  min,
  max,
  step = 1,
  onChange,
  left,
  right,
  label,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
  left: string;
  right: string;
  label: string;
}) {
  return (
    <div className="w-full max-w-md">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
        className="sv-slider w-full"
        style={{ "--sv-slider-fill": `${((value - min) / (max - min)) * 100}%` } as React.CSSProperties}
      />
      <div className="mt-1.5 flex justify-between text-[11px] text-sv-muted">
        <span>{left}</span>
        <span className="tabular-nums text-sv-text">{value}</span>
        <span>{right}</span>
      </div>
    </div>
  );
}

export function Segmented({
  value,
  options,
  onChange,
  label,
}: {
  value: string;
  options: [string, string][];
  onChange: (v: string) => void;
  label: string;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex rounded-lg bg-sv-bg p-0.5 ring-1 ring-sv-border">
      {options.map(([v, l]) => (
        <button
          key={v}
          type="button"
          role="radio"
          aria-checked={value === v}
          onClick={() => onChange(v)}
          className={`rounded-md px-3 py-1 text-[12.5px] transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
            value === v ? "bg-sv-surface-2 font-medium text-sv-text" : "text-sv-muted hover:text-sv-text"
          }`}
        >
          {l}
        </button>
      ))}
    </div>
  );
}

export function GhostButton({
  children,
  onClick,
  disabled = false,
}: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="inline-flex items-center gap-1.5 rounded-lg border border-sv-border px-3 py-1.5 text-[12.5px] text-sv-text transition-colors duration-150 hover:border-sv-muted/60 hover:bg-sv-surface-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent disabled:pointer-events-none disabled:opacity-40"
    >
      {children}
    </button>
  );
}

export function IconButton({
  label,
  onClick,
  danger = false,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={`grid h-8 w-8 shrink-0 place-items-center rounded-lg text-sv-muted transition-colors duration-150 hover:bg-sv-surface-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
        danger ? "hover:text-sv-bad" : "hover:text-sv-text"
      }`}
    >
      {children}
    </button>
  );
}
