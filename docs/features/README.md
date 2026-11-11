# Feature docs

One file per area of the app, written 2026-11-12 from the code (v0.2.0). Each
file lists the features, how they work (files, commands, events), which parts
depend on the OS, a mobile portability table, and where `docs/HANDBOOK.md` is
out of date. Where a feature doc and the handbook disagree, the feature doc is
newer.

| File | Covers |
|---|---|
| [dictation-pipeline.md](dictation-pipeline.md) | Hotkey, audio, speech engines, text clean-up, paste, pill, history, Home stats |
| [ai-and-proofreading.md](ai-and-proofreading.md) | AI modes, local and cloud LLM, per-app styles, grammar models, inline proofreading |
| [notetaker-and-file-transcription.md](notetaker-and-file-transcription.md) | Meeting recorder, speakers, summaries, Transcribe a file, transcript storage |
| [models-and-tts.md](models-and-tts.md) | Model catalogs, Model Store, downloads, every engine and how it runs, read-aloud |
| [app-shell-and-frontend.md](app-shell-and-frontend.md) | Every page and setting, the React-to-Rust bridge map, tray, updater, logs, CI |

## What the mobile tables add up to

- **Carries over:** text clean-up, vocabulary, replacements, number formatting,
  AI modes, cloud speech and cloud LLM, the transcript and meeting data model,
  file decoding, history, stats, stores and settings.
- **Needs rework:** every speech, grammar and voice engine (sidecar programs and
  desktop library paths become phone library builds), downloads, the microphone
  path, and every page layout (fixed sidebar, hover actions).
- **Desktop-only:** global hotkey, pill, paste at cursor, clipboard restore,
  paste verification, inline proofreading, per-app profiles, tray, autostart,
  updater, recording other apps' sound.
