// Smart size formatting: show MB when under 1 GB, GB otherwise.
// Keeps the Model Store and device info easy to read at a glance.

export function formatMB(mb: number): string {
  if (mb <= 0) return "—";
  if (mb < 1024) return `${Math.round(mb)} MB`;
  const gb = mb / 1024;
  return `${gb % 1 === 0 ? gb.toFixed(0) : gb.toFixed(1)} GB`;
}

export function formatGB(gb: number): string {
  return formatMB(gb * 1024);
}
