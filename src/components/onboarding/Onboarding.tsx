import { useEffect, useMemo, useState } from "react";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { STT_MODELS } from "../../services/catalog";
import {
  listInputDevices,
  listenEvent,
  recommendDeviceDefaults,
  startMicProbe,
  stopMicProbe,
  startRecording,
  stopAndTranscribe,
} from "../../services/tauriBridge";
import HotkeyRecorder from "../shared/HotkeyRecorder";
import ProviderLogo from "../shared/ProviderLogo";
import type { DeviceRecommendation, HardwareInfo, SttPreset } from "../../types";

// Curated starter choices — one per speed/accuracy tier plus a multilingual
// option. The user picks; we only mark a recommendation.
const CHOICES: { id: string; tagline: string; preset: SttPreset }[] = [
  { id: "tiny.en", tagline: "Fastest — best for older PCs and laptops", preset: "speed" },
  { id: "base.en", tagline: "Good balance of speed and accuracy", preset: "balanced" },
  { id: "parakeet-tdt-0.6b-v2", tagline: "Fast and highly accurate English", preset: "accuracy" },
  { id: "distil-small.en", tagline: "Fast + accurate English — runs fast even on CPU", preset: "accuracy" },
  { id: "distil-large-v3.5", tagline: "Most accurate English — wants a GPU", preset: "accuracy" },
  { id: "small", tagline: "For dictating in other languages", preset: "multilingual" },
];

// What actually determines local Whisper speed is compute (a real GPU, or raw
// CPU power) — NOT how much RAM the machine has. Most laptops/PCs have no
// dedicated GPU, so the default lean is toward the fast small models.
function recommendId(hw: HardwareInfo | null): string {
  if (!hw) return "parakeet-tdt-0.6b-v2";
  const vram = hw.gpu_vram_gb ?? 0;
  if (vram >= 4) return "distil-large-v3.5"; // real dedicated GPU
  // Parakeet is light (640MB, ~1.4GB RAM) and fast even on CPU alone, so it's
  // the default reach for anything past a bare-minimum machine.
  if (vram >= 2 || hw.logical_cores >= 4) return "parakeet-tdt-0.6b-v2";
  return "tiny.en";
}

const STEPS = [
  "Choose a model",
  "Microphone",
  "Push-to-talk key",
  "Say something",
] as const;

const MIC_STEP = 1;

