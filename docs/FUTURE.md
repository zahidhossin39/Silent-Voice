# Future features

Planned, not being worked on now. Each entry says what it is, why, and what we
already know, so it can be picked up later without redoing the research.

## Notetaker: real names for speakers

**Today:** after Stop, the Others track is split by voice into Speaker 1,
Speaker 2… (sherpa-onnx diarization, `src-tauri/src/system/diarize.rs`). It
never knows anyone's name.

**Goal:** show "Siam" instead of "Speaker 1".

**Ways to get names (best first):**

1. **Read the meeting app's window.** Windows accessibility (UI Automation,
   the same layer screen readers use, and that inline proofreading already
   uses) can read another app's UI. Checked 2026-10-21: the Discord window
   exposes 468 readable elements, 63 about voice/mute/call, so participant
   names are readable. Open question: does it also expose *who is speaking
   now* (the green ring)? If yes, sample it every ~0.5 s while recording,
   line it up with the Others audio, and label lines with the name directly.
   - Next step: a 1-minute probe during a live Discord call, logging what
     changes in Discord's UIA tree when each person talks.
   - Then Zoom desktop (shows the active speaker's name) and Google Meet
     (runs in Chrome; its page is readable through the same layer).
   - Each app needs its own small reader, and an app update can break it.
2. **Remember voices.** Name a speaker once ("Speaker 1 is Siam"); save a
   voice print (the bundled sherpa-onnx DLL has SpeakerEmbeddingManager).
   Later meetings match voices to saved names automatically. Prints stay on
   this PC.
3. **Rename by hand.** Click "Speaker 1", type a name; every line by that
   speaker in the meeting changes. Needed anyway as the fallback, and it is
   how voice prints get their names.
4. **AI guess from the conversation.** People say each other's names
   ("সিয়াম ভাই…"); after the summary, suggest "Speaker 1 is probably Siam"
   for one-click confirm. Only works when names are spoken.

**Suggested order:** the Discord probe first. If the speaking signal exists,
build 1 for Discord, plus 3 as the fallback. Then add 2, then Zoom and Meet.

## Notetaker: record-only meetings + an MCP server for any agent

**Idea (2026-11-11):** a "Record only" meeting mode. Silent Voice records the
two tracks (You, Others) and transcribes nothing: no STT API, no local model.
The app runs an MCP server, and any agent with MCP support (Claude, Gemini,
others) connects to it and does the transcription, speaker labels and summary.

**Decided:**
- The app's own transcription stays as an option. Record-only is a choice,
  not a replacement.
- Any MCP-capable agent can connect, not one vendor.
- The rejected alternative: a built-in "Refine with Gemini" step.

**What we know:**
- Tools for a first version: `list_meetings`, `get_meeting_audio` (gives the
  WAV paths, not the bytes), `save_transcript` (lines with time, You/Others,
  speaker).
- Audio is big: an hour is 50-100 MB of WAV. MCP messages are JSON/base64, so
  hand the agent a file path and let it upload the file itself (for example
  Gemini's Files API with the agent's own key).
- Not every agent can listen. Gemini takes audio directly; it was our
  reference for speaker labels (~88% agreement with our clustering). Claude
  models take no audio input, so a Claude agent needs a transcriber of its own
  (an STT tool or API). Document that per agent.
- Transport: stdio (`silent-voice --mcp`, forwarded via single-instance like
  `--start/--stop`) or streamable HTTP on localhost. Decide when building.

**Next step:** grill the design (transport, tool shapes, how a saved
transcript shows up on the Notetaker page), then test with agy / Gemini CLI.

## Mobile app (Android / iOS)

**Status (2026-11-12):** starting in a new session. Nothing built yet.

**What carries over:** Tauri v2 builds for Android and iOS, so the React
dashboard (`src/`), the Zustand stores, `services/catalog.ts`, the cloud
providers (speech-to-text and summaries over HTTP), and the transcript /
meeting data model (`transcripts.rs`) can be reused. `services/tauriBridge.ts`
is the seam: every call into Rust goes through it.

**What is desktop-only and needs a mobile answer:**
- *Getting text into other apps.* Desktop pastes at the cursor after a global
  hotkey (`system/hotkey.rs`, `system/paste.rs`) and shows the overlay pill
  (`system/overlay.rs`). Phones have neither: the usual answer is a custom
  keyboard (Android input method / iOS keyboard extension), which Tauri does
  not build. This is the main product decision.
- *Local engines run as sidecar programs.* whisper.cpp (`transcription/whisper.rs`)
  and llama-server (`llm/`) are separate .exe files the app starts; iOS does not
  allow that and Android makes it awkward. sherpa-onnx (Parakeet, Moonshine,
  SenseVoice, TTS, speaker labels) is a library called over FFI
  (`system/sherpa.rs`, `sherpa_stt.rs`) and has Android/iOS builds, so it is the
  likely local engine. Its version is pinned (ADR 0002).
- *Desktop integrations:* tray, inline proofreading (UIA / Accessibility /
  AT-SPI), per-app profiles (`system/foreground.rs`), system-audio capture for
  meetings (ADR 0001: WASAPI / ScreenCaptureKit / PulseAudio; phones generally
  don't allow recording other apps' sound), the in-app updater (app stores
  update instead).
- *Paths:* data root is `%APPDATA%\SilentVoice\` on Windows (`system/paths.rs`);
  mobile needs the app sandbox directories.
