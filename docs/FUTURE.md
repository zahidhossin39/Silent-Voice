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
