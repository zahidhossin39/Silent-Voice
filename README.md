# Silent Voice

A free, open-source, **local-first** voice-to-text desktop app — like Wispr Flow / SuperWhisper, but unlimited, private, and offline by default.

Hold a hotkey, speak, release → your words are transcribed by a local Whisper model and pasted at your cursor. No subscription, no word limits, no telemetry. Optional cloud APIs (OpenAI, Anthropic, OpenRouter, …) if you want them.

## Status

🚧 **In active development.**

| Phase | Goal | State |
|-------|------|-------|
| 1 | Foundation: record → transcribe → clipboard, tray, model downloader, hardware detection | ✅ Code complete |
| 2 | Core UX: paste-at-cursor, recording overlay, history (SQLite), STT presets | 🔜 |
| 3 | AI processing modes (Ollama / llama.cpp) | 🔜 |
| 4 | Model store + hardware recommendations | ✅ UI done · backend wired |
| 5 | Always-listening (Silero VAD + openWakeWord) | 🔜 |
| 6 | Cloud API integration | 🟡 UI done |
| 7 | Polish, onboarding, installer | 🔜 |

## Tech stack

- **Tauri v2** (Rust backend + React 19 / TypeScript / Tailwind v4 frontend)
- **whisper.cpp** as a bundled sidecar for speech-to-text (CPU, AVX2; Vulkan for iGPUs)
- **Ollama** / **llama.cpp** for optional AI text processing
- `cpal` audio capture, `sysinfo` + DXGI hardware detection, `arboard` + `enigo` paste-at-cursor

## Project layout

```
src/                 React frontend (dashboard, model store, modes, settings…)
  components/        UI components
  services/          model catalog, recommendation engine, Tauri bridge
  stores/            Zustand state (settings, models, history)
src-tauri/           Rust backend
  src/audio/         cpal microphone capture → 16 kHz WAV
  src/transcription/ whisper.cpp sidecar wrapper
  src/models/        download manager + storage registry
  src/system/        hardware detection, paste, system tray
```

## Installation

To install Silent Voice, download the latest installer from the [GitHub Releases](https://github.com/zahidhossin39/Silent-Voice/releases) page. Once installed, the application will automatically check for and apply updates silently in the background on startup (v0.1.4+). Note that existing installations built or installed before v0.1.4 lack update capabilities and must be manually reinstalled once to begin receiving automatic updates.

## Develop

### Frontend only (no Rust required)

The full dashboard UI runs in the browser with mock hardware data:

```bash
npm install
npm run dev          # http://localhost:1420
```

### Full desktop app

Requires the **Rust toolchain** and, on Windows, the **MSVC C++ Build Tools**
(plus WebView2, preinstalled on Windows 11).

```bash
# one-time toolchain setup (Windows)
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools   # "Desktop development with C++"

npm install
npm run tauri:dev    # launches the native app
npm run tauri:build  # produces an NSIS installer
```

Models, history, and audio are stored under `%APPDATA%\SilentVoice\`.

## Privacy

Audio never leaves your device unless you explicitly configure and enable a
cloud provider. Everything works fully offline once a model is downloaded.

## License

MIT (planned).
