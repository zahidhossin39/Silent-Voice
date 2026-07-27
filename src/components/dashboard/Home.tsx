import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import Page from "../shared/Page";
import WaveformVisualizer from "../shared/WaveformVisualizer";
import TranscriptList from "../shared/TranscriptList";
import { buildAccelerator } from "../shared/HotkeyRecorder";
import { STT_MODELS, LANGUAGES } from "../../services/catalog";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useUiStore } from "../../stores/uiStore";
import { isTauri } from "../../services/tauriBridge";
import { formatGB } from "../../services/format";

const STATUS_LABEL: Record<string, string> = {
  idle: "Idle",
  listening: "Listening",
  recording: "Recording",
  processing: "Processing…",
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
  const lastError = useUiStore((s) => s.lastError);

  const downloadedModels = STT_MODELS.filter((m) => downloadedStt.has(m.id));

  return (
    <Page
      title="Silent Voice"
      subtitle={
        (
          <>
            Hold <Kbd>{settings.hotkey}</Kbd> anywhere in Windows, speak, and your words land at the cursor.
          </>
        ) as any
      }
    >
      {!isTauri() && (
        <div className="mb-5 rounded-lg border border-sv-warn/30 bg-sv-warn/10 px-4 py-3 text-xs text-sv-warn">
          Running in browser preview. Audio capture, transcription, and paste
          require the desktop (Tauri) build. Hardware shown below is sample data.
        </div>
      )}

      {lastError && (
        <div className="mb-5 rounded-lg border border-sv-bad/30 bg-sv-bad/10 px-4 py-3 text-xs text-sv-bad">
          {lastError}
        </div>
      )}

      {downloadedStt.size === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-xl border border-sv-border bg-sv-surface py-16 px-6 text-center">
          <h2 className="mb-3 text-lg font-semibold text-sv-text">
            A voice model is required
          </h2>
          <p className="mb-6 max-w-md text-sm text-sv-muted leading-relaxed">
            Download a model once to transcribe your speech. It runs entirely on this device and nothing is uploaded.
          </p>
          <Link
            to="/models"
            className="rounded-lg bg-sv-accent px-5 py-2.5 text-sm font-medium text-white transition hover:bg-sv-accent-hover"
          >
            Choose a voice model
          </Link>
        </div>
      ) : (
        <>
          {/* Status strip */}
          <div className="mb-4 flex items-center justify-between rounded-xl border border-sv-border bg-sv-surface px-4 py-2.5">
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-2">
                <span
                  className={`h-2.5 w-2.5 rounded-full ${
                    recordingState === "recording" || recordingState === "listening"
                      ? "bg-sv-accent animate-pulse"
                      : recordingState === "processing"
                      ? "bg-sv-muted animate-pulse"
                      : "bg-sv-muted"
                  }`}
                />
                <span className="text-sm font-medium">
                  {STATUS_LABEL[recordingState] ?? "Idle"}
                </span>
              </div>
              <WaveformVisualizer
                active={
                  recordingState === "recording" || recordingState === "listening"
                }
              />
            </div>
            <InlineHotkey
              value={settings.hotkey}
              onChange={(hk) => setSettings({ hotkey: hk })}
            />
          </div>

          {/* Quick controls */}
          <div className="mb-6 grid grid-cols-2 gap-4">
            <QuickControl
              label="Speech model"
              hint={
                downloadedModels.length === 0
                  ? "No models downloaded yet"
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
                className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent disabled:opacity-50"
              >
                {downloadedModels.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.label}
                  </option>
                ))}
                {!downloadedModels.some(
                  (m) => m.id === settings.active_stt_model
                ) && (
                  <option value={settings.active_stt_model}>
                    {settings.active_stt_model}
                  </option>
                )}
              </select>
            </QuickControl>

            <QuickControl label="Language">
              <select
                value={settings.language}
                onChange={(e) => setSettings({ language: e.target.value })}
                className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
              >
                {LANGUAGES.map((l) => (
                  <option key={l.code} value={l.code}>
                    {l.name}
                  </option>
                ))}
              </select>
            </QuickControl>
          </div>

          <TranscriptList
            emptyState={
              <div className="rounded-xl border border-sv-border bg-sv-surface px-6 py-9 text-center">
                <div className="text-sm font-medium text-sv-text">
                  Everything you dictate shows up here.
                </div>
                <div className="mt-5 flex items-center justify-center gap-3 text-sm">
                  <span className="flex items-center gap-1.5">
                    <Kbd>{settings.hotkey}</Kbd>
                    <span className="text-sv-muted">hold</span>
                  </span>
                  <span className="text-sv-muted">→</span>
                  <span className="font-medium text-sv-text">speak</span>
                  <span className="text-sv-muted">→</span>
                  <span className="font-medium text-sv-text">release</span>
                </div>
                <p className="mt-5 text-xs text-sv-muted">
                  Nothing is uploaded — transcription runs on this device.
                </p>
              </div>
            }
          />

          <div className="mt-5">
            <details className="group rounded-xl border border-sv-border bg-sv-surface">
              <summary className="flex cursor-pointer items-center justify-between px-4 py-3 text-xs text-sv-muted hover:text-sv-text">
                {loading || !hardware ? (
                  <span>Scanning device…</span>
                ) : (
                  <span>
                    {tidyCpuName(hardware.cpu_brand)} · {hardware.total_ram_gb.toFixed(0)} GB · {hardware.gpu_name ?? "None detected"} — best fit:{" "}
                    {hardware.gpu_vram_gb && hardware.gpu_vram_gb >= 2
                      ? "GPU-class models"
                      : "fast, small models"}
                  </span>
                )}
                <span className="transition-transform group-open:rotate-180">▼</span>
              </summary>
              <div className="border-t border-sv-border p-4">
                {loading || !hardware ? (
                  <p className="text-sm text-sv-muted">Scanning…</p>
                ) : (
                  <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
                    <DeviceTile
                      className="col-span-2"
                      label="Processor"
                      value={tidyCpuName(hardware.cpu_brand)}
                      sub={`${hardware.physical_cores} cores · ${hardware.logical_cores} threads${
                        hardware.has_avx2 ? " · AVX2" : ""
                      }${hardware.has_avx512 ? " · AVX-512" : ""}`}
                    />
                    <DeviceTile
                      label="Memory"
                      value={`${hardware.total_ram_gb.toFixed(0)} GB`}
                      sub={`${hardware.available_ram_gb.toFixed(1)} GB free`}
                    />
                    <DeviceTile
                      label="Free disk"
                      value={`${hardware.free_disk_gb.toFixed(0)} GB`}
                    />
                    <DeviceTile
                      className="col-span-2"
                      label="Graphics"
                      value={hardware.gpu_name ?? "None detected"}
                      sub={
                        hardware.gpu_vram_gb && hardware.gpu_vram_gb > 0
                          ? `${formatGB(hardware.gpu_vram_gb)} dedicated${
                              hardware.gpu_vram_gb < 1
                                ? " — too little to accelerate models; they'll run on the CPU"
                                : ""
                            }`
                          : "No dedicated memory — models run on the CPU"
                      }
                    />
                    <DeviceTile
                      className="col-span-2"
                      label="Best fit"
                      value={
                        hardware.gpu_vram_gb && hardware.gpu_vram_gb >= 2
                          ? "GPU-class models"
                          : "Fast, small models"
                      }
                      sub={
                        hardware.gpu_vram_gb && hardware.gpu_vram_gb >= 2
                          ? "This device can handle the larger, more accurate models"
                          : "Tiny / Base run best here — larger models will feel slow"
                      }
                    />
                  </div>
                )}
              </div>
            </details>
          </div>
        </>
      )}
    </Page>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="mx-1 rounded-md border border-sv-border border-b-[3px] bg-sv-surface-2 px-2 py-0.5 text-xs font-mono font-bold text-sv-text">
      {children}
    </kbd>
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
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <label className="text-xs font-medium text-sv-muted">{label}</label>
        {hint && <span className="text-[10px] text-sv-warn">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

function DeviceTile({
  label,
  value,
  sub,
  className = "",
}: {
  label: string;
  value: string;
  sub?: string;
  className?: string;
}) {
  return (
    <div className={`rounded-lg bg-sv-surface-2 px-3.5 py-3 ${className}`}>
      <div className="text-[10px] uppercase tracking-wide text-sv-muted">
        {label}
      </div>
      <div className="mt-0.5 truncate text-sm font-medium">{value}</div>
      {sub && <div className="mt-0.5 truncate text-[11px] text-sv-muted">{sub}</div>}
    </div>
  );
}
