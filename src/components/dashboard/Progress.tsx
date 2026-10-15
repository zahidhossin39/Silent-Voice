import { useMemo } from "react";
import Page from "../shared/Page";
import { useStatsStore, savedMinutes, speakingMs, TYPING_WPM, type DayStat } from "../../stores/statsStore";

// Everything here is computed from the durable per-day totals in statsStore
// (never from history), so it survives clearing history. Nothing leaves the PC.

const DAY = 86_400_000;

/** Real books, so a word count means something. Approximate published lengths. */
const MILESTONES = [
  { words: 500, name: "a long email" },
  { words: 2_000, name: "a school essay" },
  { words: 7_500, name: "a short story" },
  { words: 30_000, name: "Animal Farm" },
  { words: 47_000, name: "The Great Gatsby" },
  { words: 77_000, name: "Harry Potter and the Philosopher's Stone" },
  { words: 95_000, name: "The Hobbit" },
  { words: 122_000, name: "Pride and Prejudice" },
  { words: 480_000, name: "The Lord of the Rings" },
];

/** Friendly names for common apps; anything else is its exe name, tidied. */
const APP_NAMES: Record<string, string> = {
  "chrome.exe": "Chrome",
  "msedge.exe": "Edge",
  "firefox.exe": "Firefox",
  "brave.exe": "Brave",
  "code.exe": "VS Code",
  "cursor.exe": "Cursor",
  "winword.exe": "Word",
  "outlook.exe": "Outlook",
  "olk.exe": "Outlook",
  "ms-teams.exe": "Teams",
  "teams.exe": "Teams",
  "slack.exe": "Slack",
  "discord.exe": "Discord",
  "whatsapp.exe": "WhatsApp",
  "whatsapp.root.exe": "WhatsApp",
  "telegram.exe": "Telegram",
  "notion.exe": "Notion",
  "obsidian.exe": "Obsidian",
  "notepad.exe": "Notepad",
  "windowsterminal.exe": "Terminal",
  "claude.exe": "Claude",
  "chatgpt.exe": "ChatGPT",
  "explorer.exe": "File Explorer",
};

function appName(exe: string): string {
  if (APP_NAMES[exe]) return APP_NAMES[exe];
  const bare = exe.replace(/\.exe$/i, "");
  return bare.charAt(0).toUpperCase() + bare.slice(1);
}

function formatDuration(min: number): { value: string; unit: string } {
  if (min < 60) return { value: String(Math.round(min)), unit: "min" };
  const h = Math.floor(min / 60);
  const m = Math.round(min % 60);
  return { value: m ? `${h}h ${m}` : String(h), unit: m ? "min" : h === 1 ? "hour" : "hours" };
}

function hourLabel(h: number): string {
  return new Date(2000, 0, 1, h).toLocaleTimeString(undefined, { hour: "numeric" });
}

