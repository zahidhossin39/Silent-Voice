# Porting to macOS and Linux

Status: **not ported.** The Windows-only subsystems are now gated so the rest of
the crate can compile elsewhere, but nothing has been built or run on another
platform yet. `.github/workflows/cross-platform.yml` is what tells us how far it
gets — it is expected to fail at first, and its failures are the to-do list.

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

## What still needs a per-platform implementation

These already contain some `cfg(windows)` guards, but have no non-Windows path
behind them. Each is one function with three implementations, not a rewrite.

| Module | Windows uses | macOS | Linux |
| --- | --- | --- | --- |
| `autostart.rs` | registry Run key | LaunchAgent plist | `.desktop` in autostart |
| `foreground.rs` | `GetForegroundWindow` | `NSWorkspace` | X11 active window |
| `job.rs` | Job Objects to kill sidecars | process group | process group |
| `clipboard_file.rs` | clipboard formats | `NSPasteboard` | X11 targets |
| `secure_field.rs` | UIA password detection | accessibility API | AT-SPI |
| `hardware.rs` | DXGI for GPU | Metal / `system_profiler` | `/sys` or `lspci` |
| `hotkey.rs` | `GetAsyncKeyState` for hold | `CGEventSource.keyState` | `XQueryKeymap` |

The hotkey plumbing itself is already cross-platform — it goes through
`tauri-plugin-global-shortcut`. Only the "is the key still held" check is
Windows-specific.

## Sidecars

Every bundled binary is a Windows build: whisper.cpp, llama.cpp, piper,
sherpa-onnx, and the ONNX runtime. All of them publish macOS and Linux builds
upstream, so this is packaging work rather than porting. Swap the Vulkan build
for Metal on macOS.

`tauri.conf.json` currently targets `nsis` only, and the release workflow runs on
`windows-latest`. Both need extending for `.dmg` / `.AppImage`, along with
per-platform updater signing.

## Wayland

Not viable. Wayland deliberately blocks one application from sending synthetic
keystrokes to another, which is exactly what "hold a key, speak, paste at the
cursor" requires. X11 is fine. This is a security boundary, not a missing
feature, and it will not be lifted.

## Suggested order

1. Get `cross-platform.yml` green — fix whatever the gating missed.
2. Build the sidecars for the target platform and bundle them.
3. Fill in the shims in the table above.
4. Add `.dmg` / `.AppImage` targets and extend the release workflow.
5. Inline proofreading, if ever — `AXUIElement` on macOS, AT-SPI on Linux.
