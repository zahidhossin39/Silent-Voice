# Silent Voice — AI Agent Handoff Document

This file is the single source of truth for any AI agent continuing work on this project.
Read it fully before touching any code. It contains the architecture, what's done, what's
left, and — critically — a list of things that **must never be changed** because they fix
hard-won bugs.

---

## 1. What This App Is

**Silent Voice** — a free, local-first voice-to-text desktop app for Windows.
Hold a hotkey → speak → release → transcribed (and optionally AI-processed) text is
pasted at the cursor. Like SuperWhisper / Wispr Flow but free, offline, and open-source.

**Original spec:** Documented in `CLAUDE.md`.

---

## 2. Tech Stack

| Layer | Technology |
|---|---|
| App framework | Tauri v2 (Rust + WebView2) |
| Frontend | React 19, TypeScript, Tailwind v4, Vite |
| State | Zustand (settings, models, history) |
| Backend | Rust (Tauri commands, audio, transcription, LLM) |
| STT | whisper.cpp v1.9.1 (bundled sidecar binary) |
| Local LLM | llama.cpp server b9830 (bundled sidecar) |
| Cloud LLM | Generic OpenAI-compatible client (OpenRouter, Groq, Together, etc.) |
| Audio capture | cpal → 16 kHz mono WAV |
| Paste at cursor | arboard + enigo |
| Hardware detection | sysinfo + Windows DXGI |
| History | Local JSON file (%APPDATA%/SilentVoice/history.json) |

**Target machine:** Intel i7-8650U, 16 GB RAM, Intel UHD 620 (no NVIDIA GPU).
All local inference runs on CPU. GPU paths exist but untested.

---

## 3. Project Structure

```
Silent voice/
├── CLAUDE.md                        ← this file
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src/                             ← React frontend
│   ├── App.tsx                      ← router; renders OverlayApp for ?view=overlay
│   ├── main.tsx
│   ├── styles.css                   ← Tailwind + CSS variables + sv-bar keyframe
│   ├── types/index.ts               ← all shared TypeScript interfaces
│   ├── components/
│   │   ├── dashboard/               ← 7 tabs: Home, ModelStore, Modes, ApiKeys, Settings, History, Guide
│   │   ├── overlay/
│   │   │   ├── OverlayApp.tsx       ← the always-on-top pill window app
│   │   │   └── RecordingOverlay.tsx ← pill content (idle line / recording dot+waveform)
│   │   └── shared/
│   │       ├── HotkeyRecorder.tsx   ← click-to-capture hotkey input (any combo OR solo key)
│   │       ├── ModelCard.tsx        ← STT model card with download + compat badge
│   │       ├── Badge.tsx
│   │       ├── Page.tsx
│   │       └── WaveformVisualizer.tsx
│   ├── hooks/
│   │   ├── useHardwareInfo.ts
│   │   ├── usePipeline.ts           ← subscribes to pipeline:// events
│   │   └── useRuntimeSync.ts        ← keeps Rust RuntimeConfig in sync with settings
│   ├── services/
│   │   ├── catalog.ts               ← STT + LLM model catalogs with verified URLs
│   │   ├── format.ts                ← smart MB/GB formatting
│   │   ├── modes.ts                 ← built-in AI mode definitions
│   │   ├── recommend.ts             ← hardware-based compat badge logic
│   │   └── tauriBridge.ts           ← ALL Tauri invoke calls (degrades in browser)
│   └── stores/
│       ├── settingsStore.ts         ← Zustand + localStorage
│       ├── modelStore.ts            ← downloaded STT + LLM model sets
│       ├── historyStore.ts          ← transcription history
│       └── uiStore.ts               ← live recordingState
│
└── src-tauri/                       ← Rust backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/default.json
    ├── sidecars/
    │   ├── whisper-cpp-x86_64-pc-windows-msvc.exe  ← whisper.cpp sidecar
    │   ├── *.dll                    ← whisper's ggml DLLs (ALSO copied to target/debug/)
    │   └── llama/
    │       ├── llama-server.exe     ← llama.cpp server sidecar
    │       └── *.dll                ← llama's DLLs (ALSO copied to target/debug/llama/)
    └── src/
        ├── main.rs                  ← windows_subsystem = "windows"
        ├── lib.rs                   ← ALL Tauri commands + AppState + run()
        ├── history.rs               ← load/save/append/clear JSON history file
        ├── logging.rs               ← append to %APPDATA%\SilentVoice\logs\silent-voice.log (2MB rotation)
        ├── audio/
        │   ├── mod.rs
        │   ├── capture.rs           ← cpal mic → 16kHz WAV + device listing
        │   └── gate.rs              ← RMS noise gate: trim_silence(samples, sensitivity) → Option<Vec<f32>>
        ├── transcription/
        │   ├── mod.rs
        │   └── whisper.rs           ← sidecar invocation (whisper-cli flags: -m -f -t -l --no-timestamps)
        ├── llm/
        │   ├── mod.rs
        │   ├── llama.rs             ← LlamaServer (start/stop/is_running/wait_ready)
        │   ├── ollama.rs            ← Ollama client (unused now but kept; user may install later)
        │   └── openai.rs            ← generic OpenAI-compatible chat + list_models
        ├── models/
        │   ├── mod.rs
        │   ├── downloader.rs        ← download_model (STT) + download_llm_model (GGUF) + download_tts_model
        │   └── registry.rs          ← data dir paths, model file names, list_downloaded*, tts_model_path()
        └── system/
            ├── mod.rs
            ├── hardware.rs          ← CPU/RAM/GPU detection (sysinfo + DXGI)
            ├── hotkey.rs            ← on_pressed / on_released pipeline; tidy_ai_output
            ├── overlay.rs           ← overlay window creation + animate_resize (snaps, no tween)
            ├── paste.rs             ← arboard + enigo paste-at-cursor
            ├── tray.rs              ← system tray menu
            ├── autostart.rs         ← HKCU Run key (set_enabled / is_enabled via winreg)
            ├── foreground.rs        ← Windows API: get foreground exe basename for per-app profiles
            ├── textfmt.rs           ← format_numbers() — always-on spoken→digit conversion
            └── tts.rs               ← Piper TTS: read_selection / speak_text / stop / synth_and_play
```

