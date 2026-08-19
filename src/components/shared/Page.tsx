import type { ReactNode } from "react";

export default function Page({
  title,
  subtitle,
  actions,
  children,
  fill = false,
}: {
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  // fill = the page fills the viewport height and never scrolls the whole
  // window; inner regions handle their own overflow. Used by Home so its
  // single-screen layout stays put on any window size.
  fill?: boolean;
}) {
  return (
    <div
      className={`mx-auto w-full max-w-[1500px] px-8 py-7 ${
        fill ? "flex h-full flex-col" : ""
      }`}
    >
      <header className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">{title}</h1>
          {subtitle && <p className="mt-1 text-sm text-sv-muted">{subtitle}</p>}
        </div>
        {actions}
      </header>
      {fill ? <div className="flex min-h-0 flex-1 flex-col">{children}</div> : children}
    </div>
  );
}
