import { useEffect, useState } from "react";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useHistoryStore } from "../../stores/historyStore";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { STT_MODELS, LANGUAGES, TTS_MODELS, TTS_SAMPLE_TEXT } from "../../services/catalog";
import {
  listInputDevices,
  setHotkey,
  getAutostart,
  ttsSpeakText,
  downloadCoeditModel,
  coeditInstalled,
  deleteCoeditModel,
  downloadGectorModel,
  gectorInstalled,
  deleteGectorModel,
  downloadVadModel,
  vadInstalled,
  deleteVadModel,
  recommendDeviceDefaults,
} from "../../services/tauriBridge";
import type { DeviceRecommendation } from "../../types";
import HotkeyRecorder from "../shared/HotkeyRecorder";
import ScrollNumberPicker from "../shared/ScrollNumberPicker";
import { checkForUpdatesManual } from "../../services/updater";
import type { Settings } from "../../types";

const RETENTION_OPTIONS: { value: Settings["history_retention"]; label: string }[] = [
  { value: "never", label: "Never" },
  { value: "3d", label: "After 3 days" },
  { value: "2w", label: "After 2 weeks" },
  { value: "3m", label: "After 3 months" },
  { value: "custom", label: "Custom…" },
];

const RETENTION_UNITS: Settings["history_retention_custom_unit"][] = [
  "hours",
  "days",
  "weeks",
  "months",
];

const SELECT_CLS =
  "h-9 rounded-lg border border-sv-border bg-sv-bg px-3 text-sm focus:outline-none focus:ring-1 focus:ring-sv-accent";

