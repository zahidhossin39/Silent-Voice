import type { AppTheme } from "../types";

export interface AppAccent {
  id: AppTheme;
  label: string;
  swatch: string;
  accent: string;
  hover: string;
  onAccent: string;
  blurb: string;
}

export const APP_THEMES: AppAccent[] = [
  { id: "orange", label: "Ember", swatch: "#f97316", accent: "#f97316", hover: "#ea580c", onAccent: "#1a1200", blurb: "The original Silent Voice warmth." },
  { id: "ocean", label: "Ocean", swatch: "#3b82f6", accent: "#3b82f6", hover: "#2563eb", onAccent: "#ffffff", blurb: "Calm, focused blue." },
  { id: "iris", label: "Iris", swatch: "#8b5cf6", accent: "#8b5cf6", hover: "#7c3aed", onAccent: "#ffffff", blurb: "Soft creative violet." },
  { id: "reef", label: "Reef", swatch: "#14b8a6", accent: "#14b8a6", hover: "#0d9488", onAccent: "#052620", blurb: "Fresh, easy teal." },
  { id: "rose", label: "Rose", swatch: "#f43f5e", accent: "#f43f5e", hover: "#e11d48", onAccent: "#ffffff", blurb: "Warm, bold pink-red." },
  { id: "meadow", label: "Meadow", swatch: "#22c55e", accent: "#22c55e", hover: "#16a34a", onAccent: "#04240f", blurb: "Vivid natural green." },
];

// "orange" is the built-in default in styles.css (and it alone adapts to
// light/dark), so for it we REMOVE the overrides and let the stylesheet win.
export function applyAppTheme(id: AppTheme) {
  const s = document.documentElement.style;
  const t = APP_THEMES.find((x) => x.id === id);
  if (!t || t.id === "orange") {
    s.removeProperty("--color-sv-accent");
    s.removeProperty("--color-sv-accent-hover");
    s.removeProperty("--color-sv-on-accent");
    return;
  }
  s.setProperty("--color-sv-accent", t.accent);
  s.setProperty("--color-sv-accent-hover", t.hover);
  s.setProperty("--color-sv-on-accent", t.onAccent);
}
