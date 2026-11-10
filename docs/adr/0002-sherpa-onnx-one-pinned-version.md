---
status: accepted (2026-10-25; pin, load-time check and layout tests built 2026-11, CI runs them on all three)
---

# sherpa-onnx: one pinned version everywhere, checked at load and in CI

We call sherpa-onnx's C API through hand-written `#[repr(C)]` structs, loaded at
runtime with libloading, so a library whose structs differ from ours reads every
field after the change at the wrong offset: no compile error, just garbage or a
crash. This happened: Windows ships v1.13.4 (DLL committed to the repo), macOS
and Linux downloaded v1.13.6, which added `window_shift_ratio` to the pyannote
segmentation config, so speaker labels would have broken there. We therefore pin
**one version for all platforms** (v1.13.4, in one `VERSION` file read by
`fetch-sidecars.sh`, next to the committed `c-api.h`), **refuse a mismatched
library at load** (`SherpaOnnxGetVersionStr` must equal `VERSION`, logged at
startup), and **test struct layouts** with `size_of`/`offset_of!` whose expected
numbers come from a C program compiled against the committed header.

## Considered options

- **bindgen from the header:** generates correct structs with layout tests, but
  adds a build-time C toolchain dependency on Windows for a few structs. Revisit
  if we mirror many more.
- **Track the newest release on every platform:** means updating the committed
  Windows DLLs and re-checking every struct on each upgrade. Pinning keeps
  upgrades a deliberate step.
- **Hand-typed layout numbers:** a first draft of the test had wrong numbers
  (32/96 instead of the real 24/56); generating them from C avoids that.

## Consequences

- Upgrading sherpa-onnx = change `VERSION`, replace the Windows DLLs and the
  header, run the layout check: any changed struct fails CI before release.
- A wrong library makes sherpa features (TTS, some STT engines, punctuation,
  speaker labels) report a clear error instead of misbehaving.
