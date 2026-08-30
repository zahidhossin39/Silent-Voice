import { useSettingsStore } from "../../stores/settingsStore";

/// Light/dark switch. The sun and moon are drawn as two separate bodies that
/// cross-fade as the slot travels — see `.sv-theme-sw` in styles.css for the
/// craters, limb darkening, and corona that tell them apart by form.
export default function ThemeSwitch({ className = "" }: { className?: string }) {
  const theme = useSettingsStore((s) => s.settings.theme);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const next = theme === "dark" ? "light" : "dark";

  return (
    <button
      type="button"
      role="switch"
      aria-checked={theme === "light"}
      aria-label={`Switch to ${next} mode`}
      title={`Switch to ${next} mode`}
      data-on={theme}
      onClick={() => setSettings({ theme: next })}
      className={`sv-theme-sw focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent focus-visible:ring-offset-2 focus-visible:ring-offset-sv-surface ${className}`}
    >
      <span className="sv-sw-stars">
        <b />
        <b />
        <b />
      </span>
      <span className="sv-sw-slot">
        <span className="sv-orb sv-orb-moon">
          <span className="sv-orb-face">
            <i className="sv-crater" />
            <i className="sv-crater" />
            <i className="sv-crater" />
            <i className="sv-crater" />
          </span>
        </span>
        <span className="sv-orb sv-orb-sun">
          <span className="sv-corona" />
          <span className="sv-orb-face" />
        </span>
      </span>
    </button>
  );
}
