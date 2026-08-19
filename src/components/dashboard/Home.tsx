import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import WaveformVisualizer from "../shared/WaveformVisualizer";
import { buildAccelerator } from "../shared/HotkeyRecorder";
import { STT_MODELS, LANGUAGES } from "../../services/catalog";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useUiStore } from "../../stores/uiStore";
import { useHistoryStore } from "../../stores/historyStore";
import type { HistoryEntry } from "../../types";
import { isTauri, listenEvent } from "../../services/tauriBridge";
import { formatGB } from "../../services/format";

const STATUS_LABEL: Record<string, string> = {
  idle: "Idle",
  listening: "Listening",
  recording: "Recording",
  processing: "Processing…",
  done: "Pasted ✓",
};

// "Intel(R) Core(TM) i7-8650U CPU @ 1.90GHz" → "Intel Core i7-8650U · 1.90GHz"
function tidyCpuName(raw: string): string {
  const cleaned = raw
    .replace(/\((R|TM|C)\)/gi, "")
    .replace(/\s+CPU\s*/i, " ")
    .replace(/\s{2,}/g, " ")
    .trim();
  const [name, clock] = cleaned.split(/\s*@\s*/);
  return clock ? `${name.trim()} · ${clock.trim()}` : cleaned;
}

export default function Home() {
  const { hardware, loading } = useHardwareInfo();
  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const downloadedStt = useModelStore((s) => s.downloaded);
  const recordingState = useUiStore((s) => s.recordingState);
  const entries = useHistoryStore((s) => s.entries);
  const today = useTodayStats(entries);
  const strip = useStripStats(entries);
  const lastError = useUiStore((s) => s.lastError);
  const setError = useUiStore((s) => s.setError);

  useEffect(() => {
    if (lastError) {
      const timeout = setTimeout(() => setError(null), 8000);
      return () => clearTimeout(timeout);
    }
  }, [lastError, setError]);

  const [micLevel, setMicLevel] = useState(0);
  useEffect(() => {
    const un = listenEvent<number>('pipeline://level', (v) => setMicLevel(v));
    return () => { un.then((f) => f()); };
  }, []);

  const sttPass = downloadedStt.size > 0 && Boolean(settings.active_stt_model) && downloadedStt.has(settings.active_stt_model);
  const sttValue = settings.active_stt_model
    ? (STT_MODELS.find((m) => m.id === settings.active_stt_model)?.label ?? settings.active_stt_model)
    : "None selected";
  
  const micPass = settings.audio_device !== "";
  const micValue = settings.audio_device ? settings.audio_device : "System default";
  
  const hotkeyPass = Boolean(settings.hotkey);
  const allPass = sttPass && micPass && hotkeyPass;

  // Only downloaded models are selectable — you can't dictate with one that
  // isn't on disk.
  const downloadedModels = STT_MODELS.filter((m) => downloadedStt.has(m.id));

  return (
    // No page header here on purpose: Home is a single-screen cockpit, and the
    // title/subtitle were the ~90px that pushed the Device card off-screen.
    <div className="flex h-full flex-col gap-3 px-6 py-5">
      {!isTauri() && (
        <div className="shrink-0 rounded-lg border border-sv-warn/30 bg-sv-warn/10 px-4 py-2 text-xs text-sv-warn">
          Running in browser preview. Audio capture, transcription, and paste
          require the desktop (Tauri) build. Hardware shown below is sample data.
        </div>
      )}

      {lastError && (
        <div className="flex shrink-0 flex-row items-start justify-between rounded-lg border border-sv-bad/30 bg-sv-bad/10 px-4 py-2 text-xs text-sv-bad">
          <div>{lastError}</div>
          <button
            aria-label="Dismiss"
            onClick={() => setError(null)}
            className="ml-3 shrink-0 p-0.5 text-sv-bad/60 hover:text-sv-bad"
          >
            ×
          </button>
        </div>
      )}

      {/* ── Split Cockpit: live console on the left, modules on the right ──
           The grid fills the remaining viewport height; the right column is the
           overflow valve (scrolls internally on tiny windows) so the page
           itself never scrolls. */}
      <div className="grid min-h-0 flex-1 gap-4 md:grid-cols-[1.4fr_1fr]">
        {/* Console — the live moment. Eyebrow pinned top-left; the live stack
            centers in whatever height the column gets. */}
        <div
          className="flex min-h-0 flex-col rounded-2xl border border-sv-border bg-sv-surface p-6"
          style={{
            backgroundImage:
              "radial-gradient(90% 80% at 50% 0%, color-mix(in srgb, var(--color-sv-accent) 8%, transparent), transparent 70%)",
          }}
        >
          <div className="shrink-0 text-[10px] font-semibold uppercase tracking-[0.13em] text-sv-muted">
            Status
          </div>
          <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-5 text-center">
            <div className="text-2xl font-semibold tracking-tight text-sv-text">
              {STATUS_LABEL[recordingState] ?? "Idle"}
            </div>
            <WaveformVisualizer
              active={
                recordingState === "recording" || recordingState === "listening"
              }
              bars={18}
              level={micLevel}
              heightClass="h-12"
            />
            <div className="flex flex-col items-center gap-1.5">
              <div className="text-[10px] font-semibold uppercase tracking-wide text-sv-muted">
                Push-to-talk
              </div>
              <InlineHotkey
                value={settings.hotkey}
                onChange={(hk) => setSettings({ hotkey: hk })}
              />
            </div>
            <p className="text-[11px] text-sv-muted">
              <span className="font-medium text-sv-text">Hold</span> to speak ·{" "}
              <span className="font-medium text-sv-text">release</span> to paste
            </p>
            {allPass ? (
              <span className="mt-1 inline-flex items-center gap-2 rounded-full border border-sv-good/35 bg-sv-good/10 px-4 py-1.5 text-[13px] font-medium text-sv-good">
                <span className="h-2 w-2 rounded-full bg-sv-good" /> Ready to
                dictate
              </span>
            ) : (
              <span className="mt-1 inline-flex items-center gap-2 rounded-full border border-sv-bad/35 bg-sv-bad/10 px-4 py-1.5 text-[13px] font-medium text-sv-bad">
                <span className="h-2 w-2 rounded-full bg-sv-bad" /> Finish setup
                to dictate
              </span>
            )}
          </div>
          <div className="mt-5 shrink-0 border-t border-sv-border pt-4">
            <div className="mb-2.5 flex items-baseline gap-2">
              <span className="text-[10px] font-semibold uppercase tracking-[0.13em] text-sv-muted">
                Last 12 weeks
              </span>
              <span className="text-[11px] text-sv-muted">each square is one day</span>
            </div>
            <div className="flex items-stretch gap-6">
              <div className="shrink-0">
                {/* Square cells across 12 columns force height = 7/12 of width, so the
                    grid needs an explicit cap or it grows to fill the panel. */}
                <div className="grid w-[300px] grid-flow-col grid-rows-7 gap-[3px]">
                  {strip.cells.map((c) => {
                    const bg = c.future
                      ? "bg-transparent"
                      : c.level === 0
                      ? "bg-sv-border/50"
                      : c.level === 1
                      ? "bg-sv-accent/25"
                      : c.level === 2
                      ? "bg-sv-accent/45"
                      : c.level === 3
                      ? "bg-sv-accent/70"
                      : "bg-sv-accent";
                    const title = c.future
                      ? undefined
                      : `${c.words === 0 ? "No dictation" : `${c.words} words`} · ${new Date(c.ms).toLocaleDateString()}`;

                    return (
                      <div
                        key={c.ms}
                        title={title}
                        className={`aspect-square rounded-[3px] ${bg}`}
                      />
                    );
                  })}
                </div>
                <div className="mt-2 flex items-center justify-end gap-1 text-[10px] text-sv-muted">
                  Fewer words
                  <span className="h-2 w-2 rounded-[2px] bg-sv-border/50" />
                  <span className="h-2 w-2 rounded-[2px] bg-sv-accent/25" />
                  <span className="h-2 w-2 rounded-[2px] bg-sv-accent/45" />
                  <span className="h-2 w-2 rounded-[2px] bg-sv-accent/70" />
                  <span className="h-2 w-2 rounded-[2px] bg-sv-accent" />
                  More
                </div>
              </div>
              <div className="flex min-w-0 flex-1 flex-col justify-center gap-5 border-l border-sv-border pl-6">
                <div>
                  <div className="text-lg font-semibold tabular-nums text-sv-text">
                    {strip.activeDays}
                    <span className="text-sm font-medium text-sv-muted">
                      {strip.activeDays === 1 ? " day" : " days"}
                    </span>
                  </div>
                  <div className="text-[11px] text-sv-muted">you dictated on</div>
                </div>
                <div>
                  <div className="text-lg font-semibold tabular-nums text-sv-text">
                    {strip.bestWords.toLocaleString()}
                    <span className="text-sm font-medium text-sv-muted"> words</span>
                  </div>
                  <div className="text-[11px] text-sv-muted">
                    {strip.bestWords > 0
                      ? `on your busiest day, ${new Date(strip.bestMs).toLocaleDateString(undefined, { month: "long", day: "numeric" })}`
                      : "on your busiest day"}
                  </div>
                </div>
                <div>
                  <div className="text-lg font-semibold tabular-nums text-sv-text">
                    {strip.avgPerActiveDay.toLocaleString()}
                    <span className="text-sm font-medium text-sv-muted"> words</span>
                  </div>
                  <div className="text-[11px] text-sv-muted">on an average day you use it</div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Right column — stacked modules, sized to content so the page never
            scrolls. overflow-y-auto is only a safety valve for absurdly short
            windows — at any normal size the stack fits and no bar appears. The
            Recent card at the end is flex-1 so it absorbs the leftover height,
            filling the column instead of leaving a gap at the bottom. */}
        <div className="flex min-h-0 flex-col gap-3 overflow-y-auto">
          {/* Setup */}
          <div className="shrink-0 rounded-xl border border-sv-border bg-sv-surface px-4 py-3">
            <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.13em] text-sv-muted">
              Setup
            </div>
            <div className="flex flex-col gap-1.5">
              <StatusCheck
                pass={sttPass}
                label="Voice model"
                value={sttPass ? sttValue : settings.active_stt_model ? "Not downloaded" : "None selected"}
                fixLink="/models"
                fixText="Choose →"
              />
              <StatusCheck
                pass={micPass}
                label="Microphone"
                value={micPass ? micValue : "Not set"}
                fixLink="/settings"
                fixText="Pick →"
              />
              <StatusCheck
                pass={hotkeyPass}
                label="Hotkey"
                value={hotkeyPass ? settings.hotkey : "Not set"}
                fixLink="/settings"
                fixText="Set →"
              />
            </div>
          </div>

          {/* Quick controls */}
          <div className="shrink-0 rounded-xl border border-sv-border bg-sv-surface px-4 py-3">
            <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.13em] text-sv-muted">
              Quick controls
            </div>
            <div className="flex flex-col gap-2">
              <QuickControl
                label="Model"
                hint={
                  downloadedModels.length === 0
                    ? "None downloaded — get one in Model Store"
                    : undefined
                }
              >
                <select
                  value={settings.active_stt_model}
                  onChange={(e) =>
                    setSettings({
                      active_stt_model: e.target.value,
                      stt_cloud_provider_id: null,
                    })
                  }
                  disabled={downloadedModels.length === 0}
                  className="min-w-0 flex-1 truncate bg-transparent py-2 pr-2 text-sm font-medium focus:outline-none disabled:opacity-50"
                >
                  {!downloadedModels.some((m) => m.id === settings.active_stt_model) && (
                    <option value={settings.active_stt_model}>
                      {settings.active_stt_model ? settings.active_stt_model : "None selected"}
                    </option>
                  )}
                  {downloadedModels.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label}
                    </option>
                  ))}
                </select>
              </QuickControl>

              <QuickControl label="Lang">
                <select
                  value={settings.language}
                  onChange={(e) => setSettings({ language: e.target.value })}
                  className="min-w-0 flex-1 truncate bg-transparent py-2 pr-2 text-sm font-medium focus:outline-none"
                >
                  {LANGUAGES.map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.name}
                    </option>
                  ))}
                </select>
              </QuickControl>
            </div>
          </div>

          {/* Device */}
          <div className="shrink-0 rounded-xl border border-sv-border bg-sv-surface px-4 py-3">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-[10px] font-semibold uppercase tracking-[0.13em] text-sv-muted">
                Your device
              </div>
              {!loading && hardware && (
                <Link to="/models" className="text-[11px] text-sv-accent hover:underline">
                  See models →
                </Link>
              )}
            </div>
            {loading || !hardware ? (
              <p className="text-sm text-sv-muted">Scanning…</p>
            ) : (
              // 2×2 of bare stats (no tile chrome) — the reference layout: big
              // value, small caption, nothing competing for height.
              <div className="grid grid-cols-2 gap-x-4 gap-y-2.5">
                <DeviceTile
                  value={tidyCpuName(hardware.cpu_brand).split(" · ")[0]}
                  sub={`${hardware.physical_cores} cores${hardware.has_avx2 ? " · AVX2" : ""}`}
                />
                <DeviceTile
                  value={`${hardware.total_ram_gb.toFixed(0)} GB`}
                  sub={`RAM · ${hardware.available_ram_gb.toFixed(1)} GB free`}
                />
                <DeviceTile
                  value={hardware.gpu_name ?? "None detected"}
                  sub={
                    hardware.gpu_vram_gb && hardware.gpu_vram_gb >= 1
                      ? `${formatGB(hardware.gpu_vram_gb)} — GPU models`
                      : "CPU models"
                  }
                />
                <DeviceTile
                  value={`${hardware.free_disk_gb.toFixed(0)} GB`}
                  sub="free disk"
                />
              </div>
            )}
          </div>

          {/* Today — flex-1 so it absorbs whatever height the three cards
              above don't use, filling the column instead of leaving a gap. */}
          <div className="flex min-h-0 flex-1 flex-col rounded-xl border border-sv-border bg-sv-surface px-4 py-3">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-[10px] font-semibold uppercase tracking-[0.13em] text-sv-muted">
                Today
              </div>
              {entries.length > 0 && (
                <Link to="/history" className="text-[11px] text-sv-accent hover:underline">
                  History →
                </Link>
              )}
            </div>
            <div className="flex min-h-0 flex-1 flex-col justify-center">
              <TodayStat label="Words dictated" value={today.words.toLocaleString()} />
              <TodayStat label="Dictations" value={String(today.count)} />
              <TodayStat
                label="Time saved vs typing"
                value={today.savedMin}
                unit="min"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// One-click hotkey control: click the keys → it listens immediately → the
