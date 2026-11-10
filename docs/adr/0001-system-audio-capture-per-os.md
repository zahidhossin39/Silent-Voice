---
status: accepted (2026-10-25; built on all three 2026-11; Linux proven in CI, macOS waits on a real call)
---

# Capturing the computer's sound: one interface, a native API per OS

The Notetaker's "Others" track is the computer's sound. cpal only gives that on
Windows (an input stream on the output device becomes WASAPI loopback), so we put
Others behind one Rust interface (`audio/system_audio.rs`: `support()`,
`open(tx)`, errors `Unsupported | PermissionDenied | Failed`) with a native
backend per OS: **Windows** keeps cpal/WASAPI loopback; **macOS** uses
ScreenCaptureKit audio-only capture (macOS 13+, the "Screen & System Audio
Recording" permission); **Linux** records the default sink's PulseAudio monitor
source through libpulse, **loaded at runtime**. When Others can't be captured, a
meeting still records "You" and the page says why and how to fix it.

## Considered options

- **macOS virtual driver (BlackHole, Zoom's own driver):** needs an admin
  install and Audio MIDI Setup changes. Rejected: ScreenCaptureKit needs neither,
  and Otter, Granola and OBS use it.
- **macOS Core Audio process taps (14.2+):** narrower OS support, and taps on
  other apps often need a privileged helper. Rejected for now.
- **Linux native PipeWire / cpal 0.18's PulseAudio host:** PipeWire desktops
  also serve the PulseAudio protocol, so libpulse covers both. Upgrading cpal
  from 0.15 would touch dictation capture on every platform for one feature.
- **Linking libpulse at build time:** simpler, but a system without libpulse
  would refuse to start the whole app, dictation included.

## Consequences

- The Notetaker needs macOS 13+ for Others; older Macs record "You" only.
- macOS 15 re-asks for the permission about monthly; the page explains it.
- GitHub's macOS runner can't grant the permission, so CI only proves the
  "permission needed" path; real Mac capture is tested on a person's Mac via a
  pre-release build (v0.1.15 before v0.2.0).
- Research and sources: `docs/research/cross-platform-meeting-audio.md`.
