import { useEffect, useRef, useState } from "react";

// A disabled control that has a REASON shouldn't sit there mute — but writing
// that reason as permanent text under every row also doesn't scale once more
// than one control can be disabled. This trades the always-visible sentence
// for a tap: the trigger looks like the control it's explaining, and the
// explanation appears on demand. Same popover chrome as Select, so it reads
// as one family of controls rather than two different ideas.
export default function InfoPopover({
  label,
  message,
  className = "",
}: {
  label: string;
  message: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function onDocDown(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    window.addEventListener("mousedown", onDocDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDocDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="dialog"
        aria-expanded={open}
        className={`flex w-full items-center gap-2 rounded-lg border bg-sv-bg px-3 py-1.5 text-left text-sm text-sv-muted transition-colors hover:border-sv-muted/60 ${
          open ? "border-sv-accent" : "border-sv-border"
        } ${className}`}
      >
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          className="shrink-0"
        >
          <circle cx="12" cy="12" r="9" />
          <path strokeLinecap="round" d="M12 11v5" />
          <circle cx="12" cy="8" r="0.25" fill="currentColor" stroke="none" />
        </svg>
      </button>

      {open && (
        <div
          role="dialog"
          className="absolute left-0 top-[calc(100%+4px)] z-50 w-64 rounded-lg border border-sv-border bg-sv-surface p-3 text-xs leading-relaxed text-sv-text shadow-xl"
        >
          {message}
        </div>
      )}
    </div>
  );
}
