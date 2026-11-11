# Dictation pipeline

Scope: everything between "press the hotkey" and "text is pasted and saved in history", plus the pill, the problem card, history and the Home/Progress numbers.

The code is the source of truth. Where it disagrees with [HANDBOOK.md](../HANDBOOK.md) (sections 6, 9, 13, 16) or [CLAUDE.md](../../CLAUDE.md), this file follows the code and section 5 lists the differences.

Main files:

- `src-tauri/src/system/hotkey.rs`: hotkey state machine, whole pipeline, sounds, Esc, paste-last, paste verification.
- `src-tauri/src/lib.rs`: `RuntimeConfig`, `AppState`, commands, shortcut routing, startup tasks.
- `src-tauri/src/audio/` (`capture.rs`, `gate.rs`, `vad.rs`, `segmenter.rs`) and `src-tauri/src/transcription/` (`whisper.rs`, `server.rs`).
- `src-tauri/src/system/` (`sherpa_stt.rs`, `textfmt.rs`, `soundalike.rs`, `paste.rs`, `foreground.rs`, `secure_field.rs`, `overlay.rs`, `notice.rs`, `tray.rs`).
- Frontend: `src/hooks/useRuntimeSync.ts`, `src/hooks/usePipeline.ts`, `src/components/overlay/`, `src/components/dashboard/` (Home, History, Progress, settings), `src/stores/`, `src/services/tauriBridge.ts`.

## 1. Feature list

Setting names are the keys in `settings.json` (see `src/stores/settingsStore.ts`). "none" means there is no setting.

### Trigger and capture

| Feature | Controlled by |
|---|---|
| Hold-to-talk global hotkey (default Ctrl+Shift+Space) | `hotkey` |
| Double-tap to lock hands-free recording | `toggle_mode` |
| Ignore false key-release events while the key is still held (Windows) | none |
| Esc cancels the current recording (Esc is grabbed only while recording) | none (off if the user made Esc one of their own hotkeys) |
| Tray menu "Start / Stop Recording" | none |
| Wayland compositor keybind: `silent-voice --start / --stop / --toggle` | none |
| Microphone choice | `audio_device` |
| Mic failure reported while recording (mic dies mid-dictation) | none |
| Live loudness to the pill and the Home waveform | none |
| Mic level meter in onboarding, plus onboarding "try it" recording | none |
| Start and stop sounds (4 packs plus off) and a preview button | `dictation_sound` |
| Background noise filter slider, backed by Silero voice detection (auto-downloaded, 2 MB) with an RMS fallback | `input_sensitivity` |
| "No speech" and "Is your microphone muted?" messages | none |

### Speech-to-text

| Feature | Controlled by |
|---|---|
| Whisper engine: resident `whisper-server`, with the one-shot `whisper-cpp` sidecar as fallback | `active_stt_model`, `language`, `use_gpu` |
| Parakeet (transducer), Moonshine and SenseVoice engines through sherpa-onnx | `active_stt_model` |
| Qwen3-ASR engine (speech LLM on the bundled llama-server) | `active_stt_model` |
| Cloud transcription with automatic fallback to the local model | `stt_cloud_provider_id` |
| CPU thread count | `high_performance`, `performance_threads` |
| Transcribe while you speak (background chunks) with the 7 MB punctuation-repair model | `chunk_on_silence` |
| Keep the model warm, preload on change, free it when idle | `model_unload_minutes` |
| Long-clip splitting for sherpa models (clips over 18 s) | none |
| Custom vocabulary: Whisper prompt, Parakeet hotwords, and the sound-alike fixer | `custom_vocabulary`, `use_custom_vocabulary`, `vocabulary_strength` |

### Text clean-up (after the engine, in order)

| Feature | Controlled by |
|---|---|
| Drop empty or "..." Whisper output, strip wrapping quotes | none |
| Strip spoken fillers (um, uh, hmm and similar) | none |
| Collapse repeated words | none |
| Spoken symbols ("slash", "dot md", "at sign", "hashtag", email "x at y dot com") | none |
| Sound-alike vocabulary fixer ("quad code" becomes "Claude Code") | `use_custom_vocabulary` |
| Trailing voice command "press enter" | none |
| AI writing mode (local or API) with per-app profile override | `active_mode_id`, `appProfiles` (owned by the Modes area) |
| "Fix grammar before pasting" (CoEdit) | `coedit_enabled` (owned by the grammar area) |
| Spoken structure commands ("new line", "new paragraph", "bullet point", "number one") | none |
| Number formatting ("twenty five percent" becomes "25%") | none |
| Text replacement shortcuts ("my email" becomes an address) | `snippets` |

