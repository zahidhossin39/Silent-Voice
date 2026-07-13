import { useMemo, useState } from "react";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { STT_MODELS } from "../../services/catalog";
import HotkeyRecorder from "../shared/HotkeyRecorder";
import ProviderLogo from "../shared/ProviderLogo";
import type { HardwareInfo, SttPreset } from "../../types";

// Curated starter choices — one per speed/accuracy tier plus a multilingual
// option. The user picks; we only mark a recommendation.
const CHOICES: { id: string; tagline: string; preset: SttPreset }[] = [
  { id: "tiny.en", tagline: "Fastest — best for older PCs and laptops", preset: "speed" },
  { id: "base.en", tagline: "Good balance of speed and accuracy", preset: "balanced" },
  { id: "distil-small.en", tagline: "Fast + accurate English — runs fast even on CPU", preset: "accuracy" },
  { id: "distil-large-v3.5", tagline: "Most accurate English — wants a GPU", preset: "accuracy" },
  { id: "small", tagline: "For dictating in other languages", preset: "multilingual" },
];

// What actually determines local Whisper speed is compute (a real GPU, or raw
// CPU power) — NOT how much RAM the machine has. Most laptops/PCs have no
// dedicated GPU, so the default lean is toward the fast small models.
function recommendId(hw: HardwareInfo | null): string {
  if (!hw) return "base.en";
  const vram = hw.gpu_vram_gb ?? 0;
  if (vram >= 4) return "distil-large-v3.5"; // real dedicated GPU
  if (vram >= 2) return "distil-small.en";
  // CPU-only (integrated/shared graphics): speed comes from the CPU alone.
  if (hw.logical_cores >= 12) return "base.en";
  return "tiny.en";
}

const STEPS = ["Welcome", "Pick a model", "Set your hotkey", "Done"] as const;

