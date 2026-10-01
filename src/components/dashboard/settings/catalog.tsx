// Every setting on the Settings page, described once: which topic it belongs
// to, whether a beginner sees it (basic) or it lives in the fold-open advanced
// card, its plain-language label, the explanation behind its info icon, and
// the control. The page only decides layout.
import { useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import HotkeyRecorder from "../../shared/HotkeyRecorder";
import Select from "../../shared/Select";
import { useSettingsStore } from "../../../stores/settingsStore";
import { useModelStore } from "../../../stores/modelStore";
import { LANGUAGES, STT_MODELS, TTS_MODELS, TTS_SAMPLE_TEXT, honorsLanguage } from "../../../services/catalog";
import { copyDiagnostics, copyToClipboard, ttsSpeakText } from "../../../services/tauriBridge";
import { checkForUpdatesManual } from "../../../services/updater";
import type { Settings } from "../../../types";
import type { TopicId } from "./topics";
import { GhostButton, IconButton, Segmented, Slider, Toggle, inputCls } from "./parts";
import { ArrowIcon, PlayIcon, PlusIcon, XIcon } from "./icons";
import { CoeditControl, GectorControl } from "./downloads";

export interface Ctx {
  s: Settings;
  set: (patch: Partial<Settings>) => void;
  onHotkey: (accelerator: string) => void;
  hotkeyError: string | null;
  devices: string[];
  gpuName: string | null;
  coeditReady: boolean;
  setCoeditReady: (v: boolean) => void;
  gectorReady: boolean;
  setGectorReady: (v: boolean) => void;
  toggleRule: (rule: string, enabled: boolean) => void;
}

export interface SettingDef {
  id: string;
  topic: TopicId;
  tier: "basic" | "advanced";
  label: string;
  info?: string | ((c: Ctx) => string);
  keywords?: string;
  // The control sits under the label instead of beside it.
  wide?: boolean;
  visible?: (c: Ctx) => boolean;
  // When hidden, the setting that has to be turned on first. Search jumps
  // there instead of changing anything on the user's behalf.
  requires?: string;
  requiresLabel?: string;
  render: (c: Ctx, label: string) => ReactNode;
}

const proofreadOn = (c: Ctx) => c.s.inline_proofread;
const ttsOn = (c: Ctx) => c.s.tts_enabled;
const UNDERLINES = "Underline mistakes as I type";
const READ_ALOUD = "Read selected text aloud";

export const SETTINGS: SettingDef[] = [
  // ── Dictation ────────────────────────────────────────────────────────────
  {
    id: "hotkey",
    topic: "dictation",
    tier: "basic",
    label: "Push-to-talk key",
    info: "Hold this key anywhere in Windows, speak, then let go. Your words are typed wherever your cursor is. Pick a combination no other app uses.",
    keywords: "hotkey shortcut keyboard global",
    wide: true,
    render: (c) => <HotkeyRecorder value={c.s.hotkey} onChange={c.onHotkey} error={c.hotkeyError} />,
  },
  {
    id: "mic",
    topic: "dictation",
    tier: "basic",
    label: "Microphone",
    keywords: "input device audio",
    render: (c) => (
      <Select value={c.s.audio_device ?? ""} onChange={(v) => c.set({ audio_device: v || null })} className="w-52 xl:w-60">
        <option value="">System default</option>
        {c.devices.map((d) => (
          <option key={d} value={d}>
            {d}
          </option>
        ))}
      </Select>
    ),
  },
  {
    id: "language",
    topic: "dictation",
    tier: "basic",
    label: "Language you speak",
    info: "Auto-detect guesses from the first few seconds and can slip on short clips. Picking your language is faster and more accurate.",
    keywords: "locale english auto detect",
    render: (c) => <LanguageControl c={c} />,
  },
  {
    id: "model",
    topic: "dictation",
    tier: "basic",
    label: "Voice model",
    info: "The model that turns your speech into text. Download, compare and switch models in Model Store.",
    keywords: "speech to text stt whisper parakeet moonshine model",
    render: (c) => <VoiceModelLink c={c} />,
  },
  {
    id: "lock",
    topic: "dictation",
    tier: "advanced",
    label: "Double-tap to keep recording",
    info: "Tap the key twice quickly to record hands-free. Tap once more to stop and paste.",
    keywords: "toggle hands free lock",
    render: (c, label) => <Toggle label={label} checked={c.s.toggle_mode} onChange={(v) => c.set({ toggle_mode: v })} />,
  },
  {
    id: "noise",
    topic: "dictation",
    tier: "advanced",
    label: "Background noise filter",
    info: "Sounds quieter than this count as silence and are trimmed before transcription. Stricter cuts more hum and wind; more sensitive picks up quiet speech but also more noise.",
    keywords: "input sensitivity gate silence quiet",
    wide: true,
    render: (c, label) => (
      <Slider
        label={label}
        value={c.s.input_sensitivity}
        min={0}
        max={100}
        onChange={(v) => c.set({ input_sensitivity: v })}
        left="Strict"
        right="Sensitive"
      />
    ),
  },
  {
    id: "space",
    topic: "dictation",
    tier: "advanced",
    label: "Add a space after each paste",
    info: "Stops back-to-back dictations from running into each other.",
    keywords: "trailing space",
    render: (c, label) => (
      <Toggle label={label} checked={c.s.append_trailing_space} onChange={(v) => c.set({ append_trailing_space: v })} />
    ),
  },
  {
    id: "pill",
    topic: "dictation",
    tier: "advanced",
    label: "Hide the recording pill when idle",
    info: "The pill fades about five seconds after pasting and comes back the moment you press the key.",
    keywords: "overlay pill auto hide",
    render: (c, label) => <Toggle label={label} checked={c.s.pill_auto_hide} onChange={(v) => c.set({ pill_auto_hide: v })} />,
  },
  {
    id: "chunk",
    topic: "dictation",
    tier: "advanced",
    label: "Transcribe while you speak",
    info: "Transcribes finished sentences mid-recording. It rarely helps and can slow 4-core CPUs down. For faster dictation, pick a lighter voice model instead.",
    keywords: "chunk silence streaming",
    render: (c, label) => (
      <Toggle label={label} checked={c.s.chunk_on_silence} onChange={(v) => c.set({ chunk_on_silence: v })} />
    ),
  },
  {
    id: "cloud",
    topic: "dictation",
    tier: "advanced",
    label: "Transcribe in the cloud",
    info: "Sends your audio to a cloud provider you added instead of transcribing on this PC. Often faster, but not private or offline, and it may cost money.",
    keywords: "cloud provider openai api stt source",
    render: (c) => <CloudSourceSelect c={c} />,
  },

  // ── Writing help ─────────────────────────────────────────────────────────
  {
    id: "proofread",
    topic: "writing",
    tier: "basic",
    label: UNDERLINES,
    info: "Underlines spelling and grammar mistakes in almost any Windows app, like a word processor does. Click an underline to see the fix. Runs on this PC, English only.",
    keywords: "proofread spell check squiggle grammar inline",
    render: (c, label) => (
      <Toggle label={label} checked={c.s.inline_proofread} onChange={(v) => c.set({ inline_proofread: v })} />
    ),
  },
  {
    id: "oxford",
    topic: "writing",
    tier: "advanced",
    label: "Suggest Oxford commas",
    info: 'Suggests a comma before "and" in lists, as in "apples, oranges, and bananas".',
    keywords: "comma rule",
    visible: proofreadOn,
    requires: "proofread",
    requiresLabel: UNDERLINES,
    render: (c, label) => (
      <Toggle
        label={label}
        checked={!c.s.proofread_disabled_rules.includes("OxfordComma")}
        onChange={(v) => c.toggleRule("OxfordComma", v)}
      />
    ),
  },
  {
    id: "filler",
    topic: "writing",
    tier: "advanced",
    label: "Flag filler words",
    info: 'Underlines spoken fillers like "um" and "uh".',
    keywords: "um uh rule filler",
    visible: proofreadOn,
    requires: "proofread",
    requiresLabel: UNDERLINES,
    render: (c, label) => (
      <Toggle
        label={label}
        checked={!c.s.proofread_disabled_rules.includes("Filler")}
        onChange={(v) => c.toggleRule("Filler", v)}
      />
    ),
  },
  {
    id: "gector",
    topic: "writing",
    tier: "advanced",
    label: "Smarter grammar checks",
    info: "A small model that reads the whole sentence to catch correctly spelled wrong words, like their/there or affect/effect. Basic underlines work without it. Balanced is 122 MB; Best quality is 512 MB.",
    keywords: "gector context grammar ai model download",
    visible: proofreadOn,
    requires: "proofread",
    requiresLabel: UNDERLINES,
    render: (c, label) => <GectorControl c={c} label={label} />,
  },
  {
    id: "gector-sens",
    topic: "writing",
    tier: "advanced",
    label: "How picky the grammar checks are",
    info: "Picky finds more mistakes but raises more false alarms. Relaxed only flags what it's sure about.",
    keywords: "sensitivity relaxed aggressive picky",
    visible: (c) => c.s.inline_proofread && c.gectorReady && !c.s.proofread_disabled_rules.includes("Gector"),
    requires: "gector",
    requiresLabel: "Smarter grammar checks",
    render: (c, label) => (
      <Segmented
        label={label}
        value={c.s.gector_sensitivity}
        options={[
          ["relaxed", "Relaxed"],
          ["balanced", "Balanced"],
          ["aggressive", "Picky"],
        ]}
        onChange={(v) => c.set({ gector_sensitivity: v as Settings["gector_sensitivity"] })}
      />
    ),
  },
  {
    id: "ignore",
    topic: "writing",
    tier: "advanced",
    label: "Don't check these apps",
    info: 'App names separated by commas, for example "code, photoshop". No underlines are drawn in those apps.',
    keywords: "exclude ignore apps",
    visible: proofreadOn,
    requires: "proofread",
    requiresLabel: UNDERLINES,
    wide: true,
    render: (c, label) => (
      <input
        value={c.s.proofread_ignore_apps}
        onChange={(e) => c.set({ proofread_ignore_apps: e.target.value })}
        placeholder="code, photoshop"
        aria-label={label}
        className={inputCls + " w-full max-w-md"}
      />
    ),
  },
  {
    id: "coedit",
    topic: "writing",
    tier: "advanced",
    label: "Fix grammar before pasting",
    info: "A larger on-device model corrects your dictated text before it's pasted. Skipped when a writing style is active. Needs an 818 MB download.",
    keywords: "coedit grammar correct dictated",
    render: (c, label) => <CoeditControl c={c} label={label} />,
  },
  {
    id: "profiles",
    topic: "writing",
    tier: "advanced",
    label: "Writing style per app",
    info: 'Switches the writing style automatically based on the app you dictate into. Match on the program name, for example "code" for VS Code or "outlook".',
    keywords: "per app profiles mode style",
    wide: true,
    render: () => <ProfilesEditor />,
  },

  // ── Read aloud ───────────────────────────────────────────────────────────
  {
    id: "tts",
    topic: "readaloud",
    tier: "basic",
    label: READ_ALOUD,
    info: "Select text in any app, press the read-aloud key, and hear it. Press the key again to stop.",
    keywords: "tts text to speech voice speak",
    render: (c, label) => <Toggle label={label} checked={c.s.tts_enabled} onChange={(v) => c.set({ tts_enabled: v })} />,
  },
  {
    id: "tts-key",
    topic: "readaloud",
    tier: "basic",
    label: "Read-aloud key",
    keywords: "tts hotkey shortcut",
    visible: ttsOn,
    requires: "tts",
    requiresLabel: READ_ALOUD,
    wide: true,
    render: (c) => <HotkeyRecorder value={c.s.tts_hotkey} onChange={(v) => c.set({ tts_hotkey: v })} />,
  },
  {
    id: "tts-voice",
    topic: "readaloud",
    tier: "basic",
    label: "Voice",
    info: "Voices are downloaded in Model Store, under Text-to-Speech.",
    keywords: "tts voice kokoro piper",
    visible: ttsOn,
    requires: "tts",
    requiresLabel: READ_ALOUD,
    render: (c) => <VoicePicker c={c} />,
  },
  {
    id: "tts-test",
    topic: "readaloud",
    tier: "basic",
    label: "Hear a sample",
    keywords: "tts test play preview",
    // Nothing to play until a voice is picked.
    visible: (c) => c.s.tts_enabled && !!c.s.active_tts_voice,
    requires: "tts-voice",
    requiresLabel: "Voice",
    render: (c) => <PlaySample c={c} />,
  },

  // ── Words ────────────────────────────────────────────────────────────────
  {
    id: "vocab",
    topic: "words",
    tier: "basic",
    label: "Words it should recognize",
    info: "Names or jargon the voice model keeps getting wrong. Separate them with commas, most important first. Works with Whisper and Parakeet voice models.",
    keywords: "custom vocabulary hotwords names jargon",
    wide: true,
    render: (c, label) => (
      <textarea
        value={c.s.custom_vocabulary}
        onChange={(e) => c.set({ custom_vocabulary: e.target.value })}
        placeholder="Tauri, Kubernetes, your name"
        aria-label={label}
        rows={2}
        className={inputCls + " w-full max-w-xl resize-y"}
      />
    ),
  },
  {
    id: "replacements",
    topic: "words",
    tier: "basic",
    label: "Shortcuts",
    info: 'Say a short phrase and the full text is pasted instead. For example "my email" becomes your address. Not case-sensitive.',
    keywords: "text replacements snippets expand",
    wide: true,
    render: () => <ReplacementsEditor />,
  },
  {
    id: "vocab-on",
    topic: "words",
    tier: "advanced",
    label: "Use my words when transcribing",
    info: "Turn off for the fastest transcription. Your word list is kept, it just isn't used until you turn this back on.",
    keywords: "custom vocabulary bias toggle speed",
    render: (c, label) => (
      <Toggle
        label={label}
        checked={c.s.use_custom_vocabulary}
        onChange={(v) => c.set({ use_custom_vocabulary: v })}
      />
    ),
  },
  {
    id: "vocab-strength",
    topic: "words",
    tier: "advanced",
    label: "How strongly to favor your words",
    info: "Parakeet only. Higher makes your words win more often; too high can swap in your words where a similar-sounding word was right. Whisper ignores this.",
    keywords: "vocabulary strength boost parakeet hotword",
    wide: true,
    visible: (c) => c.s.use_custom_vocabulary,
    requires: "vocab-on",
    requiresLabel: "Use my words when transcribing",
    render: (c, label) => (
      <Slider
        label={label}
        value={c.s.vocabulary_strength}
        min={0.5}
        max={1.5}
        step={0.25}
        onChange={(v) => c.set({ vocabulary_strength: v })}
        left="Gentle"
        right="Strong"
      />
    ),
  },

  // ── System ───────────────────────────────────────────────────────────────
  {
    id: "startup",
    topic: "system",
    tier: "basic",
    label: "Start with Windows",
    info: "Opens Silent Voice in the background when you sign in, so dictation is ready right away.",
    keywords: "launch startup autostart boot login",
    render: (c, label) => <Toggle label={label} checked={c.s.auto_start} onChange={(v) => c.set({ auto_start: v })} />,
  },
  {
    id: "updates",
    topic: "system",
    tier: "basic",
    label: "Updates",
    info: "Silent Voice checks for updates when it starts. Checking now installs an update right away if one is found.",
    keywords: "update version check upgrade",
    render: () => <UpdateCheck />,
  },
  {
    id: "unload",
    topic: "system",
    tier: "advanced",
    label: "Free memory when idle",
    info: "Unloads the voice model after a quiet period. That frees memory, but the next dictation takes a few seconds to start.",
    keywords: "unload model memory ram idle",
    render: (c) => <UnloadSelect c={c} />,
  },
  {
    id: "gpu",
    topic: "system",
    tier: "advanced",
    label: "Use graphics card",
    info: (c) =>
      c.gpuName
        ? `Runs transcription on your graphics card (${c.gpuName}), which is usually much faster. Turn it off if dictation errors or crashes.`
        : "No compatible graphics card was found, so the CPU is used. Leave this off.",
    keywords: "gpu vulkan acceleration performance",
    render: (c, label) => <Toggle label={label} checked={c.s.use_gpu} onChange={(v) => c.set({ use_gpu: v })} />,
  },
  {
    id: "diagnostics",
    topic: "system",
    tier: "advanced",
    label: "Diagnostics",
    info: "Copies app and system info plus recent logs, so you can paste them into a bug report.",
    keywords: "logs debug support bug report",
    render: () => <CopyDiagnostics />,
  },
];

// ─── Controls with their own state ─────────────────────────────────────────

function LanguageControl({ c }: { c: Ctx }) {
  const model = STT_MODELS.find((m) => m.id === c.s.active_stt_model);
  // A model that ignores the language setting must not show a live picker —
  // that silent mismatch is what made Bangla come back as English.
  if (!honorsLanguage(model))
    return (
      <span className="text-[13px] text-sv-muted">
        {model?.multilingual ? "Detected automatically" : "English only (set by the voice model)"}
      </span>
    );
  return (
    <Select value={c.s.language} onChange={(v) => c.set({ language: v })} className="w-52 xl:w-60">
      {LANGUAGES.map((l) => (
        <option key={l.code} value={l.code}>
          {l.name}
        </option>
      ))}
    </Select>
  );
}

function VoiceModelLink({ c }: { c: Ctx }) {
  const downloaded = useModelStore((s) => s.downloaded);
  const providers = useSettingsStore((s) => s.providers);
  const cloud = providers.find((p) => p.id === c.s.stt_cloud_provider_id);
  const model = STT_MODELS.find((m) => m.id === c.s.active_stt_model);
  const ready = !!model && downloaded.has(model.id);
  const name = cloud ? `Cloud · ${cloud.name}` : model ? model.label : "None chosen";
  return (
    <span className="inline-flex items-center gap-3 text-[13px]">
      <span className={cloud || ready ? "text-sv-text" : "text-sv-muted"}>{name}</span>
      <Link to="/models" className="font-medium text-sv-accent hover:underline">
        {model || cloud ? "Change" : "Choose"}
      </Link>
    </span>
  );
}

function CloudSourceSelect({ c }: { c: Ctx }) {
  const providers = useSettingsStore((s) => s.providers).filter((p) => p.uses.includes("stt"));
  if (providers.length === 0)
    return (
      <Link to="/api" className="text-[13px] font-medium text-sv-accent hover:underline">
        Add a cloud provider
      </Link>
    );
  return (
    <Select
      value={c.s.stt_cloud_provider_id ?? "local"}
      onChange={(v) => c.set({ stt_cloud_provider_id: v === "local" ? null : v })}
      className="w-52 xl:w-60"
    >
      <option value="local">Off, transcribe on this PC</option>
      {providers.map((p) => (
        <option key={p.id} value={p.id}>
          {p.name}
        </option>
      ))}
    </Select>
  );
}

function VoicePicker({ c }: { c: Ctx }) {
  const downloadedTts = useModelStore((s) => s.downloadedTts);
  const voices = TTS_MODELS.filter((v) => downloadedTts.has(v.id));
  if (voices.length === 0)
    return (
      <Link to="/models" className="text-[13px] font-medium text-sv-accent hover:underline">
        Get a voice
      </Link>
    );
  return (
    <Select value={c.s.active_tts_voice ?? ""} onChange={(v) => c.set({ active_tts_voice: v || null })} className="w-52 xl:w-60">
      <option value="">None selected</option>
      {voices.flatMap((v) =>
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
    </Select>
  );
}

function PlaySample({ c }: { c: Ctx }) {
  return (
    <GhostButton
      disabled={!c.s.active_tts_voice}
      onClick={() => {
        // A voice can only pronounce its own language, so speak a sample in the
        // voice's language (English through a Bangla model is gibberish).
        const baseId = (c.s.active_tts_voice ?? "").split("#")[0];
        const voice = TTS_MODELS.find((v) => v.id === baseId);
        ttsSpeakText(TTS_SAMPLE_TEXT[voice?.language ?? ""] ?? TTS_SAMPLE_TEXT.default);
      }}
    >
      <PlayIcon />
      Play
    </GhostButton>
  );
}

function ReplacementsEditor() {
  const snippets = useSettingsStore((s) => s.snippets);
  const add = useSettingsStore((s) => s.addSnippet);
  const update = useSettingsStore((s) => s.updateSnippet);
  const remove = useSettingsStore((s) => s.deleteSnippet);
  return (
    <div className="w-full max-w-xl space-y-2">
      {snippets.map((sn) => (
        <div key={sn.id} className="flex items-center gap-2">
          <input
            value={sn.trigger}
            onChange={(e) => update(sn.id, { trigger: e.target.value })}
            placeholder="When I say"
            aria-label="Phrase you say"
            className={inputCls + " w-40"}
          />
          <ArrowIcon />
          <input
            value={sn.replacement}
            onChange={(e) => update(sn.id, { replacement: e.target.value })}
            placeholder="type this"
            aria-label="Text to paste instead"
            className={inputCls + " min-w-0 flex-1"}
          />
          <IconButton label="Remove shortcut" danger onClick={() => remove(sn.id)}>
            <XIcon />
          </IconButton>
        </div>
      ))}
      <AddButton onClick={add}>Add a shortcut</AddButton>
    </div>
  );
}

function ProfilesEditor() {
  const profiles = useSettingsStore((s) => s.appProfiles);
  const modes = useSettingsStore((s) => s.modes);
  const add = useSettingsStore((s) => s.addAppProfile);
  const update = useSettingsStore((s) => s.updateAppProfile);
  const remove = useSettingsStore((s) => s.deleteAppProfile);
  return (
    <div className="w-full max-w-xl space-y-2">
      {profiles.map((p) => (
        <div key={p.id} className="flex items-center gap-2">
          <input
            value={p.app_match}
            onChange={(e) => update(p.id, { app_match: e.target.value })}
            placeholder="App name"
            aria-label="App name"
            className={inputCls + " w-40"}
          />
          <ArrowIcon />
          <Select value={p.mode_id} onChange={(v) => update(p.id, { mode_id: v })} className="min-w-0 flex-1">
            {modes.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </Select>
          <IconButton label="Remove app" danger onClick={() => remove(p.id)}>
            <XIcon />
          </IconButton>
        </div>
      ))}
      <AddButton onClick={add}>Add an app</AddButton>
    </div>
  );
}

function AddButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-[12.5px] font-medium text-sv-accent transition-colors duration-150 hover:bg-sv-surface-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
    >
      <PlusIcon />
      {children}
    </button>
  );
}

function UpdateCheck() {
  const [msg, setMsg] = useState("");
  return (
    <span className="inline-flex items-center gap-3">
      {msg && (
        <span className="text-[12.5px] text-sv-muted" aria-live="polite">
          {msg}
        </span>
      )}
      <GhostButton
        disabled={msg === "Checking…"}
        onClick={async () => {
          setMsg("Checking…");
          const r = await checkForUpdatesManual();
          setMsg(
            r.status === "none"
              ? "You're on the latest version"
              : r.status === "error"
              ? "Couldn't check. Try again later."
              : r.status === "unsupported"
              ? "Updates need the desktop app"
              : "Installing the update…"
          );
        }}
      >
        Check now
      </GhostButton>
    </span>
  );
}

function CopyDiagnostics() {
  const [msg, setMsg] = useState("");
  return (
    <span className="inline-flex items-center gap-3">
      {msg && (
        <span className="text-[12.5px] text-sv-muted" aria-live="polite">
          {msg}
        </span>
      )}
      <GhostButton
        onClick={async () => {
          const text = await copyDiagnostics();
          if (text) {
            await copyToClipboard(text);
            setMsg("Copied");
          } else {
            setMsg("Needs the desktop app");
          }
        }}
      >
        Copy
      </GhostButton>
    </span>
  );
}

const UNLOAD_PRESETS = [0, 5, 15, 30, 60, 120];

function UnloadSelect({ c }: { c: Ctx }) {
  const m = c.s.model_unload_minutes;
  const label = (n: number) =>
    n === 0 ? "Never, keep it instant" : n % 60 === 0 ? `After ${n / 60} hour${n === 60 ? "" : "s"}` : `After ${n} minutes`;
  // Keep a value set on the old page's "Custom" field selectable.
  const options = UNLOAD_PRESETS.includes(m) ? UNLOAD_PRESETS : [...UNLOAD_PRESETS, m].sort((a, b) => a - b);
  return (
    <Select value={String(m)} onChange={(v) => c.set({ model_unload_minutes: Number(v) })} className="w-52 xl:w-60">
      {options.map((n) => (
        <option key={n} value={String(n)}>
          {label(n)}
        </option>
      ))}
    </Select>
  );
}

// What the page no longer asks about, because the app handles it.
export const AUTOMATIC: { label: string; detail: string }[] = [
  {
    label: "Smart voice detection",
    detail: "A tiny 2 MB model that tells your voice apart from fans, keyboards and music. It downloads by itself once you have a voice model.",
  },
  {
    label: "Tuned for this PC",
    detail: "Graphics card, CPU threads and grammar defaults are picked for your hardware when you first set up Silent Voice.",
  },
];
