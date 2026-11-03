# Notetaker on macOS and Linux (v0.2.0): tasks

Plan and decisions: `tasks/plan.md`. Testing reality: the user has Windows
only; CI builds and smoke-tests macOS (macos-latest) and Linux (ubuntu-22.04,
plus AppImage launches on other distros). Today `cargo test` runs in CI on
Windows only (`ci.yml`).

---

## Task 1: One sherpa-onnx version file plus the committed header
**Description:** Add `src-tauri/sidecars/sherpa/VERSION` (`1.13.4`) and the
matching `include/c-api.h`. `scripts/fetch-sidecars.sh` reads the tag from
VERSION instead of its own `SHERPA_TAG`.

**Acceptance criteria:**
- [ ] VERSION and c-api.h (byte-identical to upstream v1.13.4) are committed
- [ ] fetch-sidecars.sh has no hard-coded sherpa version left

**Verification:**
- [ ] macOS and Linux CI builds download v1.13.4 (build log shows the URL)
- [ ] `diff` of the committed header against upstream v1.13.4 is empty

**Dependencies:** None
**Files:** `src-tauri/sidecars/sherpa/VERSION`, `src-tauri/sidecars/sherpa/include/c-api.h`, `scripts/fetch-sidecars.sh`
**Scope:** S

## Task 2: Version check at load, logged at startup
**Description:** `sherpa::lib()` calls `SherpaOnnxGetVersionStr` right after
loading. If it isn't the VERSION file's text (`include_str!`), loading fails
with "sherpa-onnx X found, 1.13.4 expected — reinstall the app", and every
sherpa feature reports that error instead of crashing. Startup logs
"sherpa-onnx 1.13.4 loaded".

**Acceptance criteria:**
- [ ] A mismatched library is refused before any struct is passed to it
- [ ] The version is logged once at startup on all three platforms

**Verification:**
- [ ] Windows: `cargo test --lib`; run the app, find the log line
- [ ] Unit test: the version-compare function rejects "1.13.6" and accepts "1.13.4"
- [ ] CI: `scripts/smoke-test.sh` greps the log for "sherpa-onnx 1.13.4" on macOS and Linux and fails without it

**Dependencies:** Task 1
**Files:** `src-tauri/src/system/sherpa.rs`, `scripts/smoke-test.sh`
**Scope:** S

## Task 3: Struct layout tests, checked against C in CI
**Description:** A small C program (`scripts/sherpa-layout.c`) prints
`sizeof`/`offsetof` for every struct the Rust code mirrors, compiled against
the committed header. Rust tests assert the same numbers with
`size_of`/`offset_of!`. A CI step on the Linux job compiles and runs the C
program and diffs its output against the numbers in the Rust tests.

**Acceptance criteria:**
- [ ] Every `#[repr(C)]` struct in `sherpa.rs`, `sherpa_stt.rs` and `diarize.rs` is covered
- [ ] Changing one field in the Rust code makes `cargo test` fail
- [ ] Changing the header (as v1.13.6 did) makes the CI diff fail

**Verification:**
- [ ] Windows: `cargo test --lib layout`. Mutation check: add a field to `PyannoteConfig`, see red, restore
- [ ] CI Linux: the C-vs-Rust diff step passes on 1.13.4

**Dependencies:** Task 1
**Files:** `scripts/sherpa-layout.c`, `src-tauri/src/system/sherpa_layout_tests.rs` (new), `src-tauri/sidecars/sherpa/include/layout.txt` (new), `.github/workflows/cross-platform.yml` (runs on every Rust push, macOS and Linux)
**Scope:** M

### Checkpoint A (after Tasks 1-3)
- [ ] `cargo test --lib` green on Windows; the app starts and logs the version
- [ ] CI green on macOS and Linux, including the version grep and the layout diff

---

## Task 4: The system-audio interface, Windows moved behind it
**Description:** New `src-tauri/src/audio/system_audio.rs` with `support()`
and `open(tx)`, and `CaptureError { Unsupported(reason), PermissionDenied,
Failed(msg) }`. The current WASAPI loopback code moves into its Windows
backend unchanged. `meeting_job.rs` uses the interface for Others. macOS and
Linux return `Unsupported` for now.