export default function Onboarding() {
  const { hardware, loading } = useHardwareInfo();
  const setSettings = useSettingsStore((s) => s.setSettings);
  const settings = useSettingsStore((s) => s.settings);
  const download = useModelStore((s) => s.download);
  const downloaded = useModelStore((s) => s.downloaded);
  const progress = useModelStore((s) => s.progress);

  const [step, setStep] = useState(0);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [selfTestRecording, setSelfTestRecording] = useState(false);
  const [selfTesting, setSelfTesting] = useState(false);
  const [selfTestText, setSelfTestText] = useState<string | null>(null);
  const [selfTestErr, setSelfTestErr] = useState<string | null>(null);

  const recommendedId = useMemo(() => recommendId(hardware), [hardware]);
  const [choice, setChoice] = useState<string | null>(null);
  const selectedId = choice ?? recommendedId;

  const [reco, setReco] = useState<DeviceRecommendation | null>(null);
  useEffect(() => {
    recommendDeviceDefaults().then(setReco);
  }, []);

  const [devices, setDevices] = useState<string[]>([]);
  const [micLevel, setMicLevel] = useState(0);
  useEffect(() => {
    listInputDevices().then(setDevices);
  }, []);

  // The probe holds the mic open, so it runs only while this step is showing.
  useEffect(() => {
    if (step !== MIC_STEP) return;
    startMicProbe(settings.audio_device);
    const unlisten = listenEvent<number>("mic://level", setMicLevel);
    return () => {
      stopMicProbe();
      unlisten.then((f) => f());
    };
  }, [step, settings.audio_device]);

  function finish() {
    if (reco) {
      setSettings({
        use_gpu: reco.use_gpu,
        high_performance: reco.high_performance,
        performance_threads: reco.performance_threads,
        coedit_enabled: reco.coedit_enabled,
      });
    }
    setSettings({ onboarded: true });
  }

  function applySelection(id: string) {
    const meta = CHOICES.find((c) => c.id === id);
    setSettings({
      active_stt_model: id,
      stt_preset: meta?.preset ?? "balanced",
    });
  }

  async function startSelfTest() {
    setSelfTestErr(null);
    setSelfTestText(null);
    try {
      await startRecording(settings.audio_device);
      setSelfTestRecording(true);
    } catch (e) {
      setSelfTestErr(`Couldn't start recording — check your microphone in Settings. (${String(e)})`);
    }
  }

  async function stopSelfTest() {
    setSelfTestRecording(false);
    setSelfTesting(true);
    try {
      const text = await stopAndTranscribe(
        settings.active_stt_model,
        settings.language
      );
      setSelfTestText(text);
    } catch (e) {
      setSelfTestErr(`Transcription failed — try again. (${String(e)})`);
    } finally {
      setSelfTesting(false);
    }
  }

  const selDl = progress[selectedId];
  const selDownloading = selDl?.status === "downloading";
  const selDownloaded = downloaded.has(selectedId);
  const selfTestReady =
    !!settings.active_stt_model && downloaded.has(settings.active_stt_model);
  const selPct =
    selDl && selDl.total_bytes > 0
      ? Math.round((selDl.downloaded_bytes / selDl.total_bytes) * 100)
      : 0;

  return (
    <div className="flex h-full bg-sv-bg">
      <aside className="flex w-56 shrink-0 flex-col border-r border-sv-border bg-sv-surface px-6 py-7">
        <div>
          {STEPS.map((label, i) => {
            const isCompleted = i < step;
            const isCurrent = i === step;
            return (
              <div key={label} className="flex flex-row items-center gap-3 py-2">
                <div
                  className={`flex h-[19px] w-[19px] shrink-0 items-center justify-center rounded-full border text-[10px] font-semibold ${
                    isCompleted
                      ? "border-sv-good bg-sv-good text-sv-bg"
                      : isCurrent
                      ? "border-sv-accent text-sv-accent"
                      : "border-sv-border text-sv-muted"
                  }`}
                >
                  {isCompleted ? "✓" : i + 1}
                </div>
                <div
                  className={`text-[13px] ${
                    isCompleted
                      ? "text-sv-muted"
                      : isCurrent
                      ? "font-semibold text-sv-text"
                      : "text-sv-muted"
                  }`}
                >
                  {label}
                </div>
              </div>
            );
          })}
        </div>

        <div className="mt-6 border-t border-sv-border pt-5">
          {selDownloading || selDownloaded ? (
            <div>
              <div className="text-[10px] font-semibold uppercase tracking-wide text-sv-muted">
                {selDownloaded ? "Ready" : "Downloading"}
              </div>
              <div className="mt-1 text-[12.5px]">
                {STT_MODELS.find((m) => m.id === selectedId)?.label}
              </div>
              {selDownloading ? (
                <div className="mt-2">
                  <div className="h-1 w-full overflow-hidden rounded-full bg-sv-surface-2">
                    <div
                      className="h-full bg-sv-accent transition-all"
                      style={{ width: `${selPct}%` }}
                    />
                  </div>
                  <div className="mt-1 text-[11px] text-sv-muted">
                    {selPct}% · {Math.max(0, Math.round(((selDl?.total_bytes ?? 0) - (selDl?.downloaded_bytes ?? 0)) / 1048576))} MB left
                  </div>
                </div>
              ) : (
                <div className="mt-1 text-[11px] text-sv-good">Ready to use</div>
              )}
            </div>
          ) : !loading && hardware ? (
            <div>
              <div className="text-[10px] font-semibold uppercase tracking-wide text-sv-muted">
                Your PC
              </div>
              <div className="mt-1 text-[12.5px]">
                {hardware.logical_cores} cores · {hardware.gpu_vram_gb ? "dedicated GPU" : "no dedicated GPU"}
              </div>
              <div className="mt-1 text-[11px] text-sv-muted">
                {reco?.reason ?? "Smaller models will feel faster here."}
              </div>
            </div>
          ) : null}
        </div>

        {step < 4 && (
          <div className="mt-auto">
            <button
              onClick={finish}
              className="text-left text-xs text-sv-muted hover:text-sv-text"
            >
              Skip setup
            </button>
          </div>
        )}
      </aside>

      <div className="flex-1 overflow-y-auto px-9 py-8">
        <div className="mx-auto max-w-lg">
          {step === 0 && (
            <div>
              <h2 className="text-lg font-semibold">Which voice model?</h2>
              <p className="mt-1 text-sm text-sv-muted">
                This does the listening. You can change it later in Model Store.
              </p>

              <div className="mt-4 space-y-2">
                {loading ? (
                  <div className="rounded-xl border border-sv-border bg-sv-surface-2 p-4 text-sm text-sv-muted">
                    Scanning your device…
                  </div>
                ) : (
                  CHOICES.map((c) => {
                    const m = STT_MODELS.find((x) => x.id === c.id);
                    if (!m) return null;
                    const isSel = selectedId === c.id;
                    const isRec = recommendedId === c.id;
                    return (
                      <button
                        key={c.id}
                        onClick={() => {
                          setChoice(c.id);
                          applySelection(c.id);
                        }}
                        className={`flex w-full items-center gap-3 rounded-xl border px-3.5 py-2.5 text-left transition ${
                          isSel
                            ? "border-sv-accent bg-sv-accent/5 ring-1 ring-sv-accent/40"
                            : "border-sv-border bg-sv-surface hover:border-sv-muted/40"
                        }`}
                      >
                        <span
                          className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                            isSel ? "border-sv-accent" : "border-sv-border"
                          }`}
                        >
                          {isSel && (
                            <span className="h-2 w-2 rounded-full bg-sv-accent" />
                          )}
                        </span>
                        <ProviderLogo provider={m.provider} size={28} />
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span className="truncate text-sm font-medium">
                              {m.label}
                            </span>
                            {isRec && (
                              <span className="shrink-0 rounded-full bg-sv-accent px-2 py-0.5 text-[10px] font-medium text-sv-on-accent">
                                Recommended
                              </span>
                            )}
                          </div>
                          <div className="truncate text-[11px] text-sv-muted">
                            {c.tagline} · {m.size_mb} MB · {m.wer} errors
                          </div>
                        </div>
                      </button>
                    );
                  })
                )}
              </div>

              <div className="mt-5">
                <button
                  onClick={() => {
                    applySelection(selectedId);
                    if (!selDownloaded && !selDownloading) {
                      download(selectedId);
                    }
                    setStep(1);
                  }}
                  className="w-full rounded-lg bg-sv-accent px-4 py-2.5 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover"
                >
                  {selDownloaded ? "Continue" : "Download and continue"}
                </button>
              </div>
            </div>
          )}

          {step === 1 && (
            <div>
              <h2 className="text-lg font-semibold">Can it hear you?</h2>
              <p className="mt-1 text-sm text-sv-muted">
                Speak normally. If the meter doesn't move, pick a different microphone below.
              </p>

              <select
                value={settings.audio_device ?? ""}
                onChange={(e) =>
                  setSettings({ audio_device: e.target.value || null })
                }
                className="mt-4 w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
              >
                <option value="">System default</option>
                {devices.map((d) => (
                  <option key={d} value={d}>
                    {d}
                  </option>
                ))}
              </select>

              <div className="mt-4 h-2.5 w-full overflow-hidden rounded-full bg-sv-surface-2">
                <div
                  className="h-full rounded-full bg-sv-accent transition-[width] duration-75"
                  style={{ width: `${Math.min(micLevel, 100)}%` }}
                />
              </div>
              <p className="mt-1.5 text-[11px] text-sv-muted">
                Speak now — the bar should move.
              </p>

              <div className="mt-6 flex gap-2">
                <button
                  onClick={() => setStep(0)}
                  className="rounded-lg border border-sv-border px-4 py-2 text-sm text-sv-muted hover:text-sv-text"
                >
                  Back
                </button>
                <button
                  onClick={() => setStep(2)}
                  className="flex-1 rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover"
                >
                  Continue
                </button>
              </div>
            </div>
          )}

          {step === 2 && (
            <div>
              <h2 className="text-lg font-semibold">Set your push-to-talk key</h2>
              <p className="mt-1 text-sm text-sv-muted">
                Hold this key while you speak, release to paste. Pick
                something you won't hit by accident while typing.
              </p>
              <div className="mt-5">
                <HotkeyRecorder
                  value={settings.hotkey}
                  onChange={(accelerator) => {
                    setHotkeyError(null);
                    setSettings({ hotkey: accelerator });
                  }}
                  error={hotkeyError}
                />
              </div>
              <div className="mt-6 flex gap-2">
                <button
                  onClick={() => setStep(1)}
                  className="rounded-lg border border-sv-border px-4 py-2 text-sm text-sv-muted hover:text-sv-text"
                >
                  Back
                </button>
                <button
                  onClick={() => setStep(3)}
                  className="flex-1 rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover"
                >
                  Continue
                </button>
              </div>
            </div>
          )}

          {step === 3 && (
            <div>
              <h2 className="text-lg font-semibold">Try it out</h2>
              <p className="mt-1 text-sm text-sv-muted">
                A quick check so you know it works before you rely on it. Record
                a sentence, then stop — your words appear right here (nothing is
                pasted).
              </p>
              {!selfTestReady ? (
                <div className="mt-4 rounded-xl border border-sv-border bg-sv-surface-2 p-4 text-sm text-sv-muted">
                  Your model is still downloading — watch the rail on the left.
                  You can skip this and try it anytime later.
                </div>
              ) : (
                <div className="mt-4 space-y-3">
                  <button
                    onClick={selfTestRecording ? stopSelfTest : startSelfTest}
                    disabled={selfTesting}
                    className={`w-full rounded-lg px-4 py-2.5 text-sm font-medium text-white disabled:opacity-50 ${
                      selfTestRecording
                        ? "bg-sv-bad hover:opacity-90"
                        : "bg-sv-accent hover:bg-sv-accent-hover"
                    }`}
                  >
                    {selfTesting
                      ? "Transcribing…"
                      : selfTestRecording
                      ? "Stop & transcribe"
                      : "Record a test"}
                  </button>
                  {selfTestText !== null && (
                    <div className="rounded-xl border border-sv-border bg-sv-bg p-3 text-sm">
                      {selfTestText.trim()
                        ? `“${selfTestText.trim()}”`
                        : "No speech detected — try again, a little louder."}
                    </div>
                  )}
                  {selfTestErr && (
                    <p className="text-xs text-sv-bad">{selfTestErr}</p>
                  )}
                </div>
              )}
              <div className="mt-6 flex gap-2">
                <button
                  onClick={() => setStep(2)}
                  disabled={selfTestRecording || selfTesting}
                  className="rounded-lg border border-sv-border px-4 py-2 text-sm text-sv-muted hover:text-sv-text disabled:opacity-50"
                >
                  Back
                </button>
                <button
                  onClick={() => setStep(4)}
                  disabled={selfTestRecording || selfTesting}
                  className="flex-1 rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover disabled:opacity-50"
                >
                  Continue
                </button>
              </div>
            </div>
          )}

          {step === 4 && (
            <div className="text-center">
              <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-sv-good/15 text-sv-good">
                <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M20 6 9 17l-5-5" />
                </svg>
              </div>
              <h2 className="text-lg font-semibold">You're all set</h2>
              <p className="mt-2 text-sm text-sv-muted">
                Hold <kbd className="rounded bg-sv-surface-2 px-1.5 py-0.5 text-sv-text">{settings.hotkey}</kbd>{" "}
                anywhere on your PC to dictate. If your model is still
                downloading, dictation will work as soon as it finishes — you
                can watch progress in Model Store.
              </p>
              <button
                onClick={finish}
                className="mt-6 w-full rounded-lg bg-sv-accent px-4 py-2.5 text-sm font-medium text-sv-on-accent hover:bg-sv-accent-hover"
              >
                Start using Silent Voice
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
