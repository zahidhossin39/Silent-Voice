# Implementation Plan: Transcribe a file (Meeting Recorder, step 1)

## Overview
Drop an audio or video file (up to 1-2 hours) into a new "Transcribe" page and get a
full transcript with timestamps, which you can read, copy and save. Fully offline.
Built as three reusable parts so step 2 (record mic + computer sound) feeds its
audio into the same chunker and job runner.

## What research + our code showed
- **Decoding:** `symphonia` (pure Rust) reads mp3, m4a/aac, mp4/mov audio, wav, flac,
  ogg-vorbis, mkv. **No Opus** (WhatsApp voice notes, .webm) — needs a C library or
  a clear "not supported" message. No ffmpeg shipped.
- **Our WAV reader is not usable for user files:** `read_wav_mono_f32`
  (sherpa_stt.rs:516) ignores the sample rate, and scales 24/32-bit ints wrong.
- **Long-audio splitting already exists:** `segment_bounds` / `silence_midpoints` /
  `build_segments` / `quietest_cut` in sherpa_stt.rs split at pauses into ~12 s
  pieces, lossless, with an empty-segment retry. Parakeet must stay segmented
  (attention memory grows with length squared). Whisper's window is 30 s, so the
  same 12 s pieces suit both engines.
- **But it needs the whole clip in memory and one VAD pass over all of it.**
  1 h of 16 kHz f32 = 230 MB, 2 h = 460 MB. Streaming (decode → resample → VAD →
  cut → transcribe → drop) keeps memory flat and gives progress for free.
- **Resampling:** the mic path uses plain linear resampling (capture.rs:216) and
  dictation is fine. Gemini says a filtered resampler (`rubato`) is needed for
  44.1/48 kHz files. Unproven: we measure on a real file first and add it only if
  accuracy drops.
- **Step 2 check (not built now):** cpal 0.15.3 already does WASAPI loopback
  (input stream on an output device, device.rs:570). Known gaps verified in its
  source: it waits forever when nothing plays (stream.rs:241, INFINITE) and never
  checks the SILENT flag, so step 2 must pad silence by the clock. Keep mic and
  speaker as two separate tracks: no mixing, and You/Others labels come free.

## Architecture Decisions
- **New `audio/decode.rs`:** streams any supported file as 16 kHz mono f32 chunks
  through a callback. Why: memory stays flat for 2 h files and step 2 never needs it.
- **New `audio/longform.rs` (`Chunker`):** push samples in, get finished
  `Segment { start_ms, end_ms, samples }` out, cut at pauses. Move the existing
  splitting helpers here so dictation and files share one tested copy. Why: step 2
  pushes live audio into the same struct.
- **New `transcribe_job.rs`:** one job at a time, runs decode → chunker → engine per
  segment, emits progress and segments as events, cancel flag checked between
  segments. Takes the STT lock per segment, so dictating still works mid-job.
- **Text clean-up reuses** `strip_fillers`, `collapse_repeated_words`, and
  `repunctuate` (English), applied per paragraph so the punctuation model never
  sees a whole hour at once.
- **Storage:** one JSON per transcript in `%APPDATA%\SilentVoice\transcripts\`,
  saved as segments arrive, so a crash or close keeps what was done.
- **File picking:** drag-and-drop (built into Tauri, gives the path) plus a Browse
  button via the official `tauri-plugin-dialog`. Why: a browser file input gives
  bytes, not a path, which would mean copying a 1 GB video through IPC.

## Task List
See `tasks/todo.md`.

## Risks and Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| AAC in mp4/m4a decodes badly in symphonia | High | Task 1 tests a real m4a and mp4 first; fail fast before UI work |
| Linear resampling hurts accuracy on 44.1/48 kHz music-backed audio | Med | Compare against a clean 16 kHz version of the same file; add `rubato` only if worse |
| Silero VAD too slow over 2 h (~225k frames) | Med | Streaming VAD runs alongside decoding; measure on the 1 h file |
| Whisper path slow on long files (CPU) | Med | Report speed honestly; Parakeet is the recommended engine for files |
| Punctuation model on very long text | Low | Repunctuate per paragraph group, not the whole transcript |
| A dictation during a job | Low | STT lock per segment; dictation waits at most one segment (~1-3 s) |

## Decisions (from /grill-me, 2026-10-16)
1. **Opus:** not supported for now; a clear message names the formats that work.
2. **History:** transcripts live only on the Transcribe page. History and Progress
   stats stay for dictations.
3. **Closed mid-job:** the partial transcript is kept, marked "Stopped at mm:ss",
   with a **Resume** button that continues from the last finished segment if the
   file is still there.
4. **Save:** Save-as dialog (tauri-plugin-dialog), formats .txt, .md (with
   timestamps), .srt.
5. **Test files:** public-domain LibriVox mp3s (~10 min and ~1 h) in the scratch
   folder, not the repo.