### Paste

| Feature | Controlled by |
|---|---|
| Paste at cursor, then restore the user's clipboard | none |
| Add a space after each paste | `append_trailing_space` |
| Paste guards: password field, focus moved (re-focus the original window), macOS Accessibility missing | none |
| Paste verification: warns and copies to the clipboard if the text did not land (Windows) | none |
| Learn from edits: underline a word the user fixed after pasting (needs inline proofreading) | `inline_proofread` |
| Paste last dictation hotkey (default Alt+Shift+Z) and tray "Copy last dictation" | `paste_last_hotkey` |

### On-screen feedback

| Feature | Controlled by |
|---|---|
| Overlay pill: idle line, recording bars, processing, green check, blue read-aloud state | none |
| Drag the pill, position remembered, right-click menu (Hide, Quit) | none |
| Hide the pill 5 s after idle | `pill_auto_hide` |
| Tray icon and menu (Show/Hide Overlay, Open Dashboard, History, Quit) | none |
| On-screen problem card with a Copy logs / Open settings / Select microphone button | none |
| Error banner and screen-reader announcement in the dashboard | none |
| One-line paste log and error log (Settings > Logs) | none |

### History and stats

| Feature | Controlled by |
|---|---|
| History list: search, copy, edit, delete with undo, clear all | `history_limit`, `history_retention*` |
| Saved audio per dictation: play, copy file, keep last N | `save_audio`, `audio_clip_limit` |
| Re-transcribe a saved clip with the current model | none |
| Edit an entry and learn new words into the vocabulary | none |
| Blur history until hover | `blur_history` |
| Home: live status, 12-week dictation strip, today's words and time saved | none |
| Progress page: time saved, speaking speed, streaks, year heatmap, busiest hours, top apps | none |

## 2. How it works

### 2.1 Settings reach Rust

`useRuntimeSync.ts` watches the settings store and pushes them to Rust, so the hotkey pipeline (which runs with no UI) knows them.

| Tauri command | Sets |
|---|---|
| `update_runtime_config` | model, language, mic, vocabulary + toggle + strength, cloud STT fields, GPU. Rebuilds the model 1.5 s after the last change. |
| `set_behavior` | toggle_mode, input_sensitivity, inline_proofread, threads, pill_auto_hide, append_trailing_space, coedit, chunk_on_silence, save_audio, clip limit, idle unload, sound pack. Debounced 150 ms. |
| `set_hotkey`, `set_paste_last_hotkey` | Re-register the global shortcut. |
| `set_text_replacements`, `set_active_mode`, `set_app_profiles` | Snippets, AI mode, per-app mode rules. |

All of it lives in the `RuntimeConfig` struct inside `AppState` (`lib.rs`). `config_pushed` flips to true on the first `update_runtime_config`; startup preload waits for it.

### 2.2 Hotkey and tap state machine

`lib.rs` registers one global-shortcut handler. It routes by which shortcut fired: paste-last first, then Esc (cancel), then the read-aloud key, and everything else goes to `hotkey::on_pressed` or `on_released`. Defaults are registered in `setup`; the frontend then calls `set_hotkey` with the saved key.

State is `TapState` in `lib.rs`: `locked`, `key_down`, `press_at`, `last_tap_at`, `press_seq`, `ignore_release`.

1. Press while idle: `start_capture` begins recording. `key_down` filters OS key repeat.
2. Release after more than 300 ms (`TAP_MS`): normal push-to-talk, `finalize_recording` runs.
3. Release after a quick tap (and `toggle_mode` on): wait 480 ms. If no second press arrives, finalize. A second press within 450 ms (`DOUBLE_TAP_WINDOW_MS`) sets `locked`.
4. Press while `locked`: finalize.
5. On Windows, `on_released` first calls `key_still_down` (`GetAsyncKeyState`) and ignores the release if the key is physically held. This hides a polling artifact in the `global-hotkey` crate.

The tray toggle and the Wayland flags call `start_capture` / `stop_capture` directly and never touch `TapState` (hard rule 10). `--start`, `--stop` and `--toggle` arrive in the single-instance callback in `lib.rs` and go to `hotkey::cli_record`. The flags only work when the app is already running.

### 2.3 Recording

`start_capture` (in `hotkey.rs`):