export default function Settings() {
  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const reprune = useHistoryStore((s) => s.reprune);

  useEffect(() => {
    reprune();
  }, [settings.history_limit, settings.history_retention, settings.history_retention_custom_value, settings.history_retention_custom_unit, reprune]);

  // Toggle ON = rule active = NOT in the disabled list.
  const toggleProofreadRule = (rule: string, enabled: boolean) => {
    const rest = settings.proofread_disabled_rules.filter((r) => r !== rule);
    setSettings({ proofread_disabled_rules: enabled ? rest : [...rest, rule] });
  };
  const providers = useSettingsStore((s) => s.providers);
  const sttProviders = providers.filter((p) => p.uses.includes("stt"));
  const snippets = useSettingsStore((s) => s.snippets);
  const addSnippet = useSettingsStore((s) => s.addSnippet);
  const updateSnippet = useSettingsStore((s) => s.updateSnippet);
  const deleteSnippet = useSettingsStore((s) => s.deleteSnippet);
  const modes = useSettingsStore((s) => s.modes);
  const appProfiles = useSettingsStore((s) => s.appProfiles);
  const addAppProfile = useSettingsStore((s) => s.addAppProfile);
  const updateAppProfile = useSettingsStore((s) => s.updateAppProfile);
  const deleteAppProfile = useSettingsStore((s) => s.deleteAppProfile);
  const downloadedStt = useModelStore((s) => s.downloaded);
  const downloadedTts = useModelStore((s) => s.downloadedTts);
  const { hardware } = useHardwareInfo();
  const [updateMsg, setUpdateMsg] = useState("");
  const [devices, setDevices] = useState<string[]>([]);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);

  const [reco, setReco] = useState<DeviceRecommendation | null>(null);
  useEffect(() => { recommendDeviceDefaults().then(setReco); }, []);

  const [coeditReady, setCoeditReady] = useState(false);
  const [coeditFetching, setCoeditFetching] = useState(false);
  useEffect(() => { coeditInstalled().then(setCoeditReady); }, []);
  const coeditProgress = useModelStore((s) => s.progress["coedit"]);
  useEffect(() => {
    if (coeditProgress?.status === "downloaded") {
      coeditInstalled().then(setCoeditReady);
    }
  }, [coeditProgress?.status]);

  const [vadReady, setVadReady] = useState(false);
  useEffect(() => { vadInstalled().then(setVadReady); }, []);
  const vadProgress = useModelStore((s) => s.progress["vad"]);
  useEffect(() => {
    if (vadProgress?.status === "downloaded") {
      vadInstalled().then(setVadReady);
    }
  }, [vadProgress?.status]);

  const [gectorReady, setGectorReady] = useState(false);
  useEffect(() => { gectorInstalled().then(setGectorReady); }, []);
  const gectorProgress = useModelStore((s) => s.progress["gector"]);
  useEffect(() => {
    if (gectorProgress?.status === "downloaded") {
      gectorInstalled().then(setGectorReady);
    }
  }, [gectorProgress?.status]);
  const [gectorVariant, setGectorVariant] = useState("int8");
  // GECToR is 5 separate files and download_to emits "downloaded" after each
  // one, so the progress event alone leaves a gap between files where the
  // Download button would reappear and a second fetch could be started.
  const [gectorFetching, setGectorFetching] = useState(false);

  useEffect(() => {
    listInputDevices().then(setDevices);
    // The registry is the truth for "Launch at startup" — sync the toggle to
    // it so the UI can't show ON while no Run-key entry actually exists.
    getAutostart().then((real) => {
      const current = useSettingsStore.getState().settings.auto_start;
      if (current !== real) setSettings({ auto_start: real });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleHotkeyChange(accelerator: string) {
    setHotkeyError(null);
    setSettings({ hotkey: accelerator });
    try {
      await setHotkey(accelerator);
    } catch (e) {
      setHotkeyError(`Failed to register: ${e}`);
    }
  }

  const hasGpu = !!hardware?.gpu_vram_gb && hardware.gpu_vram_gb >= 1;

  return (
    <Page title="Settings" subtitle="Dictation, audio, and appearance">
      <div className="gap-5 lg:columns-2 lg:gap-5">
        <Section title="Dictation" accent="var(--color-sv-sec-dictation)" icon={<MicrophoneIcon />}>

          <div className="border-b border-sv-border/60 py-3.5">
            <div className="text-sm">Global hotkey (push-to-talk)</div>
            <div className="mt-0.5 text-xs text-sv-muted">
              Hold to record, release to paste
            </div>
            <div className="mt-3">
              <HotkeyRecorder
                value={settings.hotkey}
                onChange={handleHotkeyChange}
                error={hotkeyError}
              />
            </div>
          </div>
          <SubGroup label="While recording">
            <Row
              label="Double-tap to lock recording"
              hint="Tap the hotkey twice quickly to keep recording hands-free; press once to stop & paste"
            >
              <Toggle
                checked={settings.toggle_mode}
                onChange={(v) => setSettings({ toggle_mode: v })}
              />
            </Row>
            <Row
              label="Auto-hide the pill"
              hint="Pill disappears ~5s after it's done, and reappears the moment you press the hotkey again"
            >
              <Toggle
                checked={settings.pill_auto_hide}
                onChange={(v) => setSettings({ pill_auto_hide: v })}
              />
            </Row>
            <Row
              label="Append trailing space"
              hint="Add a space after each pasted transcription so the next one doesn't run into it"
            >
              <Toggle
                checked={settings.append_trailing_space}
                onChange={(v) => setSettings({ append_trailing_space: v })}
              />
            </Row>
          </SubGroup>
          <Row
            label="Speech-to-text source"
            hint={
              sttProviders.length === 0
                ? "Add a provider in API Keys with \"STT\" checked to unlock cloud options"
                : undefined
            }
          >
            <select
              value={settings.stt_cloud_provider_id ?? "local"}
              onChange={(e) =>
                setSettings({
                  stt_cloud_provider_id:
                    e.target.value === "local" ? null : e.target.value,
                })
              }
              className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
            >
              <option value="local">Local (on this device)</option>
              {sttProviders.map((p) => (
                <option key={p.id} value={p.id}>
                  Cloud — {p.name}
                </option>
              ))}
            </select>
          </Row>
          {settings.stt_cloud_provider_id === null && (
            <Row label="Active STT model">
              <select
                value={settings.active_stt_model}
                onChange={(e) =>
                  setSettings({ active_stt_model: e.target.value })
                }
                className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
              >
                {[...STT_MODELS]
                  .sort((a, b) => {
                    const aDown = downloadedStt.has(a.id) ? 0 : 1;
                    const bDown = downloadedStt.has(b.id) ? 0 : 1;
                    return aDown - bDown;
                  })
                  .map((m) => (
                    <option key={m.id} value={m.id}>
                      {downloadedStt.has(m.id) ? "✓ " : ""}
                      Whisper {m.label}
                    </option>
                  ))}
              </select>
            </Row>
          )}
          <Row
            label="Language"
            hint={
              settings.language === "auto"
                ? "Tip: pick your language instead of Auto-detect for better accuracy"
                : undefined
            }
          >
            <select
              value={settings.language}
              onChange={(e) => setSettings({ language: e.target.value })}
              className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
            >
              {LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>
                  {l.name}
                </option>
              ))}
            </select>
          </Row>
          <Row label="Microphone">
            <select
              value={settings.audio_device ?? ""}
              onChange={(e) =>
                setSettings({ audio_device: e.target.value || null })
              }
              className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
            >
              <option value="">System default</option>
              {devices.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </Row>

          <Row
            label="Grammar correction"
            hint="Fix grammar in dictated text before pasting (Raw mode). Skipped when an AI mode is active."
          >
            {(() => {
              if (coeditReady) {
                return (
                  <div className="flex items-center gap-3">
                    <button
                      onClick={async () => {
                        await deleteCoeditModel();
                        setCoeditReady(false);
                      }}
                      className="rounded-lg border border-sv-border px-2.5 py-1 text-xs text-sv-text hover:border-red-500 hover:text-red-500"
                    >
                      Remove
                    </button>
                    <Toggle
                      checked={settings.coedit_enabled}
                      onChange={(v) => setSettings({ coedit_enabled: v })}
                    />
                  </div>
                );
              }
              const downloading = coeditFetching || coeditProgress?.status === "downloading";
              const pct = coeditProgress && coeditProgress.total_bytes > 0
                ? Math.round((coeditProgress.downloaded_bytes / coeditProgress.total_bytes) * 100)
                : 0;
              if (downloading) {
                return <span className="text-xs text-sv-muted">Downloading… {pct}%</span>;
              }
              return (
                <button
                  onClick={async () => {
                    setCoeditFetching(true);
                    try {
                      await downloadCoeditModel();
                      const ok = await coeditInstalled();
                      setCoeditReady(ok);
                    } finally {
                      setCoeditFetching(false);
                    }
                  }}
                  className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:border-sv-accent hover:text-sv-accent"
                >
                  Download · 818 MB
                </button>
              );
            })()}
          </Row>
          <Row
            label="Inline proofreading"
            hint="Red/blue underlines beneath spelling & grammar mistakes as you type in any app (English only)"
          >
            <Toggle
              checked={settings.inline_proofread}
              onChange={(v) => setSettings({ inline_proofread: v })}
            />
          </Row>
          {settings.inline_proofread && (
            <div className="ml-4 border-l border-sv-border pl-4">
              <Row
                label="Oxford comma suggestions"
                hint='Suggest a comma before "and" in lists ("apples, oranges, and bananas")'
              >
                <Toggle
                  checked={!settings.proofread_disabled_rules.includes("OxfordComma")}
                  onChange={(v) => toggleProofreadRule("OxfordComma", v)}
                />
              </Row>
              <Row
                label="Flag filler words"
                hint='Underline spoken fillers like "um" and "uh" that slip into dictation'
              >
                <Toggle
                  checked={!settings.proofread_disabled_rules.includes("Filler")}
                  onChange={(v) => toggleProofreadRule("Filler", v)}
                />
              </Row>
              <Row
                label="Context grammar (neural)"
                hint="AI pass that catches correctly-spelled wrong words"
              >
                {(() => {
                  if (gectorReady) {
                    return (
                      <div className="flex items-center gap-3">
                        <button
                          onClick={async () => {
                            await deleteGectorModel();
                            setGectorReady(false);
                          }}
                          className="rounded-lg border border-sv-border px-2.5 py-1 text-xs text-sv-text hover:border-red-500 hover:text-red-500"
                        >
                          Remove
                        </button>
                        <Toggle
                          checked={!settings.proofread_disabled_rules.includes("Gector")}
                          onChange={(v) => toggleProofreadRule("Gector", v)}
                        />
                      </div>
                    );
                  }
                  const downloading = gectorFetching || gectorProgress?.status === "downloading";
                  const pct = gectorProgress && gectorProgress.total_bytes > 0
                    ? Math.round((gectorProgress.downloaded_bytes / gectorProgress.total_bytes) * 100)
                    : 0;
                  if (downloading) {
                    return <span className="text-xs text-sv-muted">Downloading… {pct}%</span>;
                  }
                  return (
                    <div className="flex items-center gap-2">
                      <select
                        value={gectorVariant}
                        onChange={(e) => setGectorVariant(e.target.value)}
                        className="w-40 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                      >
                        <option value="int8">Balanced · 122 MB</option>
                        <option value="fp32">Best quality · 512 MB</option>
                      </select>
                      <button
                        onClick={async () => {
                          setGectorFetching(true);
                          try {
                            await downloadGectorModel(gectorVariant);
                            setGectorReady(await gectorInstalled());
                          } finally {
                            setGectorFetching(false);
                          }
                        }}
                        className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:border-sv-accent hover:text-sv-accent"
                      >
                        Download
                      </button>
                    </div>
                  );
                })()}
              </Row>
              {gectorReady && !settings.proofread_disabled_rules.includes("Gector") && (
                <Row
                  label="Sensitivity"
                  hint="How eager the grammar AI is to flag mistakes. Aggressive finds more but makes more false alarms."
                >
                  <select
                    value={settings.gector_sensitivity}
                    onChange={(e) =>
                      setSettings({
                        gector_sensitivity: e.target.value as "relaxed" | "balanced" | "aggressive",
                      })
                    }
                    className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                  >
                    <option value="relaxed">Relaxed</option>
                    <option value="balanced">Balanced</option>
                    <option value="aggressive">Aggressive</option>
                  </select>
                </Row>
              )}
              <div className="py-3.5">
                <div className="text-sm">Don't check in these apps</div>
                <div className="mt-0.5 text-xs text-sv-muted">
                  Comma-separated app names, e.g. "code, photoshop" — squiggles
                  are never shown there
                </div>
                <input
                  value={settings.proofread_ignore_apps}
                  onChange={(e) =>
                    setSettings({ proofread_ignore_apps: e.target.value })
                  }
                  placeholder="code, photoshop"
                  className="mt-2 w-full rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                />
              </div>
            </div>
          )}
          <div className="py-3.5">
            <div className="flex items-center justify-between">
              <div className="text-sm">Input sensitivity</div>
              <span className="text-xs tabular-nums text-sv-muted">
                {settings.input_sensitivity}
              </span>
            </div>
            <div className="mt-0.5 text-xs text-sv-muted">
              Sounds quieter than this are treated as silence and trimmed
              before transcription — cuts wind and background hum. Lower =
              stricter (only clear speech counts); higher = more sensitive.
            </div>
            <input
              type="range"
              min={0}
              max={100}
              step={5}
              value={settings.input_sensitivity}
              onChange={(e) =>
                setSettings({ input_sensitivity: Number(e.target.value) })
              }
              className="sv-slider mt-3 w-full"
              style={
                {
                  "--sv-slider-fill": `${settings.input_sensitivity}%`,
                } as React.CSSProperties
              }
            />
          </div>
          <Row
            label="Smart voice detection"
            hint="Uses a tiny neural model to tell your voice apart from fans, keyboards and music, instead of just going by loudness. Applies automatically once downloaded; the slider still controls how strict it is."
          >
            {(() => {
              if (vadReady) {
                return (
                  <button
                    onClick={async () => {
                      await deleteVadModel();
                      setVadReady(false);
                    }}
                    className="rounded-lg border border-sv-border px-2.5 py-1 text-xs text-sv-text hover:border-red-500 hover:text-red-500"
                  >
                    Remove
                  </button>
                );
              }
              const downloading = vadProgress?.status === "downloading";
              const pct = vadProgress && vadProgress.total_bytes > 0
                ? Math.round((vadProgress.downloaded_bytes / vadProgress.total_bytes) * 100)
                : 0;
              if (downloading) {
                return <span className="text-xs text-sv-muted">Downloading… {pct}%</span>;
              }
              return (
                <button
                  onClick={async () => {
                    await downloadVadModel();
                    setVadReady(await vadInstalled());
                  }}
                  className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:border-sv-accent hover:text-sv-accent"
                >
                  Download · 2 MB
                </button>
              );
            })()}
          </Row>
        </Section>

        <Section
          title="Read aloud (text-to-speech)"
          desc="Select text in any app, press the hotkey, and hear it spoken. Press again to stop. Voices are downloaded in Model Store → Text-to-Speech."
          accent="var(--color-sv-sec-tts)"
          icon={<SpeakerIcon />}
        >
          <Row
            label="Enable read-aloud"
            hint="Turn the read-aloud hotkey on or off"
          >
            <Toggle
              checked={settings.tts_enabled}
              onChange={(v) => setSettings({ tts_enabled: v })}
            />
          </Row>
          <div className={settings.tts_enabled ? "" : "opacity-40 pointer-events-none"}>
            <div className="border-b border-sv-border/60 py-3.5">
              <div className="text-sm">Read-aloud hotkey</div>
              <div className="mt-3">
                <HotkeyRecorder
                  value={settings.tts_hotkey}
                  onChange={(accelerator) =>
                    setSettings({ tts_hotkey: accelerator })
                  }
                />
              </div>
            </div>
            <Row
              label="Voice"
              hint={
                downloadedTts.size === 0
                  ? "No voices downloaded yet — get one in Model Store → Text-to-Speech"
                  : undefined
              }
            >
              <select
                value={settings.active_tts_voice ?? ""}
                onChange={(e) =>
                  setSettings({ active_tts_voice: e.target.value || null })
                }
                className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
              >
                <option value="">None selected</option>
                {TTS_MODELS.filter((v) => downloadedTts.has(v.id)).map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.label}
                  </option>
                ))}
              </select>
            </Row>
            <Row label="Test voice" hint="Speaks a short sample sentence">
              <button
                onClick={() => {
                  // A voice can only pronounce its own language — use a sample
                  // sentence in the voice's language (English text through e.g.
                  // a Bangla model comes out as gibberish).
                  const voice = TTS_MODELS.find(
                    (v) => v.id === settings.active_tts_voice
                  );
                  ttsSpeakText(
                    TTS_SAMPLE_TEXT[voice?.language ?? ""] ??
                      TTS_SAMPLE_TEXT.default
                  );
                }}
                disabled={!settings.active_tts_voice}
                className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:border-sv-accent hover:text-sv-accent disabled:opacity-40"
              >
                ▶ Play sample
              </button>
            </Row>
          </div>
        </Section>

        <Section
          title="Custom vocabulary"
          desc="Names or jargon Whisper mishears — fed to the model as a hint so it spells them right. Not AI; it does not rewrite your text. Comma-separated, most important first."
          accent="var(--color-sv-sec-vocab)"
          icon={<BookIcon />}
        >
          <div className="py-4">
            <textarea
              value={settings.custom_vocabulary}
              onChange={(e) => setSettings({ custom_vocabulary: e.target.value })}
              placeholder="e.g. Tauri, whisper.cpp, Kubernetes"
              rows={3}
              className="w-full max-w-[520px] resize-y rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
            />
          </div>
        </Section>

        <Section
          title="Text replacements"
          desc="Say a short trigger and have it typed out in full — e.g. “my email” → your address. Applied to the final text just before pasting. Case-insensitive."
          accent="var(--color-sv-sec-replace)"
          icon={<ArrowsSwapIcon />}
        >
          <div className="py-4">
            {snippets.length > 0 && (
              <div className="mb-3 space-y-2">
                {snippets.map((sn) => (
                  <div key={sn.id} className="flex items-center gap-2">
                    <input
                      value={sn.trigger}
                      onChange={(e) =>
                        updateSnippet(sn.id, { trigger: e.target.value })
                      }
                      placeholder="When I say…"
                      className="w-40 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                    />
                    <span className="text-sv-muted">→</span>
                    <input
                      value={sn.replacement}
                      onChange={(e) =>
                        updateSnippet(sn.id, { replacement: e.target.value })
                      }
                      placeholder="type this instead"
                      className="flex-1 max-w-[320px] rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                    />
                    <button
                      onClick={() => deleteSnippet(sn.id)}
                      className="rounded-lg px-2 py-2 text-sv-muted hover:text-sv-bad"
                      title="Delete"
                    >
                      ✕
                    </button>
                  </div>
                ))}
              </div>
            )}
            <button
              onClick={addSnippet}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:bg-sv-surface-2"
            >
              + Add replacement
            </button>
          </div>
        </Section>

        <Section
          title="Per-app profiles"
          desc="Automatically switch AI mode based on the app you're dictating into. Match is on the program's file name — e.g. “code” for VS Code, “chrome” for Chrome, “outlook” for Outlook."
          accent="var(--color-sv-sec-profiles)"
          icon={<LayersIcon />}
        >
          <div className="py-4">
            {appProfiles.length > 0 && (
              <div className="mb-3 space-y-2">
                {appProfiles.map((p) => (
                  <div key={p.id} className="flex items-center gap-2">
                    <input
                      value={p.app_match}
                      onChange={(e) =>
                        updateAppProfile(p.id, { app_match: e.target.value })
                      }
                      placeholder="app name contains…"
                      className="w-44 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                    />
                    <span className="text-sv-muted">→</span>
                    <select
                      value={p.mode_id}
                      onChange={(e) =>
                        updateAppProfile(p.id, { mode_id: e.target.value })
                      }
                      className="flex-1 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
                    >
                      {modes.map((m) => (
                        <option key={m.id} value={m.id}>
                          {m.name}
                        </option>
                      ))}
                    </select>
                    <button
                      onClick={() => deleteAppProfile(p.id)}
                      className="rounded-lg px-2 py-2 text-sv-muted hover:text-sv-bad"
                      title="Delete"
                    >
                      ✕
                    </button>
                  </div>
                ))}
              </div>
            )}
            <button
              onClick={addAppProfile}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:bg-sv-surface-2"
            >
              + Add profile
            </button>
          </div>
        </Section>

        <Section title="Performance" accent="var(--color-sv-sec-perf)" icon={<GaugeIcon />}>
          {reco && (
            <Row
              label={`Recommended for this device · ${reco.tier}`}
              hint={`${reco.reason}. Suggested speech model size: ${reco.stt_size}. Applies GPU, thread, and grammar settings — not the model download.`}
            >
              <button
                onClick={() =>
                  setSettings({
                    use_gpu: reco.use_gpu,
                    high_performance: reco.high_performance,
                    performance_threads: reco.performance_threads,
                    coedit_enabled: reco.coedit_enabled,
                  })
                }
                className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:border-sv-accent hover:text-sv-accent"
              >
                Apply
              </button>
            </Row>
          )}
          <Row
            label="Use GPU acceleration"
            hint={
              hasGpu
                ? `Detected ${hardware?.gpu_name} — takes effect on the next dictation`
                : "No compatible GPU detected — leave off; CPU will be used"
            }
          >
            <Toggle
              checked={settings.use_gpu}
              onChange={(v) => setSettings({ use_gpu: v })}
            />
          </Row>
          <Row
            label="High performance mode"
            hint="Uses more CPU threads for faster transcription (may slow other apps)"
          >
            <Toggle
              checked={settings.high_performance}
              onChange={(v) => setSettings({ high_performance: v })}
            />
          </Row>
          {settings.high_performance &&
            (() => {
              const cores = hardware?.logical_cores ?? 4;
              const def = Math.max(2, Math.floor(cores / 2));
              // 0 = auto (all cores). Show the effective value on the slider.
              const value = Math.min(
                cores,
                Math.max(def, settings.performance_threads || cores)
              );
              const fill =
                cores > def ? ((value - def) / (cores - def)) * 100 : 100;
              return (
                <div className="py-3.5">
                  <div className="flex items-center justify-between">
                    <div className="text-sm">CPU threads</div>
                    <span className="text-xs tabular-nums text-sv-muted">
                      {value} / {cores}
                    </span>
                  </div>
                  <div className="mt-0.5 text-xs text-sv-muted">
                    How many CPU threads transcription may use. Default (balanced)
                    is {def} — you can't go below that. Higher = faster, but
                    leaves less for other apps. Your CPU has {cores} threads.
                  </div>
                  <input
                    type="range"
                    min={def}
                    max={cores}
                    step={1}
                    value={value}
                    onChange={(e) =>
                      setSettings({
                        performance_threads: Number(e.target.value),
                      })
                    }
                    className="sv-slider mt-3 w-full"
                    style={
                      {
                        "--sv-slider-fill": `${fill}%`,
                      } as React.CSSProperties
                    }
                  />
                </div>
              );
            })()}
        </Section>

        <Section title="System" accent="var(--color-sv-sec-system)" icon={<CogIcon />}>
          <Row label="History limit" hint="Keep at most this many transcriptions">
            <div className="flex items-center gap-2.5">
              <ScrollNumberPicker
                value={settings.history_limit}
                onChange={(v) => setSettings({ history_limit: v })}
                min={1}
                max={10000}
              />
              <span className="text-sm text-sv-muted">entries</span>
            </div>
          </Row>
          <Row label="Auto-delete" hint="Remove entries older than">
            <div className="flex items-center gap-2.5">
              <select
                value={settings.history_retention}
                onChange={(e) =>
                  setSettings({
                    history_retention: e.target.value as Settings["history_retention"],
                  })
                }
                className={SELECT_CLS}
              >
                {RETENTION_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
              {settings.history_retention === "custom" && (
                <>
                  <ScrollNumberPicker
                    value={settings.history_retention_custom_value}
                    onChange={(v) => setSettings({ history_retention_custom_value: v })}
                    min={1}
                    max={999}
                    width={60}
                  />
                  <select
                    value={settings.history_retention_custom_unit}
                    onChange={(e) =>
                      setSettings({
                        history_retention_custom_unit: e.target
                          .value as Settings["history_retention_custom_unit"],
                      })
                    }
                    className={SELECT_CLS}
                  >
                    {RETENTION_UNITS.map((u) => (
                      <option key={u} value={u}>
                        {u}
                      </option>
                    ))}
                  </select>
                </>
              )}
            </div>
          </Row>
          <Row label="Blur transcripts until hover" hint="Keeps what you dictated hidden when your screen is visible to others">
            <Toggle
              checked={settings.blur_history}
              onChange={(v) => setSettings({ blur_history: v })}
            />
          </Row>
          <Row label="Launch at startup">
            <Toggle
              checked={settings.auto_start}
              onChange={(v) => setSettings({ auto_start: v })}
            />
          </Row>
          <Row label="Theme">
            <ThemeToggle
              value={settings.theme}
              onChange={(t) => setSettings({ theme: t })}
            />
          </Row>
          <Row
            label="App updates"
            hint={updateMsg || "Checks automatically on launch"}
          >
            <button
              onClick={async () => {
                setUpdateMsg("Checking…");
                const r = await checkForUpdatesManual();
                setUpdateMsg(
                  r.status === "none"
                    ? "You're on the latest version"
                    : r.status === "error"
                    ? "Update check failed"
                    : r.status === "unsupported"
                    ? "Updates require the desktop app"
                    : "Update found — installing…"
                );
              }}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs text-sv-text hover:bg-sv-surface-2"
            >
              Check for updates
            </button>
          </Row>
        </Section>
      </div>
    </Page>
  );
}

