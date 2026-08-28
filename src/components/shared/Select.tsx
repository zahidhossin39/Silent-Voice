import { useEffect, useRef, useState, Children, isValidElement } from "react";

// Custom-rendered dropdown, API-compatible with a native <select> (value,
// onChange, <option> children). WebView2 on this hardware repaints the
// native <select>'s own border incorrectly the moment its OS popup opens —
// the top edge goes missing. Owning the popup ourselves sidesteps that
// rendering bug entirely instead of fighting it.
export default function Select({
  value,
  onChange,
  className = "",
  disabled = false,
  children,
}: {
  value: string;
  onChange: (v: string) => void;
  className?: string;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const options = Children.toArray(children)
    .filter(isValidElement)
    .map((el) => {
      const props = el.props as { value: string; children: React.ReactNode };
      return { value: props.value, label: props.children };
    });
  const selected = options.find((o) => o.value === value);

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
    listRef.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
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
        aria-haspopup="listbox"
        aria-expanded={open}
        disabled={disabled}
        className={`flex items-center gap-2 rounded-lg border bg-sv-bg px-3 py-1.5 text-sm text-sv-text transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${
          open
            ? "border-sv-accent"
            : "border-sv-border enabled:hover:border-sv-muted/60"
        } ${className}`}
      >
        <span className="min-w-0 flex-1 truncate text-left">{selected?.label ?? value}</span>
        <svg
          viewBox="0 0 24 24"
          width="12"
          height="12"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={`shrink-0 text-sv-muted transition-transform ${open ? "rotate-180" : ""}`}
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      {open && !disabled && (
        <div
          ref={listRef}
          role="listbox"
          className="absolute left-0 top-[calc(100%+4px)] z-50 max-h-64 min-w-full overflow-y-auto rounded-lg border border-sv-border bg-sv-surface p-1 shadow-xl"
        >
          {options.map((o) => (
            <button
              key={o.value}
              type="button"
              role="option"
              aria-selected={o.value === value}
              onClick={() => {
                onChange(o.value);
                setOpen(false);
              }}
              className={`block w-full rounded-md px-3 py-1.5 text-left text-sm transition-colors ${
                o.value === value
                  ? "bg-sv-accent/15 text-sv-accent"
                  : "text-sv-text hover:bg-sv-surface-2"
              }`}
            >
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
