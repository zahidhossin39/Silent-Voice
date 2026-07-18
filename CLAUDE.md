# Silent Voice — Agent Summary

Free, local-first voice-to-text app for Windows. Hold hotkey → speak → release →
transcribed (optionally AI-processed) text pastes at the cursor. Tauri v2
(Rust backend + React 19/TS/Tailwind frontend), whisper.cpp sidecar for STT,
bundled llama.cpp for local LLM, Piper + sherpa-onnx for TTS.

**Deep reference:** `docs/HANDBOOK.md` holds the full architecture, subsystem
histories, and the WHY behind every rule below. **Read the matching handbook
section before touching:** overlay/pill, hotkey pipeline, inline proofreading
(squiggles/UIA), TTS engines, model catalogs, the updater, or release steps.

## Layout

- `src/` — React frontend. `components/dashboard/` (7 tabs), `components/overlay/`
  (pill window), `stores/` (Zustand), `services/tauriBridge.ts` (all invokes,
  browser-safe), `services/catalog.ts` (model catalogs).
- `src-tauri/src/` — Rust. `lib.rs` (all commands + AppState), `system/`
  (hotkey, overlay, paste, tts, sherpa FFI, inline_check, squiggle, textfmt),
  `audio/`, `transcription/whisper.rs`, `llm/`, `models/`, `proofread.rs`.
- `src-tauri/sidecars/` — whisper (Vulkan build), `llama/`, `piper/`, `sherpa/`.

## Run / Build

```powershell
npm run dev                    # frontend only, browser, mock data
$env:PATH += ";$env:USERPROFILE\.cargo\bin"   # cargo NOT on system PATH
npm run tauri:dev              # full dev app
npx tauri build                # NSIS installer → Install/
```

After `cargo build`, whisper DLLs must be copied from `sidecars/` into
`target/debug/` (and `sidecars/llama/` → `target/debug/llama/`) — not automatic.

## Hard rules (never change — full rationale in HANDBOOK §8, §16)

1. Overlay window stays **opaque**: no `.transparent(true)`, no
   `additional_browser_args`. Transparent always-on-top WebView2 goes invisible
   on this hardware.
2. Overlay `shadow(false)` stays off (shadow inflated outer_size → pill drift).
   Fixed 68×22 size; state transitions are CSS inside the window, never window
   resize tweens.
3. whisper.cpp boolean flags take no value (`--no-timestamps`, never
   `--no-timestamps false`).
4. Mode source `"local"` = bundled llama-server, NOT Ollama.
5. reqwest keeps the `"json"` feature.
6. `tauri.conf.json` `bundle.resources` stays in object-map form (array form
   broke installed builds).
7. STT catalog invariant: every `STT_MODELS` entry has `file === "ggml-<id>.bin"`
   and a curl-verified URL (HANDBOOK §15).
8. Sherpa TTS synthesis goes through the C-API FFI (`system/sherpa.rs`), never
   the CLI exe (ANSI codepage garbles non-Latin text). Pre-load
   `sherpa/onnxruntime.dll` by absolute path first.
9. Inline proofread: keep the editability gate, the CRLF `range_for()`
   verification, and the layered-window popup rendering (no WM_PAINT regions).
   Squiggle overlays are tiny Win32 layered windows, never a webview.
10. Tray record toggle bypasses the hotkey tap state machine — don't route it
    through on_pressed/on_released.
11. Main window close = hide (tray owns quit); single-instance plugin stays
    registered first.

## Key facts

- Data root `%APPDATA%\SilentVoice\` (models, llm, tts, history.json, logs).
  Settings live in localStorage key `silent-voice-settings`.
- Pipeline: hotkey release → trim_silence gate → whisper sidecar →
  collapse_repeated_words → optional LLM → replacements → format_numbers →
  paste. Any AI failure falls back to raw text — user never loses words.
- Accent color orange `#f97316`. Icon source `app-icon.svg`
  (`npx tauri icon app-icon.svg` regenerates).
- Updater is non-silent: sidebar "Update available" button installs on click;
  Settings' manual check installs immediately.
- Release: bump version in `package.json` + `tauri.conf.json` + `Cargo.toml`,
  commit, `git tag vX.Y.Z`, push tag → CI builds draft release → set label
  "None" (not pre-release) → publish. Details in HANDBOOK §16.
- Phase 5 (always-listening VAD/wake-word) is the only unbuilt phase.

**Current state:** v0.1.6 released. In progress since: hyphenated-word
duplication fix (`format_numbers`), Win10 rounded-corner fallback for the pill
(`SetWindowRgn`), this CLAUDE.md split (full text now in `docs/HANDBOOK.md`).