Frontend additions since initial build:
```
src/
├── components/
│   ├── dashboard/
│   │   └── Guide.tsx               ← "How to use" page with visual pill mockups
│   └── onboarding/
│       └── Onboarding.tsx          ← 4-step first-launch wizard; hardware-based model recommendation
└── services/
    └── catalog.ts                  ← now also exports TTS_MODELS (29 Piper voices)
```

Sidecars:
```
src-tauri/sidecars/
├── piper/                          ← Piper TTS engine (release 2023.11.14-2); installed as piper/ next to exe
│   ├── piper.exe
│   └── *.dll
└── (whisper + llama unchanged)
```

---

## 4. How to Run / Build

```powershell
# Frontend only (browser, no Rust needed)
cd "D:\Vibe-coding\Silent voice"
npm run dev    # → http://localhost:1420

# Run the already-built desktop app (fastest)
cd "D:\Vibe-coding\Silent voice\src-tauri\target\debug"
.\silent-voice.exe

# Full build (Rust must be on PATH)
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
cd "D:\Vibe-coding\Silent voice"
npm run tauri:dev     # dev mode with HMR
# OR
cd "D:\Vibe-coding\Silent voice\src-tauri"
cargo build           # just the Rust binary
```

**Rust toolchain:** `$USERPROFILE\.cargo\bin\` (NOT on system PATH — must add manually or use full path).
Cargo version: 1.96.0 MSVC. VS 2022 Build Tools with C++ workload already installed.

---

## 5. Phase Completion Status

| Phase | Goal | Status |
|---|---|---|
| 1 | Foundation: record → transcribe → clipboard, tray, model downloader, hardware detection | ✅ Complete + verified |
| 2 | Core UX: paste-at-cursor, overlay, JSON history, STT presets, device selector | ✅ Complete + verified |
| 3 | AI processing modes (local LLM via bundled llama.cpp) | ✅ Complete + end-to-end verified |
| 4 | Model Store + hardware recommendations | ✅ UI complete; STT download works; LLM download works. Catalog expanded to ~49 STT models (OpenAI + distil-whisper + 16 language-specific fine-tunes from BELLE-2, Kotoba Technologies, ReazonSpeech, VinAI, KBLab, etc — see §15) |
| 5 | Always-listening (Silero VAD + openWakeWord) | ❌ Not started |
| 6 | Cloud API integration (OpenAI, OpenRouter, etc.) | ✅ Complete (generic OpenAI client) |
| 7 | Polish + Windows installer + onboarding + auto-updater | ✅ Complete (v0.1.4). Done: onboarding wizard, app icon, error logging, NSIS installer, resource bundling fix, auto-updater. |

---

## 6. The Pipeline (How Dictation Works End-to-End)

```
User holds hotkey
  → on_pressed()  [hotkey.rs]  → start Recorder (cpal)  → show overlay (recording state)
  → [user speaks]
User releases hotkey
  → on_released() [hotkey.rs]  → stop Recorder → write 16kHz WAV → transcribe via whisper.cpp sidecar
  → if active mode is "local" AND model downloaded:
        run_local_llm() → ensure llama-server running → POST /v1/chat/completions → tidy_ai_output()
  → if active mode is "api":
        openai::chat() → provider base_url/key/model
  → paste_at_cursor() [paste.rs] → arboard set_text + enigo Ctrl+V
  → history::append() → save to history.json
  → emit pipeline://result → frontend adds to historyStore
  → overlay returns to idle state
