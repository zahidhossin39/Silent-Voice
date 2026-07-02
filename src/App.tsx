import { NavLink, Navigate, Route, Routes } from "react-router-dom";
import { useEffect } from "react";
import Home from "./components/dashboard/Home";
import ModelStore from "./components/dashboard/ModelStore";
import Modes from "./components/dashboard/Modes";
import ApiKeys from "./components/dashboard/ApiKeys";
import Settings from "./components/dashboard/Settings";
import History from "./components/dashboard/History";
import OverlayApp from "./components/overlay/OverlayApp";
import Onboarding from "./components/onboarding/Onboarding";
import { useModelStore } from "./stores/modelStore";
import { useSettingsStore } from "./stores/settingsStore";
import { useHistoryStore } from "./stores/historyStore";
import { usePipeline } from "./hooks/usePipeline";
import { useRuntimeSync } from "./hooks/useRuntimeSync";
import {
  HomeIcon,
  StoreIcon,
  ModesIcon,
  KeyIcon,
  GearIcon,
  HistoryIcon,
} from "./components/shared/NavIcons";
import { Titlebar } from "./components/shared/Titlebar";

const NAV = [
  { to: "/home", label: "Home", Icon: HomeIcon },
  { to: "/models", label: "Model Store", Icon: StoreIcon },
  { to: "/modes", label: "Modes", Icon: ModesIcon },
  { to: "/api", label: "API Keys", Icon: KeyIcon },
  { to: "/settings", label: "Settings", Icon: GearIcon },
  { to: "/history", label: "History", Icon: HistoryIcon },
];

export default function App() {
  // The transparent overlay window loads the same bundle with ?view=overlay.
  const isOverlay =
    new URLSearchParams(window.location.search).get("view") === "overlay";

  const theme = useSettingsStore((s) => s.settings.theme);
  const onboarded = useSettingsStore((s) => s.settings.onboarded);
  useEffect(() => {
    // Toggle the light-theme class on <html>; default (no class) is dark.
    document.documentElement.classList.toggle("theme-light", theme === "light");
  }, [theme]);

  if (isOverlay) {
    return <OverlayApp />;
  }

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-sv-base">
      <Titlebar />
      <div className="flex-1 overflow-hidden relative">
        {!onboarded ? <Onboarding /> : <Dashboard />}
      </div>
    </div>
  );
}

function Dashboard() {
  const refresh = useModelStore((s) => s.refresh);
  const hydrate = useHistoryStore((s) => s.hydrate);

  // Subscribe to backend pipeline + download events and keep Rust in sync.
  usePipeline();
  useRuntimeSync();

  useEffect(() => {
    refresh();
    hydrate();
  }, [refresh, hydrate]);

  return (
    <div className="flex h-full text-sv-text">
      {/* Sidebar */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-sv-border bg-sv-surface">
        <div className="flex items-center gap-2 px-5 py-5">
          <svg viewBox="0 0 1024 1024" className="h-8 w-8 rounded-lg shrink-0">
            <rect x="0" y="0" width="1024" height="1024" rx="224" fill="#0d0f14"/>
            <rect x="232" y="432" width="80" height="160" rx="40" fill="#f97316"/>
            <rect x="360" y="352" width="80" height="320" rx="40" fill="#f97316"/>
            <rect x="488" y="252" width="80" height="520" rx="40" fill="#ffffff"/>
            <rect x="616" y="352" width="80" height="320" rx="40" fill="#f97316"/>
            <rect x="744" y="432" width="80" height="160" rx="40" fill="#f97316"/>
          </svg>
          <div>
            <div className="text-sm font-semibold leading-tight">
              Silent Voice
            </div>
            <div className="text-[11px] text-sv-muted">local-first dictation</div>
          </div>
        </div>

        <nav className="flex-1 space-y-1 px-3">
          {NAV.map((n) => (
            <NavLink
              key={n.to}
              to={n.to}
              className={({ isActive }) =>
                `flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition ${
                  isActive
                    ? "bg-sv-accent text-white"
                    : "text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text"
                }`
              }
            >
              <n.Icon className="h-[18px] w-[18px] shrink-0" />
              {n.label}
            </NavLink>
          ))}
        </nav>

        <div className="px-5 py-4 text-[11px] text-sv-muted">
          v0.1.0 · offline-ready
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-y-auto">
        <Routes>
          <Route path="/" element={<Navigate to="/home" replace />} />
          <Route path="/home" element={<Home />} />
          <Route path="/models" element={<ModelStore />} />
          <Route path="/modes" element={<Modes />} />
          <Route path="/api" element={<ApiKeys />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/history" element={<History />} />
        </Routes>
      </main>
    </div>
  );
}
