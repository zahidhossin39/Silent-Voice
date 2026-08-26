# Porting to macOS and Linux

Status: **packages cleanly for macOS and Linux; never launched on either.**

`.github/workflows/cross-platform-build.yml` is green on both runners and
produces a `.dmg` (~94 MB), a `.deb`, and an `.AppImage`. `cross-platform.yml`
stays as the fast `cargo check` gate on every push — but note it did NOT catch
the missing `libxdo`, because checking does not link. Only the packaging build
proves the link step.

What that does *not* mean: nobody has run the result. Every claim below about
runtime behaviour on macOS or Linux is reasoned from the code, not observed.
The first person to launch it should expect to find things.

## What already works everywhere

Audio capture (`cpal`), transcription orchestration, the LLM and model layers,
downloads, proofreading logic, text formatting, history, and the entire React
frontend are platform-neutral. The frontend's only Windows references are
wording in the UI copy.

## What is gated off

| Module | Lines | Why |
| --- | --- | --- |
| `system/squiggle.rs` | 1,183 | Draws underlines with Win32 layered windows and GDI |
| `system/inline_check.rs` | 953 | Reads other apps' text via UI Automation |

Both are `#[cfg(windows)]` at the module declaration, with their four call sites
in `lib.rs` gated the same way. Together they are the inline-proofreading
feature — over half the total porting cost, for one feature. **Ship without it
first.**

## Per-platform shims

| Module | Windows | macOS | Linux | Done |
| --- | --- | --- | --- | --- |
| `paste.rs` / `tts.rs` | Ctrl+V / Ctrl+C | Cmd+V / Cmd+C | Ctrl+V / Ctrl+C | yes |
| `autostart.rs` | registry Run key | LaunchAgent plist | XDG `.desktop` | yes |
| `foreground.rs` | `GetForegroundWindow` | `osascript` | `xprop` | yes |
| `hardware.rs` | DXGI | `system_profiler` | `lspci` | yes |
| `sherpa.rs` | `.dll` | `.dylib` | `.so` | yes |
| `job.rs` | Job Objects kill sidecars | `pkill` at startup | `pkill` at startup | yes |
| `clipboard_file.rs` | clipboard file formats | — | — | no |
| `secure_field.rs` | UIA password detection | — | — | no |
| `overlay.rs` round corners | DWM | — | — | no |

The three unfinished ones all degrade to a no-op, not a crash — a square-
cornered pill, a disabled "copy audio" button, and no password-field guard.

`job.rs` is weaker off Windows rather than absent. Windows Job Objects kill the
sidecars even on power loss; the Unix path instead reaps leftovers at the next
startup, so a crash can leave `llama-server` holding memory until the app is
opened again.

`hotkey.rs`'s `key_still_down` is deliberately Windows-only. It works around a
`global-hotkey` polling artifact that only exists on Windows; returning false
elsewhere is the correct behaviour, not a stub.

## Permissions

macOS gates both halves of the core feature behind user consent:

- **Microphone** — `Info.plist` carries `NSMicrophoneUsageDescription`; without
  it macOS kills the process on first mic access.
- **Accessibility** — required for `enigo` to synthesize the paste keystroke
  and for the global hotkey. The OS prompts once, but nothing in the app
  explains it yet. First-run onboarding should, and should link to
  `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`.

Linux needs `x11-utils` (for `xprop`) for per-app profiles; the `.deb` declares
it. The AppImage does not, so per-app profiles silently degrade without it.

## Sidecars

`scripts/fetch-sidecars.sh` populates `src-tauri/sidecars/` for the host
platform. Pinned versions live at the top of that file.

| | macOS | Linux |
| --- | --- | --- |
| whisper.cpp | built from source (no upstream release asset; Metal by default) | `whisper-bin-ubuntu-x64` release asset |
| llama.cpp | `bin-macos-arm64` asset | `bin-ubuntu-x64` asset |
| piper | `piper_macos_aarch64` | `piper_linux_x86_64` |
| sherpa-onnx | `osx-arm64-shared` | `linux-x64-shared` |

Only whisper is fatal to the build. The other three back optional features and
are skipped with a log line if their download fails.

The script also fixes each binary's rpath (`@loader_path` / `$ORIGIN`), since
Tauri flattens the resources next to the app executable and they must find
their sibling libraries there.

Still missing: **code signing and notarization**. An unsigned `.dmg` is
refused by Gatekeeper until the user right-clicks → Open. Updater artifacts are
disabled on both platforms (`createUpdaterArtifacts: false`) because no signing
key is wired up for them — a release with no `.sig` would be permanently
un-updatable.

## Wayland

Not viable. Wayland deliberately blocks one application from sending synthetic
keystrokes to another, which is exactly what "hold a key, speak, paste at the
cursor" requires. X11 is fine. This is a security boundary, not a missing
feature, and it will not be lifted.

## What is left

1. Run it. Nothing below is worth doing before someone launches the `.dmg`
   or installs the `.deb`. Grab them from the workflow's artifacts.
2. macOS permission onboarding (see Permissions above).
3. Code signing + notarization, then re-enable updater artifacts.
4. Inline proofreading, if ever — `AXUIElement` on macOS, AT-SPI on Linux.
   Over half the remaining porting cost, for one feature. Ship without it.
