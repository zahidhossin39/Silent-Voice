import { useEffect, useState } from "react";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { STT_MODELS, LANGUAGES } from "../../services/catalog";
import {
  listInputDevices,
  setHotkey,
  getDataLocation,
  setDataLocation,
  pickFolder,
} from "../../services/tauriBridge";
import type { DataLocation } from "../../services/tauriBridge";
import HotkeyRecorder from "../shared/HotkeyRecorder";
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
  const { hardware } = useHardwareInfo();
  const [devices, setDevices] = useState<string[]>([]);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [dataLoc, setDataLoc] = useState<DataLocation>({
    models_root: null,
    history_root: null,
  });
  const [storageMsg, setStorageMsg] = useState<string | null>(null);

  useEffect(() => {
    listInputDevices().then(setDevices);
    getDataLocation().then(setDataLoc);
  }, []);

  async function handlePickModelsFolder() {
    const folder = await pickFolder();
    if (!folder) return;
    const updated = { ...dataLoc, models_root: folder };
    setDataLoc(updated);
    await setDataLocation(updated);
    setStorageMsg("Models folder saved. New downloads will go here.");
  }

  async function handlePickHistoryFolder() {
    const folder = await pickFolder();
    if (!folder) return;
    const updated = { ...dataLoc, history_root: folder };
    setDataLoc(updated);
    await setDataLocation(updated);
    setStorageMsg("History folder saved.");
  }

  async function handleResetStorage() {
    const reset: DataLocation = { models_root: null, history_root: null };
    setDataLoc(reset);
    await setDataLocation(reset);
    setStorageMsg("Reset to default (C: drive AppData).");
  }

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
    <Page title="Settings" subtitle="Dictation, audio, storage, and appearance">
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
        </Section>

        <Section
          title="Custom vocabulary"
          desc="Names or jargon Whisper mishears — fed to the model as a hint so it spells them right. Not AI; it does not rewrite your text. Comma-separated, most important first."
        >
          <div className="py-4">
            <textarea
              value={settings.custom_vocabulary}
              onChange={(e) => setSettings({ custom_vocabulary: e.target.value })}
              placeholder="e.g. Zaid, Tauri, whisper.cpp, Kubernetes, Nirjhor"
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
        </Section>

        <Section
          title="Storage locations"
          desc="Move models and history off the C drive to any folder. The app creates subfolders inside the root you choose. Existing downloads are NOT moved automatically — copy them yourself, then change the path."
        >
          {storageMsg && (
            <div className="mt-4 rounded-lg border border-sv-good/30 bg-sv-good/10 px-3 py-2 text-xs text-sv-good">
              {storageMsg}
            </div>
          )}
          <Row
            label="AI models folder"
            hint={
              dataLoc.models_root
                ? dataLoc.models_root
                : "%APPDATA%\\SilentVoice\\models (default C drive)"
            }
          >
            <div className="flex gap-2">
              <button
                onClick={handlePickModelsFolder}
                className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:bg-sv-surface-2"
              >
                Browse…
              </button>
            </div>
          </Row>
          <Row
            label="History folder"
            hint={
              dataLoc.history_root
                ? dataLoc.history_root
                : "%APPDATA%\\SilentVoice (default C drive)"
            }
          >
            <div className="flex gap-2">
              <button
                onClick={handlePickHistoryFolder}
                className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:bg-sv-surface-2"
              >
                Browse…
              </button>
            </div>
          </Row>
          {(dataLoc.models_root || dataLoc.history_root) && (
            <Row label="Reset to defaults" hint="Go back to C drive AppData">
              <button
                onClick={handleResetStorage}
                className="rounded-lg border border-sv-border px-3 py-1.5 text-xs text-sv-muted hover:text-sv-bad"
              >
                Reset
              </button>
            </Row>
          )}
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
