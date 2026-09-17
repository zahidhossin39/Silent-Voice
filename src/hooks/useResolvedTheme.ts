import { useEffect, useState } from "react";
import { useSettingsStore } from "../stores/settingsStore";

const query = "(prefers-color-scheme: dark)";

// The theme actually on screen. "system" follows the Windows light/dark
// setting (WebView2 reports it through prefers-color-scheme) and updates live
// when Windows switches.
export function useResolvedTheme(): "dark" | "light" {
  const theme = useSettingsStore((s) => s.settings.theme);
  const [osDark, setOsDark] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia(query);
    setOsDark(mq.matches);
    const onChange = (e: MediaQueryListEvent) => setOsDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme]);
  return theme === "system" ? (osDark ? "dark" : "light") : theme;
}
