import { useState } from "react";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { APP_THEMES } from "../../services/appThemes";
import type { PopupTheme, PopupStyle } from "../../types";

// Accent palettes for the grammar-suggestion popup. The surface is always the
// dark card below; only the accent (border ring, primary pill, chips, footer
// highlight) swaps. Order + ids match squiggle.rs PALETTES exactly.
const PALETTES: {
  id: PopupTheme;
  label: string;
  dot: string;
  bg: string;
  pill: string;
  deep: string; // deep tonal fill for the primary pill (accent · ~0.28 over black)
  deepHover: string; // brighter pill fill on hover (deep · 0.6 + accent · 0.4)
  blurb: string;
}[] = [
  { id: "violet", label: "Violet", dot: "#a78bfa", bg: "rgba(167,139,250,.14)", pill: "rgba(167,139,250,.30)", deep: "#2f2746", deepHover: "#5f4f8e", blurb: "Calm, creative. Reads as “improve”, not “wrong”." },
  { id: "teal", label: "Teal", dot: "#2dd4bf", bg: "rgba(45,212,191,.13)", pill: "rgba(45,212,191,.28)", deep: "#0d3b35", deepHover: "#1a786c", blurb: "Fresh and quiet. Easy on the eyes for long edits." },
  { id: "amber-blue", label: "Amber → Blue", dot: "#388bfd", bg: "rgba(56,139,253,.15)", pill: "rgba(56,139,253,.30)", deep: "#102747", deepHover: "#204f90", blurb: "Diff-style: warm original, cool fix. Colorblind-safe." },
  { id: "orange", label: "Orange", dot: "#f97316", bg: "rgba(249,115,22,.13)", pill: "rgba(249,115,22,.28)", deep: "#462006", deepHover: "#8e410c", blurb: "Matches Silent Voice’s own accent." },
  { id: "brightness", label: "Brightness", dot: "#8b93a7", bg: "rgba(255,255,255,.10)", pill: "rgba(255,255,255,.16)", deep: "#27292f", deepHover: "#4f535f", blurb: "No hue at all — pure light/dark contrast." },
];

type Accent = (typeof PALETTES)[number];

// The recommended fix: a deep tonal pill with the fix text in the accent color
// and a small enter-key badge (press-to-apply hint) on the RIGHT. No border.
// Shared by both layouts.
function PrimaryPill({ accent, text }: { accent: Accent; text: string }) {
  const [hover, setHover] = useState(false);
  return (
    <span
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      className="inline-flex cursor-pointer items-center gap-2 rounded-lg py-[5px] pl-3.5 pr-[5px] transition-colors"
      style={{ background: hover ? accent.deepHover : accent.deep }}
    >
      <span className="text-[15px] font-semibold" style={{ color: accent.dot }}>
        {text}
      </span>
      <span
        className="grid h-6 w-6 place-items-center rounded-md text-[13px] leading-none"
        style={{ background: "rgba(255,255,255,0.08)", color: accent.dot }}
      >
        ↵
      </span>
    </span>
  );
}

const LAYOUTS: { id: PopupStyle; label: string; blurb: string }[] = [
  { id: "insights", label: "Insights", blurb: "Title, subtitle, and each fix on its own line — roomy and calm." },
  { id: "compact", label: "Compact", blurb: "Before → after with a filled pill and numbered alternatives — dense and quick." },
];

// Dark surface shared by every palette (mirrors squiggle.rs + the design).
const SURFACE = {
  bg: "#151a26",
  fg: "#e8ebf2",
  muted: "#8b93a7",
  alt: "#aab2c4",
  line: "rgba(255,255,255,.10)",
  footer: "rgba(255,255,255,.045)",
  chip: "rgba(255,255,255,.05)",
};