```

**Error safety:** if any AI processing step fails, the raw transcription is pasted instead.
The user never loses their words.

---

## 7. Key Data Locations (Windows)

| Data | Path |
|---|---|
| App data root | `%APPDATA%\SilentVoice\` |
| STT (Whisper) models | `%APPDATA%\SilentVoice\models\ggml-<id>.bin` |
| LLM (GGUF) models | `%APPDATA%\SilentVoice\llm\<id>.gguf` |
| Transcription history | `%APPDATA%\SilentVoice\history.json` |
| Temp audio | `%APPDATA%\SilentVoice\audio\last.wav` |
| TTS temp WAV | `%APPDATA%\SilentVoice\audio\tts.wav` |
| TTS (Piper) voices | `%APPDATA%\SilentVoice\tts\<id>.onnx` + `<id>.onnx.json` (pair required) |
| Error log | `%APPDATA%\SilentVoice\logs\silent-voice.log` (rotates to `.old` at 2MB) |
| Settings | localStorage (key: `silent-voice-settings`) |

---

## 8. CRITICAL: Things That Must Never Be Changed

These are hard-won fixes. Reverting them will break the app in ways that took many hours to debug.

### 8.1 Overlay Window — OPAQUE, NO transparent=true, NO additional_browser_args

**DO NOT** add `.transparent(true)` to the overlay WebviewWindowBuilder.
**DO NOT** add `.additional_browser_args(...)` to ANY window.

**Why:** On this machine (Intel UHD 620, Windows 11), transparent always-on-top WebView2 windows are flagged as "occluded" by Windows after ~3–5 seconds and WebView2 stops rendering → pill becomes invisible. `additional_browser_args` was tried as a fix and made things worse (broken render, pill never shows at all). This is a confirmed WebView2/Windows limitation on this hardware. The only reliable solution is an **opaque** window.

The overlay is currently:
- `transparent(false)` — opaque, always visible
- `shadow(false)` — shadow off; DWM rounds corners instead (round_corners() in overlay.rs)
- `always_on_top(true)` + keep-alive loop every 2s
- Win32_Graphics_Dwm feature in Cargo.toml for DWM corner rounding

### 8.2 Overlay Drift — shadow(false) Must Stay Off

The drift bug (pill creeping downward with each resize) was caused by `shadow(true)` inflating the `outer_size()` measurement used to find the window center. With shadow off, `outer_size == inner_size` and the center-anchored resize is exact. **Keep shadow(false).**

### 8.3 whisper.cpp CLI Flags — No Value for Boolean Flags

`--no-timestamps` is a toggle flag. **Do not** pass a value after it (`--no-timestamps false` crashes). Current call in whisper.rs:
```
-m <model> -f <audio> -t <threads> -l <lang> --no-timestamps
```

### 8.4 DXGI GetDesc1() — Returns Result, Not Out-Pointer

```rust
// CORRECT:
if let Ok(desc) = adapter.GetDesc1() { ... }
// WRONG (compile error):
let mut desc = DXGI_ADAPTER_DESC1::default();
adapter.GetDesc1(&mut desc)  // method takes 0 arguments
```

### 8.5 Local LLM Source = "local" Means Bundled llama.cpp, NOT Ollama

When `mode_source == "local"` in the pipeline, it calls `run_local_llm()` which starts the **bundled `llama-server`** from `exe_dir/llama/`. Ollama is not required and not used (though `llm/ollama.rs` is kept). Do not reroute "local" to Ollama.

### 8.6 reqwest Must Have the "json" Feature

```toml
reqwest = { version = "0.12", features = ["stream", "json", "rustls-tls"], default-features = false }
```
Without `"json"`, `.json()` and `.json::<T>()` don't compile.

### 8.7 Model Source "api" Uses mode_base_url + mode_api_key from RuntimeConfig

The pipeline reads these from `AppState.config` (set by `set_active_mode` command). They are pushed from `useRuntimeSync` hook whenever the active mode or providers change. Do not hardcode provider URLs in Rust.

---

## 9. Overlay Transitions (How the Smooth Animation Works)

**The pill window is a FIXED size (68 × 22) for ALL dictation states.** Never
re-introduce a window-resize tween between idle/recording/processing: WebView2
window resizing on Windows is unavoidably janky/flickery (tauri#4236, #6322,
discussion #2970) — a multi-step tween was tried and looked like glitching.

- All idle/recording/processing transitions are **CSS animations inside the
  window** (RecordingOverlay.tsx): the same 7 bars are always rendered and
  morph via `transition-all` — idle = 2.5px muted dots, recording = orange
  `sv-bar` pulsing, processing = medium muted `sv-bar-slow` pulsing. The pill
  fill is near-black `#0e1116` with a subtle `border-white/10` outline
  (user-approved reference look; sized down from 96×26 at user request).
- **Input sensitivity (Discord-style slider, Settings → Dictation):**
  `settings.input_sensitivity` (0–100, default 50) → `set_behavior` →
  `RuntimeConfig.input_sensitivity` → `audio/gate.rs::trim_silence()` runs in
  the pipeline BEFORE write_wav: RMS gate over 30ms frames, trims sub-threshold
  lead/tail (wind, hum) with ~240ms padding; a clip with no frame above
  threshold is skipped entirely (logged, no error banner). Honest limitation:
  an RMS gate cannot remove wind that is as LOUD as speech — it only cuts
  quieter-than-speech noise. Unit tests in the module.
- `animate_resize()` in overlay.rs now snaps in ONE step and is used only for
  the right-click menu (190 × 152), which reads as a popup opening.
- `overlay_resize_gen` is still bumped for backward compatibility.

(Storage-locations UI was removed from Settings at the user's request; the
`get_data_location`/`set_data_location`/`pick_folder` commands and registry
plumbing remain — harmless dead code, same convention as overlay_opacity.)

---

## 10. AI Mode System

Modes live in `src/services/modes.ts` (built-in) and `settingsStore.modes` (custom).

```
Mode {
  id, name, icon,
  system_prompt,       // sent to LLM as system message
  model_source,        // "none" | "local" | "api"
  model_id,            // GGUF id for local, or model name for api
  provider_id?,        // points to ApiProvider in settingsStore.providers (api source only)
  builtin              // built-ins can be viewed but not deleted
}
```

**Built-in model_id is `"llama-3.2-1b-instruct-q4"`** — this is the GGUF stored at
`%APPDATA%\SilentVoice\llm\llama-3.2-1b-instruct-q4.gguf`.

The `tidy_ai_output()` function in `hotkey.rs` strips common preamble lines ("Here is the cleaned text:") and surrounding quotes from small-model responses before pasting.

---

## 11. What's Left to Build

### Phase 5 — Always-Listening (Not started)
- Silero VAD: ~1MB ONNX model, detects speech start/end (16kHz audio chunks)
- openWakeWord: ONNX, wake word detection ("hey jarvis" etc.)
- Integration point: continuous mic loop in Rust → VAD → optional wake word check → trigger same pipeline as hotkey on_released
- Requires `ort` crate (ONNX Runtime for Rust)

### Phase 7 — Polish + Installer (v0.1.4 — complete)

