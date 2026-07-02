import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

export function Titlebar() {
  return (
    <div
      data-tauri-drag-region
      className="flex h-10 w-full shrink-0 select-none items-center justify-end bg-sv-surface pl-4 transition-colors"
    >
      <div className="flex h-full">
        {/* Minimize */}
        <button
          className="flex h-full w-12 items-center justify-center text-sv-muted hover:bg-white/10 hover:text-sv-text"
          onClick={() => appWindow.minimize()}
          title="Minimize"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <path d="M5 11h14v2H5z" />
          </svg>
        </button>

        {/* Maximize */}
        <button
          className="flex h-full w-12 items-center justify-center text-sv-muted hover:bg-white/10 hover:text-sv-text"
          onClick={() => appWindow.toggleMaximize()}
          title="Maximize"
        >
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
        </button>

        {/* Close */}
        <button
          className="flex h-full w-12 items-center justify-center text-sv-muted hover:bg-red-500 hover:text-white"
          onClick={() => appWindow.close()}
          title="Close"
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
