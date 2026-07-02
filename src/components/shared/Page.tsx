import type { ReactNode } from "react";

export default function Page({
  title,
  subtitle,
  actions,
  children,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="mx-auto w-full max-w-[1500px] px-8 py-7">
      <header className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">{title}</h1>
          {subtitle && (
            <p className="mt-1 text-sm text-sv-muted">{subtitle}</p>
          )}
        </div>
        {actions}
      </header>
      {children}
    </div>
  );
}
