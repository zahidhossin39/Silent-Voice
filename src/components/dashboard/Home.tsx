import Page from "../shared/Page";
import WaveformVisualizer from "../shared/WaveformVisualizer";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { useSettingsStore } from "../../stores/settingsStore";
import { useHistoryStore } from "../../stores/historyStore";
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
  const modes = useSettingsStore((s) => s.modes);
  const entries = useHistoryStore((s) => s.entries);
  const downloadedCount = useModelStore((s) => s.downloaded.size);
  const recordingState = useUiStore((s) => s.recordingState);
  const lastError = useUiStore((s) => s.lastError);

  const activeMode =
    modes.find((m) => m.id === settings.active_mode_id)?.name ?? "Raw";

  return (
    <Page
      title="Home"
      subtitle="Quick status and your device at a glance"
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
          <div className="text-right text-xs text-sv-muted">
            <div>
              Hotkey:{" "}
              <kbd className="rounded bg-sv-surface-2 px-1.5 py-0.5 text-sv-text">
                {settings.hotkey}
              </kbd>
            </div>
            <div className="mt-1">
              Hold to talk · release to transcribe &amp; paste
            </div>
          </div>
        </div>
      </div>

      {/* Quick stats */}
      <div className="mb-5 grid grid-cols-3 gap-4">
        <StatCard label="Active STT model" value={settings.active_stt_model} />
        <StatCard label="Active mode" value={activeMode} />
        <StatCard
          label="Downloaded models"
          value={String(downloadedCount)}
        />
      </div>

      {/* Device info */}
      <div className="mb-5 rounded-xl border border-sv-border bg-sv-surface p-5">
        <h2 className="mb-4 text-sm font-semibold">Your device</h2>
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

      {/* Recent transcriptions */}
      <div className="rounded-xl border border-sv-border bg-sv-surface p-5">
        <h2 className="mb-3 text-sm font-semibold">Recent transcriptions</h2>
        {entries.length === 0 ? (
          <p className="text-sm text-sv-muted">
            Nothing yet. Your dictations will appear here.
          </p>
        ) : (
          <ul className="space-y-2">
            {entries.slice(0, 5).map((e) => (
              <li
                key={e.id}
                className="truncate rounded-lg bg-sv-surface-2 px-3 py-2 text-sm"
              >
                {e.processed_text || e.raw_text}
              </li>
            ))}
          </ul>
        )}
      </div>
    </Page>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-sv-border bg-sv-surface p-4">
      <div className="text-xs uppercase tracking-wide text-sv-muted">
        {label}
      </div>
      <div className="mt-1 truncate text-lg font-semibold">{value}</div>
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