**Acceptance criteria:**
- [ ] No behaviour change on Windows
- [ ] `meeting_job.rs` has no platform-specific code for Others

**Verification:**
- [ ] Windows: `cargo test --lib` plus `cargo test --lib real_loopback -- --ignored` (the existing loopback test)
- [ ] Windows: the user records one short Discord or YouTube meeting; You and Others both appear
- [ ] CI: macOS and Linux still build and pass the smoke test

**Dependencies:** None (can run alongside Phase 1)
**Files:** `src-tauri/src/audio/system_audio.rs` (new), `src-tauri/src/audio/live.rs`, `src-tauri/src/audio/mod.rs`, `src-tauri/src/meeting_job.rs`
**Scope:** M

## Task 5: Release-safe capture self-test
**Description:** `SV_SELFTEST_CAPTURE=<secs>` (works in release builds,
unlike the debug-only meeting self-test) opens Others through the interface
for N seconds and logs one line: either "captured X s, peak Y, rms Z" or
the `CaptureError`. Nothing is saved or shown to the user.

**Acceptance criteria:**
- [ ] One log line in every outcome: audio, unsupported, permission denied, failed
- [ ] Starts no meeting and writes no transcript

**Verification:**
- [ ] Windows: run the app with `SV_SELFTEST_CAPTURE=5` while a video plays; the log shows rms above silence. With nothing playing, rms near 0
- [ ] CI: the smoke test runs it and prints the line as a notice on macOS and Linux

**Dependencies:** Task 4
**Files:** `src-tauri/src/lib.rs`, `scripts/smoke-test.sh`
**Scope:** S

## Task 6: Linux backend (PulseAudio monitor)
**Description:** Find the default sink, open `<sink>.monitor` as a 16 kHz
mono float record stream, and feed Others the same way the Windows path does.
Load libpulse at runtime, or link it, per Open question 3. With no PulseAudio
or PipeWire running: `Unsupported("needs PulseAudio or PipeWire")`.

**Acceptance criteria:**
- [ ] Captures what plays through the default output on PulseAudio and on PipeWire (pipewire-pulse)
- [ ] The app still starts on a system without libpulse (if runtime loading is chosen)

**Verification:**
- [ ] CI Linux: start PulseAudio with a null sink as the default, play a 440 Hz tone with `paplay`, run `SV_SELFTEST_CAPTURE=5`, and fail unless rms is above a silence threshold
- [ ] CI distro smoke tests (AppImage on other distros) still launch
- [ ] Windows: `cargo check` passes (code is `cfg`-gated)

**Dependencies:** Tasks 4, 5
**Files:** `src-tauri/src/audio/system_audio_linux.rs` (new), `src-tauri/Cargo.toml`, `.github/workflows/cross-platform-build.yml`, `scripts/smoke-test.sh`
**Scope:** M

## Task 7: macOS backend (ScreenCaptureKit): permission, start, stop
**Description:** Pick the crate (`screencapturekit` or `cidre`). Check macOS
13+ (otherwise `Unsupported("needs macOS 13")`), check or request the
permission (`PermissionDenied` if refused), then start an audio-only `SCStream`
on the main display with `capturesAudio`, 16 kHz, 1 channel,
`excludesCurrentProcessAudio`, and stop it cleanly. Add the usage string to
`src-tauri/Info.plist`.

**Acceptance criteria:**
- [ ] Never crashes: every failure becomes a `CaptureError`
- [ ] Stop releases the stream (no recording indicator left in the menu bar)

**Verification:**
- [ ] CI macOS: the build is signed (existing check) and `SV_SELFTEST_CAPTURE=5` logs `PermissionDenied`, or audio if the runner allows it, never a crash report
- [ ] Real Mac (Open question 1): record one call; Others has the other person
- [ ] Windows: `cargo check` passes

**Dependencies:** Tasks 4, 5
**Files:** `src-tauri/src/audio/system_audio_macos.rs` (new), `src-tauri/Cargo.toml`, `src-tauri/Info.plist`
**Scope:** M