**DONE:**
- First-launch onboarding wizard (`src/components/onboarding/Onboarding.tsx`) — 4 steps, hardware-based model recommendation, skippable
- App icon: `app-icon.svg` (black rounded square, orange waveform bars, white center peak). Regenerate platform icons: `npx tauri icon app-icon.svg`
- Error logging: `logging.rs` → `%APPDATA%\SilentVoice\logs\silent-voice.log`
- NSIS installer: `npx tauri build` → `Install/Silent Voice_<version>_x64-setup.exe`
- Resource bundling fix: `tauri.conf.json` uses **object-map** form so DLLs land next to exe:
  ```json
  "resources": {
    "sidecars/*.dll": "./",
    "sidecars/llama/": "llama/",
    "sidecars/piper/": "piper/",
    "sidecars/sherpa/": "sherpa/"
  }
  ```
  Do NOT revert to the array form — it broke local STT + LLM in installed builds.
- Auto-updater (v0.1.4): Tauri v2 updater plugin integration (`tauri-plugin-updater` and `tauri-plugin-process`).

**REMAINING:**
- None.

---

## 12. Tauri Commands Reference

All commands are registered in `src-tauri/src/lib.rs` → `tauri::generate_handler![]`.

| Command | What it does |
|---|---|
| `get_hardware_info` | Returns CPU/RAM/GPU info |
| `list_input_devices` | Lists mic device names |
| `update_runtime_config` | Sets model_id, language, audio_device, vocabulary + cloud STT fields |
| `set_text_replacements` | Spoken trigger → inserted text pairs (applied before paste) |
| `set_behavior` | live_preview / toggle_mode / number_formatting flags |
| `set_app_profiles` | Per-app profile rules (resolved mode configs, matched on exe name) |
| `set_hotkey` | Unregisters old, registers new global shortcut |
| `set_active_mode` | Sets mode_id/source/prompt/model/base_url/api_key |
| `ollama_status` | Checks if Ollama is running |
| `ollama_generate` | Generate via Ollama (kept for potential future use) |
| `api_generate` | Generic OpenAI-compatible chat call |
| `api_list_models` | Fetch model list from provider |
| `list_downloaded_llm` | Lists downloaded GGUF model ids |
| `download_llm_model` | Download GGUF with progress events |
| `delete_llm_model` | Delete a GGUF model |
| `local_llm_generate` | Run text through bundled llama-server |
| `list_downloaded_models` | Lists downloaded Whisper models |
| `download_model` | Download Whisper GGML with progress events |
| `delete_model` | Delete a Whisper model |
| `load_history` | Read history.json |
| `save_history` | Write history.json |
| `clear_history` | Empty history.json |
| `start_recording` | Start mic capture manually |
| `stop_and_transcribe` | Stop capture, transcribe, return text |
| `paste_text` | Paste text at cursor |
| `hide_overlay` | Hide pill + set user-hidden flag |
| `show_overlay` | Show pill + clear user-hidden flag |
| `set_overlay_size` | Animated resize of the overlay window |
| `set_overlay_opacity` | (harmless dead code — transparency was dropped) |
| `quit_app` | Exit the app |
| `set_autostart` | Write/remove HKCU Run key (bool) |
| `get_autostart` | Read HKCU Run key → bool |
| `list_downloaded_tts` | Lists downloaded Piper voice ids (complete .onnx+.json pairs only) |
| `download_tts_model` | Download Piper voice pair (.onnx.json first, then .onnx) with progress events |
| `delete_tts_model` | Delete a Piper voice pair |
| `set_tts` | Set active voice id + TTS hotkey in RuntimeConfig; re-registers global shortcut |
| `speak_text` | Synthesize + play explicit text string (Settings "Test voice" button) |
| `stop_tts` | Stop active TTS playback |

---

## 13. Tauri Events (Frontend ↔ Backend)

| Event | Direction | Payload | Purpose |
|---|---|---|---|
| `pipeline://state` | Rust → Frontend | `{state: "idle"\|"recording"\|"processing"\|"listening"}` | Updates overlay + Home status |
| `pipeline://result` | Rust → Frontend | `{raw_text, processed_text, model_id, duration_ms}` | Adds to history |
| `pipeline://error` | Rust → Frontend | `string` | Shows error banner (also logged to file, see §16) |
| `pipeline://partial` | Rust → Overlay | `string` | Live-preview text while recording (when enabled) |
| `download://progress` | Rust → Frontend | `{model_id, downloaded_bytes, total_bytes, status, error?}` | Progress bars in Model Store |
| `overlay://opacity` | Rust → Overlay | `number` | (dead code — kept but not used) |
| `tray://toggle-record` | Rust → Frontend | `()` | Tray menu record toggle |
| `tray://toggle-listen` | Rust → Frontend | `()` | Tray menu always-listen toggle |
| `tts://state` | Rust → Frontend | `"synthesizing"\|"speaking"\|"idle"` | TTS playback state (for future UI indicator) |

---

## 14. Developer Notes

- **Cargo is NOT on the system PATH.** Always use `$USERPROFILE\.cargo\bin\cargo.exe` or add to PATH first.
- **whisper DLLs** must be in `target/debug/` alongside `whisper-cpp.exe`. They're not auto-copied on `cargo build` — copy them manually from `sidecars/` if they go missing.
- **llama DLLs** must be in `target/debug/llama/` alongside `llama-server.exe`. Same issue.
- **Browser preview** (`npm run dev`) works fully with mock hardware data — no Rust needed for UI work.
- **tauriBridge.ts** checks `isTauri()` before every invoke and returns sensible defaults in browser.
- The `overlay_opacity` field in Settings and the `set_overlay_opacity` command are **dead code** — transparency was dropped. Don't remove them (harmless), don't implement new features on top of them.

---

## 15. STT Model Catalog Invariant (READ BEFORE ADDING MODELS)

