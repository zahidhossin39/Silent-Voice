import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "../../services/tauriBridge";

const appWindow = isTauri() ? getCurrentWindow() : null;

export function Titlebar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!appWindow) return;
    appWindow.isMaximized().then(setMaximized);
    const unlisten = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div
      data-tauri-drag-region
      className="flex h-10 w-full shrink-0 select-none items-center justify-end bg-sv-surface pl-4 transition-colors"
    >
      <div className="flex h-full">
        {/* Minimize */}
        <button
          className="flex h-full w-12 items-center justify-center text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text"
          onClick={() => appWindow?.minimize()}
          title="Minimize" aria-label="Minimize"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <path d="M5 11h14v2H5z" />
          </svg>
        </button>

        {/* Maximize / Restore */}
        <button
          className="flex h-full w-12 items-center justify-center text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text"
          onClick={() => appWindow?.toggleMaximize()}
          title={maximized ? "Restore" : "Maximize"}
        >
          {maximized ? (
            <svg
              viewBox="0 0 24 24"
              width="12"
              height="12"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <rect x="6" y="3" width="15" height="15" rx="2" ry="2" />
              <path d="M3 8v13h13" />
            </svg>
          ) : (
            <svg
              viewBox="0 0 24 24"
              width="12"
              height="12"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
            </svg>
          )}
        </button>

        {/* Close */}
        <button
          className="flex h-full w-12 items-center justify-center text-sv-muted hover:bg-sv-bad hover:text-white"
          onClick={() => appWindow?.close()}
          title="Close" aria-label="Close"
        >
          <svg
            viewBox="0 0 24 24"
            width="16"
            height="16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  );
}