// next combo you press replaces it. No intermediate "Change"/"Done" steps.
function InlineHotkey({
  value,
  onChange,
}: {
  value: string;
  onChange: (accelerator: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const [rejected, setRejected] = useState<string | null>(null);

  const stop = useCallback(() => {
    setRecording(false);
    setRejected(null);
  }, []);

  useEffect(() => {
    if (!recording) return;

    function onKeyDown(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        stop();
        return;
      }
      // Modifier-only presses just keep waiting for the real key.
      if (["Control", "Meta", "Alt", "Shift"].includes(e.key)) return;

      const accel = buildAccelerator(e);
      if (accel) {
        onChange(accel);
        stop();
      } else {
        setRejected(`${e.key} can't be used`);
      }
    }

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recording, onChange, stop]);

  if (recording) {
    return (
      <button
        onClick={stop}
        className="flex items-center gap-2 rounded-lg border border-sv-accent bg-sv-accent/10 px-3 py-1.5 ring-1 ring-sv-accent"
      >
        <span className="animate-pulse text-xs text-sv-accent">
          {rejected ?? "Press any key combo…"}
        </span>
        <span className="text-[10px] text-sv-muted">Esc</span>
      </button>
    );
  }

  return (
    <button
      onClick={() => setRecording(true)}
      className="flex items-center gap-1 rounded-lg border border-sv-border bg-sv-bg px-2.5 py-1.5 transition hover:border-sv-accent/60"
      title="Click to record a new push-to-talk hotkey"
    >
      {value.split("+").map((k, i, arr) => (
        <span key={i} className="flex items-center gap-1">
          <kbd className="rounded-md border border-sv-border bg-sv-surface-2 px-2 py-0.5 text-xs font-medium text-sv-text shadow-sm">
            {k}
          </kbd>
          {i < arr.length - 1 && <span className="text-xs text-sv-muted">+</span>}
        </span>
      ))}
    </button>
  );
}

function QuickControl({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  // Label sits inside the control so each row costs one line, not two.
  return (
    <div>
      <div className="flex items-center gap-2 rounded-lg border border-sv-border bg-sv-bg pl-3 focus-within:ring-1 focus-within:ring-sv-accent">
        <span className="shrink-0 text-xs font-medium text-sv-muted">{label}</span>
        {children}
      </div>
      {hint && <div className="mt-1 text-[11px] text-sv-muted">{hint}</div>}
    </div>
  );
}

function StatusCheck({
  pass,
  label,
  value,
  fixLink,
  fixText,
}: {
  pass: boolean;
  label: string;
  value: string;
  fixLink: string;
  fixText: string;
}) {
  // One line per check: dot + label left, value right — three checks in the
  // vertical space the stacked version spent on one.
  return (
    <div className="flex items-center gap-2">
      <div
        className={`h-2 w-2 shrink-0 rounded-full ${pass ? "bg-sv-good" : "bg-sv-bad"}`}
      />
      <div className="shrink-0 text-sm text-sv-muted">{label}</div>
      <div className="ml-auto min-w-0 truncate text-right text-sm font-medium text-sv-text">
        {value}
      </div>
      {!pass && (
        <Link to={fixLink} className="shrink-0 text-sm text-sv-accent hover:underline">
          {fixText}
        </Link>
      )}
    </div>
  );
}

function countWords(text: string): number {
  const t = text.trim();
  return t ? t.split(/\s+/).length : 0;
}

// Aggregate today's dictations. "Time saved" = how long those words would take
// to type minus the time actually spent speaking them.
// ponytail: 40 wpm is a fixed average-typing baseline; make it a setting only if
// someone actually asks to tune it.
const TYPING_WPM = 40;

function useTodayStats(entries: HistoryEntry[]) {
  return useMemo(() => {
    const start = new Date();
    start.setHours(0, 0, 0, 0);
    const startMs = start.getTime();

    let words = 0;
    let count = 0;
    let speakMs = 0;
    for (const e of entries) {
      if (e.timestamp < startMs) continue;
      count++;
      words += countWords(e.processed_text || e.raw_text);
      speakMs += e.duration_ms;
    }
    const typeMs = (words / TYPING_WPM) * 60_000;
    const savedMin = Math.max(0, Math.round((typeMs - speakMs) / 60_000));
    return { words, count, savedMin: String(savedMin) };
  }, [entries]);
}

function useStripStats(entries: HistoryEntry[]) {
  return useMemo(() => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const start = new Date(today);
    // Sunday of 11 weeks ago, so the 84 cells form 12 whole Sun..Sat columns
    start.setDate(start.getDate() - start.getDay() - 77);

    const startMs = start.getTime();
    const todayMs = today.getTime();

    const buckets = new Map<number, number>();
    for (const e of entries) {
      if (e.timestamp < startMs) continue;
      const d = new Date(e.timestamp);
      d.setHours(0, 0, 0, 0);
      const dayMs = d.getTime();

      buckets.set(
        dayMs,
        (buckets.get(dayMs) ?? 0) + countWords(e.processed_text || e.raw_text)
      );
    }

    const cells = [];
    let windowWords = 0;
    let activeDays = 0;

    for (let i = 0; i < 84; i++) {
      const d = new Date(startMs);
      d.setDate(d.getDate() + i);
      const ms = d.getTime();

      const words = buckets.get(ms) ?? 0;

      if (words > 0) activeDays++;

      let level = 0;
      if (words > 0) {
        if (words < 100) level = 1;
        else if (words < 300) level = 2;
        else if (words < 700) level = 3;
        else level = 4;
      }

      cells.push({
        ms,
        words,
        future: ms > todayMs,
        level,
      });

      windowWords += words;
    }

    let bestWords = 0;
    let bestMs = 0;
    for (const c of cells) {
      if (c.words > bestWords) {
        bestWords = c.words;
        bestMs = c.ms;
      }
    }
    const avgPerActiveDay = activeDays > 0 ? Math.round(windowWords / activeDays) : 0;

    return { cells, activeDays, bestWords, bestMs, avgPerActiveDay };
  }, [entries]);
}

function TodayStat({
  label,
  value,
  unit,
}: {
  label: string;
  value: string;
  unit?: string;
}) {
  return (
    <div className="flex items-baseline justify-between border-b border-sv-border py-2 last:border-0">
      <span className="text-xs text-sv-muted">{label}</span>
      <span className="text-xl font-bold tracking-tight tabular-nums text-sv-text">
        {value}
        {unit && <span className="ml-0.5 text-xs font-medium text-sv-muted">{unit}</span>}
      </span>
    </div>
  );
}

function DeviceTile({ value, sub }: { value: string; sub?: string }) {
  return (
    <div className="min-w-0">
      <div className="truncate text-sm font-semibold" title={value}>
        {value}
      </div>
      {sub && <div className="truncate text-[11px] text-sv-muted">{sub}</div>}
    </div>
  );
}
