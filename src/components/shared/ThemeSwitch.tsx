import { useState } from "react";
import { useSettingsStore } from "../../stores/settingsStore";

// Eight rays at exact 45° steps. Generated rather than hand-placed so they
// cannot drift, and so the radius stays inside the 22px slot (transform-origin
// 50% 11px, height 4px from the top edge = a ray spanning radius 7→11).
const RAYS = Array.from({ length: 8 }, (_, i) => i);

/// Light/dark switch. The moon contracts into the sun's core while its shadow
/// slips away and the rays spring out — see `.sv-theme-sw` in styles.css.
export default function ThemeSwitch({ className = "" }: { className?: string }) {
  const theme = useSettingsStore((s) => s.settings.theme);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const [pulse, setPulse] = useState(false);

  const next = theme === "dark" ? "light" : "dark";

  return (
    <button
      type="button"
      role="switch"
      aria-checked={theme === "light"}
      aria-label={`Switch to ${next} mode`}
      title={`Switch to ${next} mode`}
      data-on={theme}
      onClick={() => {
        setSettings({ theme: next });
        // The arrival bloom only belongs to the sunrise, not the sunset.
        if (next === "light") {
          setPulse(false);
          requestAnimationFrame(() => setPulse(true));
        } else {
          setPulse(false);
        }
      }}
      onAnimationEnd={() => setPulse(false)}
      className={`sv-theme-sw focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface ${
        pulse ? "sv-sw-pulse" : ""
      } ${className}`}
    >
      <span className="sv-sw-bloom" />
      <span className="sv-sw-stars">
        <b />
        <b />
        <b />
        <b />
      </span>
      <span className="sv-sw-slot">
        <span className="sv-sw-body">
          <span className="sv-sw-shade" />
        </span>
        <span className="sv-sw-rays">
          {RAYS.map((i) => (
            <i
              key={i}
              style={{ "--a": `${i * 45}deg`, "--i": i } as React.CSSProperties}
            />
          ))}
        </span>
      </span>
    </button>
  );
}
