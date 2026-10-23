# Notetaker on macOS and Linux (v0.2.0): plan

## Overview
The Notetaker records two tracks: the mic ("You") and the computer's sound
("Others"). Others uses WASAPI loopback (`src-tauri/src/audio/live.rs`,
`Source::Speakers`), which only works on Windows; on macOS and Linux starting a
meeting fails. This plan adds Others capture on macOS (ScreenCaptureKit) and
Linux (PulseAudio monitor source, which PipeWire also serves), behind one Rust
interface; makes sherpa-onnx one checked version everywhere; and gives the
Notetaker page clear states when capture can't work. Goal: release Windows,
macOS and Linux together as v0.2.0.

Research: `docs/research/cross-platform-meeting-audio.md` (Gemini, checked
2026-10-24). Tasks: `tasks/todo.md`.

## Architecture decisions
- **One interface, three backends.** A new `audio/system_audio.rs` exposes
  `support() -> Support` and `open(tx) -> Result<Capture, CaptureError>`, where
  `CaptureError` is `Unsupported(reason) | PermissionDenied | Failed(msg)`.
  Windows keeps today's cpal/WASAPI code (moved, not rewritten); macOS and Linux
  get their own files behind `#[cfg(target_os)]`. `meeting_job.rs` calls only
  the interface. Why: the meeting job stays platform-free, and the page can ask
  "can Others be captured here, and if not, why?" before Start.
- **macOS: ScreenCaptureKit audio-only** (macOS 13+), 16 kHz mono,
  `excludesCurrentProcessAudio`. Permission: "Screen & System Audio Recording",
  string added to `src-tauri/Info.plist`. Works with our self-signed,
  persistent certificate. Crate: `screencapturekit` or `cidre`; pick in Task 7
  by which gives `CMSampleBuffer` audio with the least unsafe code.
- **Linux: PulseAudio monitor of the default sink** via
  `libpulse-simple-binding`. Covers PulseAudio and PipeWire (`pipewire-pulse`),
  X11, Hyprland and Sway; no permission prompt. Not cpal 0.18: upgrading cpal
  touches dictation capture on every platform.
- **Testable without a Mac or Linux PC.** A release-safe self-test
  (`SV_SELFTEST_CAPTURE=<secs>`) opens only the Others capture, logs how much
  audio came in and its level, and saves nothing. CI runs it on the Linux job
  (with a PulseAudio null sink playing a tone) and on the macOS job (where the
  expected result is a clean "permission needed", not a crash).
- **sherpa-onnx: one version file, checked at load.** `sidecars/sherpa/VERSION`
  (1.13.4) is read by `fetch-sidecars.sh` and compiled into the app with
  `include_str!`. `sherpa::lib()` calls `SherpaOnnxGetVersionStr` and refuses a
  mismatch with a clear log line. Struct layouts are pinned by tests whose
  expected numbers come from a C program compiled against the committed
  `c-api.h`, not hand-typed.

## Task list (details in tasks/todo.md)

### Phase 1: sherpa-onnx, one version (b)
- [ ] Task 1: One version file plus the committed header
- [ ] Task 2: Version check at load, logged at startup
- [ ] Task 3: Struct layout tests, checked against C in CI

### Checkpoint A
- [ ] Windows: cargo test, app starts and logs "sherpa-onnx 1.13.4"
- [ ] CI green on macOS and Linux, smoke test finds the version line

### Phase 2: capture the computer's sound (a)
- [ ] Task 4: The system-audio interface, Windows moved behind it
- [ ] Task 5: Release-safe capture self-test
- [ ] Task 6: Linux backend (PulseAudio monitor)
- [ ] Task 7: macOS backend (ScreenCaptureKit), permission and start/stop
- [ ] Task 8: macOS sample conversion to 16 kHz mono, unit-tested

### Checkpoint B
- [ ] Windows meeting unchanged (user records one call)
- [ ] Linux CI: self-test captures the tone (level above silence)
- [ ] macOS CI: self-test reports "permission needed" cleanly, no crash
- [ ] Review with the user before the page work

### Phase 3: the page when capture can't work (c)
- [ ] Task 9: Support command and the Notetaker page states

### Checkpoint C
- [ ] Each state seen in the browser preview (mocked) and on Windows

### Phase 4: release
- [ ] Task 10: Run cargo test on macOS and Linux in CI
- [ ] Task 11: Docs, then v0.2.0 (version bump, tag, publish on the user's go)

### Checkpoint: release
- [ ] Windows: the user's 10-minute check
- [ ] macOS: a real person records one call (see Open questions)
- [ ] Linux: CI capture self-test green; the distro smoke tests still launch

## Risks and mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| Nobody has run macOS capture on a real Mac before release | High | CI proves it builds, starts and handles "no permission". Ask a Mac user to record one call before tagging (Open question 1). Otherwise ship macOS Others marked "beta" |
| `libpulse` missing on a minimal Linux install: a linked library stops the app from starting at all | High | Load libpulse at runtime (libloading, like sherpa) or list it as an AUR/deb dependency; the distro smoke tests show whether the app still starts (Open question 3) |
| GitHub's macOS runner can't grant Screen Recording | Med | Plan for it: the CI check expects the permission-denied path, and real audio is tested by a person |
| ScreenCaptureKit needs a display in its content filter even for audio | Med | Use the main display with the smallest video settings and drop frames; measure CPU on the runner |
| Sequoia re-asks for screen/audio permission monthly | Low | The page's permission state says why it's asking again, with the Settings button |
| Moving the Windows path behind the interface breaks Windows meetings | Med | Task 4 changes no behaviour; the existing loopback test plus one real meeting before moving on |

## Open questions
1. Who can run macOS capture on a real Mac before release? If nobody, ship
   macOS Others as "beta", or release Windows and Linux first?
2. When Others can't be captured (old macOS, permission refused, no
   PulseAudio): record "You" only with a note, or block Start? Recommended:
   record "You" only, with a banner that says why and how to fix it.
3. Linux: load libpulse at runtime (the app always starts, a bit more code) or
   link it and declare the dependency? Recommended: load at runtime.
4. Minimum macOS for the Notetaker: 13 (ScreenCaptureKit audio). The rest of
   the app keeps its current minimum.
