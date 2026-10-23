# Cross-Platform Meeting Audio Capture & Native FFI Safety

> Researched by Gemini (agy), 2026-10-24; checked by Claude. Corrections are marked "Corrected by Claude". Verified: `SherpaOnnxGetVersionStr` exists in v1.13.4 and in the committed Windows DLL; struct sizes measured with rustc; PR #3870 date from the GitHub API; cpal features from docs.rs.

Research findings for **Silent Voice** (Tauri v2: Rust + React), addressing system audio loopback capture across macOS and Linux, and native C-library ABI safety for dynamically loaded libraries (`sherpa-onnx`).

---

## 1. macOS: Capturing System Audio (Call Participants)

### Comparison of macOS Approaches

| Option | Min macOS | TCC Permission Prompt | Self-Signed / Non-Notarized | Rust Crates & Maintenance |
| :--- | :--- | :--- | :--- | :--- |
| **ScreenCaptureKit (Audio Stream)** | macOS 13.0 (Ventura) | **Screen & System Audio Recording** (`kTCCServiceScreenCapture`) | **Yes** (with persistent keychain cert) | `screencapturekit` (active), `cidre` (very active), `objc2-screen-capture-kit` (active) |
| **Core Audio Process Taps (`AudioHardwareCreateProcessTap`)** | macOS 14.2 (Sonoma) | **System Audio Recording** (`kTCCServiceAudioCapture`, via `NSAudioCaptureUsageDescription`) | **Conditional** (public API, but process-isolation limits often need helper tools) | `objc2-core-audio` (low-level raw bindings); no dedicated high-level safe crate |
| **Virtual Audio Drivers (BlackHole, ACE, etc.)** | macOS 10.10+ | **Microphone only** at runtime (`kTCCServiceMicrophone`); driver install requires root (`sudo`) | **Yes** for runtime capture; **No** for driver installation from self-signed app | Direct support in `cpal` as standard input devices |

---

### Evidence & Technical Details

