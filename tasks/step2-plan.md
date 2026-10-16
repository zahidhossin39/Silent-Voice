# Implementation Plan: Record a meeting (Meeting Recorder, step 2)

## Overview
Press Record, talk on Zoom/Meet/Teams, and watch a live transcript fill in with
each line labelled **You** (your mic) or **Others** (the computer's sound). Stop
gives a saved transcript like step 1's: read, copy, save as .txt/.md/.srt.
Fully offline. Step 3 (local-AI summary) builds on the saved transcript.
Step 1 is archived in `step1-plan.md` / `step1-todo.md`.

## What research + our code showed (Gemini, 2026-10-16)
- **Computer sound:** cpal 0.15 opens WASAPI loopback as an input stream on the
  default *output* device. It stalls while nothing plays (an OS behaviour; newer
  cpal doesn't fix it). Fix: pad silence by the wall clock, so the Others track
  stays in time with the mic. (Alternative: play silence to keep it pumping.)
- **Bleed:** on laptop speakers the mic also hears the others, so their words land
  under "You" too. Real echo cancellation (WebRTC AEC3) is heavy to build on Windows
  and drifts between two clocks. Granola/MacWhisper simply say "best with
  headphones". Cheap fix: drop a You line whose words match an Others line from
  the same few seconds.
- **Bluetooth headsets:** opening a BT headset's mic drops the whole call to
  low-quality "Hands-Free" audio. Warn when the chosen mic is Bluetooth.
- **CPU:** Parakeet runs ~5x real time, so two tracks of non-stop talk use ~40% of
  that budget. One worker transcribes pieces in arrival order; the STT lock already
  serializes them. Fix step 1's fixed `file-piece.wav` name before two tracks share it.
- **Consent:** all-party-consent laws (12 US states, GDPR) — show a clear
  recording indicator; offer a "tell people you're recording" reminder.
- **Audio size:** 16 kHz mono WAV is 115 MB/h per track (FLAC ~half).

## Architecture Decisions
- **New `audio/live.rs`:** (built as one module for the mic and loopback) system sound as 16 kHz mono chunks, silence padded
  by the clock. Same output shape as the mic, so both feed a `longform::Chunker`.
- **New `meeting_job.rs`:** two capture sources → two chunkers → one transcribe
  worker → one `Transcript` with a `speaker` on each line. Reuses step 1's storage,
  `transcript://update` event, paragraphing and punctuation repair.
- **`Line` gains `speaker: Option<Speaker>`** (`you`/`others`, absent for files), so
  old transcripts still load and the Transcribe page lists meetings too.
- **Shared runner bits:** move "transcribe one piece" out of `transcribe_job.rs`
  so both jobs call it (unique temp file per call).

## Task List
See `tasks/todo.md`.

## Risks and Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| Loopback stalls / drifts from the mic clock | High | Task 1 first: clock padding, measured on a real 10-min playback |
| Bleed duplicates lines under You | Med | Headphones hint + lexical dedupe, measured on speakers |
| Device unplugged mid-meeting | Med | Stream error ends the track cleanly; what's done is kept |
| Falling behind on slow PCs | Med | Show backlog; finish the queue after Stop |
| Bluetooth mic ruins call audio | Med | Warn before recording |

## Decisions (grill, 2026-10-17)
- **Placement:** a new **Notetaker** sidebar section (separate from Transcribe) laid out like Wispr Flow's
  Notetaker (screenshot 2026-10-17): title + "Start recording" button top right;
  "Past meetings" list grouped by day; a right panel with the meeting's title,
  date, Overview (filled by step 3's summary) and the You/Others transcript.
  Skipped from Flow: calendar connect (online), Shared/Folders tabs (teams),
  the chat box (step 3). The Transcribe page keeps showing files only
  (`Transcript.kind`: file | meeting, files by default).
- **Audio:** keep it, as 16 kHz mono WAV per track (~230 MB/h for both), next to
  the transcript, so a meeting can be re-transcribed or played back later.
- **Echo:** dedupe You lines that repeat Others + "Best with headphones" hint.
- **Consent:** recording indicator the whole time + a one-time "Let people know
  you're recording" note.