export default function Theme() {
  const popupTheme = useSettingsStore((s) => s.settings.popup_theme);
  const popupStyle = useSettingsStore((s) => s.settings.popup_style);
  const appTheme = useSettingsStore((s) => s.settings.app_theme);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const accent = PALETTES.find((p) => p.id === popupTheme) ?? PALETTES[0];

  return (
    <Page
      title="Theme"
      subtitle="Give Silent Voice its color. The app accent recolors the whole window and the dictation pill live; below it you can style the grammar-suggestion popup separately."
    >
      {/* App accent — selecting one applies instantly across the app, so this
          page itself is the live preview. */}
      <section className="mb-10">
        <div className="mb-1 flex items-center gap-2">
          <h2 className="text-sm font-medium text-sv-text">App accent</h2>
          <span className="rounded-full border border-sv-accent/40 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-sv-accent">Whole app</span>
        </div>
        <p className="mb-4 text-xs text-sv-muted">
          Colors the sidebar, buttons, links, focus rings, and the recording
          pill. Applies the moment you pick it.
        </p>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
          {APP_THEMES.map((t) => {
            const selected = t.id === appTheme;
            return (
              <button
                key={t.id}
                onClick={() => setSettings({ app_theme: t.id })}
                aria-pressed={selected}
                className={`group flex items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition ${
                  selected
                    ? "border-sv-accent bg-sv-surface-2"
                    : "border-sv-border bg-sv-surface hover:border-sv-muted/50 hover:bg-sv-surface-2"
                }`}
              >
                <span
                  className="grid h-9 w-9 shrink-0 place-items-center rounded-lg ring-1 ring-white/10"
                  style={{ background: t.accent }}
                >
                  {selected && <Check style={{ color: t.onAccent }} />}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium text-sv-text">
                    {t.label}
                  </span>
                  <span className="block truncate text-xs text-sv-muted">
                    {t.blurb}
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      </section>

      <div className="mb-4 border-t border-sv-border pt-8">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-sv-text">Grammar popup</h2>
          <span className="rounded-full border border-sv-border px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-sv-muted">Popup only</span>
        </div>
        <p className="mt-1 text-xs text-sv-muted">
          The suggestion card that appears over your text as you write. Its
          accent is independent of the app accent — the card stays dark so it
          looks right in any app.
        </p>
      </div>

      <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        {/* Left: pickers */}
        <div className="space-y-8">
          {/* Layout */}
          <section>
            <h2 className="mb-3 text-sm font-medium text-sv-muted">Layout</h2>
            <div className="grid grid-cols-2 gap-3">
              {LAYOUTS.map((l) => {
                const selected = l.id === popupStyle;
                return (
                  <button
                    key={l.id}
                    onClick={() => setSettings({ popup_style: l.id })}
                    aria-pressed={selected}
                    className={`flex flex-col gap-3 rounded-xl border p-3 text-left transition ${
                      selected
                        ? "border-sv-accent bg-sv-surface-2"
                        : "border-sv-border bg-sv-surface hover:border-sv-muted/50 hover:bg-sv-surface-2"
                    }`}
                  >
                    <div className="flex h-[172px] items-center justify-center overflow-hidden rounded-lg bg-sv-bg/60 p-3">
                      {/* `zoom` (not transform:scale) shrinks the popup's actual
                          layout footprint, so it centers cleanly and never clips. */}
                      <div style={{ zoom: 0.52 }}>
                        <Popup accent={accent} style={l.id} />
                      </div>
                    </div>
                    <div>
                      <div className="flex items-center gap-1.5 text-sm font-medium text-sv-text">
                        {l.label}
                        {selected && <Check className="text-sv-accent" />}
                      </div>
                      <div className="mt-0.5 text-xs text-sv-muted">{l.blurb}</div>
                    </div>
                  </button>
                );
              })}
            </div>
          </section>

          {/* Accent */}
          <section>
            <h2 className="mb-3 text-sm font-medium text-sv-muted">Accent</h2>
            <div className="space-y-2.5">
              {PALETTES.map((p) => {
                const selected = p.id === popupTheme;
                return (
                  <button
                    key={p.id}
                    onClick={() => setSettings({ popup_theme: p.id })}
                    aria-pressed={selected}
                    className={`flex w-full items-center gap-3.5 rounded-xl border px-4 py-3 text-left transition ${
                      selected
                        ? "border-sv-accent bg-sv-surface-2"
                        : "border-sv-border bg-sv-surface hover:border-sv-muted/50 hover:bg-sv-surface-2"
                    }`}
                  >
                    <span
                      className="h-6 w-6 shrink-0 rounded-full ring-2 ring-white/10"
                      style={{ background: p.dot }}
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block text-sm font-medium text-sv-text">
                        {p.label}
                      </span>
                      <span className="block truncate text-xs text-sv-muted">
                        {p.blurb}
                      </span>
                    </span>
                    <span
                      className={`grid h-5 w-5 shrink-0 place-items-center rounded-full border transition ${
                        selected
                          ? "border-sv-accent bg-sv-accent text-sv-on-accent"
                          : "border-sv-border"
                      }`}
                    >
                      {selected && <Check />}
                    </span>
                  </button>
                );
              })}
            </div>
          </section>

        </div>

        {/* Right: live preview */}
        <section className="lg:sticky lg:top-7 lg:self-start">
          <h2 className="mb-3 text-sm font-medium text-sv-muted">Preview</h2>
          <div className="grid min-h-[340px] place-items-center rounded-xl border border-sv-border bg-[repeating-linear-gradient(135deg,#0d1017,#0d1017_12px,#0f131b_12px,#0f131b_24px)] p-6">
            <Popup accent={accent} style={popupStyle} />
          </div>
          <p className="mt-3 text-center text-xs text-sv-muted">
            Hover a flagged word in your real text to see this. Click the fix to
            apply it — no extra button.
          </p>
        </section>
      </div>
    </Page>
  );
}

function Check({ className = "", style }: { className?: string; style?: React.CSSProperties }) {
  return (
    <svg viewBox="0 0 24 24" className={`h-3 w-3 ${className}`} style={style} fill="none" stroke="currentColor" strokeWidth={3} strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 12l5 5 9-10" />
    </svg>
  );
}

function Popup({ accent, style }: { accent: Accent; style: PopupStyle }) {
  return style === "compact" ? (
    <CompactPreview accent={accent} />
  ) : (
    <InsightsPreview accent={accent} />
  );
}

// Faithful replica of squiggle.rs render_insights.
function InsightsPreview({ accent }: { accent: Accent }) {
  return (
    <div
      className="w-[300px] overflow-hidden rounded-[20px] shadow-2xl"
      style={{ background: SURFACE.bg, border: `1.5px solid ${accent.dot}` }}
    >
      <div className="px-4 pb-3 pt-3.5">
        <div className="text-[15px] font-semibold" style={{ color: SURFACE.fg }}>
          Grammar Insights
        </div>
        <div className="mt-0.5 text-[12px]" style={{ color: SURFACE.muted }}>
          This sentence needs a small fix
        </div>
      </div>
      <div className="px-2.5">
        <PrimaryPill accent={accent} text="their meeting" />
      </div>
      <div className="mt-0.5 px-2.5 pb-1">
        <div className="px-3.5 py-2 text-[14px]" style={{ color: SURFACE.alt }}>
          there meeting
        </div>
        <div
          className="border-t px-3.5 py-2 text-[14px]"
          style={{ borderColor: SURFACE.line, color: SURFACE.alt }}
        >
          they’re meeting
        </div>
      </div>
      <Footer accent={accent} />
    </div>
  );
}

// Faithful replica of squiggle.rs render_compact.
function CompactPreview({ accent }: { accent: Accent }) {
  return (
    <div
      className="w-[300px] overflow-hidden rounded-[20px] px-4 pb-0 pt-3.5 shadow-2xl"
      style={{ background: SURFACE.bg, border: `1.5px solid ${accent.dot}` }}
    >
      {/* Category label */}
      <div className="flex items-center gap-2">
        <span className="h-2 w-2 rounded-sm" style={{ background: accent.dot }} />
        <span
          className="text-[11px] font-bold tracking-[0.12em]"
          style={{ color: accent.dot }}
        >
          GRAMMAR
        </span>
      </div>

      {/* before → pill */}
      <div className="mt-2.5 flex flex-wrap items-center gap-2">
        <span className="text-[15px] line-through" style={{ color: SURFACE.muted }}>
          more better
        </span>
        <span className="text-[15px]" style={{ color: SURFACE.muted }}>
          →
        </span>
        <PrimaryPill accent={accent} text="better" />
      </div>

      {/* alternative chips */}
      <div className="mt-3 flex flex-wrap gap-2">
        {[
          { t: "much better", n: 2 },
          { t: "far better", n: 3 },
        ].map((c) => (
          <span
            key={c.t}
            className="inline-flex items-start rounded-lg px-3 py-1.5 text-[14px]"
            style={{ background: SURFACE.chip, color: SURFACE.alt }}
          >
            {c.t}
            <sup className="ml-0.5 text-[10px]" style={{ color: SURFACE.muted }}>
              {c.n}
            </sup>
          </span>
        ))}
      </div>

      {/* explanation */}
      <p className="mt-3 text-[13px] leading-snug" style={{ color: SURFACE.muted }}>
        Using{" "}
        <span className="font-semibold" style={{ color: SURFACE.fg }}>
          more
        </span>{" "}
        with a comparative adjective is redundant.
      </p>

      <div className="-mx-4 mt-3">
        <Footer accent={accent} />
      </div>
    </div>
  );
}

function Footer({ accent }: { accent: Accent }) {
  return (
    <div
      className="flex items-center gap-6 px-4 py-3 text-[13px]"
      style={{ background: SURFACE.footer, color: SURFACE.fg }}
    >
      <span className="flex items-center gap-1.5">
        <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke={accent.dot} strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 5.5A1.5 1.5 0 0 1 5.5 4H18a1 1 0 0 1 1 1v13.5a1 1 0 0 1-1 1H6a2 2 0 0 0-2 2z" />
          <path d="M4 17.5A1.5 1.5 0 0 1 5.5 16H19" />
        </svg>
        Add to dictionary
      </span>
      <span className="flex items-center gap-1.5" style={{ color: SURFACE.muted }}>
        <svg viewBox="0 0 24 24" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
        Dismiss
      </span>
    </div>
  );
}