#### 1. ScreenCaptureKit Audio Capture
* **API Details & Minimum Version**: ScreenCaptureKit was introduced in macOS 12.3 for video, but native system audio capture via `SCStreamConfiguration.capturesAudio = true` was introduced in **macOS 13.0 (Ventura)** ([Apple Developer: `capturesAudio`](https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/capturesaudio), last updated 2024). It also provides `excludesCurrentProcessAudio = true`, `sampleRate` (supports 16000 Hz, 24000 Hz, 48000 Hz), and `channelCount` ([Apple Developer: `SCStreamConfiguration`](https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration), 2024).
* **Permissions & TCC**: ScreenCaptureKit operates under the Screen Recording privacy umbrella (`kTCCServiceScreenCapture`). In macOS 15 (Sequoia, released September 2024), Apple renamed the privacy tab to **Screen & System Audio Recording** ([Apple Support: Control access to screen and system audio recording on Mac](https://support.apple.com/guide/mac-help/control-access-to-screen-recording-on-mac-mchld6aa7d23/mac), Sept 2024). Sequoia added recurring monthly re-prompts for applications capturing screen and audio outside the system window picker.
* **Self-Signed / Non-Notarized Status**: ScreenCaptureKit does **not** require a paid Apple Developer Team ID or notarization. Any binary can invoke `SCShareableContent` and `SCStream`. However, ad-hoc signing (`codesign -s -`) causes macOS TCC to recalculate the code directory hash (CDHash) on every build, triggering permission prompts on every compile. Signing with a **persistent self-signed code-signing certificate** in Keychain keeps the designated requirement stable across rebuilds ([Apple Developer Forums: TCC and Code Signing](https://developer.apple.com/forums/thread/705707), 2022). Gatekeeper will prompt users on first launch if downloaded without notarization, but TCC permissions operate identically once launched.
* **Rust Ecosystem**:
  * [`screencapturekit`](https://crates.io/crates/screencapturekit) / [`doom-fish/screencapturekit-rs`](https://github.com/doom-fish/screencapturekit-rs): Actively maintained (v0.3.x in 2025; major release line v11 in 2025/2026). Supports system audio capture and `SCStreamOutput` sample buffer callbacks.
  * [`cidre`](https://crates.io/crates/cidre) / [`yury/cidre`](https://github.com/yury/cidre): Very actively maintained (v0.29+, 2025/2026). Idiomatic, zero-cost Rust bindings for Metal, AVFoundation, and ScreenCaptureKit.
  * [`objc2-screen-capture-kit`](https://crates.io/crates/objc2-screen-capture-kit): Part of the `objc2` framework project, actively maintained (2025/2026). Generates direct low-level FFI bindings.

#### 2. Core Audio Process Taps (`AudioHardwareCreateProcessTap`)
* **API Details & Minimum Version**: Introduced in **macOS 14.2 (Sonoma)** and expanded in macOS 14.4 ([Apple Developer: `AudioHardwareCreateProcessTap`](https://developer.apple.com/documentation/coreaudio/audiohardwarecreateprocesstap), Dec 2023). It allows capturing audio from a target PID or global system output without receiving video frames ([Apple Developer: Capturing system audio with Core Audio taps](https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps), 2024).
* **Permissions & TCC**: Uses `kTCCServiceAudioCapture`. The application must include the key `NSAudioCaptureUsageDescription` (or `NSSystemAudioCaptureUsageDescription`) in `Info.plist` ([Apple Developer: `NSAudioCaptureUsageDescription`](https://developer.apple.com/documentation/bundleresources/information_property_list/nsaudiocaptureusagedescription), 2024). The prompt requests audio recording access without asking for screen recording.
* **Self-Signed / Non-Notarized Status**: The API is public, but developers frequently report `kAudioHardwareIllegalOperationError` (OSStatus `2003329396` / `'what'`) when attempting to tap arbitrary third-party processes from a standard user-space app bundle ([insidegui/AudioCap](https://github.com/insidegui/AudioCap), March 2024). Production apps tapping third-party processes often require a privileged background helper daemon (via `launchd` or `SMJobBless`).
* **Rust Ecosystem**: Exposed in [`objc2-core-audio`](https://docs.rs/objc2-core-audio/latest/objc2_core_audio/fn.AudioHardwareCreateProcessTap.html) (2025). No high-level safe Rust crate wraps this yet; developers must manually build a Core Audio Aggregate Device with `kAudioAggregateDeviceTapListKey` and register an `AudioDeviceIOProc`.

#### 3. Virtual Audio Drivers (BlackHole, ACE, ZoomAudioDevice)
* **Mechanics**: A CoreAudio HAL plug-in installed in `/Library/Audio/Plug-Ins/HAL/`. Requires root privileges (`sudo` or `.pkg` admin authentication) to install ([ExistentialAudio/BlackHole](https://github.com/ExistentialAudio/BlackHole), 2024).
* **Permissions**: At runtime, BlackHole exposes standard audio input and output endpoints. Capturing from it via `cpal` triggers only the microphone permission (`kTCCServiceMicrophone`).
* **Drawbacks**: Requires the user to configure a Multi-Output Device in *Audio MIDI Setup* so they can hear audio while routing it to the virtual driver. Multi-Output Devices lock the Mac keyboard volume keys unless third-party utilities are used.

#### 4. What Shipping Recorders Actually Use
* **Granola**: Uses native macOS APIs (ScreenCaptureKit and Core Audio), requiring "Screen & System Audio Recording" permission. It does not install or use virtual audio drivers ([Granola Help Center](https://help.granola.ai/), 2024; links to the help home, not a specific article: unverified).
* **Otter.ai**: Its native macOS desktop app ("botless recording") captures system audio via ScreenCaptureKit. (The source's "macOS 12.3+" conflicts with SCK audio needing macOS 13; unverified.) It explicitly prompts users for "Screen & System Audio Recording" permission in System Settings ([Otter.ai Help Guide: Desktop Recording on Mac](https://support.otter.ai/hc/en-us/articles/12379768608279), 2024).
* **OBS Studio**: OBS Studio 28.0 (released August 31, 2022) introduced native macOS desktop and application audio capture via ScreenCaptureKit (`plugins/mac-capture/mac-sck-audio-capture.m`), deprecating the requirement for BlackHole/iShowU on macOS 13+ ([OBS Studio 28.0 Release Notes](https://github.com/obsproject/obs-studio/releases/tag/28.0.0), Aug 2022).
* **Zoom Desktop Client**: Zoom installs its own HAL plug-in, `ZoomAudioDevice.driver`, in `/Library/Audio/Plug-Ins/HAL/` to capture system audio when a user checks "Share Sound" during screen sharing ([Zoom Support: Sharing computer sound in screen share](https://support.zoom.com/), 2024).
* **Screenpipe** (Open-source 24/7 AI recorder in Rust): Captures screen and system audio via ScreenCaptureKit using the [`cidre`](https://github.com/yury/cidre) crate through its `sck-rs` component ([mediar-ai/screenpipe](https://github.com/mediar-ai/screenpipe), 2024–2026).

---

### Recommendation for Silent Voice (macOS)

Use **ScreenCaptureKit audio capture** via the [`cidre`](https://crates.io/crates/cidre) or [`screencapturekit`](https://crates.io/crates/screencapturekit) crate.

* **Target OS**: macOS 13.0+.
* **Why**: It is a fully user-space API that requires no root permissions, no virtual sound drivers, and no audio MIDI routing. It functions with self-signed certificates as long as a persistent signing identity is used.
* **Concrete Work**:
  1. Add `cidre = { version = "0.29", default-features = false, features = ["sc", "cm", "av"] }` (or `screencapturekit = "0.3"`) under `target.'cfg(target_os = "macos")'.dependencies` in `Cargo.toml`.
  2. Implement a macOS audio module replacing `cpal` for `Source::Speakers`. Create an `SCStream` with `SCStreamConfiguration` setting `captures_audio = true`, `sample_rate = 16000`, `channel_count = 1`, and `excludes_current_process_audio = true`.
  3. In the stream output callback, extract f32 audio samples from the incoming `CMSampleBuffer` and send chunks through the existing mpsc channel to `meeting_job.rs`.
  4. In `src-tauri/Info.plist`, declare `NSScreenCaptureUsageDescription` so macOS presents an informative prompt when requesting permission.

---

## 2. Linux: PulseAudio Monitor Sources vs. PipeWire Capture

### Comparison of Linux Capture Mechanisms

| Mechanism | CPAL 0.15 Support | Native X11 | Native Wayland (Hyprland / Sway) | Flatpak Sandbox | Used By |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **PulseAudio Monitor (`<sink>.monitor`)** | No in 0.15; yes in cpal 0.18 (`pulseaudio` feature) | Direct socket; **no portal/prompt** | Direct socket; **no portal/prompt** | Needs `--socket=pulseaudio` | OBS (`linux-pulseaudio`), RecApp (`pulsesrc`), Audacity |
| **Native PipeWire (`stream.capture.sink`)** | No in 0.15; yes in cpal 0.18 (`pipewire` feature) | Direct socket; **no portal/prompt** | Direct socket; **no portal/prompt** | Needs `--filesystem=xdg-run/pipewire-0` | OBS (`obs-pipewire-audio-capture`), Kooha |
| **Current `live.rs` (`cpal` default ALSA)** | **Fails** on output device | Fails or captures mic input | Fails or captures mic input | Fails | None (broken on Linux) |

---

### Evidence & Technical Details

#### 1. Current `cpal` (0.15) Behavior in Silent Voice
* In `src-tauri/Cargo.toml`, line 30 specifies `cpal = "0.15"` with default features. On Linux, default features enable **ALSA only** ([CPAL `Cargo.toml`](https://github.com/RustAudio/cpal/blob/master/Cargo.toml), docs.rs/cpal/0.15.3).
* ALSA does not provide a universal virtual loopback mechanism. In `live.rs` (lines 140–145):
  ```rust
  Source::Speakers => {
      let d = host.default_output_device().ok_or("No speakers or headphones found.")?;
      let c = d.default_output_config().map_err(|e| e.to_string())?;
      (d, c)
  }
  ```
  On Windows WASAPI, `build_input_stream` on an output device triggers `AUDCLNT_STREAMFLAGS_LOOPBACK`. On Linux ALSA, calling `build_input_stream` on an output device calls `snd_pcm_open(..., SND_PCM_STREAM_CAPTURE, ...)`. This either returns an error or opens the hardware card's capture subdevice (the microphone), not system loopback ([CPAL Issue #586: ALSA loopback](https://github.com/RustAudio/cpal/issues/586)).
* Because `meeting_job.rs` (lines 81–87) aborts the entire meeting if the `Speakers` track fails to open, meeting recording immediately crashes or records the mic twice on Linux.
* **Corrected by Claude (2026-10-24):** cpal 0.15.3 has **no** PulseAudio or PipeWire backend; its `Cargo.toml` has no such feature (checked on docs.rs). The latest cpal, **0.18.2**, has both a `pulseaudio` and a `pipewire` feature ("audio I/O on Linux … via the PulseAudio sound server" / "PipeWire multimedia server"). Whether cpal 0.18 lists `.monitor` sources as input devices is **unverified**; check before choosing it over `libpulse-binding`. Upgrading cpal from 0.15 to 0.18 also touches dictation capture on every platform.

#### 2. Permissions and Portals on Linux Desktops
* **X11**:
  * No permission model exists. Any process running as the local user connects directly to the PulseAudio / PipeWire UNIX domain socket (`$XDG_RUNTIME_DIR/pulse/native` or `$XDG_RUNTIME_DIR/pipewire-0`). Monitor capture requires no authorization or user prompts ([Freedesktop PulseAudio Documentation](https://www.freedesktop.org/wiki/Software/PulseAudio/Documentation/Developer/Clients/)).
* **Wayland (Hyprland / Sway)**:
  * Wayland compositors (Hyprland, Sway) isolate display protocol surfaces and input devices. **Wayland does not govern audio**.
  * Audio is managed entirely by PipeWire or PulseAudio in user space. Because Hyprland and Sway do not intercept UNIX socket connections to PipeWire/PulseAudio, a native application can capture the default sink's monitor with **zero prompts, zero permissions, and no portal interaction** ([PipeWire Architecture & Security](https://docs.pipewire.org/page_security.html), 2024).
* **Flatpak Sandbox**:
  * Flatpak sandboxes block socket access by default ([Flatpak Documentation: Sandbox Permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)).
  * To capture audio, a Flatpak application requires the static permission `--socket=pulseaudio` (which bridges to `pipewire-pulse`) or `--filesystem=xdg-run/pipewire-0`.
  * Flathub and GNOME Software display a warning flag for applications requesting `--socket=pulseaudio` because PulseAudio sockets grant both playback and recording (mic and monitor) permissions without runtime consent prompts.
  * The dynamic portal `org.freedesktop.portal.ScreenCast` can negotiate audio streams via PipeWire, but is primarily designed for combined screen-video and audio capture, not standalone audio recording ([Flatpak Portal Documentation](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)).

#### 3. What Linux Recorders Actually Use
* **OBS Studio**:
  * Uses `plugins/linux-pulseaudio/linux-pulseaudio.c` to capture desktop audio by connecting to the default sink's monitor source (`<default_sink_name>.monitor`) via `libpulse` ([OBS Studio `linux-pulseaudio`](https://github.com/obsproject/obs-studio/tree/master/plugins/linux-pulseaudio), 2024).
  * Uses `obs-pipewire-audio-capture` / native PipeWire capture to tap application or sink nodes directly using `PW_KEY_STREAM_CAPTURE_SINK` ([dimtpap/obs-pipewire-audio-capture](https://github.com/dimtpap/obs-pipewire-audio-capture), 2024).
* **Kooha** (GNOME/Wayland recorder written in Rust): Uses GStreamer with `pipewiresrc` via `xdg-desktop-portal` for video and desktop audio ([SeaDve/Kooha](https://github.com/SeaDve/Kooha), 2024).
* **RecApp**: Uses GStreamer's `pulsesrc` referencing the monitor source of the active sink ([RecApp GitLab](https://gitlab.gnome.org/World/recapp)).

---

### Recommendation for Silent Voice (Linux)

Capture the **PulseAudio monitor source** of the default sink using [`libpulse-binding`](https://crates.io/crates/libpulse-binding) and [`libpulse-simple-binding`](https://crates.io/crates/libpulse-simple-binding) (or `pulsectl-rs`):

* **Target Compatibility**: Works across both native PulseAudio and modern PipeWire setups (via `pipewire-pulse`), covering X11, Hyprland, and Sway without configuration.
* **Why**: Every modern Linux desktop running PipeWire runs `pipewire-pulse` by default. Connecting as a PulseAudio client avoids needing `libpipewire-0.3` C development headers and bypasses cpal's ALSA limitations.
* **Concrete Work**:
  1. Add `libpulse-binding = "2"` and `libpulse-simple-binding = "2"` under `target.'cfg(target_os = "linux")'.dependencies`.
  2. In `live.rs`, when running on Linux with `Source::Speakers`:
     - Query the default sink name via `pa_context_get_sink_info_by_name("@DEFAULT_SINK@")`.
     - Append `.monitor` to obtain the monitor source name (e.g. `alsa_output.pci-0000_00_1f.3.analog-stereo.monitor`).
     - Open a `pa_simple` record stream on that monitor source requesting 16,000 Hz, 1-channel, 32-bit floating point PCM (`PA_SAMPLE_FLOAT32LE`).
     - Read samples in a dedicated loop and forward them to `Msg::Audio(Speaker::Others, ...)`.
  3. If packaged for Flatpak in the future, add `--socket=pulseaudio` to the build manifest.

---

## 3. Native Library Pinning & C Struct Layout Safety

### Comparison of Prevention & Detection Mechanisms

| Technique | Where it Runs | Catches Struct Layout Changes? | Catches Library Version Mismatch? | Fits Runtime `libloading`? |
| :--- | :--- | :--- | :--- | :--- |
| **Load-Time Version Check (`SherpaOnnxGetVersionStr`)** | Runtime (at startup) | Indirectly (via version equality) | **Yes** (100% reliable) | **Best fit** (minimal code) |
| **Bindgen Struct Generation from Pinned Header** | Build time (`build.rs`) | **Yes** (compilation error on field change) | No (only checks header) | **Yes** (generates Rust structs) |
| **Static Layout Tests (`size_of`, `offset_of!`)** | Test time (`cargo test` in CI) | **Yes** (fails CI before shipping) | No | **Yes** (zero runtime cost) |
| **Committed Header (`c-api.h`) in Repo** | Repository source | Provides ground truth for bindgen/tests | No | **Yes** (audit trail) |
| **Single Shared Version Constant** | Build / Fetch scripts | Prevents multi-platform drift | **Yes** (keeps assets in sync) | **Yes** (eliminates human error) |

---

### Evidence & Technical Details

#### 1. How the Bug Occurred in Silent Voice
* In `fetch-sidecars.sh`, `SHERPA_TAG` was originally unpinned or set ahead of the committed Windows DLL.
* Upstream `sherpa-onnx` pull request [#3870](https://github.com/k2-fsa/sherpa-onnx/pull/3870) ("Expose the pyannote window shift ratio in the C API", merged 2026-08-13, shipped in v1.13.6) added `float window_shift_ratio` to `SherpaOnnxOfflineSpeakerSegmentationPyannoteModelConfig`:
  ```c
  // Upstream c-api.h v1.13.6:
  struct SherpaOnnxOfflineSpeakerSegmentationPyannoteModelConfig {
      const char *model;
      float window_shift_ratio; // Added in v1.13.6
  };
  ```
* In `src-tauri/src/system/diarize.rs` (lines 9–12), the hand-written struct matched v1.13.4:
  ```rust
  #[repr(C)]
  struct PyannoteConfig {
      model: *const c_char,
  }
  ```
* Because `PyannoteConfig` is embedded directly by value inside `SegmentationConfig`, which is embedded inside `DiarizationConfig`, the extra 4-byte float (plus 4-byte padding on 64-bit systems) shifted every subsequent field (`num_threads`, `debug`, `provider`, `embedding`, `clustering`) by 8 bytes. When passed across the FFI boundary, the native library read pointers and integers from the wrong memory offsets.

#### 2. How Professional Projects Prevent ABI Drift
* **Load-Time Version Validation**:
  `sherpa-onnx`'s `c-api.h` exports `const char *SherpaOnnxGetVersionStr()` ([`k2-fsa/sherpa-onnx` `c-api.h`](https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/c-api/c-api.h), line 42). A professional `libloading` wrapper queries this symbol immediately after loading the library and verifies that the dynamic library's version matches the compiled expectations.
* **Layout Assertions in Rust**:
  Rust 1.77.0 (stabilized March 2024) introduced [`std::mem::offset_of!`](https://doc.rust-lang.org/stable/std/mem/macro.offset_of.html). In `cargo test`, developers write static assertions verifying `size_of` and `offset_of!` for each `#[repr(C)]` struct against expected reference values.
* **`bindgen` Generation & Layout Tests**:
  By default, `bindgen` generates compile-time layout unit tests (`--layout-tests`, enabled by default) for every C struct, generating `assert_eq!(mem::size_of::<...>, ...)` and field offset checks ([Rust `bindgen` Documentation: Code Generation](https://rust-lang.github.io/rust-bindgen/)).
* **Single Version Source of Truth**:
  Professional monorepos define third-party native versions in a single configuration file (e.g., `versions.json` or `.env`) read by both Windows CI scripts and Unix fetch scripts.

---

### Recommendation for Silent Voice (Native Library ABI)

For Silent Voice, which uses `libloading` to dynamically load `sherpa-onnx` at runtime, adopt a **four-layer defense**:

1. **Runtime Version Check at Initialization**:
   In `src-tauri/src/system/sherpa.rs`, inside `lib()`, immediately load and call `SherpaOnnxGetVersionStr`:
   ```rust
   let get_ver: Symbol<unsafe extern "C" fn() -> *const c_char> =
       lib.get(b"SherpaOnnxGetVersionStr\0").map_err(|e| e.to_string())?;
   let ver_str = unsafe { std::ffi::CStr::from_ptr(get_ver()) }.to_string_lossy();
   if !ver_str.starts_with("1.13.4") {
       return Err(format!("sherpa-onnx version mismatch: expected 1.13.4, found {ver_str}. Reinstall sidecars."));
   }
   ```
   *Effort: ~10 lines of code.* Guarantees that an incompatible DLL or dylib cannot be loaded.

2. **Commit `c-api.h` in the Repository**:
   Place `c-api.h` from sherpa-onnx v1.13.4 in `src-tauri/sidecars/sherpa/include/c-api.h`. This freezes the definitive C ABI contract in git history.

3. **Add Layout Unit Tests in CI**:
   In `src-tauri/src/system/sherpa.rs` and `diarize.rs`, add unit tests verifying struct sizes and field offsets using `std::mem::size_of` and `std::mem::offset_of!`:
   ```rust
   #[test]
   fn verify_diarization_struct_abi() {
       // 64-bit, sherpa-onnx v1.13.4. Measured with rustc on the real structs (2026-10-24);
       // Gemini's first draft said 32 and 96 here, which were wrong.
       assert_eq!(std::mem::size_of::<PyannoteConfig>(), 8);
       assert_eq!(std::mem::size_of::<SegmentationConfig>(), 24);
       assert_eq!(std::mem::offset_of!(SegmentationConfig, num_threads), 8);
       assert_eq!(std::mem::offset_of!(DiarizationConfig, min_duration_on), 56);
       assert_eq!(std::mem::size_of::<DiarizationConfig>(), 64);
   }
   ```
   Better still, take the expected numbers from a tiny C program compiled against the committed `c-api.h`, so the test compares Rust to C rather than to hand-typed numbers.
   *Effort: ~20 lines of unit tests.* Fails `cargo test` instantly if a developer edits a struct incorrectly.

4. **Single Version Source of Truth**:
   Create a single file (e.g. `scripts/sherpa-version.txt` containing `1.13.4`). Have both `fetch-sidecars.sh` and any Windows fetch/build script read that file rather than hardcoding tags.

---

## Open Questions

1. **ScreenCaptureKit Audio-Only Stream Negotiation**:
   Apple documentation specifies that `SCStream` requires an `SCContentFilter` initialized with at least one display or window. Even when video frame sample buffers are ignored and discarded, does ScreenCaptureKit consume measurable GPU/WindowServer resources when running purely for audio capture on low-end M1/M2 MacBooks?
2. **PipeWire Direct Tap vs. PulseAudio Wrapper Latency**:
   On Linux setups running native PipeWire, does capturing via `pipewire-pulse` (PulseAudio emulation socket) introduce any measurable audio latency or clock jitter compared to native `pipewire-rs` node connections? (For 16 kHz Whisper speech chunking, 5–10 ms latency differences are typically inconsequential, but buffer-period alignment remains unverified).
3. **Windows `sherpa-onnx` DLL Compilation Toolchain**:
   Are the committed Windows `sherpa-onnx-c-api.dll` and `onnxruntime.dll` built using MSVC or MinGW? While `#[repr(C)]` layouts for simple scalar fields and pointers match across MSVC and GCC on x86_64, verifying the compiler flags used upstream ensures alignment compatibility across complex nested structs.
