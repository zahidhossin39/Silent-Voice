# Notetaker and file transcription

Area: the Notetaker meeting recorder, "Transcribe a file", the transcript data
model, summaries, and export. Written 2026-11-12 from the code (the code wins
where it disagrees with the docs). Deep detail that already exists:
[HANDBOOK section 17](../HANDBOOK.md) (capture per OS, echo gate, echo removal,
speaker labels, replay tests), [ADR 0001](../adr/0001-system-audio-capture-per-os.md)
(why one interface and a native API per OS), [ADR 0002](../adr/0002-sherpa-onnx-one-pinned-version.md)
(why sherpa-onnx is pinned), [HANDBOOK section 12](../HANDBOOK.md) (command table)
and [section 13](../HANDBOOK.md) (events). Planned, not built: real speaker names and
a record-only mode with an MCP server, both in [FUTURE.md](../FUTURE.md).

## 1. Feature list

Settings live in `settings.notetaker` (type `NotetakerSettings` in
`src/types/index.ts`, saved with the rest of the settings). "Gear" means the
settings popover on the Notetaker page.

Notetaker (meetings):

1. Record a meeting: your mic as "You" plus the computer's sound as "Others". Start / Stop button on the page. No setting.
2. Live transcript while recording, one line per pause-cut piece, saved as it grows. No setting.
3. Recording indicator: red dot on the sidebar item and on the tray icon. No setting.
4. Others capture check and fix-it note ("Not recorded", Open System Settings on macOS). No setting.
5. You-only fallback when Others cannot be captured or the device dies mid-way (the transcript `error` says why). No setting.
6. Mic warnings: Bluetooth headset, mic sending silence, mic hearing the speakers. No setting.
7. Echo gate and per-piece echo removal, plus a text-level echo check. No setting (always on).
8. Meeting language, including several languages at once ("Also spoken"). Gear > Language spoken (`notetaker.language`, empty = dictation language).
9. Wrong-script redo for mixed-language calls (automatic when several languages are picked). Same setting.
10. Transcribe with this PC's voice model or a cloud provider. Gear > Transcription (`notetaker.stt`, `notetaker.stt_models`).
11. Tell speakers apart (Speaker 1, 2...) after Stop. Gear > Tell speakers apart (`notetaker.speakers`; needs the speaker models download).
12. "People besides you" picker (fixes the voice count, relabels only). Meeting header.
13. Punctuation repair at Stop (English only, needs the small repair model). No setting.
14. Transcribe again: redo a saved meeting from its saved audio with today's model. Meeting header; Cancel keeps the old text.
15. Overview (AI summary, key points, decisions, action items), Summarize / Redo. Gear > Summary (`notetaker.summary_with`, `notetaker.summary_models`).
16. Play the meeting (both tracks mixed), click a timestamp to play from there, seek bar, speed 1x to 2x. No setting.
17. "N min of silence" markers in the transcript. No setting.
18. Search across meeting names, Overviews and words. Search box on the page.
19. Rename (click the title), delete (two clicks), unnamed meetings show "Meeting at <time>".
20. Copy (Overview plus labelled paragraphs) and Save as .txt / .md / .srt.
21. "Let people know you're recording" note, shown once. `notetaker.consent_seen`.
22. Recovery: a meeting cut short by closing the app is kept ("cut short") and can be transcribed again from its audio.

Transcribe a file:

23. Drop or choose an audio / video file and get a timestamped transcript. No setting (uses the dictation model, language and vocabulary).
24. Live progress, percent and time left, Cancel.
25. Resume after Cancel, an error, or closing the app ("Resume" / "Try again").
26. Rename, delete, Copy, Save as .txt / .md / .srt (same header as meetings).
27. Local only: a file never goes to a cloud speech-to-text provider.

## 2. How it works

### Shared pieces