function useProgress() {
  const daily = useStatsStore((s) => s.daily);
  const hours = useStatsStore((s) => s.hours);
  const apps = useStatsStore((s) => s.apps);

  return useMemo(() => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const todayMs = today.getTime();
    const days = Object.entries(daily).map(([k, v]) => [Number(k), v] as [number, DayStat]);
    const since = (n: number) => days.filter(([ms]) => ms > todayMs - n * DAY).map(([, d]) => d);

    const words = days.reduce((a, [, d]) => a + d.words, 0);
    const dictations = days.reduce((a, [, d]) => a + d.dictations, 0);

    // Speaking speed from the last 30 days of timed dictations only.
    const recent = since(30);
    const talkMs = recent.reduce((a, d) => a + (d.talkMs ?? 0), 0);
    const talkWords = recent.reduce((a, d) => a + (d.talkWords ?? 0), 0);
    // Under a minute of timing is too little to quote a speed.
    const wpm = talkMs >= 60_000 ? Math.round(talkWords / (talkMs / 60_000)) : null;

    // Runs of active days. Today not dictated yet doesn't break the run.
    const active = new Set(days.filter(([, d]) => d.words > 0).map(([ms]) => ms));
    let run = 0;
    for (let ms = active.has(todayMs) ? todayMs : todayMs - DAY; active.has(ms); ms -= DAY) run++;
    let best = 0;
    for (const ms of active) {
      if (active.has(ms - DAY)) continue; // only count from the start of a run
      let n = 0;
      while (active.has(ms + n * DAY)) n++;
      best = Math.max(best, n);
    }

    // A full year, Sunday-aligned columns like GitHub's graph.
    const start = new Date(today);
    start.setDate(start.getDate() - start.getDay() - 52 * 7);
    const cells = Array.from({ length: 53 * 7 }, (_, i) => {
      const d = new Date(start);
      d.setDate(d.getDate() + i);
      const ms = d.getTime();
      const w = daily[ms]?.words ?? 0;
      const level = w === 0 ? 0 : w < 100 ? 1 : w < 300 ? 2 : w < 700 ? 3 : 4;
      return { ms, words: w, level, future: ms > todayMs };
    });

    const topApps = Object.entries(apps ?? {})
      .filter(([, w]) => w > 0)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5);
    const appWords = Object.values(apps ?? {}).reduce((a, b) => a + b, 0);

    const hourly = hours?.length === 24 ? hours : Array<number>(24).fill(0);
    const peak = hourly.some((h) => h > 0) ? hourly.indexOf(Math.max(...hourly)) : null;

    return {
      words,
      dictations,
      wpm,
      run,
      best,
      saved: {
        total: savedMinutes(days.map(([, d]) => d)),
        week: savedMinutes(since(7)),
        month: savedMinutes(since(30)),
      },
      spokenMin: days.reduce((a, [, d]) => a + speakingMs(d), 0) / 60_000,
      cells,
      topApps,
      appWords,
      hourly,
      peak,
    };
  }, [daily, hours, apps]);
}