Every entry in `STT_MODELS` (`src/services/catalog.ts`) **must** have `file` equal to
exactly `` `ggml-${id}.bin` ``. This is not cosmetic — `download_model` in Rust
(`downloader.rs`) saves the downloaded bytes under whatever `file_name` the frontend
passes (`model.file`), but `whisper::transcribe()` always looks the model up via
`registry::model_path(model_id)`, which independently recomputes `ggml-{model_id}.bin`
(`registry.rs::model_file_name`). If `file` and `id` diverge, the model downloads fine
but transcription can never find it. (Found and fixed one existing instance of this:
`large-v1` was shipping with `file: "ggml-large.bin"`.)

The `url` field is what decouples this from reality — it can point at whatever the
model's real filename is on Hugging Face (or elsewhere); only the **locally saved**
name is forced to the `ggml-<id>.bin` convention. When adding a new model, always
verify the `url` actually resolves (`curl -I -L <url>` → `200`) before adding it —
do not trust an AI-generated or web-searched URL without checking it directly, since a
broken URL silently breaks that model's download button.

As of now the catalog has ~49 STT models: OpenAI's official Whisper family, two
distil-whisper variants, and ~35 GGML-format fine-tunes for other
languages/companies (BELLE-2 for Chinese, Kotoba Technologies/ReazonSpeech/Aratako for
Japanese, VinAI's PhoWhisper for Vietnamese, KBLab/NB-AI-Lab for Swedish/Norwegian,
Bofeng Huang for French, community fine-tunes for Korean/Cantonese/Polish/
Portuguese/German/Russian/Arabic/Hebrew, plus a couple of general-purpose quantized
multilingual large-v2 variants). **Hard constraint:** whisper.cpp only runs
whisper-architecture models converted to its custom GGML `.bin` format — non-Whisper
architectures (NVIDIA Parakeet/Canary, Moonshine, CTranslate2/faster-whisper, ONNX,
GGUF) are NOT usable here without a separate runtime integration.

### Custom vocabulary / accuracy-boosting (Settings → "Custom vocabulary")

`Settings.custom_vocabulary` (comma-separated words/names/jargon) flows:
`settingsStore` → `useRuntimeSync` → `updateRuntimeConfig(..., vocabulary)` →
`RuntimeConfig.vocabulary` (Rust) → `whisper::transcribe()`, which passes it to the
sidecar as `--prompt "<vocabulary>"`. This is whisper.cpp's native
`initial_prompt` mechanism — it biases the decoder toward those tokens, which is the
same underlying technique commercial tools like SuperWhisper/Wispr Flow use for their
"custom dictionary" feature (their apps additionally run an AI correction pass on top;
this project intentionally does NOT — it stays a pure STT-layer improvement, no LLM
involved). Known limitation inherited from whisper.cpp itself: the prompt only
influences roughly the first 30 seconds of audio in a clip, since it gets overwritten
by decoded output after that — irrelevant for typical push-to-talk dictation length.

There is no online/continuous learning — whisper.cpp has no training loop, and
fine-tuning on-device on this CPU-only hardware is not practical. The closest
approximation IS built: History → Edit on any entry saves a correction and diffs it
against the displayed text; genuinely new words (3+ chars, letters, max 10/edit) are
auto-appended to `custom_vocabulary` (see `newWordsFromCorrection` in History.tsx).

---

## 16. Behavior Features (added after §15)

All toggles live in Settings and are pushed to Rust via `set_behavior` /
`set_app_profiles` / `set_text_replacements` (useRuntimeSync keeps them in sync).

- **Double-tap lock (`toggle_mode`, default ON):** tap the hotkey twice quickly
  (<300ms tap, <450ms gap) → recording locks on hands-free; a single press stops &
  pastes. Long hold = classic push-to-talk, unchanged. Implemented as a tap state
  machine in `hotkey.rs` (`TapState` in lib.rs); OS key-repeat is filtered via
  `key_down`. A lone quick tap defers finalize ~480ms to wait for a second tap.
  The tray toggle uses `start_capture`/`stop_capture` which BYPASS tap bookkeeping —
  don't route tray through on_pressed/on_released (it would wedge `key_down`).
  (Live preview was prototyped then removed at the user's request — do not
  re-add `pipeline://partial` / a live loop unless asked.)
- **Smart number formatting (always on, no toggle):**
  `system/textfmt.rs::format_numbers`, applied unconditionally in the pipeline
  AFTER text replacements so digits in replacement output are never touched.
  **Product decision (user-mandated): ALL spelled numbers become digits,
  including 0–9** ("one"→"1", "one two three"→"1 2 3"). Also: "five percent"→
  "5%", years ("twenty twenty six"→2026), decimals ("three point five"→3.5).
  Known accepted trade-off: "no one knows"→"no 1 knows". Unit tests in module.
- **GPU toggle (Settings → Performance) is wired:** `use_gpu` flows
  settingsStore → updateRuntimeConfig → RuntimeConfig → whisper.rs, which
  passes `-ng` (force CPU) when OFF. When ON, the sidecar uses its GPU backend
  only if the bundled whisper binary was built with one — the toggle can't
  create GPU support that isn't compiled in.
- **First-launch onboarding** (`src/components/onboarding/Onboarding.tsx`):
  shows until `settings.onboarded` is true (skippable). Step 2 offers 5 curated
  starter models with a hardware-based recommendation. IMPORTANT lesson from
  the user's own machine: local Whisper speed is bound by GPU/CPU compute, NOT
  RAM — recommendation logic keys off `gpu_vram_gb` (≥4GB → distil-large-v3.5,
  ≥2GB → small.en) and falls back to base.en/tiny.en for CPU-only machines.
- **Branding:** accent color is ORANGE (`#f97316`, hover `#ea580c`; light theme
  `#ea580c`/`#c2410c`) — was purple, changed per user. App icon source is
  `app-icon.svg` at the project root (black rounded square, orange waveform
  bars, white center peak); regenerate all platform icons with
  `npx tauri icon app-icon.svg`.
