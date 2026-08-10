import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import Page from "../shared/Page";
import WaveformVisualizer from "../shared/WaveformVisualizer";
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
  const lastError = useUiStore((s) => s.lastError);

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
    <Page
      title="Home"
      subtitle={
        <>
            Hold{" "}
            <kbd className="rounded border border-sv-border bg-sv-surface-2 px-1.5 py-0.5 text-xs font-mono">
              {settings.hotkey}
            </kbd>{" "}
            anywhere in Windows, speak, and your words land at the cursor.
        </>
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

      {/* Status card */}
      <div className="mb-5 rounded-xl border border-sv-border bg-sv-surface p-5">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-xs uppercase tracking-wide text-sv-muted">
              Status
            </div>
            <div className="mt-1 flex items-center gap-3">
              <span className="text-lg font-semibold">
                {STATUS_LABEL[recordingState] ?? "Idle"}
              </span>
              <WaveformVisualizer
                active={
                  recordingState === "recording" ||
                  recordingState === "listening"
                }
              />
            </div>
          </div>
          <div className="flex flex-col items-end">
            <div className="mb-1.5 text-[11px] text-sv-muted">
              Push-to-talk hotkey
            </div>
            <InlineHotkey
              value={settings.hotkey}
              onChange={(hk) => setSettings({ hotkey: hk })}
            />
            <p className="mt-2 max-w-[15rem] text-right text-[11px] leading-relaxed text-sv-muted">
              <span className="font-medium text-sv-text">Hold</span> it to speak,{" "}
              <span className="font-medium text-sv-text">release</span> to drop
              the text right at your cursor.
            </p>
          </div>
        </div>
      </div>

      {/* Ready to dictate row */}
      <div className="mb-5 rounded-xl border border-sv-border bg-sv-surface p-4 sm:p-5">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="grid flex-1 gap-3 sm:grid-cols-3 sm:gap-6">
            <StatusCheck
              pass={sttPass}
              label="Voice model"
              value={sttPass ? sttValue : (settings.active_stt_model ? "Not downloaded" : "None selected")}
              fixLink="/models"
              fixText="Choose one →"
            />
            <StatusCheck
              pass={micPass}
              label="Microphone"
              value={micPass ? micValue : "Not set"}
              fixLink="/settings"
              fixText="Pick one →"
            />
            <StatusCheck
              pass={hotkeyPass}
              label="Hotkey"
              value={hotkeyPass ? settings.hotkey : "Not set"}
              fixLink="/settings"
              fixText="Set one →"
            />
          </div>
          <div className="text-sm sm:text-right">
            {allPass ? (
              <span className="sv-ready inline-block text-sv-text">
                Ready — hold{" "}
                <kbd className="sv-ready-key rounded border border-sv-border bg-sv-surface-2 px-1.5 py-0.5 text-xs font-mono">
                  {settings.hotkey}
                </kbd>{" "}
                to dictate.
              </span>
            ) : (
              <span className="text-sv-bad">Dictation won't work yet.</span>
            )}
          </div>
        </div>
      </div>

      {/* Device info */}
      <div className="mb-5 rounded-xl border border-sv-border bg-sv-surface p-5">
        <div className="mb-1 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Your device</h2>
          {!loading && hardware && (
            <div className="flex items-center gap-3">
              <span className="rounded bg-sv-surface-2 px-2 py-0.5 text-[11px] text-sv-text">
                {hardware.gpu_vram_gb && hardware.gpu_vram_gb >= 2
                  ? "GPU-class models"
                  : "Fast, small models"}
              </span>
              <Link to="/models" className="text-sm text-sv-accent hover:underline">
                See models →
              </Link>
            </div>
          )}
        </div>
        
        {loading || !hardware ? (
          <p className="mt-3 text-sm text-sv-muted">Scanning…</p>
        ) : (
          <>
            <p className="mb-4 text-[13px] text-sv-muted">
              {hardware.gpu_vram_gb && hardware.gpu_vram_gb >= 2
                ? "This device can handle the larger, more accurate models"
                : "Tiny / Base run best here — larger models will feel slow"}
            </p>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
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
                className="col-span-2 md:col-span-4"
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
            </div>
          </>
        )}
      </div>

      {/* Quick controls — same settings as the Settings tab, surfaced here */}
      <div className="rounded-xl border border-sv-border bg-sv-surface p-5">
        <div className="mb-4">
          <h2 className="text-sm font-semibold">Quick controls</h2>
          <p className="mt-1 text-[13px] text-sv-muted">Changing these here is the same as changing them in Settings.</p>
        </div>
        <div className="grid gap-4 sm:grid-cols-2">
          <QuickControl
            label="Speech model"
            hint={
              downloadedModels.length === 0
                ? "No models downloaded yet — get one in Model Store"
                : "Which model transcribes your voice"
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
              className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent disabled:opacity-50"
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

          <QuickControl label="Language" hint="What language you dictate in">
            <select
              value={settings.language}
              onChange={(e) => setSettings({ language: e.target.value })}
              className="w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent"
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
    </Page>
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
    <div>
      <div className="mb-1.5">
        <div className="text-sm font-medium">{label}</div>
        {hint && <div className="text-[11px] text-sv-muted">{hint}</div>}
      </div>
      {children}
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
  return (
    <div className="flex items-start gap-2">
      <div
        className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${
          pass ? "bg-sv-good" : "bg-sv-bad"
        }`}
      />
      <div className="flex flex-col">
        <div className="text-[10px] uppercase tracking-wide text-sv-muted">
          {label}
        </div>
        <div className="text-sm font-medium">
          {value}
          {!pass && (
            <Link to={fixLink} className="ml-2 font-normal text-sv-accent hover:underline">
              {fixText}
            </Link>
          )}
        </div>
      </div>
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