- **Data model** (`src-tauri/src/transcripts.rs`): a `Transcript` has `kind` (`file` or `meeting`), `status` (`running`, `done`, `stopped`, `failed`), `lines` (`start_ms`, `end_ms`, `text`, optional `speaker` You/Others and `who` 1, 2...), `done_ms`, `duration_ms`, `work_ms`, `error`, `summary`, `redo` and `others` (the picked voice count). The page mirrors it as `Transcript` in `src/services/tauriBridge.ts`.
- **Storage**: one folder, `transcripts/` next to `history.json` (`%APPDATA%\SilentVoice\transcripts\` by default; it follows the data location setting because it is derived from `history_path()`). Per id (the start time in ms): `<id>.json`, and for meetings `<id>-you.wav` and `<id>-others.wav` (16 kHz mono 16-bit). `<id>.json.before` is the old text while a Transcribe again runs. Writes go to a temp file then rename, so a crash never leaves half a file.
- **One job slot** (`transcribe_job.rs`: `claim`, `release`, `finish`, `unless_running`): a file job, a meeting recording, or a Transcribe again at a time. Rename, delete and summarize save only when no job is writing that transcript.
- **Events**: the backend sends the full `Transcript` on `transcript://update`. Files: after each piece and at most once a second for progress. Meetings: on new text, on a problem, at the end, and when speaker labels arrive. The Transcribe page ignores `kind: "meeting"` and the Notetaker ignores files.
- **Startup**: `transcripts::mark_interrupted()` (called in `lib.rs` setup) turns any `running` transcript into `stopped`; an unfinished Transcribe again gets its `.before` text back.
- **Per-piece engine** (`transcribe_job::transcribe_piece`): trim leading silence (`audio/gate.rs`), write a temp WAV, call `transcription::whisper::transcribe_dispatch` (local engine, or cloud with a local fallback), then strip spoken fillers and repeated words (`system/textfmt.rs`). Used by both files and meetings.
- **Cutting audio into pieces** (`audio/longform.rs`, `audio/vad.rs`): Silero VAD finds pauses; a piece ends at a pause, at most 12 s (`MAX_SEGMENT`). Meetings also cut at the first pause once 2 s is held (`cut_at_pauses(TURN)`), so a line is usually one person.
- **Rendering and export** (`transcripts.rs`): `paragraphs()` groups about 6 lines of the same speaker (waiting for a sentence end, up to 12), `render()` writes `.txt` (paragraphs), `.md` (title, Overview, timestamped paragraphs) or `.srt` (one cue per line). The page has a TypeScript copy of `paragraphs` and `label` in `tauriBridge.ts`; they must stay in step (a test, `src/services/transcriptParagraphs.test.ts`, covers it).

### Notetaker flows (`src-tauri/src/meeting_job.rs`, `src/components/dashboard/Notetaker.tsx`)

- **Start** (`meeting_start(cloud, language, speakers)`): reads settings (`Cfg::load`, refuses until the dashboard has pushed them), claims the slot, saves an empty transcript, then opens two `audio::live::Live` captures: the mic (`Source::Mic`, cpal) and the computer's sound (`Source::Speakers`, `audio/system_audio.rs`). Each capture thread writes its WAV as audio arrives and sends 0.1 s chunks to one worker thread over a channel. If You cannot start, nothing is kept. If Others cannot start, the meeting records You only and sets `error`. The tray icon gets a red dot (`system/tray.rs::set_meeting_recording`). `live.rs` pads each track with silence from the wall clock so the two stay in step when loopback stalls.
- **Worker** (`meeting_job::run`): per track a chunker cuts pieces; pieces are transcribed one at a time and inserted in time order (`add_line`). Three echo defences, in order: the echo gate mutes mic parts that are only speaker sound (`audio/echo.rs::EchoGate`), `only_yours` / `echo::without_echo` subtract Others from each You piece and drop pieces with no voice left, and `add_line` / `is_echo` drop a You line whose words repeat an Others line within 3 s. Details: HANDBOOK 17. Three pieces failing in a row sets an error but keeps recording. Progress is saved every 10 s and on new text.
- **Stop** (`meeting_stop`): stops both captures; the worker finishes the queue, repairs punctuation per paragraph (`repunctuate`, English only, via `system/sherpa_stt.rs::repunctuate_from`), sets `Done`, then runs `tell_speakers_apart` if the switch was on.
- **Speaker labels** (`tell_speakers_apart`, `system/diarize.rs`): runs in a background thread after the text is saved (about 15 s per minute of audio). Needs the models in `models/speakers/` (`segmentation.int8.onnx` + `voice.onnx`, downloaded by `download_diarize_model`, checked by `diarize_installed`, removed by `delete_diarize_model`). Labels Others lines, may cut a two-voice line and transcribe the parts again, then emits an update. `meeting_set_people(id, others)` stores the count and reruns only this step.
- **Language and wrong-script redo** (`cfg_for` in `meeting_job.rs`, `transcribe_job.rs`): a value like `"bn,en"` becomes `language = "auto"` plus a `languages` list. If a piece comes back in a script none of the picked languages use (`foreign_script`), it is decoded again in the first language that has its own script (`redo_language`), and the rest of that job goes straight to that language (`redo_first`). Latin letters always pass.
- **Cloud transcription**: the page passes the provider (`base_url`, `api_key`, `model`) to `meeting_start`; each piece goes to `openai::transcribe_audio` and falls back to the local model on failure. The API key travels from the page's settings on every call.
- **Transcribe again** (`meeting_retranscribe`): needs both WAVs; saves `.before`, clears the lines, sets `redo`, then `feed()` replays the WAVs as if live through the same worker. Cancel is `file_transcribe_cancel`; the old text is restored.
- **Playback and search**: `meeting_audio_clip(id, speaker, start_ms, end_ms)` returns a WAV (both tracks mixed when `speaker` is null, capped at 10 min); the page fetches 2-minute windows (`usePlayer`). `meetings_search(query)` scans every saved transcript's name, Overview and lines.
- **Overview** (`meeting_summarize`, `meeting_summary.rs`): `source` "local" uses the bundled llama-server (`ensure_local_llm`, 2,000-token pieces) and "api" an OpenAI-compatible provider (`llm::openai::chat_long`, 16,000-token pieces, 15 min timeout). Long meetings are noted in pieces, combined in up to 4 rounds, then one final call writes Summary, Key points, Decisions, Action items. Result saved on `summary`. A second Summarize on the same meeting is refused. Reasoning-model `<think>` text is stripped.
- **Capture check** (`meeting_capture_support`, `meeting_open_capture_settings`, `meeting_devices`): the page asks on load which of Others can be recorded and which mic and speakers will be used (for the Bluetooth warning, a name match on "hands-free" or "bluetooth").
- **Export** (`transcript_export(id)`): Rust shows the Save dialog (`tauri-plugin-dialog`) so the page never picks the path; the extension picks the format. Copy uses `copy_to_clipboard` (arboard).

### Transcribe a file (`transcribe_job.rs`, `src/components/dashboard/Transcribe.tsx`)

`file_transcribe_start(path?, resume?)` claims the slot, opens `audio::decode::FileDecoder` (symphonia, pure Rust: mp3, m4a/aac, mp4/mov, wav, flac, ogg/vorbis, mkv; Opus and most webm are refused with a plain message), and runs on its own thread: 1-second chunks go through the chunker, each speech piece is transcribed, lines are saved and sent after every piece, and every 6 lines are punctuation-repaired together (English only). `Resume` decodes again and discards audio up to `done_ms` (exact, no seeking). `file_transcribe_cancel` sets the stop flag; what is done stays saved. Paths come from the Tauri drag-drop event or `pickMediaFile` (dialog plugin), so a large video is read in place.

## 3. Platform dependence

Pure logic (portable Rust or TypeScript):

- `transcripts.rs` (data model, JSON storage, search, WAV clip cutting with `hound`, paragraphs, txt/md/srt rendering). Only `dir()` is OS-tied (via `models::registry::history_path`).
- `transcribe_job.rs` job logic (`run`, slot, resume, script check, `split_like`) and `meeting_job.rs` logic (`run`, `add_line`, `is_echo`, `only_yours`, `mic_hint`, `Recent`).
- `audio/echo.rs` (rustfft, GCC-PHAT, linear echo fit), `audio/longform.rs`, `audio/segmenter.rs`, `audio/decode.rs` (symphonia plus a resampler), `meeting_summary.rs` (token budgeting and prompts), `system/textfmt.rs`.
- Frontend: `Notetaker.tsx`, `Transcribe.tsx`, `MeetingOverview.tsx`, `TranscriptHeader.tsx`, the paragraph helpers in `tauriBridge.ts`, `useNotetakerSettings`. They only call Tauri commands through `services/tauriBridge.ts`. HTML `<audio>` does playback.
- HTTP only: cloud speech-to-text (`llm/openai.rs`, `transcription/whisper.rs` cloud branch), cloud summaries, the speaker-model download.

Calls into the OS or into native code:

- Mic capture and speaker names: cpal 0.15 (`audio/live.rs`): WASAPI on Windows, CoreAudio on macOS, ALSA/Pulse on Linux.
- Others capture (`audio/system_audio.rs`): Windows cpal WASAPI loopback (an input stream on the default output device); macOS ScreenCaptureKit audio-only stream (`system_audio_macos.rs`, crate `screencapturekit`, plus `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`, macOS 13+) ; Linux PulseAudio monitor through `libpulse-simple.so.0` loaded at run time (`system_audio_linux.rs`, `libloading`).
- `meeting_open_capture_settings`: macOS `open x-apple.systempreferences:...`.
- Silero VAD (`audio/vad.rs`) uses the `ort` crate in `load-dynamic` mode and loads sherpa's `onnxruntime.dll` by absolute path (`onnx.rs`). `gate.rs` falls back to a loudness gate if the model is missing.
- Local speech-to-text: the whisper.cpp sidecar program (Vulkan build), or sherpa-onnx engines (Parakeet, Moonshine, SenseVoice) over the C API via `libloading` (`system/sherpa.rs`, `sherpa_stt.rs`).
- Punctuation repair and speaker labels: sherpa-onnx C API via FFI (`sherpa_stt.rs`, `system/diarize.rs`, with `#[repr(C)]` structs pinned by ADR 0002).
- Local summaries: the llama-server sidecar program (`llm/llama.rs`).
- Tray red dot: Tauri tray icon API (`system/tray.rs`).
- Save dialog and file picker: `tauri-plugin-dialog`. Drag and drop: the Tauri webview drag-drop event. Copy: `arboard`.
- Data paths: `%APPDATA%\SilentVoice` logic in `models/registry.rs` and `system/paths.rs`.

## 4. Mobile portability

Notes for the table: Tauri mobile can reuse the React UI and every pure Rust module. Phones need app-sandbox paths, a microphone permission, and (to record with the screen off) a foreground service on Android or the background-audio mode on iOS.

| Feature | Carries over as-is / Needs rework / Desktop-only | Why |
| --- | --- | --- |
| Meeting data model, JSON storage, atomic save, startup recovery (`transcripts.rs`) | Carries over as-is | Pure Rust, serde and hound. Only `dir()` must point at the app sandbox. |
| Export .txt / .md / .srt (`render`) | Carries over as-is | Pure string code. |
| Save dialog (`transcript_export`) | Needs rework | Desktop file-save dialog; mobile needs the share sheet or a document-create intent. |
| Copy | Needs rework (small) | Uses `arboard`; use the webview clipboard or a mobile clipboard plugin. |
| Rename, delete, list, search, open a transcript | Carries over as-is | Plain JSON files and UI. Search re-reads every file, which may need an index on a phone. |
| Record You (mic track) | Needs rework | cpal can open a mic on Android and iOS, but permissions, audio session setup and background recording are needed. Channel and WAV writer logic carries over. |
| Record Others (computer's sound) | Desktop-only | WASAPI loopback, ScreenCaptureKit and PulseAudio have no phone equivalent; phones do not let an app record another app's sound (ADR 0001, FUTURE.md). |
| Capture support check and "Open System Settings" | Desktop-only | Exists only because Others is OS-specific. A mobile version would be a plain mic-permission prompt. |
| You-only fallback and device-failure message | Carries over as-is | Worker logic only; on mobile You-only becomes the normal case. |
| Echo gate, echo removal, text-level echo check | Carries over as-is (probably unused) | Pure Rust (rustfft). Needs an Others track to do anything, so it matters only if a mobile Others source is ever added. |
| Mic warnings (silent mic, hearing speakers) | Carries over as-is | `mic_hint` is pure. |
| Bluetooth mic warning | Needs rework | Name match on a cpal device name; mobile device names and routing differ. |
| Live chunking and transcription worker | Needs rework | Cutting and queue logic are pure, but VAD loads `onnxruntime.dll` through `ort` load-dynamic and the speech engine is a desktop sidecar or FFI DLL. |
| Local speech-to-text per piece | Needs rework | whisper.cpp sidecar is desktop-only; sherpa-onnx has Android and iOS builds but is loaded by `libloading` from a DLL today. |
| Cloud speech-to-text for meetings | Carries over as-is | HTTP only. |
| Meeting language list and wrong-script redo | Carries over as-is | Pure string and Unicode-range logic; effect depends on the engine. |
| Punctuation repair at Stop | Needs rework | Logic is pure, but it calls sherpa-onnx through desktop DLL loading. |
| Tell speakers apart (diarization) and People picker | Needs rework | sherpa-onnx FFI; about 15 s of CPU per minute of audio may be too slow or hot for a phone. Model download is plain HTTP. |
| Transcribe again (redo from saved audio) | Carries over as-is | Reads saved WAVs; depends on the engine rows above. |
| Overview with a cloud provider | Carries over as-is | `meeting_summary.rs` plus HTTP chat. |
| Overview with the local model | Desktop-only | Needs the llama-server sidecar; a phone needs an in-process llama.cpp library instead. |
| Playback, seek, speed, timestamp-to-play | Carries over as-is | WAV bytes over IPC into an HTML audio element. Check IPC size limits for 2-minute windows on mobile. |
| Silence markers, consent note, Notetaker settings | Carries over as-is | UI and settings store only. |
| Recording indicator on the sidebar | Carries over as-is | UI only. |
| Recording indicator on the tray icon | Desktop-only | Tray does not exist on phones. A foreground-service notification replaces it. |
| Transcribe a file: decode (symphonia), chunk, run, progress, cancel | Carries over as-is | Pure Rust; the engine rows above apply. |
| Transcribe a file: drop and choose a file | Needs rework | Drag-drop is desktop-only; Android and iOS return content URIs or security-scoped URLs, not plain paths, so `File::open(path)` needs a copy or a mobile file plugin. |
| Resume, Try again | Carries over as-is | Needs the source file to still be readable (a URI grant may expire on mobile). |
| Files stay local, never cloud | Carries over as-is | A rule in `Cfg::load`. Revisit for mobile: local engines may be too slow, so a cloud option may be wanted. |
| Single job slot | Carries over as-is | In-process lock. |
| Self-test env vars (`SV_SELFTEST_MEETING` and similar) | Desktop-only | Developer and CI hooks. |

## 5. Gaps: code that the HANDBOOK does not describe or describes wrongly

Not described:

1. **Commands missing from section 12**: `meeting_retranscribe`, `meeting_set_people`, `meetings_search`, `meeting_audio_clip`, `download_diarize_model`, `diarize_installed`, `delete_diarize_model`. The `meeting_start` row also lacks its arguments (`cloud`, `language`, `speakers`).
2. **Section 3 (project structure) and section 7 (data locations)** list none of: `transcripts.rs`, `transcribe_job.rs`, `meeting_job.rs`, `meeting_summary.rs`, `audio/echo.rs`, `audio/live.rs`, `audio/longform.rs`, `audio/decode.rs`, `system/diarize.rs`, nor the `transcripts/` folder (`<id>.json`, `<id>-you.wav`, `<id>-others.wav`, `<id>.json.before`) or the `models/speakers/` folder. The transcripts folder follows the custom data location, which section 7 does not say.
3. **Transcribe a file** has no section: the pipeline, local-only rule, Resume by decoding and skipping, supported formats, Opus refusal, and the 6-line punctuation repair are only in code comments.
4. **Wrong-script redo and multi-language meetings** (`"bn,en"` becomes auto plus a list, `redo_first`) are not in the handbook at all.
5. **Cloud transcription for meetings**, its local fallback, and that the cloud punctuation is repaired at Stop for English only.
6. **Meeting summary internals**: only the command row exists. The two token budgets, up to 4 combine rounds, `<think>` stripping, the 15 minute timeout and the busy guard are undocumented. The Gemini `frequency_penalty` skip is in `llm/openai.rs` only.
7. **Transcribe again** (backup file, cancel restores, startup restore), playback windowing, search, the 10 minute clip cap, the `kind` filter between the two pages, and the sidebar and tray recording dots.
8. **Text-level echo check** (`add_line` / `is_echo`, 3 s window) is a fourth echo defence that section 17 does not list.
9. **Self-test hooks** `SV_SELFTEST_TRANSCRIBE` and `SV_SELFTEST_MEETING` (section 17 only mentions `SV_SELFTEST_CAPTURE`).

Wrong or stale:

10. `meeting_job.rs` header says meetings are "Always local, like files". The code supports cloud speech-to-text for meetings (`cfg_for`, `Cloud`). Only files are local-only.
11. The speaker-model download size is inconsistent: the Notetaker page says "47 MB" (`SpeakersSetting`), the downloader comment says about 35 MB download and 30 MB on disk, section 17 says `voice.onnx` is 28 MB. The 47 MB looks left over from the 40 MB titanet model.
12. **"Tell speakers apart" default**: `legacyNotetaker()` sets `speakers: true`, but the backend does nothing until the models are downloaded, so the switch looks on with no effect on a fresh install. Not stated anywhere.
13. Section 13 says `transcript://update` carries meetings "on new text, a device problem, and the end"; it also fires when speaker labels arrive after the end.