export default function Onboarding() {
  const { hardware, loading } = useHardwareInfo();
  const setSettings = useSettingsStore((s) => s.setSettings);
  const settings = useSettingsStore((s) => s.settings);
  const download = useModelStore((s) => s.download);
  const downloaded = useModelStore((s) => s.downloaded);
  const progress = useModelStore((s) => s.progress);

  const [step, setStep] = useState(0);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);

  const recommendedId = useMemo(() => recommendId(hardware), [hardware]);
  const [choice, setChoice] = useState<string | null>(null);
  const selectedId = choice ?? recommendedId;

  function finish() {
    setSettings({ onboarded: true });
  }

  function applySelection(id: string) {
    const meta = CHOICES.find((c) => c.id === id);
    setSettings({
      active_stt_model: id,
      stt_preset: meta?.preset ?? "balanced",
    });
  }

  const selDl = progress[selectedId];
  const selDownloading = selDl?.status === "downloading";
  const selDownloaded = downloaded.has(selectedId);
  const selPct =
    selDl && selDl.total_bytes > 0
      ? Math.round((selDl.downloaded_bytes / selDl.total_bytes) * 100)
      : 0;

  return (
    <div className="flex h-full items-center justify-center overflow-y-auto bg-sv-bg px-6 py-8">
      <div className="w-full max-w-xl">
        {/* Step dots */}
        <div className="mb-6 flex items-center justify-center gap-2">
          {STEPS.map((label, i) => (
            <div
              key={label}
              className={`h-1.5 rounded-full transition-all ${
                i === step
                  ? "w-6 bg-sv-accent"
                  : i < step
                  ? "w-1.5 bg-sv-accent/50"
                  : "w-1.5 bg-sv-surface-2"
              }`}
            />
          ))}
        </div>

        <div className="rounded-2xl border border-sv-border bg-sv-surface p-8 shadow-xl">
          {step === 0 && (
            <div className="text-center">
              <svg viewBox="0 0 1024 1024" className="mx-auto mb-4 h-14 w-14 rounded-2xl">
                <rect x="0" y="0" width="1024" height="1024" rx="224" fill="#0d0f14"/>
                <rect x="232" y="432" width="80" height="160" rx="40" fill="#f97316"/>
                <rect x="360" y="352" width="80" height="320" rx="40" fill="#f97316"/>
                <rect x="488" y="252" width="80" height="520" rx="40" fill="#ffffff"/>
                <rect x="616" y="352" width="80" height="320" rx="40" fill="#f97316"/>
                <rect x="744" y="432" width="80" height="160" rx="40" fill="#f97316"/>
              </svg>
              <h1 className="text-xl font-semibold">Welcome to Silent Voice</h1>
              <p className="mt-2 text-sm text-sv-muted">
                Free, local-first voice-to-text. Hold a hotkey, speak, release
                — your words appear at the cursor. Everything runs on your
                device; nothing is sent anywhere unless you turn on a cloud
                provider yourself.
              </p>
              <p className="mt-3 text-sm text-sv-muted">
                Two quick steps to get set up.
              </p>
              <button
                onClick={() => setStep(1)}
                className="mt-6 w-full rounded-lg bg-sv-accent px-4 py-2.5 text-sm font-medium text-white hover:bg-sv-accent-hover"
              >
                Get started
              </button>
            </div>
          )}

          {step === 1 && (
            <div>
              <h2 className="text-lg font-semibold">Pick a starting model</h2>
              <p className="mt-1 text-sm text-sv-muted">
                Speed depends on your graphics card and CPU — bigger isn't
                better on a machine without a dedicated GPU. You can switch
                anytime in the Model Store.
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
                              <span className="shrink-0 rounded-full bg-sv-accent px-2 py-0.5 text-[10px] font-medium text-white">
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

              {/* Download state for the selected model */}
              <div className="mt-4">
                {selDownloaded ? (
                  <div className="rounded-lg bg-sv-good/10 px-3 py-2 text-xs text-sv-good">
                    ✓ Downloaded and ready
                  </div>
                ) : selDownloading ? (
                  <div>
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-sv-surface-2">
                      <div
                        className="h-full bg-sv-accent transition-all"
                        style={{ width: `${selPct}%` }}
                      />
                    </div>
                    <p className="mt-1 text-[11px] text-sv-muted">
                      Downloading… {selPct}%
                    </p>
                  </div>
                ) : (
                  <button
                    onClick={() => {
                      applySelection(selectedId);
                      download(selectedId);
                    }}
                    className="w-full rounded-lg bg-sv-accent px-3 py-2 text-xs font-medium text-white hover:bg-sv-accent-hover"
                  >
                    Download selected model
                  </button>
                )}
              </div>

              <div className="mt-5 flex gap-2">
                <button
                  onClick={() => setStep(0)}
                  className="rounded-lg border border-sv-border px-4 py-2 text-sm text-sv-muted hover:text-sv-text"
                >
                  Back
                </button>
                <button
                  onClick={() => {
                    applySelection(selectedId);
                    setStep(2);
                  }}
                  className="flex-1 rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-white hover:bg-sv-accent-hover"
                >
                  {selDownloaded ? "Continue" : "Continue — I'll download later"}
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
                  className="flex-1 rounded-lg bg-sv-accent px-4 py-2 text-sm font-medium text-white hover:bg-sv-accent-hover"
                >
                  Continue
                </button>
              </div>
            </div>
          )}

          {step === 3 && (
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
                can watch progress in the Model Store.
              </p>
              <button
                onClick={finish}
                className="mt-6 w-full rounded-lg bg-sv-accent px-4 py-2.5 text-sm font-medium text-white hover:bg-sv-accent-hover"
              >
                Start using Silent Voice
              </button>
            </div>
          )}
        </div>

        {step > 0 && step < 3 && (
          <button
            onClick={finish}
            className="mx-auto mt-4 block text-xs text-sv-muted hover:text-sv-text"
          >
            Skip setup
          </button>
        )}
      </div>
    </div>
  );
}
