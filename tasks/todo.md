# Record a meeting (Meeting Recorder, step 2): tasks

## Phase 1: Capture (highest risk first)

### Task 1: Computer sound as a 16 kHz stream
**Description:** `audio/live.rs` (mic + loopback): open WASAPI loopback (cpal input stream on the default output device), downmix + resample to 16 kHz mono, pad silence by the wall clock while nothing plays.
**Acceptance criteria:**
- [ ] 10 min of YouTube playback gives 10 min ±1 s of audio, including paused stretches
- [ ] Stop returns promptly even while nothing plays
- [ ] Clock-padding logic unit-tested (gaps become silence of the right length)
**Verification:** `cargo test --lib loopback`; ignored real-device test with a playback
**Files:** `src-tauri/src/audio/live.rs`, `src-tauri/src/audio/mod.rs`

### Task 2: Shared "transcribe a piece"
**Description:** Move piece transcription + cleanup out of `transcribe_job.rs` into one function both jobs use, with a unique temp WAV per call.
**Acceptance criteria:**
- [ ] File transcription unchanged (existing tests green)
- [ ] No fixed `file-piece.wav` name left
**Verification:** `cargo test --lib`
**Files:** `src-tauri/src/transcribe_job.rs`, new shared module or `transcripts.rs`

## Checkpoint A
- [ ] Tests pass; loopback measured on real playback

## Phase 2: The meeting job

### Task 3: Two tracks into one live transcript
**Description:** `meeting_job.rs`: start/stop commands; mic + loopback → two chunkers → one worker; lines carry `speaker`; saved and sent via `transcript://update` like files; Stop finishes the queued pieces.
**Acceptance criteria:**
- [ ] Lines from both tracks interleave by start time with You/Others labels
- [ ] Stop keeps everything heard; closing the app mid-way keeps what's done
- [ ] One job at a time across files and meetings
**Verification:** `cargo test --lib meeting` (fake sources); real 10-min run
**Files:** `src-tauri/src/meeting_job.rs`, `src-tauri/src/transcripts.rs`, `src-tauri/src/lib.rs`

### Task 4: Drop bleed duplicates
**Description:** A You line whose words mostly match an Others line within ±3 s is dropped.
**Acceptance criteria:**
- [ ] Unit test: echoed line dropped, a real reply kept
**Verification:** `cargo test --lib meeting`

### Task 5: Exports with speakers
**Description:** .txt/.md/.srt show "You:" / "Others:" when the speaker changes.
**Acceptance criteria:**
- [ ] File transcripts export exactly as before
**Verification:** `cargo test --lib transcripts`

## Checkpoint B
- [ ] Tests pass; a real 10-min call (video playing + talking) transcribed live

## Phase 3: UI

### Task 6: Record button and live view
**Description:** Record/Stop with a clear recording indicator, live You/Others lines, Bluetooth-mic warning, headphones hint. Meetings appear in the transcript list.
**Acceptance criteria:**
- [ ] Start, live text, Stop, then read/copy/save like a file transcript
- [ ] Indicator visible the whole time it records
**Verification:** `npx tsc --noEmit`, `npx vitest run`, manual run
**Files:** `src/components/dashboard/Transcribe.tsx` (or a new page), `src/services/tauriBridge.ts`

## Checkpoint C: Verify for real
- [ ] Real 30-min+ meeting: lag behind live, CPU and memory reported
- [ ] /code-review on experiment vs main, fixes applied
- [ ] Handed to the user to test
