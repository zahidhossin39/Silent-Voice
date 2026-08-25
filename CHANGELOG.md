# Changelog

All notable changes to Silent Voice are recorded here.
This project follows [Semantic Versioning](https://semver.org/).

## [0.1.8] — unreleased

### Added
- **Dictation activity strip on Home.** The Status panel now ends with a
  12-week heatmap — one square per day — alongside days dictated, your busiest
  day, and words per dictation.
- **Rebuilt first run.** Onboarding is now a left-rail flow: the model
  downloads in the background while you set up your microphone and hotkey,
  instead of making you wait on a download before continuing.
- **Model downloads can be paused and cancelled**, and resume rather than
  restarting after an interruption.
- **Recordings are kept and playable.** Each dictation stores its audio and
  Home gives it a real player.
- **Grammar model (GECToR)** is downloadable in two variants.

### Changed
- **Spoken fillers are removed from every transcript.** "um", "uh", "erm" and
  friends are stripped before pasting, so Parakeet and other verbatim models
  now match Whisper's cleaner output.
- Inline proofreading underlines hide instantly on scroll instead of drifting
  out of alignment.
- Home was redesigned around the live console, with device specs and a Today
  panel.
- Rewrite modes show what they do rather than their raw prompt.

### Fixed
- Whisper and llama sidecars are no longer orphaned when the app exits.
- Underlines no longer blink on every keystroke, or when UIA loses the focused
  field mid-typing.
- Two stray symbols could disable all proofreading.
- Audio is placed on the clipboard in formats other apps actually accept.
- Quote stripping is indexed by character instead of byte, fixing non-ASCII
  transcripts.
- Truncated model downloads are rejected instead of being treated as complete.
- The NSIS installer now carries the app icon.

### Security
- A release that is not properly signed can no longer be published: CI fails
  if the signing key is missing or no `.sig` is produced.
- `postcss` bumped to 8.5.23 to clear a path-traversal advisory.
- Added a frontend XSS regression guard around rendered HuggingFace READMEs.

### Removed
- Dead code: the unused `paste_text` command, `sttCompatibility`,
  `deviceRealtimeLabel`, and the unused `@tauri-apps/plugin-dialog` and
  `@tauri-apps/plugin-shell` dependencies.

## [0.1.7] — 2026-07-19

GECToR sensitivity presets (Relaxed / Balanced / Aggressive) wired from
Settings through to both thresholds.

## [0.1.6] — 2026-07-14

Non-silent auto-updater, adjustable CPU thread count, Model Store pinning,
doubled-word transcription fix, and spurious mid-hold hotkey release fix.

## [0.1.5] — 2026-07-13

## [0.1.4] — 2026-07-13

Auto-updater and release pipeline (Tauri updater + signed NSIS installer).
