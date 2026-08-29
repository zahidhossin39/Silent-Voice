import { createContext, useContext, useEffect, useState } from "react";
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
  downloadCoeditModel,
  coeditInstalled,
  deleteCoeditModel,
  pauseDownload,
  cancelDownload,
  downloadGectorModel,
  gectorInstalled,
  deleteGectorModel,
  downloadVadModel,
  vadInstalled,
  deleteVadModel,
  recommendDeviceDefaults,
  copyDiagnostics,
  copyToClipboard,
} from "../../services/tauriBridge";
import type { DeviceRecommendation } from "../../types";
import HotkeyRecorder from "../shared/HotkeyRecorder";
import { checkForUpdatesManual } from "../../services/updater";
import type { Settings } from "../../types";
import Select from "../shared/Select";

export default function Settings() {
  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const PREDEFINED_UNLOADS = [0, 5, 15, 30, 60, 120];
  const [isCustomUnload, setIsCustomUnload] = useState(!PREDEFINED_UNLOADS.includes(settings.model_unload_minutes));
  const [customUnloadVal, setCustomUnloadVal] = useState(
    PREDEFINED_UNLOADS.includes(settings.model_unload_minutes) ? "" : settings.model_unload_minutes.toString()
  );
  const [customUnloadUnit, setCustomUnloadUnit] = useState("Minutes");

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
  const [diagMsg, setDiagMsg] = useState("");
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
    } catch {
      setHotkeyError(
        "Couldn't set that shortcut — another app or the system may already use it. Try a different combination."
      );
    }
  }

  const hasGpu = !!hardware?.gpu_vram_gb && hardware.gpu_vram_gb >= 1;
  const [cat, setCat] = useState("recording");

  return (
    <Page title="Settings" subtitle="Fine-tune dictation, grammar, voice, and more">
      <div className="flex gap-6">
        {/* Category rail */}
        <nav className="sticky top-0 w-48 shrink-0 self-start">
          {CATS.map((c) => {
            const on = c.id === cat;
            return (
              <button
                key={c.id}
                onClick={() => setCat(c.id)}
                aria-current={on}
                className={`mb-0.5 flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-[13px] transition ${
                  on
                    ? "bg-sv-surface-2 font-medium text-sv-text"
                    : "text-sv-muted hover:bg-sv-surface-2/60 hover:text-sv-text"
                }`}
              >
                <span style={{ color: on ? c.accent : undefined }} className="shrink-0">
                  {c.icon}
                </span>
                {c.label}
              </button>
            );
          })}
        </nav>

        {/* Active category pane */}
        <div className="min-w-0 flex-1">
        <Cat id="recording" active={cat}>
        <Section title="Recording" desc="How the push-to-talk hotkey behaves and how your voice is captured." accent="var(--color-sv-sec-dictation)" icon={<MicrophoneIcon />}>

          <div className="border-b border-sv-border/60 py-3.5">
            <div className="flex items-center gap-1.5 text-sm">
              Global hotkey (push-to-talk)
              <InfoTip text="The key you hold to dictate anywhere in Windows. Hold it, speak, release, and your words are typed at the cursor. Pick a combo no other app uses — if the system already claims it, you'll see an error and can try another." />
            </div>
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
              label="Transcribe while you speak"
              hint="Transcribes finished sentences mid-recording. Rarely helps and can slow things down on 4-core CPUs — for faster dictation, pick a lighter speech model instead (Model Store)."
            >
              <Toggle
                checked={settings.chunk_on_silence}
                onChange={(v) => setSettings({ chunk_on_silence: v })}
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
        </Section>
        </Cat>

        <Cat id="stt" active={cat}>
        <Section title="Speech-to-text" desc="The model that turns your speech into text, plus the microphone it listens to." accent="var(--color-sv-sec-perf)" icon={<WaveIcon />}>
          <Row
            label="Speech-to-text source"
            info="Choose whether transcription runs on your own machine (private, offline, and free) or through a cloud provider you've added (often faster and more accurate, but it sends your audio to their servers and may cost money). Local is the default and needs a downloaded speech model."
            hint={
              sttProviders.length === 0
                ? "Add a provider in Cloud providers with \"STT\" checked to unlock cloud options"
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
            <Row
              label="Speech model"
              info="The Whisper model that converts speech to text on your device. Larger models are more accurate but slower — on a CPU every clip pays a big fixed cost, so even short phrases can feel slow. For speed, pick a smaller model, or try Moonshine in the Model Store, which is built for short dictation."
            >
              <select
                value={settings.active_stt_model}
                onChange={(e) =>
                  setSettings({ active_stt_model: e.target.value })
                }
                className="w-56 rounded-lg border border-sv-border bg-sv-bg px-3 py-2 text-sm"
              >
                <option value="">None selected</option>
                {!STT_MODELS.some((m) => m.id === settings.active_stt_model) && settings.active_stt_model !== "" && (
                  <option value={settings.active_stt_model}>
                    {settings.active_stt_model}
                  </option>
                )}
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
            info="Tell Whisper which language you're speaking. Auto-detect works, but it guesses from the first few seconds and can slip on short clips or strong accents — choosing your language explicitly is faster and noticeably more accurate."
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
        </Section>
        </Cat>

        <Cat id="grammar" active={cat}>
        <Section title="Grammar" desc="Spelling and grammar help that appears as you type in any app (English only)." accent="var(--color-sv-sec-replace)" icon={<SpellIcon />}>
          <Row
            label="Grammar model"
            info="An optional on-device AI model that corrects grammar in your dictated text before it is pasted (used when no AI writing style is active). Runs fully offline. Safe to remove if you're short on space."
            hint="Corrects grammar in dictated text before pasting, when no writing style is active."
          >
            {(() => {
              if (coeditReady) {
                return (
                  <button
                    onClick={async () => {
                      await deleteCoeditModel();
                      setCoeditReady(false);
                    }}
                    className="rounded-lg border border-sv-border px-2.5 py-1 text-xs text-sv-text hover:border-sv-bad hover:text-sv-bad"
                  >
                    Remove
                  </button>
                );
              }
              const downloading = coeditFetching || coeditProgress?.status === "downloading";
              const pct = coeditProgress && coeditProgress.total_bytes > 0
                ? Math.round((coeditProgress.downloaded_bytes / coeditProgress.total_bytes) * 100)
                : 0;
              const isPaused = coeditProgress?.status === "paused";
              if (downloading || isPaused) {
                return (
                  <div className="flex items-center gap-1.5">
                    <div className="flex w-28 items-center gap-2">
                      <div className="h-1.5 flex-1 overflow-hidden rounded-full border border-sv-border bg-sv-surface-2">
                        {!coeditProgress || coeditProgress.total_bytes === 0 ? (
                          <div className="h-full w-1/3 rounded-full bg-sv-accent animate-[sv-indeterminate_1.1s_ease-in-out_infinite]" />
                        ) : (
                          <div className={`h-full rounded-full transition-all duration-75 ${isPaused ? "bg-sv-muted" : "bg-sv-accent"}`} style={{ width: `${pct}%` }} />
                        )}
                      </div>
                      <span className="w-9 shrink-0 text-right tabular-nums text-[11px] text-sv-muted">
                        {isPaused ? "Paused" : !coeditProgress || coeditProgress.total_bytes === 0 ? "…" : `${pct}%`}
                      </span>
                    </div>
                    {isPaused ? (
                      <button
                        onClick={async () => {
                          setCoeditFetching(true);
                          try {
                            const ok = await downloadCoeditModel();
                            if (ok) setCoeditReady(await coeditInstalled());
                          } finally {
                            setCoeditFetching(false);
                          }
                        }}
                        title="Resume download" aria-label="Resume download"
                        className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                      >
                        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
                      </button>
                    ) : (
                      <button
                        onClick={() => pauseDownload("coedit")}
                        title="Pause download" aria-label="Pause download"
                        className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                      >
                        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                      </button>
                    )}
                    <button
                      onClick={() => cancelDownload("coedit")}
                      title="Cancel download" aria-label="Cancel download"
                      className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-bad"
                    >
                      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
                    </button>
                  </div>
                );
              }
              return (
                <button
                  onClick={async () => {
                    setCoeditFetching(true);
                    try {
                      const ok = await downloadCoeditModel();
                      if (ok) setCoeditReady(await coeditInstalled());
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
          {coeditReady && (
            <Row
              label="Fix dictated grammar"
              info="When on, the downloaded grammar model corrects your dictated text before it's pasted — skipped automatically when an AI writing style is already active. Runs fully offline."
              hint="Corrects grammar in dictated text before pasting (when no writing style is active)."
            >
              <Toggle
                checked={settings.coedit_enabled}
                onChange={(v) => setSettings({ coedit_enabled: v })}
              />
            </Row>
          )}
          <Row
            label="Proofread as you type"
            info="Underlines spelling and grammar mistakes right where you type — in almost any Windows app, not just this one — the way a word processor does. Click an underline to see fixes. Runs entirely on your device and is English-only for now."
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
                label="Context grammar"
                info="A small AI model that catches mistakes a spell-checker can't — correctly-spelled but wrong words like their/there or affect/effect — by reading the whole sentence for context. Optional download; the basic spelling and grammar underlines work without it."
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
                          className="rounded-lg border border-sv-border px-2.5 py-1 text-xs text-sv-text hover:border-sv-bad hover:text-sv-bad"
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
                  const gectorPaused = gectorProgress?.status === "paused";
                  if (downloading || gectorPaused) {
                    return (
                      <div className="flex items-center gap-1.5">
                        <div className="flex w-28 items-center gap-2">
                          <div className="h-1.5 flex-1 overflow-hidden rounded-full border border-sv-border bg-sv-surface-2">
                            {!gectorProgress || gectorProgress.total_bytes === 0 ? (
                              <div className="h-full w-1/3 rounded-full bg-sv-accent animate-[sv-indeterminate_1.1s_ease-in-out_infinite]" />
                            ) : (
                              <div className={`h-full rounded-full transition-all duration-75 ${gectorPaused ? "bg-sv-muted" : "bg-sv-accent"}`} style={{ width: `${pct}%` }} />
                            )}
                          </div>
                          <span className="w-9 shrink-0 text-right tabular-nums text-[11px] text-sv-muted">
                            {gectorPaused ? "Paused" : !gectorProgress || gectorProgress.total_bytes === 0 ? "…" : `${pct}%`}
                          </span>
                        </div>
                        {gectorPaused ? (
                          <button
                            onClick={async () => {
                              setGectorFetching(true);
                              try {
                                const ok = await downloadGectorModel(gectorVariant);
                                if (ok) setGectorReady(await gectorInstalled());
                              } finally {
                                setGectorFetching(false);
                              }
                            }}
                            title="Resume download" aria-label="Resume download"
                            className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                          >
                            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
                          </button>
                        ) : (
                          <button
                            onClick={() => pauseDownload("gector")}
                            title="Pause download" aria-label="Pause download"
                            className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-text"
                          >
                            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                          </button>
                        )}
                        <button
                          onClick={() => cancelDownload("gector")}
                          title="Cancel download" aria-label="Cancel download"
                          className="p-1 text-sv-muted transition-colors duration-75 hover:text-sv-bad"
                        >
                          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
                        </button>
                      </div>
                    );
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
                            const ok = await downloadGectorModel(gectorVariant);
                            if (ok) setGectorReady(await gectorInstalled());
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
        </Section>
        </Cat>

        <Cat id="recording" active={cat}>
        <Section title="Audio input" desc="How your microphone signal is filtered before transcription." accent="var(--color-sv-sec-dictation)" icon={<MicrophoneIcon />}>
          <div className="py-3.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5 text-sm">
                Input sensitivity
                <InfoTip text="This is the loudness gate. Anything quieter than the line is treated as silence and trimmed before transcription, which removes wind, hum, and room noise. Lower = stricter, so only clear speech gets through; higher picks up quieter speech but also more background noise." />
              </div>
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
              step={1}
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
            info="Normally the app decides 'is this speech?' purely by loudness, so fans, keyboards, or music can be mistaken for talking. This tiny neural model recognizes the actual shape of a human voice, so it trims non-speech far more reliably. Applies automatically once downloaded."
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
                    className="rounded-lg border border-sv-border px-2.5 py-1 text-xs text-sv-text hover:border-sv-bad hover:text-sv-bad"
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
                    const ok = await downloadVadModel();
                    if (ok) setVadReady(await vadInstalled());
                  }}
                  className="rounded-lg border border-sv-border px-3 py-1.5 text-xs hover:border-sv-accent hover:text-sv-accent"
                >
                  Download · 2 MB
                </button>
              );
            })()}
          </Row>
        </Section>
        </Cat>

        <Cat id="tts" active={cat}>
        <Section
          title="Read aloud (text-to-speech)"
          desc="Select text in any app, press the hotkey, and hear it spoken. Press again to stop. Voices are downloaded in Model Store → Text-to-Speech."
          accent="var(--color-sv-sec-tts)"
          icon={<SpeakerIcon />}
        >
          <Row
            label="Enable read-aloud"
            info="Turns on the text-to-speech hotkey: select text in any app, press the key, and hear it read aloud in a downloaded voice. Press again to stop. This is separate from dictation — it speaks text, it doesn't transcribe your voice."
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
                {TTS_MODELS.filter((v) => downloadedTts.has(v.id)).flatMap((v) =>
                  v.voices
                    ? v.voices.map((vo) => (
                        <option key={`${v.id}#${vo.sid}`} value={`${v.id}#${vo.sid}`}>
                          {v.label} · {vo.label}
                        </option>
                      ))
                    : [
                        <option key={v.id} value={v.id}>
                          {v.label}
                        </option>,
                      ]
                )}
              </select>
            </Row>
            <Row label="Test voice" hint="Speaks a short sample sentence">
              <button
                onClick={() => {
                  // A voice can only pronounce its own language — use a sample
                  // sentence in the voice's language (English text through e.g.
                  // a Bangla model comes out as gibberish).
                  const baseId = (settings.active_tts_voice ?? '').split('#')[0];
                  const voice = TTS_MODELS.find((v) => v.id === baseId);
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
        </Cat>

        <Cat id="words" active={cat}>
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
                      title="Delete" aria-label="Delete"
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
        </Cat>

        <Cat id="profiles" active={cat}>
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
                      title="Delete" aria-label="Delete"
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
        </Cat>

        <Cat id="performance" active={cat}>
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
            info="Runs transcription on your graphics card instead of the CPU, which is much faster — but only if you have a compatible GPU. If none is detected, leave this off; turning it on without one does nothing or can error."
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
            label="Speed boost (uses more CPU)"
            info="Unlocks a slider that lets transcription use more CPU cores at once. On most laptops the app already uses every core by default, so this changes nothing — real speed comes from a lighter speech model, not more threads."
            hint="Lets you tune how many CPU cores transcription uses. On CPUs that already use all their cores by default, there's nothing extra to unlock."
          >
            <Toggle
              checked={settings.high_performance}
              onChange={(v) => setSettings({ high_performance: v })}
            />
          </Row>
          {settings.high_performance &&
            (() => {
              const physical = hardware?.physical_cores ?? 4;
              const logical = Math.max(hardware?.logical_cores ?? physical, physical);
              // Balanced default = one thread per physical core. The slider can
              // go up to the logical (hyper-thread) count for anyone who wants
              // to try, but on-device tests show it barely moves the needle.
              const def = Math.min(8, Math.max(2, physical));
              if (logical <= def) {
                return (
                  <div className="mt-0.5 py-3.5 text-xs text-sv-muted">
                    Transcription already uses all {physical} of your CPU's
                    cores. Speed comes from your speech model, not more threads —
                    pick a lighter model in the Model Store for faster dictation.
                  </div>
                );
              }
              const value = Math.min(
                logical,
                Math.max(def, settings.performance_threads || def)
              );
              const fill = ((value - def) / (logical - def)) * 100;
              return (
                <div className="py-3.5">
                  <div className="flex items-center justify-between">
                    <div className="text-sm">CPU threads</div>
                    <span className="text-xs tabular-nums text-sv-muted">
                      {value} / {logical}
                    </span>
                  </div>
                  <div className="mt-0.5 text-xs text-sv-muted">
                    Threads transcription may use — default {def} (one per core).
                    You can push it to {logical}, but on this CPU it makes almost
                    no difference: your speech-model choice matters far more.
                    Fewer leaves more for other apps.
                  </div>
                  <input
                    type="range"
                    min={def}
                    max={logical}
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
          <Row
            label="Unload model when idle"
            info="Keeping the model loaded makes dictation instant, but uses system memory. Unloading it frees memory, but your next dictation will take a few seconds to start while the model reloads."
          >
            <div className="flex flex-col items-end gap-2">
              <Select
                value={isCustomUnload ? "custom" : String(settings.model_unload_minutes)}
                onChange={(v) => {
                  if (v === "custom") {
                    setIsCustomUnload(true);
                  } else {
                    setIsCustomUnload(false);
                    setSettings({ model_unload_minutes: Number(v) });
                  }
                }}
                className="w-40"
              >
                <option value="0">Never</option>
                <option value="5">5 minutes</option>
                <option value="15">15 minutes</option>
                <option value="30">30 minutes</option>
                <option value="60">1 hour</option>
                <option value="120">2 hours</option>
                <option value="custom">Custom</option>
              </Select>
              {isCustomUnload && (
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min="1"
                    value={customUnloadVal}
                    onChange={(e) => {
                      setCustomUnloadVal(e.target.value);
                      const n = parseInt(e.target.value, 10);
                      if (!isNaN(n)) {
                        setSettings({
                          model_unload_minutes: customUnloadUnit === "Hours" ? n * 60 : n,
                        });
                      }
                    }}
                    className="w-20 rounded-lg border border-sv-border bg-sv-bg px-3 py-1.5 text-sm transition-colors focus:border-sv-accent focus:outline-none"
                  />
                  <Select
                    value={customUnloadUnit}
                    onChange={(v) => {
                      setCustomUnloadUnit(v);
                      const n = parseInt(customUnloadVal, 10);
                      if (!isNaN(n)) {
                        setSettings({
                          model_unload_minutes: v === "Hours" ? n * 60 : n,
                        });
                      }
                    }}
                    className="w-[104px]"
                  >
                    <option value="Minutes">Minutes</option>
                    <option value="Hours">Hours</option>
                  </Select>
                </div>
              )}
            </div>
          </Row>
        </Section>
        </Cat>

        <Cat id="system" active={cat}>
        <Section title="System" accent="var(--color-sv-sec-system)" icon={<CogIcon />}>
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
          <Row
            label="Copy diagnostics"
            hint={diagMsg || "Copies app + system info and recent logs, so you can paste them when reporting a problem"}
          >
            <button
              onClick={async () => {
                const text = await copyDiagnostics();
                if (text) {
                  await copyToClipboard(text);
                  setDiagMsg("Copied — paste it anywhere to share");
                } else {
                  setDiagMsg("Diagnostics need the desktop app");
                }
              }}
              className="rounded-lg border border-sv-border px-3 py-1.5 text-xs text-sv-text hover:bg-sv-surface-2"
            >
              Copy diagnostics
            </button>
          </Row>
        </Section>
        </Cat>
        </div>
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

export function Section({
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
        className="flex items-start gap-2.5 px-5 pb-3.5 pt-3.5"
        style={accent ? { borderLeft: `3px solid ${accent}` } : undefined}
      >
        {icon && (
          <span
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-sv-border bg-sv-bg"
            style={accent ? { color: accent } : undefined}
          >
            {icon}
          </span>
        )}
        <div className="min-w-0">
          <h2 className="font-serif text-[16px] font-semibold leading-tight">{title}</h2>
          {desc && <p className="mt-0.5 font-serif text-[11.5px] italic leading-relaxed text-sv-muted">{desc}</p>}
        </div>
      </div>
      <div className="mx-5 border-y border-sv-border" />
      <div className="px-5">{children}</div>
    </div>
  );
}

// Lets a control inside a Row borrow the Row's label as its accessible name.
// The label is a plain <div> beside the control rather than a <label for=…>, so
// without this every Toggle announced as an unnamed button.
const RowLabelContext = createContext<string | undefined>(undefined);

// The Settings categories, shown in the left rail. Each maps to one pane.
const CATS: { id: string; label: string; icon: React.ReactNode; accent: string }[] = [
  { id: "recording", label: "Recording", icon: <MicrophoneIcon />, accent: "var(--color-sv-sec-dictation)" },
  { id: "stt", label: "Speech-to-text", icon: <WaveIcon />, accent: "var(--color-sv-sec-perf)" },
  { id: "grammar", label: "Grammar", icon: <SpellIcon />, accent: "var(--color-sv-sec-replace)" },
  { id: "tts", label: "Read aloud", icon: <SpeakerIcon />, accent: "var(--color-sv-sec-tts)" },
  { id: "words", label: "Words & shortcuts", icon: <BookIcon />, accent: "var(--color-sv-sec-vocab)" },
  { id: "profiles", label: "Per-app profiles", icon: <LayersIcon />, accent: "var(--color-sv-sec-profiles)" },
  { id: "performance", label: "Performance", icon: <GaugeIcon />, accent: "var(--color-sv-sec-perf)" },
  { id: "system", label: "System", icon: <CogIcon />, accent: "var(--color-sv-sec-system)" },
];

// Renders its children only when its category is the active one.
function Cat({ id, active, children }: { id: string; active: string; children: React.ReactNode }) {
  return active === id ? <>{children}</> : null;
}

function WaveIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
      <path d="M4 10v4M8 6v12M12 8v8M16 5v14M20 10v4" />
    </svg>
  );
}

function SpellIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 7h11M4 12h16M4 17h8" />
      <path d="M17 15l2 2 3-4" />
    </svg>
  );
}

export function Row({
  label,
  hint,
  info,
  children,
}: {
  label: string;
  hint?: string;
  info?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-sv-border/60 py-3 last:border-b-0">
      <div className="min-w-0">
        <div className="flex items-center gap-1.5 text-[13px]">
          {label}
          {info && <InfoTip text={info} />}
        </div>
        {hint && <div className="mt-0.5 text-[11px] leading-relaxed text-sv-muted max-w-[46ch]">{hint}</div>}
      </div>
      <div className="shrink-0">
        <RowLabelContext.Provider value={label}>{children}</RowLabelContext.Provider>
      </div>
    </div>
  );
}

// Small "i" affordance next to a setting's label. Hovering (mouse) or clicking
// (touch/keyboard) reveals the full, deeper explanation — the one-line hint
// stays for quick scanning, this carries the whole story.
export function InfoTip({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  return (
    <span className="relative inline-flex align-middle">
      <button
        type="button"
        aria-label="More information"
        onClick={() => setOpen((o) => !o)}
        onBlur={() => setOpen(false)}
        className="peer grid h-[15px] w-[15px] place-items-center rounded-full border border-sv-border text-sv-muted transition hover:border-sv-accent hover:text-sv-accent"
      >
        <svg viewBox="0 0 24 24" className="h-2.5 w-2.5" fill="none" stroke="currentColor" strokeWidth={2.4} strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 11v5" />
          <path d="M12 7.5v.01" />
        </svg>
      </button>
      <span
        role="tooltip"
        className={`pointer-events-none absolute left-0 top-full z-30 mt-2 w-[264px] rounded-lg border border-sv-border bg-sv-surface-2 p-2.5 text-[11.5px] font-normal leading-relaxed text-sv-text shadow-xl transition-opacity duration-150 peer-hover:opacity-100 ${
          open ? "opacity-100" : "opacity-0"
        }`}
      >
        {text}
      </span>
    </span>
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
              ? "bg-sv-accent text-sv-on-accent"
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

export function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  const rowLabel = useContext(RowLabelContext);
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={rowLabel}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 rounded-full border transition-colors duration-75 ${
        checked
          ? "border-transparent bg-sv-accent"
          : "border-sv-border bg-sv-surface-2"
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