- **Per-app profiles:** Settings rules "app name contains X → mode Y". Frontend
  resolves each rule's mode to a full config (`resolveMode` in useRuntimeSync) and
  pushes via `set_app_profiles`. `on_pressed` captures the foreground exe basename
  (`system/foreground.rs`, Windows APIs) into `AppState.active_app`; the pipeline
  overrides mode_* when a rule's `app_match` substring-matches it.
- **Error logging:** `logging.rs` appends to
  `%APPDATA%\SilentVoice\logs\silent-voice.log` (epoch-seconds timestamps, 2MB
  rotation to `.old`). Pipeline errors go through `report_error()` in hotkey.rs
  which both logs and emits `pipeline://error`.
- **Window lifecycle (do not regress):** the main window has
  `decorations: false` + custom `Titlebar.tsx`. Closing it HIDES it
  (`on_window_event` CloseRequested → prevent_close + hide in lib.rs) so the
  tray "Open Dashboard" can always bring it back; quit only via tray.
  `tauri-plugin-single-instance` (registered FIRST) focuses the existing
  dashboard when the exe is launched again — never two processes/pills.
  `toggleMaximize` needs `core:window:allow-toggle-maximize` in
  capabilities/default.json (maximize/unmaximize alone are NOT enough).
- **Autostart:** `system/autostart.rs` writes an HKCU Run key
  (`set_autostart`/`get_autostart`); Settings hydrates the toggle from the
  registry on mount (registry is truth, not localStorage).
- **Guide page:** `dashboard/Guide.tsx` ("How to use" nav item) — user-facing
  docs with visual pill mockups. Keep it in sync when behavior changes.
- **Installer:** `npx tauri build` → NSIS setup exe; a copy is kept in
  `Install/` at the project root for easy access. `bundle.resources` uses the
  object-map form so whisper DLLs land NEXT TO the exe and llama files in
  `llama/` — the array form put them under `sidecars/` where the runtime
  paths never looked (local STT + LLM silently broken in installed builds).
- **Read aloud (TTS, added v0.1.2):** bundled Piper engine (fast, offline, CPU-friendly)
  (`sidecars/piper/` → installed `piper/` next to the exe, adds ~38 MB to the install; release
  2023.11.14-2). Select text anywhere → TTS hotkey (default Ctrl+Alt+S,
  Settings → "Read aloud" — allows changing hotkey, picking downloaded voice from dropdown, and has a ▶ Play sample button to test instantly) → `system/tts.rs` copies the selection via Ctrl+C
  (clipboard preserved), synthesizes WAV via
  `piper.exe --model <voice.onnx> --output_file`, plays it with rodio; hotkey
  again = stop (toggle). Voices are Piper .onnx + .onnx.json PAIRS stored in
  `%APPDATA%\SilentVoice\tts\<id>.onnx[.json]` — both files required
  (`download_tts_model` fetches the pair; `list_downloaded_tts` only counts
  complete pairs). Catalog: `TTS_MODELS` in catalog.ts, 29 verified English
  Piper voices (mix of male/female, American/British; tiers Fast [~61 MB], Balanced,
  Natural HD [~109-116 MB]), Model Store third tab "Text-to-Speech". STT vs TTS are clearly separated
  (different tabs, different icons like speaker glyph vs provider logos, different wording). The global-shortcut
  handler in lib.rs routes by comparing the fired shortcut against
  `cfg.tts_hotkey` — TTS fires on Pressed only; everything else goes to the
  dictation press/release pipeline. NOTE: piper.exe was bundled but never
  executed during development (auto-mode restriction) — the flags follow the
  official README; first real run is the user's.
