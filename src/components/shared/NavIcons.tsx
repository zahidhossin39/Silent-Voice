// Hand-drawn line-icon set for the sidebar nav — replaces emoji, which read as
// childish. Consistent 24x24 grid, 1.75 stroke, rounded caps/joins.
import { useId } from "react";

type IconProps = { className?: string };

const base = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.75,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export function HomeIcon({ className }: IconProps) {
  return (
    <svg {...base} className={className}>
      <path d="M4 10.5 12 4l8 6.5" />
      <path d="M6 9.5V19a1 1 0 0 0 1 1h3v-5.5a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1V20h3a1 1 0 0 0 1-1V9.5" />
    </svg>
  );
}

export function StoreIcon({ className }: IconProps) {
  // A 3D package / cube — reads as "browse & download items", distinct from
  // the Home house shape.
  return (
    <svg {...base} className={className}>
      <path d="M12 2.5 21 7.2v9.6L12 21.5 3 16.8V7.2z" />
      <path d="M3 7.2 12 12l9-4.8M12 12v9.5" />
      <path d="M16.5 4.85 7.5 9.65" />
    </svg>
  );
}

export function ModesIcon({ className }: IconProps) {
  // Classic "layers" glyph (straight-line stack) — reads clearly as "pick
  // one of several variants", distinct at small sizes from every other icon
  // in this set.
  return (
    <svg {...base} className={className}>
      <path d="M12 2.5 2.5 7.5 12 12.5l9.5-5z" />
      <path d="M2.5 12l9.5 5 9.5-5" />
      <path d="M2.5 16.5 12 21.5l9.5-5" />
    </svg>
  );
}

export function KeyIcon({ className }: IconProps) {
  return (
    <svg {...base} className={className}>
      <circle cx="8" cy="15" r="4" />
      <path d="M11 12 19 4M17.5 5.5 19.5 7.5M15 8l1.8 1.8" />
    </svg>
  );
}

export function GearIcon({ className }: IconProps) {
  // A real cog with blocky teeth and a punched-out center hole (via mask) —
  // not thin radiating lines, which reads as a sun/brightness icon at small
  // sizes instead of "settings".
  const maskId = useId();
  const teeth = [0, 45, 90, 135, 180, 225, 270, 315];
  return (
    <svg viewBox="0 0 24 24" className={className}>
      <mask id={maskId}>
        <rect width="24" height="24" fill="white" />
        <circle cx="12" cy="12" r="3" fill="black" />
      </mask>
      <g fill="currentColor" mask={`url(#${maskId})`}>
        {teeth.map((deg) => (
          <rect
            key={deg}
            x="10.6"
            y="1.4"
            width="2.8"
            height="3.6"
            rx="0.6"
            transform={`rotate(${deg} 12 12)`}
          />
        ))}
        <circle cx="12" cy="12" r="7.2" />
      </g>
    </svg>
  );
}

export function GuideIcon({ className }: IconProps) {
  // Open book — reads as "guide/manual" at a glance.
  return (
    <svg {...base} className={className}>
      <path d="M12 6.5C10.5 5 8.5 4.5 6 4.5c-1 0-2 .15-3 .5v13.5c1-.35 2-.5 3-.5 2.5 0 4.5.5 6 2 1.5-1.5 3.5-2 6-2 1 0 2 .15 3 .5V5c-1-.35-2-.5-3-.5-2.5 0-4.5.5-6 2z" />
      <path d="M12 6.5V20" />
    </svg>
  );
}

export function HistoryIcon({ className }: IconProps) {
  // A clean clock face with a small "rewind" notch at the top-left — reads
  // clearly as recents/history, classy rather than busy.
  return (
    <svg {...base} className={className}>
      <circle cx="12" cy="12" r="8.25" />
      <path d="M12 7.6v4.7l3.2 1.9" />
      <path d="M4.4 8.2 3.5 5.4l2.8-.7" />
    </svg>
  );
}