1. If no local model is selected (and STT is not cloud), it opens the dashboard and reports an error instead of recording.
2. It stores the foreground exe name in `active_app` (for per-app profiles).
3. `Recorder::start` (`audio/capture.rs`) spawns a thread with a `cpal` input stream. Samples go to a mutex buffer, mixed down to mono, with a live loudness value. Resampling to 16 kHz is linear and happens at stop.
4. If chunking applies, it starts the segmenter (2.6) and warms the punctuation model.
5. It grabs Esc as a global shortcut (`grab_escape`, on its own thread so the shortcut handler never blocks), plays the start sound (`rodio`, on its own thread, WAVs embedded with `include_bytes!` from `src-tauri/sounds/`), shows the pill, and emits `pipeline://state` = `recording`.
6. A 60 ms loop emits `pipeline://level` (0 to 100). If the recorder reports an error it calls `report_error` and finalizes with whatever audio exists.

`cancel_capture` (Esc) drops the audio, sets segmenter stop, and returns the pill to idle without pasting.

### 2.4 Pipeline after release

`finalize_recording` takes the recorder, plays the stop sound, emits `processing`, stops the recorder, and waits on a global `PIPELINE` mutex so only one dictation runs at a time. Then `process_audio_pipeline` does, in this order:

1. Per-app profile: if `active_app` contains a rule's `app_match`, that rule's mode replaces the global one.
2. Gate: `audio/gate.rs::trim_silence` finds speech with Silero (`audio/vad.rs`, `ort` crate, model in `%APPDATA%\SilentVoice\vad\`). It needs at least 3 speech frames and one frame at 0.5 confidence, then pads about 420 ms before and 240 ms after. If the VAD model is missing it falls back to an RMS scan. No speech: emit `pipeline://error`, show the notice card ("Is your microphone muted?" when the peak is under 1e-4), go idle.
3. Write `last.wav` (16 kHz mono) and call `transcription::whisper::transcribe_dispatch` under a watchdog of 180 s plus 3 s per clip second. On timeout it stops the whisper server and reports.
4. Join the background-chunk prefix and the tail. If chunked and English, `repunctuate_chunks` strips fillers per piece and runs the punctuation model (`sherpa_stt::repunctuate`).
5. `strip_fillers`, `collapse_repeated_words`, `spoken_symbols`, then `soundalike::fix` when vocabulary is on.
6. `trailing_command` removes a closing "press enter" and remembers it.
7. If the mode has a prompt and source `local` or `api`: `run_local_llm` (bundled llama-server) or `llm::openai::chat`, then `tidy_ai_output`. Any failure pastes the raw text and shows an "ai-mode" notice. Else if `coedit_enabled`: `coedit::correct`.
8. `structure_text`, then `format_numbers` line by line, then `apply_replacements` last.
9. Paste (2.8), optional Enter, save the audio clip, `history::append`, emit `pipeline://result`.
10. If the paste landed: emit `done` (green check), wait 700 ms, then `idle` (which also schedules pill auto-hide).

### 2.5 Speech engines

`transcribe_dispatch` takes the one `stt_gate` lock (so a background chunk and the final tail never decode at the same time), then chooses:

- **Cloud** (`stt_source == "cloud"`): `llm::openai::transcribe_audio`. If it fails and the local model is on disk, it falls back to local and emits `pipeline://error` saying so.
- **Whisper** (`registry::stt_engine` returns Whisper): `transcribe_via_server` starts or reuses `whisper-server` on 127.0.0.1:8090 (`transcription/server.rs`) and POSTs the WAV to `/inference`. The server is restarted when its key (`model|language|vocab|gpu|threads`) changes. If anything fails it runs the `whisper-cpp` sidecar once (flags `-bs 1 -bo 1 -fa --no-timestamps`, `-ng` when GPU is off). With GPU off the server gets `GGML_VK_VISIBLE_DEVICES=-1`.
- **Parakeet, Moonshine, SenseVoice**: `sherpa_stt::transcribe_file` through the sherpa-onnx C API (`libloading`). The recognizer stays resident in `AppState.sherpa_stt`, keyed by model, threads, vocabulary and boost. The audio is peak-normalized. Clips over 18 s are split at VAD pauses into about 12 s pieces, with a padded retry if a piece comes back empty. Threads are capped at the physical core count.
- **Qwen3-ASR** (model id starts with `qwen3-asr`): `llm::llama::transcribe_asr` sends base64 WAV to llama-server `/chat/completions` and keeps the text after `<asr_text>`. It shares the one llama server with local AI modes.

Thread count: `hotkey::resolve_thread_count`. Off (default): one thread per physical core, between 2 and 8. On: the chosen count, or all logical cores if 0.

Model lifetime: a startup task preloads the model once settings arrive. Every 30 s it either keeps the sherpa engine hot with a silent decode, or, when `model_unload_minutes` is set and the idle time passed, stops the whisper server and drops the sherpa engine. `start_capture` warms the model again.

### 2.6 Transcribe while you speak (chunking)

`audio/segmenter.rs`. Starts only when `chunk_on_silence`, STT is local, the language or model is English (`english_text`), and `silero_vad.onnx` exists.

1. Every 250 ms it reads the new audio from the recorder and runs Silero only on that new part.
2. It cuts in the middle of a pause: 700 ms of silence after at least 2.5 s of speech, or a 300 ms breath once 5 s are pending.
3. It transcribes the chunk through `transcribe_dispatch` and commits text plus the sample count it covers (one lock, so they cannot be torn).
4. A chunk that looks hallucinated stops chunking, so the tail re-does that audio. A chunk error sets `failed`, and the pipeline re-transcribes the whole clip.
5. A new recording bumps a generation counter, so a late chunk from the last dictation cannot leak into the next.

The model files are fetched by `download_vad_model` (Silero plus punctuation, one download). The frontend calls it automatically once any voice model exists (`useAutoVoiceDetection.ts`).

### 2.7 Text clean-up and vocabulary

All in `system/textfmt.rs` unless noted.

- **Fillers**: `FILLER_WORDS` list. This keeps Parakeet (which writes them) the same as Whisper (which drops them).
- **Repeated words**: collapses runs of the same word. Skips one-character tokens and numbers.
- **Spoken symbols**: table `SYMBOL_WORDS` plus "dot <suffix>" and "at" email joins.
- **Structure**: fixed phrase table for new line, new paragraph, bullet, number one to ten.
- **Numbers**: all spelled numbers become digits, percent, years, decimals.
- **Replacements**: `hotkey.rs::apply_replacements`, whole-word, case-insensitive, longest trigger first, swallows punctuation that follows the trigger.
- **Vocabulary**: three separate effects. (a) Whisper and cloud get the list as an initial prompt (`prompt_for` drops an all-ASCII list when the language uses a non-Latin script). (b) Parakeet gets a hotwords file made from names and jargon only (`hotword_phrases`), using an embedded `bpe.vocab`, boost clamped to 0.5 to 1.5. Moonshine, SenseVoice and Qwen3-ASR cannot be biased. (c) `soundalike.rs` runs after every engine and swaps misheard words for vocabulary words by comparing how they sound.

### 2.8 Paste

`hotkey.rs` decides, `system/paste.rs` types.

1. Compute `to_paste` (adds a space if `append_trailing_space`).
2. Safety checks (all fail open): `secure_field::focused_is_password` (UI Automation `IsPassword`), and a focus check using `target_hwnd`. If focus moved, `foreground::refocus` tries to bring the original window back. If it cannot, or the field is a password, the text goes to the clipboard and a "copied" notice says why. On macOS, missing Accessibility also means clipboard plus a notice, and the settings pane opens once.
3. `paste::paste_at_cursor`: on Wayland with `wtype`, type the text. Otherwise snapshot the clipboard (every format on Windows, text elsewhere), set the text, wait 50 ms, press Ctrl+V (Cmd+V on macOS, run on the main thread because of a macOS 14 crash), wait 250 ms, and restore the old clipboard only if it still holds our text.
4. On Windows `verify_paste` reads the focused field twice (300 ms, 600 ms later) through UI Automation. If the field is readable and lacks the last 24 letters and digits of the text, it copies the text and shows a "copied" notice. If it landed it starts `learn_from_edits`, which watches the field for about a minute and offers underlines for corrected words (shown by the inline proofreader).
5. "press enter" presses Return 120 ms later, only when the paste landed (or there was no text), the window is unchanged and the field is not a password.
6. Paste-last: `paste_last` reads the newest history entry and pastes it on its own thread, after Alt and Shift come up. `copy_last` (tray) just copies.

### 2.9 Pill, notice card, tray

- **Pill** (`system/overlay.rs`, `components/overlay/`): a separate small window loading `index.html?view=overlay`. It is opaque on Windows (hard rule 1), transparent on macOS and Linux. Rust shows it, keeps it topmost every 2 s (`ensure_visible`), saves its position to `%APPDATA%\SilentVoice\overlay-pos.json` on move, and rounds corners (DWM, or a window region on Windows 10). It reacts to `pipeline://state`, `pipeline://level` and `tts://state`. The right-click menu calls `set_overlay_size`. Hide Overlay calls `hide_overlay`. Auto-hide uses `pill_activity_gen` so a stale timer does nothing.
- **Problem card** (`system/notice.rs`, `NoticeCard.tsx`): another opaque window at `?view=notice`, created on first use, bottom centre above the taskbar. `notice::show` stores the notice, emits `notice://show`, and the card also asks `notice_current` after mounting. It closes after 7 s. The button calls `notice_open_settings` (which emits `nav://to`) or copies diagnostics. `report_error` in `hotkey.rs` decides which errors get a card.
- **Tray** (`system/tray.rs`): menu ids `toggle_record`, `copy_last`, `toggle_overlay`, `open_dashboard`, `open_history`, `quit`. While a meeting records the icon gets a red dot.

### 2.10 History and stats

- Rust appends each result to `%APPDATA%\SilentVoice\history.json` (`history.rs`, atomic write, 1000-entry cap, deletes clips no entry points to). It also emits `pipeline://result`.
- The frontend `usePipeline.ts` adds that to `historyStore` (which applies the count and age limits and saves the list back through `save_history`) and calls `statsStore.record`.
- `statsStore` keeps per-day totals, per-hour words and per-app words in `settings.json`, separate from history so pruning history does not erase progress. Time saved compares your words to 40 wpm typing. Speaking speed uses the key-held time (`speak_ms`).
- Home (`Home.tsx`) shows live status, the waveform, today's numbers and the 12-week strip (84 cells). Progress (`Progress.tsx`) shows the year heatmap, streaks, hourly bars and top apps.
- Audio clips: `read_audio_clip`, `copy_audio_file` (puts the file on the clipboard; Windows API in `clipboard_file.rs`), `prune_audio_clips`. `retranscribe_clip` re-runs the current model on a saved clip. Editing an entry in `TranscriptCard.tsx` adds new words from the correction to `custom_vocabulary`.
- Onboarding uses `start_mic_probe` / `stop_mic_probe` (event `mic://level`) and `start_recording` / `stop_and_transcribe` for its test.

### 2.11 Events

| Event | Payload | Notes |
|---|---|---|
| `pipeline://state` | `{state}`: idle, recording, processing, done | `listening` exists in the TS type but is never sent |
| `pipeline://level` | number 0 to 100 | about every 60 ms while recording |
| `pipeline://result` | `PipelineResult` (id, raw, processed, mode, model, durations, audio file, app) | |
| `pipeline://error` | string | also logged |
| `mic://level` | number | onboarding probe |
| `notice://show` | `Notice` | to the notice window |
| `nav://to` | route string | opens a dashboard page |
| `proofread://add-vocab` | word | from the inline popup |
| `tts://state` | state string | drives the pill's blue mode |
| `app-theme://changed` | theme id | main window to pill |

## 3. Platform dependence

### Pure logic (portable Rust or TypeScript)

- `textfmt.rs` (fillers, repeats, symbols, structure, numbers, `trailing_command`, `corrected_phrases`), `apply_replacements`, `tidy_ai_output`, `soundalike.rs` (uses `harper-core`), `paste_verdict`, `join_parts`, `english_text`.
- `audio/gate.rs`, `audio/segmenter.rs` boundary logic (`find_boundary`, `looks_hallucinated`), the `Segmenter` generation guard.
- `audio/vad.rs` (Silero through `ort`, runtime-loaded ONNX Runtime) and the sherpa-onnx wrappers in `sherpa_stt.rs` (C API through `libloading`, including the punctuation model). The binaries are per platform, the code is not.
- `history.rs`, `RuntimeConfig`, `TapState` logic, the cloud STT call, the llama ASR call.
- All stores, `statsStore` maths, Home and Progress components, `tauriBridge.ts` (works in a browser with mock data).

### Uses crates that wrap OS APIs (portable on desktop)

| Piece | Crate and OS API |
|---|---|
| Microphone | `cpal` (WASAPI, CoreAudio, ALSA/PulseAudio) |
| Sounds | `rodio` |
| Global hotkey | `tauri-plugin-global-shortcut` (RegisterHotKey on Windows, Carbon/CGEvent on macOS, X11 on Linux, nothing on Wayland) |
| Clipboard | `arboard` |
| Key presses | `enigo` (SendInput, CGEvent, XTest/libxdo) |
| Windows (pill, card) | Tauri/WebView2, WKWebView, WebKitGTK |
| Tray, single instance, store | Tauri plugins |
| Child processes (`whisper-server`, `whisper-cpp`, `llama-server`) | `std::process`, `tauri-plugin-shell` |

### Direct OS calls

| Area | Windows | macOS | Linux |
|---|---|---|---|
| Release guard | `GetAsyncKeyState` | none | none |
| Foreground app (profiles, history app) | `GetForegroundWindow`, `QueryFullProcessImageNameW` | `osascript` | `xprop` |
| Foreground window id (focus check) | `GetForegroundWindow` | hash of app name (app-level only) | `xprop` window id |
| Re-focus original window | `SetForegroundWindow`, `AttachThreadInput` | none | none |
| Password-field check | UI Automation `IsPassword` | none (returns false) | none (returns false) |
| Paste verification, learn-from-edits | UI Automation (`inline_check`) | none | none |
| Full clipboard save and restore | `OpenClipboard`, `EnumClipboardFormats`, `SetClipboardData` | text only | text only |
| Paste keys | enigo | enigo on main thread via `dispatch_sync_f` | enigo, or `wtype` on Wayland |
| Permission gate | none | `AXIsProcessTrusted` | none |
| Pill z-order and corners | `SetWindowPos(TOPMOST)`, DWM corner preference, `CreateRoundRectRgn` | transparent window | transparent window |
| Copy audio file to clipboard | `DROPFILES` format (`clipboard_file.rs`) | see file | see file |
| Child process cleanup | Job Object (`system/job.rs`) | none | none |
| Dashboard memory trim | WebView2 `SetMemoryUsageTargetLevel` | none | none |

## 4. Mobile portability

Assumes a Tauri v2 mobile build (Android and iOS). Desktop-only Tauri plugins: global-shortcut, single-instance, updater, tray. Apps cannot start helper executables on iOS, and on Android they cannot do it in the desktop way, so every sidecar process counts as rework.

| Feature | Verdict | Why |
|---|---|---|
| Hold-to-talk global hotkey | Desktop-only | No system-wide key on a phone. A hold-to-record button in the app replaces it. |
| Double-tap lock | Needs rework | The idea (tap to lock, tap to stop) works on a button. `TapState` and the timings carry over, the trigger does not. |
| False-release guard | Desktop-only | Fixes a Windows `global-hotkey` artifact. |
| Esc cancel | Desktop-only | No Esc key. Becomes a cancel button. |
| Tray Start/Stop | Desktop-only | No tray. |
| Wayland `--start/--stop` | Desktop-only | Command-line flags to a running instance. |
| Microphone choice | Needs rework | `cpal` supports Android and iOS, but the permission prompt and audio-session setup are new. Device picking is mostly not needed. |
| Mic failure reporting | Carries over as-is | Pure logic on the recorder error. |
| Live loudness | Carries over as-is | Pure maths and an event. |
| Onboarding mic meter and test | Needs rework | Same code, but needs the mobile mic permission flow. |
| Start/stop sounds | Needs rework | `rodio` works on mobile, but audio focus and silent mode need handling. The packs carry over. |
| Noise filter and Silero voice detection | Carries over as-is | `ort` has Android and iOS builds. Models are portable. Needs a mobile ONNX Runtime binary. |
| No-speech and muted-mic messages | Needs rework | Logic carries over, the card window does not. Use an in-app banner. |
| Whisper engine (server and CLI) | Needs rework | Both are child processes. Link whisper.cpp as a library instead. |
| Parakeet, Moonshine, SenseVoice | Needs rework | sherpa-onnx has Android and iOS builds, but today it loads a desktop DLL by path. Needs per-platform library loading and bundling. |
| Qwen3-ASR | Desktop-only | Runs on llama-server, a child process, and the model is large. |
| Cloud transcription and fallback | Carries over as-is | Plain HTTP. The local fallback needs a local engine first. |
| Thread count | Needs rework | The logic carries over. The setting is hidden on mobile and the default changes (big.LITTLE cores). |
| Transcribe while you speak and punctuation repair | Carries over as-is | Pure logic over Silero and sherpa. Worth keeping: it cuts waiting time. Needs the two engines above. |
| Model keep-warm and idle unload | Needs rework | The idea matters more on mobile, but the timers and "do not wake the CPU" rules change. The whisper-server branch goes. |
| Long-clip splitting | Carries over as-is | Pure logic. |
| Vocabulary prompt, hotwords, sound-alike fixer | Carries over as-is | Pure logic. Hotwords need the embedded `bpe.vocab`. |
| Text clean-up (fillers, repeats, symbols, structure, numbers) | Carries over as-is | Pure Rust, the heart of the pipeline. |
| "press enter" command | Desktop-only | It presses a key in another app. |
| Replacements | Carries over as-is | Pure logic. |
| AI mode and per-app profiles | Needs rework | AI mode carries over. Per-app profiles need the foreground app, which mobile apps cannot read. |
| Fix grammar before pasting (CoEdit) | Needs rework | Pure ONNX logic, but model size and speed need a mobile check. |
| Paste at cursor | Desktop-only | A phone app cannot type into other apps. Replace it with an in-app text box, a share sheet, or an Android input method (keyboard). |
| Clipboard restore | Desktop-only | Part of paste. |
| Trailing space | Needs rework | Meaningful only for a keyboard-style insert. |
| Paste guards (password field, refocus, Accessibility) | Desktop-only | Fixes for pasting into other apps. |
| Paste verification and learn-from-edits | Desktop-only | Windows UI Automation. |
| Paste-last and Copy last | Needs rework | The history lookup carries over. The trigger becomes a button, and "copy last" is trivial. |
| Overlay pill | Desktop-only | Floating always-on-top window. Replace it with an in-app recording view or a notification. The CSS animations can be reused. |
| Drag, position memory, hide, auto-hide | Desktop-only | Part of the pill. |
| On-screen problem card | Desktop-only | Separate window. Use an in-app banner or toast. The `Notice` data and text carry over. |
| Error banner and announcement | Carries over as-is | Plain React. |
| Logs and paste log line | Carries over as-is | `logging.rs` is a file writer. |
| History list, search, undo, clear | Carries over as-is | JSON plus React. |
| History limits and retention | Carries over as-is | Pure TS. |
| Saved audio, play | Needs rework | Files and playback work, but storage and size limits matter more on a phone. |
| Copy audio file to clipboard | Desktop-only | Windows `DROPFILES`. Mobile uses the share sheet. |
| Re-transcribe a clip | Carries over as-is | Needs a mobile engine. |
| Edit-to-learn vocabulary | Carries over as-is | Pure TS. |
| Blur history | Carries over as-is | CSS. |
| Home status, strip, today's stats | Carries over as-is | Pure TS and React. Needs a layout change for small screens. |
| Progress page | Carries over as-is | Pure TS and React. Layout needs a small-screen pass. |
| Settings pushing (`useRuntimeSync`) | Carries over as-is | Same commands, fewer settings. |
| Start with Windows (`autostart`) | Desktop-only | Registry Run key. Not in this area, listed for completeness. |

## 5. Gaps: code versus the docs

HANDBOOK sections are named where they exist. Items marked (CLAUDE.md) are in that file.

### Described wrongly

1. **Pill bars and size (HANDBOOK 9, CLAUDE.md rule 2).** Docs say 7 bars and a fixed 68x22 window with only the right-click menu resizing. The code draws 5 bars. `OverlayApp.tsx` asks for 58x22 for the pill, 190x152 for the menu and 132x30 for the read-aloud pause/stop bar, so the window is resized for read-aloud too, and the idle pill is not 68 wide. Rust creates it at 68x22.
2. **Overlay transparency (HANDBOOK 8.1, CLAUDE.md rule 1).** The docs say never transparent. `overlay.rs` and `notice.rs` set `.transparent(true)` on macOS and Linux. Also `run()` in `lib.rs` sets the process-wide environment variable `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-features=CalculateNativeWinOcclusion`, which the "no additional_browser_args" rule is meant to forbid.
3. **Pipeline order (HANDBOOK 6, CLAUDE.md "Key facts").** Real order: gate, engine, repunctuate (chunked English), strip fillers, collapse repeated words, spoken symbols, sound-alike fixer, "press enter" peel-off, AI mode or CoEdit, structure commands, numbers, replacements, paste. HANDBOOK 16 says numbers run after replacements and that the collapse runs right after the engine; both are wrong. Replacements are last so their text is never touched.
4. **Gate (HANDBOOK 9).** Described as an RMS gate only. It is Silero voice detection (with minimum speech frames and a 0.5 confidence frame) with the RMS scan as fallback. Padding is 420 ms before and 240 ms after, not "about 240 ms" both sides.
5. **Thread counts (HANDBOOK 16).** Docs say `max(2, cores/2)` and a slider that appears under a "High performance mode" toggle. Code uses physical cores clamped to 2 to 8, and the UI is one "CPU cores for speech" dropdown that writes both fields. `ensure_engine` also caps sherpa at the physical core count.
6. **Vocabulary boost range.** The comment on `RuntimeConfig.vocabulary_strength` says 0.5 to 5. The slider is 0.5 to 1.5 and `ensure_engine` clamps to 1.5.
7. **Event table (HANDBOOK 13).** Lists `pipeline://partial`, `tray://toggle-record`, `tray://toggle-listen` and a `listening` state, none of which are emitted any more. Missing: `done` state, `pipeline://level`, `mic://level`, `notice://show`, `nav://to`, `app-theme://changed`, `proofread://add-vocab`.
8. **Command table (HANDBOOK 12).** Lists commands that no longer exist (`ollama_status`, `ollama_generate`, `speak_text`, `stop_tts`, `set_overlay_opacity`, `get_data_location`, `set_data_location`, `pick_folder`). Missing from it: `set_paste_last_hotkey`, `preview_dictation_sound`, `notice_current`, `notice_hide`, `notice_open_settings`, `retranscribe_clip`, `read_audio_clip`, `copy_audio_file`, `prune_audio_clips`, `start_mic_probe`, `stop_mic_probe`, `download_vad_model` / `vad_installed` / `delete_vad_model`, `accessibility_granted`, `copy_to_clipboard`, `download_stt_archive`, `download_llama_asr`, `api_test_stt`. `update_runtime_config` and `set_behavior` take many more arguments than the table says.
9. **`target_hwnd` (comment on `AppState`).** It says "focused when recording started". The code stores it at release time (`finalize_recording`). Per-app profiles use the press-time app; the history `app` field uses the app at pipeline start (after release).
10. **Idle unload comment (`lib.rs` startup task).** It says whisper-server is never auto-stopped. It is stopped (and the sherpa engine dropped) when `model_unload_minutes` is set and the idle time passes.
11. **Learn-from-vocabulary rule (HANDBOOK 16, inline proofreading).** "Fixes never auto-add to vocabulary." True for inline fixes, but editing a History entry in `TranscriptCard.tsx` does add new words to `custom_vocabulary` automatically. `retranscribe_clip` also uses a shorter clean-up: no structure commands, no "press enter", no AI mode or CoEdit, and replacements before numbers (the opposite of live dictation).
12. **Cloud STT ignores the vocabulary toggle.** `transcribe_dispatch` always passes `prompt_for(language, vocabulary)` to the cloud call, even when `use_custom_vocabulary` is off. Local Whisper respects the toggle.

### Not described at all

13. **Engines.** CLAUDE.md names whisper, Parakeet, Moonshine and SenseVoice. The code also has Qwen3-ASR on llama-server (`SttEngine::LlamaAsr`, `download_llama_asr`), the resident whisper-server on port 8090 with a CLI fallback, and the `stt_gate` mutex. HANDBOOK 16 mentions none of these engines, and `qwen3`, `SenseVoice` do not appear in it.
14. **Background transcription.** `segmenter.rs`, the punctuation model, the pause rules (700 ms / 5 s and 300 ms), the hallucination guard and the failure fallback have no HANDBOOK section (`segmenter` appears 0 times). Same for the VAD plus punctuation download (`download_vad_model`) and `useAutoVoiceDetection`.
15. **Text clean-up.** `strip_fillers`, `spoken_symbols`, `structure_text` (new line, bullets, numbered lists), `trailing_command` ("press enter") and `soundalike.rs` are in no doc. Parakeet hotwords (`hotword_phrases`, embedded `bpe.vocab`) are also missing.
16. **Paste safety.** Password-field check (`secure_field.rs`), re-focus of the original window, full-format clipboard save and restore, paste verification, learn-from-edits, the macOS Accessibility fallback, and the Wayland `wtype` path appear only in PORTING.md or code comments, not in section 16.
17. **User features:** start/stop sound packs, Esc cancel, paste-last and tray "Copy last", the problem card (`notice.rs`), the onboarding mic probe, and the Home 12-week strip and Progress page. Progress is only listed in the project tree. Pill drag, remembered position (`overlay-pos.json`), keep-alive loop, auto-hide and the green "done" check are also unwritten.
18. **Tray.** Menu items "Copy last dictation" and "History", and the red-dot recording icon, are not in HANDBOOK.
19. **History.** Search, undo delete (6 s), blur, retention, saved clips with limit, copy-audio and re-transcribe are not in HANDBOOK. The history is also written in two places (Rust `append`, then the frontend `save_history`), and `history.rs` protects a just-made clip from the frontend's older list.
20. **Not verified in this pass:** hotkey, paste, per-app profiles and paste verification on macOS and Linux are untested (see PORTING.md), and `foreground_hwnd` on macOS only knows the app, not the window.