- **Second TTS engine — sherpa-onnx (added for Bangla, v0.1.4):** Piper has no
  Bangla voices at all (verified against its voices.json; no community
  Piper-format ones exist either), so a second engine is bundled:
  `sidecars/sherpa/` → installed `sherpa/` next to the exe (3 files, ~19 MB:
  `sherpa-onnx-offline-tts.exe` + onnxruntime DLLs, from k2-fsa/sherpa-onnx
  v1.13.4 win-x64-shared-MD-Release; Apache-2.0). Sherpa voices are k2-fsa
  `.tar.bz2` archives (GitHub `tts-models` release tag) whose top-level folder
  equals the voice id; `download_tts_model` treats an EMPTY `url_json` as the
  sherpa-archive marker: downloads the archive, extracts with the system `tar`
  (bsdtar, ships with Win10+) into `%APPDATA%\SilentVoice\tts\<id>\`, deletes
  the archive. `registry::list_downloaded_tts` counts both flat Piper pairs
  AND dirs containing `tokens.txt` + a `.onnx`. Synthesis routing:
  `tts.rs::resolve_voice()` → `Voice::Piper | Voice::Sherpa`.
  **CRITICAL — sherpa synthesis MUST go through the C-API FFI
  (`system/sherpa.rs` → `sherpa-onnx-c-api.dll`), NEVER the CLI exe:** the
  CLI uses narrow `main(char* argv[])`, so on Windows the CRT converts the
  command line to the ANSI codepage, which cannot represent Bengali (or any
  non-Latin script) — the model receives `????` and speaks gibberish. This
  bug shipped once and was diagnosed the hard way. The FFI mirrors the
  v1.13.4 c-api.h struct layout exactly (do not upgrade the DLL without
  re-checking the header) and PRE-LOADS `sherpa/onnxruntime.dll` by absolute
  path before the c-api DLL — otherwise Windows can resolve the dependency
  to an incompatible onnxruntime (OS-shipped or Piper's) → access violation.
  Unit test `sherpa::tests::bengali_text_synthesizes` covers real Bengali
  text through the FFI (needs DLLs copied to `target/debug/deps/sherpa/`).
  A second sherpa layout exists: `url_json` ending in `tokens.txt` = two-file
  voice (MMS conversions: `dir/model.onnx` + `dir/tokens.txt`).
  Frontend: `TtsModel.engine: "piper"|"sherpa"` + `language` on every entry;
  catalog ~50 voices (19 non-English Piper languages + 2 Bangla sherpa
  voices — Mitra/Coqui and Meta MMS; every URL curl-verified → 200, same
  invariant as §15). `vits-mimic3-bn-multi_low` was tried and REMOVED — its
  espeak phonemization fails on Bengali (sub-second gibberish blip). TTS tab
  has search + language filter; sort order is active → downloaded → quality.
- **Proofreading (Grammarly Phase 1, v0.1.4):** `harper-core` 2.5.0 (Apache-2.0,
  pure Rust, `concurrent` feature for thread-safety) compiled into the binary.
  `src-tauri/src/proofread.rs` → `proofread_text` command → History entries
  render Grammarly-style squiggles (red = spelling, blue = grammar/style,
  hover for message + suggestions; `ProofreadText` in History.tsx). Offsets
  are CHAR indices (Harper `Span<char>`) — frontend must slice via
  `Array.from(text)`, never `text.slice()`. The custom vocabulary doubles as
  the personal dictionary: entries AND their punctuation-split sub-tokens
  (e.g. "whisper.cpp" → "whisper", "cpp") are merged into Harper's dictionary
  so they're never flagged. Unit tests in proofread.rs. English-only by
  design (Harper limitation). The full Grammarly feasibility research lives
  in the "grammarly-report" artifact (phases 2–4: fix-writing hotkey via
  LLM, tone modes, system-wide UIA overlay = deferred).
- **Inline proofreading — system-wide squiggles (Grammarly Phase 2, v0.1.4,
  TOP USER PRIORITY):** squiggles under errors in ANY app's focused text
  field, live. Architecture (validated first in the standalone `uia-probe/`
  prototype at the repo root — keep it, it's the test harness):
  - `system/inline_check.rs`: watcher thread (spawned in lib.rs setup), MTA
    COM (`COINIT_MULTITHREADED` — required for UIA clients), polls
    `GetFocusedElement` every 400ms → TextPattern → `GetText(6000)` →
    `proofread::check(text, cfg.vocabulary)` (re-lint only when text
    changed) → per-issue char ranges mapped to screen rects via
    `MoveEndpointByUnit(TextUnit_Character)` + `GetBoundingRectangles`
    (returns only VISIBLE rects, so scrolled-away text clips itself).
    Harper spans are char indices and UIA moves by character — they align
    for English. Rects re-read EVERY poll so squiggles track
    scroll/typing/window moves.
  - `system/squiggle.rs`: pool of tiny click-through layered Win32 windows
    (WS_EX_LAYERED|TRANSPARENT|NOACTIVATE|TOPMOST|TOOLWINDOW), one per
    flagged word, 4px-tall premultiplied-alpha DIB zigzag via
    `UpdateLayeredWindow`. **Deliberately NOT a webview and NOT one big
    fullscreen overlay** — Grammarly's own architecture; cheap on weak
    iGPUs, and transparent WebView2 is known-broken here (§8.1).
  - Guards: skips our own process (checks the foreground WINDOW's pid, not
    the element's — our dashboard's WebView2 child has a different pid),
    password fields (`CurrentIsPassword`), terminals + password managers
    (IGNORE_EXES), and text > 6000 chars.
  - **Editability gate (do not remove) — two routes:** (A) if the focused
    element exposes a ValuePattern, it's editable unless IsReadOnly=true
    (Notepad, WPF TextBox, Chromium <textarea>/URL bar; read-only browser
    Documents expose ValuePattern with IsReadOnly=true so YouTube/articles
    are rejected). (B) contenteditable / rich editors (Claude desktop's
    ProseMirror, Discord, Slack, WhatsApp) expose NO ValuePattern — for
    these, editable iff the TextPattern's SupportedTextSelection != None.
    NOTE: ProseMirror does NOT implement TextPattern2, so caret-based
    detection (GetCaretRange) does not work there — don't reintroduce it.
    Static read-only labels report SupportedTextSelection_None.
  - Wiring: `RuntimeConfig.inline_proofread` (default ON) ← `set_behavior`
    ← Settings → Dictation → "Inline proofreading" toggle
    (`settings.inline_proofread`).
  - Verified working in Notepad (Win11) and Edge/Chromium textareas.
    Chromium enables its accessibility tree automatically when a UIA client
    queries it — no flags needed.
  - **Hover tooltip + click-to-fix popup (BUILT + verified):** hovering a
    squiggle ~250ms shows a native GDI card (`SVSuggestPopup` in squiggle.rs:
    message + up to 3 suggestions); clicking a suggestion sends a
    `FixRequest` back to the watcher thread (COM apartment rules — only the
    watcher touches UIA), which re-verifies the flagged chars, selects the
    range via UIA `Select()`, and TYPES the replacement (enigo unicode
    input, char-by-char with 8ms gaps — web apps drop faster input). Typed
    input preserves the app's undo stack and never touches the clipboard.
    Physical Ctrl/Shift/Alt are released first so a held modifier can't
    turn typed chars into shortcuts. Popup is WS_EX_NOACTIVATE + answers
    WM_MOUSEACTIVATE with MA_NOACTIVATE so the target keeps focus.
  - **CRLF offset drift (fixed — do not regress):** Harper spans count \r\n
    as 2 chars, but WPF/RichEdit UIA providers move TextUnit_Character over
    it as ONE. `range_for()` in inline_check.rs builds candidate ranges
    (CRLF-collapsed offset first, then raw) and VERIFIES each by reading the
    range's text back before using it — used for BOTH squiggle rects and
    fixes. Without it, anything on line 2+ selects the wrong chars.
  - **Popup z-order:** show_popup() forces SetWindowPos(HWND_TOPMOST) — an
    always-on-top target app otherwise sits above the popup and eats clicks.
  - **Popup design (v2):** WHITE card (user request), 1px #dcdcdc border,
    rounded corners via SetWindowRgn, opens ABOVE the word (falls back below
    near the screen top). Underlines are STRAIGHT 2px lines (zigzag removed,
    user request); spelling red #EF4444, grammar blue #3B82F6. Note: curly
    squiggles seen alongside ours in browsers/Electron are the HOST app's
    native spellchecker — not ours, can't be removed from our side.
  - **Popup rendering (v5, do not regress to WM_PAINT):** the popup is a
    per-pixel-alpha LAYERED window (UpdateLayeredWindow + premultiplied
    ARGB DIB), NOT an opaque region-clipped window — CreateRoundRectRgn has
    1-bit edges and made corners jagged. render_popup() GDI-draws content
    into the DIB, then an SDF pass (radius 24) computes anti-aliased corner
    coverage AND paints the 2px orange border in the same math, so border
    and corner geometry cannot drift (a GDI RoundRect border was tried and
    clipped — GDI's corner param is a DIAMETER, not radius). Hover repaints
    go through a NEEDS_REDRAW atomic polled by the overlay loop.
  - **Popup v4 (layout, from a user-provided mockup):** bold "Spelling
    Insights"/"Grammar Insights" title + subtitle, 48px suggestion rows with
    rounded PEACH hover (#FAE0CE) and thin separators, gray FOOTER BAR with
    inline "Add to Dictionary" (Segoe MDL2 book glyph) and "Dismiss" (X
    glyph) side by side. Hit-testing is x+y aware (`hit_at`; codes 100/101
    for footer actions). "Add to Dictionary" opens a PICKER (popup state
    machine, `PICKER` AtomicBool in squiggle.rs): flagged word + suggestions
    listed, click one → that word is what gets added — user chooses. Picker
    has no footer.
  - **Repetition lint** (proofread.rs): consecutive duplicate words
    ("pop-up pop-up", case-insensitive, whitespace-separated) flagged with
    kind "Repetition", suggestion collapses to a single occurrence. Skipped
    when a Harper issue already overlaps the range. Unit tested.
  - **Dismiss / Add to dictionary rows** (below the suggestions, muted):
    channel enum is `OverlayAction::{Fix, Dismiss, AddToVocab}`. Dismiss =
    session-only in-memory HashSet in the watcher. AddToVocab additionally
    emits `proofread://add-vocab` → useRuntimeSync listener appends to
    settings.custom_vocabulary (deduped case-insensitive, persists, and
    thus also primes Whisper). PRODUCT DECISION: fixes never auto-add to
    vocabulary — adding is always an explicit user click.
  - Verified end-to-end via automated real-UI tests: WPF TextBox
    (single-line + multi-line CRLF), Edge/Chromium textarea, Notepad
    squiggles. WinForms TextBox exposes NO UIA TextPattern — correctly
    skipped. Note for future testing: PowerShell P/Invoke needs
    CharSet=Unicode on FindWindow* or class names silently don't match.
  - NOT YET BUILT (Week 2+): per-app compatibility hardening. The two pink
    blobs that confused testing were Windows' "text cursor indicator"
    accessibility feature, not ours.
