import type { Settings } from "../../types";
import { useSettingsStore } from "../../stores/settingsStore";

// Theme choice: Match Windows · Light · Dark. A filled circle slides to the
// selected option. Keyboard: Tab to the group, arrow keys to move (radio group).
const OPTIONS: { id: Settings["theme"]; label: string; Icon: () => React.ReactNode }[] = [
  { id: "system", label: "Match Windows", Icon: MonitorIcon },
  { id: "light", label: "Light", Icon: SunIcon },
  { id: "dark", label: "Dark", Icon: MoonIcon },
];

// Each option is 28px wide plus a 2px gap, so the indicator moves 30px a step.
const STEP = 30;

export default function ThemeSwitch({ className = "" }: { className?: string }) {
  const theme = useSettingsStore((s) => s.settings.theme);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const index = Math.max(0, OPTIONS.findIndex((o) => o.id === theme));

  const choose = (i: number) => setSettings({ theme: OPTIONS[(i + OPTIONS.length) % OPTIONS.length].id });

  return (
    <div
      role="radiogroup"
      aria-label="Theme"
      className={`relative flex shrink-0 rounded-full p-[3px] ring-1 ring-sv-border ${className}`}
      onKeyDown={(e) => {
        if (e.key === "ArrowRight" || e.key === "ArrowDown") {
          e.preventDefault();
          choose(index + 1);
        } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
          e.preventDefault();
          choose(index - 1);
        }
      }}
    >
      <span
        aria-hidden
        className="absolute left-[3px] top-[3px] h-7 w-7 rounded-full bg-sv-surface-2 transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none"
        style={{ transform: `translateX(${index * STEP}px)` }}
      />
      {OPTIONS.map(({ id, label, Icon }, i) => {
        const on = i === index;
        return (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={on}
            aria-label={label}
            title={label}
            // Roving tabindex: only the selected option is in the tab order.
            tabIndex={on ? 0 : -1}
            onClick={() => choose(i)}
            ref={(el) => {
              // Keep focus on the chosen option after arrow-key changes.
              if (on && el && el.parentElement?.contains(document.activeElement) && document.activeElement !== el) el.focus();
            }}
            className={`relative z-10 grid h-7 w-7 place-items-center rounded-full transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent ${
              i > 0 ? "ml-[2px]" : ""
            } ${on ? "text-sv-text" : "text-sv-muted hover:text-sv-text"}`}
          >
            <Icon />
          </button>
        );
      })}
    </div>
  );
}

function MonitorIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <rect x="3.5" y="4.5" width="17" height="11.5" rx="2" />
      <path d="M9 20h6M12 16v4" />
    </svg>
  );
}

function SunIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" aria-hidden>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2.5v2M12 19.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4" />
    </svg>
  );
}

function MoonIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" />
    </svg>
  );
}
