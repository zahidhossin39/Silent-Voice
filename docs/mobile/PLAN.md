# Silent Voice mobile: plan

Decided with the user on 2026-11-12. Branch: `mobile`. Android first, iOS after.
Background: `docs/features/` (what the desktop app does and what carries over).

## Restate

- **Outcome:** an Android app where you tap a floating mic bubble, speak, and the
  cleaned text lands in whatever field you are typing in.
- **User:** existing Silent Voice users who want the same free, local dictation on
  their phone.
- **Why now:** desktop v0.2.0 is shipped on three systems, and sherpa-onnx (already
  our library engine) has official Android and iOS builds.
- **Success:** offline, on a mid-range phone (4-8 GB RAM), with AI mode off, a clip
  of up to 15 seconds appears in WhatsApp, Gmail and Chrome within 1 second of
  tapping stop. Any failure still leaves the words on the clipboard.
- **Constraint:** the desktop app must not be touched. All mobile work stays on the
  `mobile` branch and in new folders.
- **Out of scope:** see "Not doing".

## Decisions

| Area | Decision | Seen in |
|---|---|---|
| Text entry | Floating bubble + Accessibility Service; no keyboard of our own | Wispr Flow Android |
| Bubble | Shows only while a text field is focused and the keyboard is up; tap to start, tap to stop and insert | Wispr Flow |
| Screen reading | Only what insertion needs: is a field focused, is it a password field, insert. No app name, no nearby text | our choice (Wispr reads both) |
| Speech | On-device sherpa-onnx, English first; cloud speech optional | Dictus, Outspoke, WhisperTypeKeyboard |
| AI | Cloud AI modes in the first version; no local LLM on the phone | Wispr Flow (cloud LLM) |
| Stack | Native Kotlin + Jetpack Compose; Rust logic shared through UniFFI | Element X, Firefox, Offline Voice Input |
| Rust sharing | New crate with a copy of the needed logic; desktop keeps its own code until Android works | our choice (safety) |
| Distribution | APK on GitHub Releases | Transcribro, WhisperIME, Dictus |
| iOS | After Android. Mac + iPhone available, free signing (7-day installs) | - |

Decided by default (say so if any is wrong):

- Code lives in this repo: `mobile/android/` (Kotlin app) and `crates/sv-core/`
  (shared Rust). One repo is what lets the crate be shared later.
- The speech engine is called through sherpa-onnx's official Kotlin API. The Rust
  crate holds text logic only (clean-up, vocabulary, replacements, numbers, AI mode
  prompts, cloud provider calls). The sherpa-onnx version follows
  `src-tauri/sidecars/sherpa/VERSION` where an Android build of it exists (ADR 0002).
- Minimum Android 10 (API 29).
- If a field cannot be written to, the text is copied to the clipboard and a notice
  says so. AI failure falls back to the raw transcript. Same rule as desktop.
- Password fields are never written to or read.
- Cloud API keys are stored in the Android Keystore, not in plain text.
- First-version screens: onboarding (microphone, draw-over-apps, accessibility,
  including the "Allow restricted settings" step that sideloaded apps need on
  Android 13+), model download, settings, vocabulary and replacements, AI modes,
  cloud providers, history.

## Build order

Each step ends with something that runs on the phone. Commit at each.

0. **Toolchain.** Android Studio, SDK, NDK, Rust Android targets, `cargo-ndk`.
   None were installed on 2026-11-12 (only Java 21).
1. **Engine spike.** Bare Kotlin app: record, run a small English model, show the
   text and the time taken. Try Moonshine tiny, Moonshine base and Parakeet 110M;
   pick the default from the measured speed and accuracy on the test phone.
   **Done 2026-11-12.** Default is Parakeet 110M: a 16.7 s clip takes 0.8 s on
   the test phone when rested and 1.2-1.3 s when busy (Moonshine Tiny 1.4 s,
   Moonshine Base 3.7 s). Numbers in `mobile/android/README.md`. The 1 s target
   is borderline, not safely met, so transcribe-while-speaking stays next in
   line after the first version.
2. **Bubble spike.** Accessibility Service + overlay bubble that appears with the
   keyboard and inserts a fixed test string into WhatsApp, Gmail and Chrome.
   Includes the password skip and the clipboard fallback.
3. **Shared crate.** `crates/sv-core` with the copied text logic and its tests,
   built for Android and called from Kotlin through UniFFI.
4. **Pipeline.** Record, trim silence, transcribe, clean up, optional cloud AI
   mode, insert. Raw-text fallback on any AI failure.
5. **App screens.** The first-version screens listed above.
6. **Measure and release.** Check the success bar; build the APK in GitHub Actions.

After the first version, in order: transcribe-while-speaking (any clip length under
1 s), more languages (Bangla needs a Whisper model), switch desktop to `sv-core`,
Play Store, iOS.

## iOS notes (for later)

iOS has no floating bubble and no accessibility insertion. Wispr Flow, Superwhisper
and Dictus all use a keyboard extension that hands recording to the main app (the
keyboard has roughly 30-50 MB of memory, too little for a model) and inserts the
text when the user returns. The shared Rust crate is what carries over; the screens
are rewritten in SwiftUI.

## Assumptions to validate

| Assumption | Test | Kills it if |
|---|---|---|
| A small English model does a 15 s clip in under 1 s on a mid-range phone | Step 1, measured on the test phone | Over 1 s: bring transcribe-while-speaking forward, or relax the target |
| The bubble can tell when the keyboard is up and insert text in the three apps | Step 2 | An app refuses accessibility insertion: paste through the clipboard instead |
| Sideload users will get through "Allow restricted settings" | Step 5 onboarding, tried by someone other than the developer | People cannot finish it: bring the Play Store forward |
| UniFFI + cargo-ndk builds cleanly on Windows | Step 3 | Build in CI on Linux only |

Figures from research that are not yet measured by us: model speed and memory on
phones, and the iOS keyboard memory limit (Apple publishes no number).

## Not doing (and why)

- **A typing keyboard.** Months of work unrelated to voice.
- **Local LLM on the phone.** Needs llama.cpp as a library and a strong phone.
- **Notetaker, Transcribe a file, read-aloud.** A second product; after dictation works.
- **Inline proofreading, per-app styles, reading nearby text.** Would need the
  bubble to read other apps' screens, which the user declined.
- **Recording other apps' sound.** Phones block it.
- **Tauri on mobile.** It cannot build the bubble or a keyboard, and every page
  needs a phone layout anyway.
- **Docker.** It packages server programs; this is a phone app. CI already gives
  repeatable builds.
- **Accounts, sync, telemetry.** Against "free, local, simple".