- **Auto-updater & Release Process (v0.1.4, shipped):** Background auto-updater powered by Tauri v2 `@tauri-apps/plugin-updater` + `@tauri-apps/plugin-process` on frontend (registered in `src-tauri/src/lib.rs` and permissions in `capabilities/default.json`).
  - *Frontend hook:* `src/services/updater.ts` exports `checkForUpdates()` which calls the plugin's `check()`, downloads/installs, and relaunches the app. `src/App.tsx` calls `checkForUpdates()` silently 5 seconds after `Dashboard` mounts (only in main window, not overlay; no-ops in browser).
  - *Configuration:* `src-tauri/tauri.conf.json` defines `"plugins.updater"` with `endpoints` pointing to `https://github.com/zahidhossin39/Silent-Voice/releases/latest/download/latest.json`, `pubkey` (embedded public signing key), and `"bundle.createUpdaterArtifacts": true`.
  - *Key Generation:* The signing keypair was generated via `npx tauri signer generate` (private key stored locally at `%USERPROFILE%\.tauri\silent-voice.key`, gitignored).
  - *Release Pipeline:* Tag pushes matching `v*` trigger `.github/workflows/release.yml`. CI runs `tauri-apps/tauri-action@v0` to build, sign (using GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), and publish a draft release (`releaseDraft: true`) containing `latest.json`, `.exe`, and `.sig` signatures.
  - *Gotcha:* Published releases MUST have the release label set to "None" (NOT "Pre-release") on GitHub, otherwise the `/releases/latest/` endpoint does not list them and the updater fails to find updates.
  - *Release Steps:*
    1. Bump matching version in three files: `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`.
    2. Commit changes (e.g., `git commit -am "bump to vX.Y.Z"`).
    3. Tag the commit: `git tag vX.Y.Z`.
    4. Push: `git push origin vX.Y.Z`.
    5. Review the draft release created by GitHub Actions. Ensure the label is "None", then click "Publish release".
  - *Caveat:* Pre-v0.1.4 instances lack update-checking code and must be manually reinstalled once from the v0.1.4 (or later) installer to receive future updates.
