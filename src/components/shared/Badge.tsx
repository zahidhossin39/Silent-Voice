import type { CompatibilityLevel } from "../../types";

const STYLES: Record<CompatibilityLevel, string> = {
  good: "bg-sv-good/15 text-sv-good border-sv-good/30",
  warn: "bg-sv-warn/15 text-sv-warn border-sv-warn/30",
  bad: "bg-sv-bad/15 text-sv-bad border-sv-bad/30",
};

const DOT: Record<CompatibilityLevel, string> = {
  good: "bg-sv-good",
  warn: "bg-sv-warn",
  bad: "bg-sv-bad",
};

export default function Badge({
  level,
  children,
}: {
  level: CompatibilityLevel;
  children: React.ReactNode;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[11px] font-medium ${STYLES[level]}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${DOT[level]}`} />
      {children}
    </span>
  );
}
