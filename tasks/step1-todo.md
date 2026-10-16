# Tasks: Transcribe a file (step 1)

## Phase 1: Foundation (riskiest first)

### Task 1: Decode any file to 16 kHz mono, streamed
**Description:** `audio/decode.rs` with `decode_file(path, on_chunk) -> Result<Info>` using symphonia (mp3, aac, isomp4, flac, wav, ogg/vorbis, mkv). Downmix to mono, resample to 16 kHz with the existing linear resampler, hand out chunks, report total duration up front for progress. Opus and unknown formats return a clear, human message.
**Acceptance criteria:**
- [x] A 44.1 kHz stereo WAV made in the test decodes to the right length at 16 kHz
- [x] A real mp3, m4a and mp4 each decode (ignored test with sample files)
- [x] An Opus/unknown file returns "This file type isn't supported yet…" not a panic
**Verification:** `cargo test --lib decode`; real-file ignored test passes
**Dependencies:** None
**Files:** `src-tauri/Cargo.toml`, `src-tauri/src/audio/decode.rs`, `src-tauri/src/audio/mod.rs`, `src-tauri/src/audio/capture.rs` (make resampler shared)
**Scope:** S

### Task 2: Streaming chunker (TDD)
**Description:** `audio/longform.rs` `Chunker`: `push(&[f32]) -> Vec<Segment>`, `finish() -> Vec<Segment>`. Runs VAD on new frames only, cuts at pause midpoints, ~12 s target, hard-splits long speech at the quietest point. Move `silence_midpoints` / `build_segments` / `quietest_cut` here; `sherpa_stt::transcribe_file` uses the moved helpers. VAD is injectable so tests run without the model.
**Acceptance criteria:**
- [x] Segments cover every sample exactly once, in order (property test over random masks and push sizes)
- [x] No segment longer than the cap; cuts land in silence when silence exists
- [x] Existing dictation tests still pass (shared helpers unchanged in behavior)
**Verification:** `cargo test --lib longform`; full `cargo test --lib`
**Dependencies:** None
**Files:** `src-tauri/src/audio/longform.rs`, `src-tauri/src/audio/mod.rs`, `src-tauri/src/system/sherpa_stt.rs`
**Scope:** S

## Checkpoint A
- [x] All tests pass; a real 10-min mp3 decodes and chunks (log segment count and time)

## Phase 2: Core flow

### Task 3: Transcription job with progress and cancel
**Description:** `transcribe_job.rs` + commands `file_transcribe_start(path)`, `file_transcribe_cancel()`. Decode → chunker → per segment through the normal local STT path via a temp segment WAV (sherpa engines could take samples directly; not needed at ~5x real time). Clean each segment (fillers, repeats), repunctuate per paragraph (English). One event, `transcript://update`, carries the whole transcript (lines, progress, status); step 2 (meeting recorder) reuses it. One job at a time.
**Acceptance criteria:**
- [x] A 10-min file produces timestamped segments and a finished event
- [x] Cancel stops within one segment and keeps the text so far
- [x] A dictation during a job still works
**Verification:** ignored integration test on a sample file; manual run in `tauri:dev`
**Dependencies:** 1, 2
**Files:** `src-tauri/src/transcribe_job.rs`, `src-tauri/src/lib.rs`
**Scope:** M

### Task 4: Save, list, export
**Description:** transcripts stored as JSON (`transcripts/<id>.json`), written as segments arrive with status running/cancelled/done/failed. Commands: list, get, delete, rename, export (.txt, .srt, .md).
**Acceptance criteria:**
- [x] Closing the app mid-job leaves a readable partial transcript marked "Stopped at mm:ss"
- [x] Resume continues from the last finished segment (decode skips to it) and the result has no gap or duplicate
- [x] Export writes valid .srt (checked by a unit test) and plain .txt
**Verification:** `cargo test --lib transcripts`
**Dependencies:** 3
**Files:** `src-tauri/src/transcripts.rs`, `src-tauri/src/transcribe_job.rs`, `src-tauri/src/lib.rs`
**Scope:** S

## Checkpoint B
- [x] End to end from a Tauri command: file in, saved transcript out

## Phase 3: The page

### Task 5: "Transcribe" page (designed with /impeccable)
**Description:** New sidebar page in the Progress page's style: drop zone + Browse button, live progress card (percent, elapsed, time left, Cancel), text appearing as it's done, list of past transcripts, reader with timestamps, Copy and Save.
**Acceptance criteria:**
- [x] Drop or browse a file and watch it fill in live
- [x] Past transcripts open, copy, save and delete
- [x] Unsupported file shows a friendly message
**Verification:** `npx tsc --noEmit`, `npx vitest run`, screenshot in light and dark
**Dependencies:** 3, 4
**Files:** `src/components/dashboard/Transcribe.tsx`, `src/App.tsx`, `src/services/tauriBridge.ts`, `src/components/shared/NavIcons.tsx`, `src-tauri/capabilities/default.json`
**Scope:** M

## Checkpoint C: Verify for real
- [x] Tests + typecheck pass
- [x] Real ~10 min file and 1 h file transcribed: time taken and peak memory reported
- [x] /code-review on experiment vs main, fixes applied
- [x] Handed to the user to test
