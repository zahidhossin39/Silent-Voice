import { useEffect, useState } from "react";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { STT_MODELS, LANGUAGES, TTS_MODELS, TTS_SAMPLE_TEXT } from "../../services/catalog";
import {
  listInputDevices,
  setHotkey,
  getAutostart,
  ttsSpeakText,
} from "../../services/tauriBridge";
import HotkeyRecorder from "../shared/HotkeyRecorder";
import { checkForUpdatesManual } from "../../services/updater";
import type { SttPreset } from "../../types";

// Preset → recommended model (build plan §4 mode presets).
const PRESET_MODEL: Record<SttPreset, string> = {
  speed: "tiny.en",
  balanced: "base.en",
  accuracy: "small.en",
  multilingual: "small",
};

export default function Settings() {
  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);
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
        <Section title="Dictation">
          <Row
            label="Speed / accuracy preset"
            hint="Picks a Whisper model that matches the tradeoff"
          >
            <select
              value={settings.stt_preset}
              onChange={(e) => {
                const preset = e.target.value as SttPreset;
                setSettings({
                  stt_preset: preset,
                  active_stt_model: PRESET_MODEL[preset],
                });
              }}
              className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
            >
              <option value="speed">Speed — Tiny (English)</option>
              <option value="balanced">Balanced — Base (English)</option>
              <option value="accuracy">Accuracy — Small (English)</option>
              <option value="multilingual">Multilingual — Small</option>
            </select>
          </Row>
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
            label="Double-tap to lock recording"
            hint="Tap the hotkey twice quickly to keep recording hands-free; press once to stop & paste"
          >
            <Toggle
              checked={settings.toggle_mode}
              onChange={(v) => setSettings({ toggle_mode: v })}
            />
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
                hint="AI pass that catches correctly-spelled wrong words (needs the GECToR model installed)"
              >
                <Toggle
                  checked={!settings.proofread_disabled_rules.includes("Gector")}
                  onChange={(v) => toggleProofreadRule("Gector", v)}
                />
              </Row>
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
        </Section>

        <Section
          title="Read aloud (text-to-speech)"
          desc="Select text in any app, press the hotkey, and hear it spoken. Press again to stop. Voices are downloaded in Model Store → Text-to-Speech."
        >
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
        </Section>

        <Section
          title="Custom vocabulary"
          desc="Names or jargon Whisper mishears — fed to the model as a hint so it spells them right. Not AI; it does not rewrite your text. Comma-separated, most important first."
        >
          <div className="py-4">
            <textarea
              value={settings.custom_vocabulary}
              onChange={(e) => setSettings({ custom_vocabulary: e.target.value })}
              placeholder="e.g. Tauri, whisper.cpp, Kubernetes"
              rows={3}
              className="w-full resize-y rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
            />
          </div>
        </Section>

        <Section
          title="Text replacements"
          desc="Say a short trigger and have it typed out in full — e.g. “my email” → your address. Applied to the final text just before pasting. Case-insensitive."
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
                      className="flex-1 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
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

        <Section title="Performance">
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

        <Section title="System">
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

function Section({
  title,
  desc,
  children,
}: {
  title: string;
  desc?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-5 break-inside-avoid overflow-hidden rounded-xl border border-sv-border bg-sv-surface">
      <div className="border-b border-sv-border px-5 py-3.5">
        <h2 className="text-sm font-semibold">{title}</h2>
        {desc && <p className="mt-0.5 text-xs text-sv-muted">{desc}</p>}
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
    <div className="flex items-center justify-between gap-4 border-b border-sv-border/60 py-3.5 last:border-b-0">
      <div className="min-w-0">
        <div className="text-sm">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-sv-muted">{hint}</div>}
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
      className={`relative h-6 w-11 rounded-full transition ${
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
