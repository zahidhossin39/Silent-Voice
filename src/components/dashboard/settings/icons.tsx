// Settings icon set: one family, 1.75 stroke.
import type { ReactNode } from "react";


function Svg({ children, size = 16 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {children}
    </svg>
  );
}
export const MicIcon = () => (
  <Svg>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
  </Svg>
);
export const PenIcon = () => (
  <Svg>
    <path d="M4 20h4L19 9l-4-4L4 16v4zM13.5 6.5l4 4" />
  </Svg>
);
export const SpeakerIcon = () => (
  <Svg>
    <path d="M4 9.5v5h3.5L12 18V6L7.5 9.5H4zM15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" />
  </Svg>
);
export const BookIcon = () => (
  <Svg>
    <path d="M5 4.5A1.5 1.5 0 0 1 6.5 3H19v15H6.5A1.5 1.5 0 0 0 5 19.5v-15zM5 19.5A1.5 1.5 0 0 0 6.5 21H19" />
  </Svg>
);
export const MonitorIcon = () => (
  <Svg>
    <rect x="3.5" y="4.5" width="17" height="11.5" rx="2" />
    <path d="M9 20h6M12 16v4" />
  </Svg>
);
export const SearchIcon = () => (
  <Svg>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m20 20-4.2-4.2" />
  </Svg>
);
export const WandIcon = () => (
  <Svg>
    <path d="M5 19 15 9M13.5 5.5V3M18.5 10.5H21M17 7l1.8-1.8M12 9.5 14.5 12" />
  </Svg>
);
export const PlayIcon = () => (
  <Svg size={14}>
    <path d="M8 5.5v13l10.5-6.5z" />
  </Svg>
);
export const PauseIcon = () => (
  <Svg size={14}>
    <path d="M9 5.5v13M15 5.5v13" />
  </Svg>
);
export const XIcon = () => (
  <Svg size={14}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Svg>
);
export const PlusIcon = () => (
  <Svg size={14}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);
export const CheckIcon = () => (
  <Svg size={14}>
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  </Svg>
);
export const ArrowIcon = () => (
  <span className="shrink-0 text-sv-muted">
    <Svg size={14}>
      <path d="M5 12h14M13 6l6 6-6 6" />
    </Svg>
  </span>
);