## Task 8: macOS sample conversion to 16 kHz mono
**Description:** Turn the `CMSampleBuffer` audio ScreenCaptureKit delivers
(float, planar or interleaved, maybe 48 kHz stereo even when 16 kHz is asked
for) into 16 kHz mono f32 chunks. Keep the maths in a plain function so it can
be tested without a Mac.

**Acceptance criteria:**
- [ ] Planar and interleaved stereo both mix to mono correctly
- [ ] 48 kHz input comes out at 16 kHz with the right duration

**Verification:**
- [ ] Unit tests on synthetic buffers (a known sine in, the expected length and level out); they run on Windows, because the function is platform-free
- [x] CI macOS: same tests (after Task 10)

**Dependencies:** Task 7
**Files:** `src-tauri/src/audio/system_audio_macos.rs`, `src-tauri/src/audio/resample.rs` (or reuse the existing resampler in `live.rs`)
**Scope:** S

### Checkpoint B (after Tasks 4-8)
- [ ] Windows meeting unchanged (the user records one)
- [x] Linux CI: the tone is captured
- [x] macOS CI: clean "permission needed", no crash
- [ ] Review with the user before the page work

---

## Task 9: Support command and the Notetaker page states
**Description:** A `meeting_capture_support` command returns `support()`. The
Notetaker page shows, before Start:
- macOS below 13: "Recording other people needs macOS 13 or newer."
- Permission not granted: "Allow Silent Voice in System Settings > Privacy &
  Security > Screen & System Audio Recording", with an "Open System Settings"
  button.
- Linux without PulseAudio/PipeWire: "Recording other people needs PulseAudio or PipeWire."

Start then records per Open question 2 (recommended: You only, with the
banner kept on the meeting).

**Acceptance criteria:**
- [ ] Each state has its own message, plus a fix where one exists
- [ ] Windows shows no banner

**Verification:**
- [ ] Browser preview: mock each support state in `tauriBridge` (browser mode) and screenshot each
- [ ] Windows dev app: no banner; recording unchanged
- [ ] macOS CI self-test log matches the state the page would show

**Dependencies:** Tasks 4, 6, 7
**Files:** `src-tauri/src/lib.rs`, `src/services/tauriBridge.ts`, `src/components/dashboard/Notetaker.tsx`, `src-tauri/src/meeting_job.rs`
**Scope:** M

### Checkpoint C
- [ ] All states seen in the preview; Windows unchanged

---

## Task 10: Run cargo test on macOS and Linux in CI
**Description:** Add `cargo test --lib` to the macOS and Linux CI jobs, so the
layout tests and the conversion tests run on the platforms they protect.

**Acceptance criteria:**
- [x] CI fails if any unit test fails on macOS or Linux

**Verification:**
- [x] Push a branch with one failing test and see CI go red, then remove it

**Dependencies:** Tasks 3, 8
**Files:** `.github/workflows/cross-platform-build.yml` (or `ci.yml`)
**Scope:** XS

## Task 11: Docs, then v0.2.0
**Description:** Update `docs/HANDBOOK.md` (Notetaker capture per OS, the
sherpa version file), `PORTING.md`, `CLAUDE.md` (current state). Bump to
0.2.0 in `package.json`, `tauri.conf.json` and `Cargo.toml`, commit, tag
`v0.2.0`. Push the tag and publish only on the user's go.

**Acceptance criteria:**
- [ ] Docs describe what works where, including anything still "beta"
- [ ] The release CI builds all three installers

**Verification:**
- [ ] Release checkpoint below

**Dependencies:** Tasks 1-10
**Files:** `docs/HANDBOOK.md`, `PORTING.md`, `CLAUDE.md`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
**Scope:** S

### Checkpoint: release
- [ ] Windows: the user's 10-minute check (player bar, silence divider, speakers, a summary)
- [ ] macOS: a real person records one call, or Others ships labelled beta (Open question 1)
- [ ] Linux: the CI capture self-test is green; the distro smoke tests launch
- [ ] The user says go: push the tag, then publish
