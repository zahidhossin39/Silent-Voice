# Porting to macOS and Linux

Status: **builds, installs, and launches on macOS and Linux.**

`.github/workflows/cross-platform-build.yml` installs the real `.deb` / `.app`
on a GitHub runner, launches it (headless under xvfb on Linux), and fails unless
the app writes its own `Silent Voice starting` line. It then runs the bundled
whisper CLI against a known clip and requires the right words back. Both are
gates, not diagnostics.

`cross-platform.yml` stays as the fast `cargo check` on every push — but note it
did NOT catch the missing `libxdo`, because checking does not link. Only the
packaging build proves the link step, and only the smoke test proves it runs.

## What is still unverified

CI proves the app starts and that transcription works. It cannot press a key, so
nothing below has ever been exercised:

| Unverified | Why CI cannot cover it |
| --- | --- |
| Hold-hotkey → speak → paste | needs a real microphone and a focused text field |
| macOS Accessibility banner | needs a Mac where the permission is denied |
| Per-app profiles | needs real windows to focus |
| Read-aloud (TTS) | needs an audio output device |
| The updater | needs a published release to update from |

**These are yours to check.** Everything in that table is reasoned from code.

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

Auto-update is on for all three platforms. The updater key is minisign, which
is platform-independent, so the one existing secret signs everything, and the
release workflow refuses to publish any platform whose `.sig` is missing.

Still missing: **code signing and notarization**, which is a different thing
from updater signing. It needs a paid Apple Developer membership and a
certificate tied to a real Apple ID — there is no way around that. Until then
Gatekeeper refuses the `.dmg` until the user right-clicks → Open.

## Wayland

Not viable. Wayland deliberately blocks one application from sending synthetic
keystrokes to another, which is exactly what "hold a key, speak, paste at the
cursor" requires. X11 is fine. This is a security boundary, not a missing
feature, and it will not be lifted.

## What is left

1. Run it. Nothing below is worth doing before someone launches the `.dmg`
   or installs the `.deb`. Grab them from the workflow's artifacts.
2. macOS permission onboarding (see Permissions above).
3. Code signing + notarization (needs a paid Apple Developer account).
4. Inline proofreading, if ever — `AXUIElement` on macOS, AT-SPI on Linux.
   Over half the remaining porting cost, for one feature. Ship without it.
