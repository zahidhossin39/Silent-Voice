# Silent Voice

A free, open-source, **local-first** voice-to-text desktop app — like Wispr Flow / SuperWhisper, but unlimited, private, and offline by default.

Hold a hotkey, speak, release → your words are transcribed by a local Whisper model and pasted at your cursor. No subscription, no word limits, no telemetry. Optional cloud APIs (OpenAI, Anthropic, OpenRouter, …) if you want them.

## Status

**Usable day to day.** Everything below works except always-listening.

| Phase | Goal | State |
|-------|------|-------|
| 1 | Foundation: record → transcribe → clipboard, tray, model downloader, hardware detection | ✅ |
| 2 | Core UX: paste-at-cursor, recording overlay, history, STT presets | ✅ |
| 3 | AI processing modes (bundled llama.cpp) | ✅ |
| 4 | Model store + hardware recommendations | ✅ |
| 5 | Always-listening (Silero VAD + wake word) | 🔜 Not started |
| 6 | Cloud API integration (OpenAI, Anthropic, OpenRouter) | ✅ |
| 7 | Polish, onboarding, installer, auto-update | ✅ |

Also shipped beyond the original plan: read-aloud (Piper + sherpa-onnx),
inline proofreading with live spelling/grammar underlines in any app, and
neural grammar correction.

## Tech stack

- **Tauri v2** (Rust backend + React 19 / TypeScript / Tailwind v4 frontend)
- **whisper.cpp** as a bundled sidecar for speech-to-text (CPU, AVX2; Vulkan for iGPUs)
- **llama.cpp** bundled for local AI text processing (or Ollama / cloud APIs if you prefer)
- **Piper** + **sherpa-onnx** for read-aloud, **ONNX Runtime** for grammar models
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
  src/llm/           local llama-server + cloud providers
  src/models/        download manager + storage registry
  src/system/        hotkey, overlay, paste, tray, TTS, inline proofreading
```

## Installation

Download the latest installer from the [GitHub Releases](https://github.com/zahidhossin39/Silent-Voice/releases) page.

Silent Voice checks for updates on startup and shows an **"Update available"** button in the sidebar — updates install when you click it, never silently. You can also check manually from Settings. Installs from before v0.1.4 have no updater and need one manual reinstall to start receiving updates.

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

MIT — see [LICENSE](LICENSE).

### Model licences

The MIT licence covers **this application's code only**. Models are not bundled —
you download the ones you want from inside the app, and each carries its own
licence from its original author.

| Model | Licence |
|---|---|
| Whisper (speech-to-text) | MIT for OpenAI's models; community fine-tunes in the catalogue vary — check the model's page |
| LLM models (AI modes) | Varies per model (Llama, Qwen, Mistral… each has its own terms) |
| Piper voices (read-aloud) | MIT |
| Other TTS voices | Varies per voice |
| Silero VAD | MIT |
| **CoEdIT** (grammar correction) | **CC BY-NC 4.0 — non-commercial use only** |
| **GECToR** (context grammar) | **Non-commercial use only** |

⚠️ **The two grammar models are restricted to non-commercial use.** CoEdIT is
[CC BY-NC 4.0](https://huggingface.co/grammarly/coedit-large), and the GECToR
checkpoint is an [unofficial reimplementation](https://huggingface.co/gotutiyan/gector-roberta-base-5k)
whose author states "Only non-commercial purposes". If you use Silent Voice
commercially, do not download those two models — everything else keeps working
without them.

GECToR is [GECToR – Grammatical Error Correction: Tag, Not Rewrite](https://aclanthology.org/2020.bea-1.16/)
(Omelianchuk et al., BEA 2020); the ONNX build Silent Voice downloads is
[here](https://huggingface.co/Zaid-Hossain/gector-roberta-onnx).