export default function Progress() {
  const p = useProgress();
  const next = MILESTONES.find((m) => m.words > p.words);
  const passed = MILESTONES.filter((m) => m.words <= p.words);
  const last = passed[passed.length - 1];
  const total = formatDuration(p.saved.total);

  return (
    <Page title="Progress" subtitle="What dictating has done for you. Worked out on this PC; nothing is sent anywhere.">
      {p.dictations === 0 ? (
        <div className="rounded-2xl border border-dashed border-sv-border bg-sv-surface px-6 py-16 text-center">
          <div className="text-lg font-semibold">Nothing here yet</div>
          <p className="mt-1 text-sm text-sv-muted">
            Hold your hotkey and say something. Your first dictation starts the count.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {/* Hero: the one number that says whether this app is worth it. */}
          <section className="relative overflow-hidden rounded-2xl border border-sv-accent/30 bg-gradient-to-br from-sv-accent/15 via-sv-surface to-sv-surface px-7 py-6">
            <div className="flex flex-wrap items-end justify-between gap-6">
              <div>
                <div className="text-[11px] font-semibold uppercase tracking-[0.14em] text-sv-accent">
                  Time saved vs typing
                </div>
                <div className="mt-1 flex items-baseline gap-2 whitespace-nowrap">
                  <span className="text-5xl font-bold tracking-tight tabular-nums">{total.value}</span>
                  <span className="text-lg font-medium text-sv-muted">{total.unit}</span>
                </div>
                <p className="mt-2 max-w-md text-xs text-sv-muted">
                  Typing your {p.words.toLocaleString()} words at {TYPING_WPM} words a minute would take{" "}
                  {formatDuration(p.words / TYPING_WPM).value} {formatDuration(p.words / TYPING_WPM).unit}. Saying
                  them took {formatDuration(p.spokenMin).value} {formatDuration(p.spokenMin).unit}.
                </p>
              </div>
              <div className="flex gap-3">
                <HeroChip label="Last 7 days" minutes={p.saved.week} />
                <HeroChip label="Last 30 days" minutes={p.saved.month} />
              </div>
            </div>
          </section>

          <section className="grid grid-cols-2 gap-4 lg:grid-cols-4">
            <Tile label="Words dictated" value={p.words.toLocaleString()} sub={`in ${p.dictations.toLocaleString()} dictations`} />
            <Tile
              label="Speaking speed"
              value={p.wpm ? String(p.wpm) : "–"}
              unit={p.wpm ? "wpm" : undefined}
              sub={
                p.wpm
                  ? `${(p.wpm / TYPING_WPM).toFixed(1)}× faster than typing`
                  : "Shows after a minute of dictating"
              }
            />
            <Tile
              label="Current run"
              value={String(p.run)}
              unit={p.run === 1 ? "day" : "days"}
              sub={`Best run: ${p.best} ${p.best === 1 ? "day" : "days"}`}
            />
            <Tile
              label="Typical dictation"
              value={String(Math.round(p.words / p.dictations))}
              unit="words"
              sub="per press of the hotkey"
            />
          </section>

          {/* Words as books: a big count only means something next to a real thing. */}
          <section className="rounded-2xl border border-sv-border bg-sv-surface px-6 py-5">
            <SectionTitle>You've dictated</SectionTitle>
            {next ? (
              <>
                <div className="mt-2 text-lg font-semibold">
                  {Math.floor((p.words / next.words) * 100)}% of <span className="text-sv-accent">{next.name}</span>
                </div>
                <div className="mt-3 h-2 w-full overflow-hidden rounded-full bg-sv-border/60">
                  <div
                    className="h-full rounded-full bg-sv-accent transition-[width] duration-700"
                    style={{ width: `${Math.max(2, (p.words / next.words) * 100)}%` }}
                  />
                </div>
                <div className="mt-1.5 flex justify-between text-[11px] tabular-nums text-sv-muted">
                  <span>{last ? `Passed: ${last.name}` : "Just getting started"}</span>
                  <span>{(next.words - p.words).toLocaleString()} words to go</span>
                </div>
              </>
            ) : (
              <div className="mt-2 text-lg font-semibold">More than all of The Lord of the Rings. Impressive.</div>
            )}
            {passed.length > 0 && (
              <div className="mt-4 flex flex-wrap gap-1.5">
                {passed.map((m) => (
                  <span
                    key={m.name}
                    className="inline-flex items-center gap-1 rounded-full border border-sv-accent/30 bg-sv-accent/10 px-2.5 py-0.5 text-[11px] font-medium text-sv-text"
                  >
                    <span className="text-sv-accent">✓</span> {m.name}
                  </span>
                ))}
              </div>
            )}
          </section>

          <section className="rounded-2xl border border-sv-border bg-sv-surface px-6 py-5">
            <div className="flex items-baseline justify-between">
              <SectionTitle>Your year</SectionTitle>
              <span className="text-[11px] text-sv-muted">each square is one day</span>
            </div>
            <YearGrid cells={p.cells} />
          </section>

          <section className="grid gap-4 lg:grid-cols-2">
            <div className="rounded-2xl border border-sv-border bg-sv-surface px-6 py-5">
              <SectionTitle>Where you speak</SectionTitle>
              {p.topApps.length === 0 ? (
                <Empty />
              ) : (
                <div className="mt-3 space-y-2.5">
                  {p.topApps.map(([exe, w]) => (
                    <div key={exe}>
                      <div className="flex justify-between text-sm">
                        <span className="truncate font-medium">{appName(exe)}</span>
                        <span className="shrink-0 tabular-nums text-sv-muted">
                          {Math.round((w / p.appWords) * 100)}%
                        </span>
                      </div>
                      <div className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-sv-border/50">
                        <div
                          className="h-full rounded-full bg-sv-accent/80"
                          style={{ width: `${(w / p.topApps[0][1]) * 100}%` }}
                        />
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="rounded-2xl border border-sv-border bg-sv-surface px-6 py-5">
              <div className="flex items-baseline justify-between gap-3">
                <SectionTitle>When you speak</SectionTitle>
                {p.peak !== null && (
                  <span className="text-[11px] text-sv-muted">busiest around {hourLabel(p.peak)}</span>
                )}
              </div>
              {p.peak === null ? <Empty /> : <HourBars hourly={p.hourly} peak={p.peak} />}
            </div>
          </section>
        </div>
      )}
    </Page>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h2 className="text-[11px] font-semibold uppercase tracking-[0.13em] text-sv-muted">{children}</h2>;
}

function Empty() {
  return <p className="mt-3 text-sm text-sv-muted">Counting starts with your next dictation.</p>;
}

function HeroChip({ label, minutes }: { label: string; minutes: number }) {
  const d = formatDuration(minutes);
  return (
    <div className="rounded-xl border border-sv-border bg-sv-bg/60 px-4 py-2.5">
      <div className="text-[11px] text-sv-muted">{label}</div>
      <div className="whitespace-nowrap text-xl font-bold tabular-nums">
        {d.value}
        <span className="ml-1 text-xs font-medium text-sv-muted">{d.unit}</span>
      </div>
    </div>
  );
}

function Tile({ label, value, unit, sub }: { label: string; value: string; unit?: string; sub: string }) {
  return (
    <div className="rounded-2xl border border-sv-border bg-sv-surface px-5 py-4">
      <div className="text-xs text-sv-muted">{label}</div>
      <div className="mt-1 text-3xl font-bold tracking-tight tabular-nums">
        {value}
        {unit && <span className="ml-1 text-sm font-medium text-sv-muted">{unit}</span>}
      </div>
      <div className="mt-1 text-[11px] text-sv-muted">{sub}</div>
    </div>
  );
}

const LEVEL_BG = ["bg-sv-border/50", "bg-sv-accent/25", "bg-sv-accent/45", "bg-sv-accent/70", "bg-sv-accent"];

function YearGrid({ cells }: { cells: { ms: number; words: number; level: number; future: boolean }[] }) {
  // A month label over the first column that starts in that month.
  const months = Array.from({ length: 53 }, (_, col) => {
    const d = new Date(cells[col * 7].ms);
    const prev = col > 0 ? new Date(cells[(col - 1) * 7].ms) : null;
    return !prev || prev.getMonth() !== d.getMonth() ? d.toLocaleDateString(undefined, { month: "short" }) : "";
  });
  return (
    <div className="mt-3">
      <div className="grid gap-[3px] text-[10px] text-sv-muted" style={{ gridTemplateColumns: "repeat(53, minmax(0, 1fr))" }}>
        {months.map((m, i) => (
          <span key={i} className="overflow-visible whitespace-nowrap">
            {m}
          </span>
        ))}
      </div>
      <div className="mt-1 grid grid-flow-col grid-rows-7 gap-[3px]" style={{ gridTemplateColumns: "repeat(53, minmax(0, 1fr))" }}>
        {cells.map((c) => (
          <div
            key={c.ms}
            title={c.future ? undefined : `${c.words ? `${c.words.toLocaleString()} words` : "No dictation"} · ${new Date(c.ms).toLocaleDateString()}`}
            className={`aspect-square rounded-[3px] ${c.future ? "bg-transparent" : LEVEL_BG[c.level]}`}
          />
        ))}
      </div>
      <div className="mt-2 flex items-center justify-end gap-1 text-[10px] text-sv-muted">
        Fewer words
        {LEVEL_BG.map((bg) => (
          <span key={bg} className={`h-2.5 w-2.5 rounded-[2px] ${bg}`} />
        ))}
        More
      </div>
    </div>
  );
}

function HourBars({ hourly, peak }: { hourly: number[]; peak: number }) {
  const max = Math.max(...hourly);
  return (
    <div className="mt-3">
      <div className="flex h-28 items-end gap-[3px]">
        {hourly.map((w, h) => (
          <div
            key={h}
            title={`${hourLabel(h)}: ${w.toLocaleString()} words`}
            className={`flex-1 rounded-t-[3px] ${h === peak ? "bg-sv-accent" : w > 0 ? "bg-sv-accent/40" : "bg-sv-border/50"}`}
            style={{ height: `${Math.max(4, (w / max) * 100)}%` }}
          />
        ))}
      </div>
      <div className="mt-1.5 flex justify-between text-[10px] text-sv-muted">
        {[0, 6, 12, 18].map((h) => (
          <span key={h}>{hourLabel(h)}</span>
        ))}
        <span>{hourLabel(23)}</span>
      </div>
    </div>
  );
}