function SubGroup({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="my-3 rounded-lg border border-sv-border bg-sv-bg/40 px-4 py-1">
      <div className="pb-1 pt-2 text-[10px] font-medium uppercase tracking-wider text-sv-muted">{label}</div>
      {children}
    </div>
  );
}

function Section({
  title,
  desc,
  accent,
  icon,
  children,
}: {
  title: string;
  desc?: string;
  accent?: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-5 break-inside-avoid overflow-hidden rounded-xl border border-sv-border bg-sv-surface">
      {/* items-start, not center: where the description wraps to two or three
          lines, centring floats the icon halfway down the block instead of
          letting it sit beside the title. */}
      <div
        className="flex items-start gap-2.5 border-b border-sv-border px-5 py-3"
        style={accent ? {
          // The radial glow centres on the icon and bleeds outward, the linear layer carries that colour to the right and dissolves.
          backgroundImage: [
            `radial-gradient(140px 70px at 34px 50%, color-mix(in srgb, ${accent} 22%, transparent), transparent 72%)`,
            `linear-gradient(to right, color-mix(in srgb, ${accent} 9%, transparent) 0%, color-mix(in srgb, ${accent} 4%, transparent) 38%, transparent 68%)`,
          ].join(", "),
          boxShadow: "inset 0 1px 0 0 color-mix(in srgb, var(--color-sv-text) 6%, transparent)"
        } : undefined}
      >
        {icon && (
          <span
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg"
            style={accent ? { backgroundColor: `color-mix(in srgb, ${accent} 20%, transparent)`, color: accent } : undefined}
          >
            {icon}
          </span>
        )}
        <div className="min-w-0">
          <h2 className="text-[13px] font-semibold">{title}</h2>
          {desc && <p className="mt-0.5 text-[11px] leading-relaxed text-sv-muted">{desc}</p>}
        </div>
      </div>
      <div className="px-5">{children}</div>
    </div>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-sv-border/60 py-3 last:border-b-0">
      <div className="min-w-0">
        <div className="text-[13px]">{label}</div>
        {hint && <div className="mt-0.5 text-[11px] leading-relaxed text-sv-muted max-w-[46ch]">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function ThemeToggle({
  value,
  onChange,
}: {
  value: "dark" | "light";
  onChange: (t: "dark" | "light") => void;
}) {
  const opts: { id: "dark" | "light"; label: string; icon: React.ReactNode }[] = [
    { id: "light", label: "Light", icon: <SunIcon /> },
    { id: "dark", label: "Dark", icon: <MoonIcon /> },
  ];
  return (
    <div className="inline-flex rounded-lg border border-sv-border bg-sv-bg p-1">
      {opts.map((o) => (
        <button
          key={o.id}
          onClick={() => onChange(o.id)}
          className={`flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition ${
            value === o.id
              ? "bg-sv-accent text-white"
              : "text-sv-muted hover:text-sv-text"
          }`}
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}

function SunIcon() {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </svg>
  );
}

function MoonIcon() {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
    </svg>
  );
}

function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 rounded-full transition-colors duration-75 ${
        checked ? "bg-sv-accent" : "bg-sv-surface-2"
      }`}
    >
      <span
        className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-all ${
          checked ? "left-[22px]" : "left-0.5"
        }`}
      />
    </button>
  );
}

function MicrophoneIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
      <path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v4M8 23h8" />
    </svg>
  );
}

function SpeakerIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
      <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
    </svg>
  );
}

function BookIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
    </svg>
  );
}

function ArrowsSwapIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <polyline points="7 10 3 6 7 2" />
      <path d="M21 6H3" />
      <polyline points="17 14 21 18 17 22" />
      <path d="M3 18h18" />
    </svg>
  );
}

function LayersIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <polygon points="12 2 2 7 12 12 22 7 12 2" />
      <polyline points="2 12 12 17 22 12" />
      <polyline points="2 17 12 22 22 17" />
    </svg>
  );
}

function GaugeIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
    </svg>
  );
}

function CogIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}
