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

**Original spec:** `Silent-Voice-Complete-Build-Plan.md` in the project root.

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
├── Silent-Voice-Complete-Build-Plan.md  ← original spec
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
│   │   ├── dashboard/               ← 6 tabs: Home, ModelStore, Modes, ApiKeys, Settings, History
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
│   │   ├── useRuntimeSync.ts        ← keeps Rust RuntimeConfig in sync with settings
│   │   └── useRuntimeSync.ts
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
        ├── audio/
        │   ├── mod.rs
        │   └── capture.rs           ← cpal mic → 16kHz WAV + device listing
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
        │   ├── downloader.rs        ← download_model (STT) + download_llm_model (GGUF) with progress events
        │   └── registry.rs          ← data dir paths, model file names, list_downloaded*
        └── system/
            ├── mod.rs
            ├── hardware.rs          ← CPU/RAM/GPU detection (sysinfo + DXGI)
            ├── hotkey.rs            ← on_pressed / on_released pipeline; tidy_ai_output
            ├── overlay.rs           ← overlay window creation + animate_resize tween
            ├── paste.rs             ← arboard + enigo paste-at-cursor
            └── tray.rs              ← system tray menu
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
| 7 | Polish + Windows installer + onboarding | ❌ Not started |

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

## 9. Overlay Animate Resize (How the Smooth Transition Works)

The pill window resizes between states via a Rust async tween — **not CSS** (because the window is opaque, CSS can't animate the window boundary):

- `animate_resize(app, target_w, target_h)` in `overlay.rs`
- 10 steps × 10ms = ~100ms total, ease-out cubic: `e = 1.0 - (1.0 - t)³`
- Generation counter (`overlay_resize_gen: AtomicU64`) cancels superseded tweens
- Each step calls `apply_size()` which resizes + repositions centered on the current drag position

**Sizes:**
- Idle: 54 × 20 px — single short line
- Recording/Processing: 96 × 26 px — red dot + waveform bars
- Right-click menu: 190 × 152 px

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

### Phase 7 — Polish + Installer (Not started)
**Packaging issue to fix:** whisper DLLs and the `llama/` folder need to be beside the exe
in the installed app. Current `tauri.conf.json` has `"resources": ["sidecars/*.dll", "sidecars/llama/*"]`
but the runtime paths in Rust assume they're next to the exe. Validate with `tauri build` and adjust
resource destination paths in `tauri.conf.json` if needed.

**Other Phase 7 items:**
- First-launch onboarding wizard (device scan → model recommendations → hotkey setup)
- Auto-updater
- NSIS/MSI installer via `tauri build`
- App icon branding polish
- Error logging to file

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
