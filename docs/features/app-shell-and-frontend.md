# App shell and frontend

What the React app and the Tauri shell around it do today, written to help plan a mobile port. Written from the code on branch `mobile` (app version 0.2.0). Where this file and `docs/HANDBOOK.md` disagree, the code is right; section 6 lists the places the handbook is wrong or silent.

Deep detail that the handbook already covers well:
- Tech stack and project tree: HANDBOOK section 2 and 3 (the tree is out of date, see section 6).
- Data folders: HANDBOOK section 7.
- Command and event tables: HANDBOOK section 12 and 13 (both out of date).
- Window lifecycle, autostart, onboarding, error log, updater and release steps: HANDBOOK section 16.

## How the shell is built

- One React bundle, several windows. `src/App.tsx` reads `?view=` from the URL. No value means the main dashboard. `overlay` is the recording pill, `squiggle` and `squiggle-popup` are the proofreading underlines and suggestion card (macOS and Linux only, Windows draws these with native layered windows), `notice` is the small on-screen problem card.
- The main window is 980 by 700, minimum 820 by 560, with no native title bar on Windows and Linux (`decorations: false`, custom `Titlebar.tsx`). macOS uses a native overlay title bar from `tauri.macos.conf.json`.
- The dashboard is a fixed 224 px sidebar (11 links) beside one scrolling `<main>`. A theme switch and the version sit at the bottom of the sidebar. The "Update available" button appears above them.
- Nothing renders until the settings store has loaded from disk. After that, a first-time user sees Onboarding and everyone else sees the dashboard.
- `src/hooks/useRuntimeSync.ts` is the glue. It watches the settings store and pushes every change to Rust through the bridge (section 3). Rust never reads `settings.json` itself; it only knows what the frontend last sent. Rust waits up to 30 seconds for the first push (`config_pushed`) before it warms the speech model.
- `src/hooks/usePipeline.ts` listens for Rust events (state, result, error, download progress) and writes results into the history and stats stores.
- A single hidden screen-reader live region (`announceStore`) speaks dictation results, errors and finished downloads.

## 1. Page list

Sidebar order. Each page is a route in `App.tsx`.

**Home** (`/home`). A single-screen cockpit that never scrolls. Left: live status (Idle, Listening, Recording, Processing), a mic-level waveform, the push-to-talk hotkey (click to record a new one), a "Ready to dictate / Finish setup" pill, and a 12-week heatmap with days active, busiest day and words per dictation. Right: a Setup checklist (voice model, microphone, hotkey), Quick controls (switch model, switch language), a Device card (CPU, RAM, GPU, disk) and Today (words, dictations, time saved against typing). Shows a warning bar in a browser and a macOS Accessibility warning with an "Open Settings" button.

**Progress** (`/progress`). Statistics read from the durable per-day totals in `statsStore`, not from history, so clearing history does not change them. Sections: what you have dictated (words compared with real books), your year, where you speak (per app), when you speak (by hour). Nothing is sent anywhere.

**Transcribe** (`/transcribe`). Turn an audio or video file into a timestamped transcript. Drop a file on the window or browse. One job at a time. A list of saved transcripts: open, rename, copy, save as .txt/.md/.srt, delete (two-step), cancel a running job, resume or retry a stopped one. Live progress arrives as `transcript://update`.

**Notetaker** (`/notetaker`). Records a meeting from the mic (You) and the computer's sound (Others) into a live transcript with speaker labels. A gear panel sets spoken language, transcription engine (local or a cloud provider), "tell speakers apart" and the summary model. A one-time consent note. A red dot shows in the sidebar and on the tray icon while recording. Each meeting can be searched, re-transcribed, saved, deleted, have its people count set, play audio from any paragraph, and get an AI Overview (summary, key points, decisions, action items; `MeetingOverview.tsx`). Shows "Not recorded" for Others with a reason when the OS cannot capture computer sound.

**Model Store** (`/models`). Three tabs: Voice models (speech to text), Read-aloud voices (TTS), AI models (local LLMs). Voice and AI tabs use `HfBrowser`: staff picks plus a live Hugging Face search and trending feed, a per-row "will it run on this PC" dot, a quant picker, pin (star), pause, resume, cancel, remove. The TTS tab has search and a language filter, download, remove (confirm), pin, and a sample button. Large or too-heavy downloads ask first.

**Writing styles** (`/modes`). The AI "modes" applied to dictated text before it pastes. Built-in styles can be edited (then marked customized) and reset. Add, delete, pin to top. Each style picks a local model or a cloud provider and model, has a system prompt, and a "Test on sample speech" button.

**Theme** (`/theme`). Six app accent colors (Ember is the default, then Ocean, Iris, Reef, Rose, Meadow) that recolor the window and the pill live. Separate pickers for the grammar-suggestion popup: card surface (dark or light) and accent palette (five).

**Settings** (`/settings`). A rail of five topics with search, a basic list and a fold-open Advanced card per topic. Definitions live in `settings/catalog.tsx`.
- Dictation: Push-to-talk key, Microphone, Language you speak, Voice model; advanced: Paste last dictation key, Double-tap to keep recording, Background noise filter, Start and stop sounds, Add a space after each paste, Hide the pill when idle, Transcribe while you speak, Transcribe in the cloud.
- Writing help: underline toggle; advanced: Oxford commas, filler words, Smarter grammar checks (GECToR), how picky they are, apps to ignore, Fix grammar before pasting (CoEdIT), Writing style per app.
- Read aloud: on/off, key, voice, hear a sample.
- Words: custom vocabulary, Shortcuts (text replacements); advanced: use my words when transcribing, how strongly.
- System: Start with Windows (autostart), Updates (manual check, installs at once), Logs (save to Downloads, copy diagnostics, open folder); advanced: Free memory when idle, CPU cores, Use graphics card. A "What Silent Voice sets up for you" note lists automatic behavior.

**History** (`/history`). Searchable past dictations. Per entry: edit the text (edits teach new vocabulary), play and copy the saved audio, re-transcribe with the current model, copy, delete with undo. "Clear all" with confirm. A gear opens a Storage sheet: history limit, auto-delete window, save audio and clip count, blur until hover. First visit shows a one-time hint (kept in `localStorage`).

**Cloud providers** (`/api`). Add OpenAI-compatible providers from presets or custom, paste a key, load the model list, pick chat and speech-to-text models, test the connection, remove (confirm). Used by Writing styles, Notetaker summaries and cloud transcription. Keys are stored in the settings file on this PC.

**How to use** (`/guide`). Static help with pill mockups: first dictation, hands-free, AI styles, proofreading, choosing a model, the pill, and a feature list.

**Onboarding** (not a route; replaces the dashboard while `onboarded` is false). A left-rail flow: Choose a model (hardware-based pick, download starts in the background), Microphone (live level meter via `start_mic_probe`), Push-to-talk key, Say something (records and transcribes a test), then an "all set" screen. Finishing applies the hardware recommendation (GPU, high-performance, thread count, CoEdIT) and sets `onboarded`.

**Other windows** (same bundle): the recording pill (`OverlayApp`), the underline layer (`SquiggleOverlay`), the suggestion card (`SquigglePopup`) and the problem card (`NoticeCard`, shows "Select microphone" or "Open settings" buttons and a copy-diagnostics button).

## 2. Settings list

### Where settings live

- `settings` is one Zustand store (`settingsStore.ts`, persist key `silent-voice-settings`, version 2). In the desktop app it is saved with `tauri-plugin-store` to `settings.json`. In a browser it falls back to `localStorage`. The first desktop launch copies an old `localStorage` value into the file once. The store is only written after the file has been read (so defaults never overwrite saved data).
- The same file also holds the stats store (key `silent-voice-stats`, `statsStore.ts`). Each store gets its own load gate (`tauriStorage.ts`).
- The store keeps more than the flat `settings` object: `modes`, `providers` (with API keys), `snippets` (text replacements), `appProfiles`, and the `notetaker` object inside `settings`.
- On load it self-heals: new default keys are filled in, bad hotkeys are reset, built-in modes the user has not edited are refreshed from the app, new built-ins are appended. Migration v1 and v2 reset `chunk_on_silence` to true.
- Not in `settings.json`: history (Rust file `history.json`, with a `localStorage` copy too, see section 6), downloaded-model lists (asked from Rust on start), custom LLM entries (`localStorage` key `sv-model-store`), the History hint (`sv-history-storage-hint`), legacy Notetaker keys (`sv-notetaker-*`, read once as defaults).

### Flat settings (`DEFAULT_SETTINGS`)

"set_behavior" means the 16-argument command sent after a 150 ms debounce from `useRuntimeSync`. "update_runtime_config" is sent whenever model, language, device, vocabulary or cloud provider changes.

| Setting | Default | Read by |
|---|---|---|
| hotkey | Ctrl+Shift+Space | `set_hotkey` (Rust global shortcut); Home, Onboarding, Settings, Guide |
| active_stt_model | "" (none) | `update_runtime_config`; Home, Notetaker, Model Store, Onboarding |
| active_mode_id | raw | `set_active_mode` (with resolved prompt, model, provider); Writing styles; history entries record it |
| stt_preset | balanced | Written by Onboarding only. Nothing reads it (see gaps) |
| language | auto | `update_runtime_config`; Home, Notetaker, Model Store fit |
| use_gpu | false | `update_runtime_config`; auto-set once on first run if a strong GPU is found |
| gpu_auto_checked | unset | `useRuntimeSync` (one-time GPU check), Onboarding |
| high_performance | false | `set_behavior`; Settings |
| performance_threads | 0 (all cores) | `set_behavior`; used only when high_performance |
| audio_device | null (system default) | `update_runtime_config`; Home, Settings, Onboarding |
| auto_start | false | `set_autostart`; Settings re-syncs it from the OS on open |
| theme | dark | `useResolvedTheme` (dark, light or system); App, pill, sidebar switch |
| custom_vocabulary | "" | `update_runtime_config`; Settings; grows from `proofread://add-vocab` and history edits |
| use_custom_vocabulary | true | `update_runtime_config` |
| vocabulary_strength | 1.5 | `update_runtime_config` (Parakeet hotword boost) |
| stt_cloud_provider_id | null | `update_runtime_config` (null means local model) |
| toggle_mode | true | `set_behavior` (double-tap lock) |
| input_sensitivity | 50 | `set_behavior` (noise gate) |
| inline_proofread | false | `set_behavior` (underlines in other apps) |
| coedit_enabled | false | `set_behavior` (grammar fix before paste); Onboarding may turn it on |
| chunk_on_silence | true | `set_behavior` (background transcription) |
| proofread_disabled_rules | [] | `set_behavior`; Settings |
| gector_sensitivity | balanced | `set_behavior` |
| proofread_ignore_apps | "" | `set_behavior` (split on commas into a list) |
| pill_auto_hide | false | `set_behavior` |
| append_trailing_space | false | `set_behavior` |
| dictation_sound | chorus | `set_behavior`; preview via `preview_dictation_sound` |
| active_tts_voice | null | `set_tts`; Model Store, Settings |
| paste_last_hotkey | Alt+Shift+Z | `set_paste_last_hotkey` |
| tts_hotkey | Ctrl+Alt+S | `set_tts` |
| tts_enabled | true | `set_tts` (false unregisters the key) |
| onboarded | false | `App.tsx` gate |
| pinned_stt, pinned_llm, pinned_tts | [] | HfBrowser, Model Store (star pins) |
| hf_show_incompatible | false | HfBrowser |
| history_limit | 100 | `historyStore` prune; History |
| blur_history | false | TranscriptList |
| history_retention | never | `historyStore` prune; History |
| history_retention_custom_value | 7 | `historyStore` prune |
| history_retention_custom_unit | days | `historyStore` prune |
| save_audio | true | `set_behavior`; History |
| audio_clip_limit | 20 | `set_behavior`, `prune_audio_clips`; History |
| model_unload_minutes | 0 (never) | `set_behavior`; Rust idle sweep |
| popup_theme | violet | `set_popup_theme`; Theme |
| popup_surface | dark | `set_popup_surface`; Theme |
| app_theme | orange (shown as Ember) | `applyAppTheme` (CSS variables); broadcast as `app-theme://changed` to the pill |

### Notetaker settings (`settings.notetaker`)

| Field | Default | Read by |
|---|---|---|
| stt | "local" (or old localStorage value) | Notetaker (local model or a cloud provider id) |
| stt_models | {} | Notetaker (model per provider) |
| language | "" (use dictation language) | `meeting_start` |
| summary_with | "" | MeetingOverview, summary picker |
| summary_models | {} | MeetingOverview |
| consent_seen | false | Notetaker consent note |
| speakers | true | `meeting_start(speakers)` |

### Lists kept beside the flat settings

| Item | Default | Read by |
|---|---|---|
| modes | built-in styles (`services/modes.ts`) | Writing styles, `set_active_mode`, `set_app_profiles` |
| providers | [] | Cloud providers, Modes, Notetaker, cloud STT |
| snippets (text replacements) | [] | `set_text_replacements`, Settings Words |
| appProfiles (app name contains X, use style Y) | [] | `set_app_profiles`, Settings Writing help |

### Rust-side settings the frontend does not own

`data-location.json` (model and history folder overrides), `overlay-pos.json` (pill position), the `.package-managed` marker on Linux. None has UI except the pill position, which is saved when the pill is dragged.

## 3. Bridge map

`src/services/tauriBridge.ts` is the only place that calls `invoke` (a few Tauri JS APIs are used elsewhere, listed at the end). Every function first checks `isTauri()`. Browser column: **mock** returns fake data, **empty** returns a harmless empty value, **throws** raises "Needs the desktop app", **no-op** does nothing, **browser API** uses a web API instead.

### Window and app
| Function | Rust command | Browser |
|---|---|---|
| isTauri | none (checks `__TAURI_INTERNALS__`) | false |
| listenEvent | none (event listen) | no-op |
| emitEvent | none (event emit) | no-op |
| setOverlaySize | set_overlay_size | no-op |
| hideSelfWindow | hide_overlay | no-op |
| quitApp | quit_app | no-op |
| packageManaged | package_managed | empty (false) |

### Hardware and microphone
| Function | Rust command | Browser |
|---|---|---|
| getHardwareInfo | get_hardware_info | mock (i7 laptop sample) |
| recommendDeviceDefaults | recommend_device_defaults | empty (null) |
| listInputDevices | list_input_devices | mock (one preview mic) |
| startMicProbe | start_mic_probe | no-op |
| stopMicProbe | stop_mic_probe | no-op |

### Speech-to-text models
| Function | Rust command | Browser |
|---|---|---|
| listDownloadedModels | list_downloaded_models | empty |
| downloadModel | download_model | empty (false) |
| downloadSttArchive | download_stt_archive | empty (false) |
| downloadLlamaAsr | download_llama_asr | empty (false) |
| pauseDownload | pause_download | no-op |
| cancelDownload | cancel_download | no-op |
| deleteModel | delete_model | no-op |
| retranscribeClip | retranscribe_clip | none (no guard, fails in a browser) |

### Runtime config, hotkeys and behavior (pushed by useRuntimeSync)
| Function | Rust command | Browser |
|---|---|---|
| updateRuntimeConfig | update_runtime_config | no-op |
| setHotkey | set_hotkey | no-op |
| setPasteLastHotkey | set_paste_last_hotkey | no-op |
| setTextReplacements | set_text_replacements | no-op |
| setBehavior | set_behavior | no-op |
| setAppProfiles | set_app_profiles | no-op |
| setActiveMode | set_active_mode | no-op |
| setPopupTheme | set_popup_theme | no-op |
| setPopupSurface | set_popup_surface | no-op |
| setAutostart | set_autostart | no-op |
| getAutostart | get_autostart | empty (false) |
| previewDictationSound | preview_dictation_sound | no-op |

### Recording
| Function | Rust command | Browser |
|---|---|---|
| startRecording | start_recording | no-op |
| stopAndTranscribe | stop_and_transcribe | empty ("") |

### Accessibility and inline proofreading
| Function | Rust command | Browser |
|---|---|---|
| accessibilityGranted | accessibility_granted | mock (true) |
| openAccessibilitySettings | open_accessibility_settings | no-op |
| applyInlineFix | inline_fix | empty (false) |

### Read aloud (TTS)
| Function | Rust command | Browser |
|---|---|---|
| setTts | set_tts | no-op |
| ttsStop | tts_stop | no-op |
| ttsPause | tts_pause | no-op |
| ttsResume | tts_resume | no-op |
| ttsSpeakText | tts_speak_text | no-op |
| listDownloadedTts | list_downloaded_tts | empty |
| downloadTtsModel | download_tts_model | throws |
| deleteTtsModel | delete_tts_model | no-op |

### Local and cloud AI
| Function | Rust command | Browser |
|---|---|---|
| listDownloadedLlm | list_downloaded_llm | empty |
| downloadLlmModel | download_llm_model | empty (false) |
| deleteLlmModel | delete_llm_model | no-op |
| localLlmGenerate | local_llm_generate | mock (a "needs the desktop app" string) |
| apiListModels | api_list_models | empty |
| apiGenerate | api_generate | mock (same kind of string) |
| apiTestStt | api_test_stt | mock (string) |

### Clipboard, history and audio clips
| Function | Rust command | Browser |
|---|---|---|
| copyToClipboard | copy_to_clipboard | browser API (`navigator.clipboard`) |
| loadHistory | load_history | empty (null, store uses localStorage) |
| saveHistory | save_history | no-op |
| clearHistoryFile | clear_history | no-op |
| readAudioClip | read_audio_clip | empty (null) |
| copyAudioFile | copy_audio_file | no-op |
| pruneAudioClips | prune_audio_clips | no-op |

### Hugging Face
| Function | Rust command | Browser |
|---|---|---|
| hfSearchModels | hf_search_models | mock (one model) |
| hfModelDetails | hf_model_details | mock |
| hfPiperVoices | hf_piper_voices | mock |

### Helper models (smart voice detection, speaker ID, grammar)
| Function | Rust command | Browser |
|---|---|---|
| downloadVadModel | download_vad_model | empty (false) |
| vadInstalled | vad_installed | empty (false) |
| deleteVadModel | delete_vad_model | no-op (no caller in the UI) |
| downloadDiarizeModel | download_diarize_model | empty (false) |
| diarizeInstalled | diarize_installed | empty (false) |
| deleteDiarizeModel | delete_diarize_model | no-op |
| downloadCoeditModel | download_coedit_model | empty (false) |
| coeditInstalled | coedit_installed | empty (false) |
| deleteCoeditModel | delete_coedit_model | no-op |
| downloadGectorModel | download_gector_model | empty (false) |
| gectorInstalled | gector_installed | empty (false) |
| deleteGectorModel | delete_gector_model | no-op |

### Logs and notice card
| Function | Rust command | Browser |
|---|---|---|
| copyDiagnostics | copy_diagnostics | empty ("") |
| exportLogs | export_logs | throws |
| openLogsFolder | open_logs_folder | no-op |
| noticeCurrent | notice_current | empty (null) |
| noticeHide | notice_hide | no-op |
| noticeOpenSettings | notice_open_settings | no-op |

### Files and meetings
| Function | Rust command | Browser |
|---|---|---|
| transcribeStart | file_transcribe_start | throws |
| transcribeCancel | file_transcribe_cancel | no-op |
| transcriptsList | transcripts_list | empty |
| transcriptGet | transcript_get | throws |
| transcriptRename | transcript_rename | throws |
| transcriptDelete | transcript_delete | throws |
| transcriptSaveAs | transcript_export | empty (false) |
| pickMediaFile | none (JS dialog plugin `open`) | empty (null) |
| meetingStart | meeting_start | throws |
| meetingStop | meeting_stop | no-op |
| meetingRetranscribe | meeting_retranscribe | throws |
| meetingSetPeople | meeting_set_people | throws |
| meetingsSearch | meetings_search | empty |
| meetingAudioClip | meeting_audio_clip | throws |
| meetingSummarize | meeting_summarize | throws |
| meetingDevices | meeting_devices | mock (nulls) |
| meetingCaptureSupport | meeting_capture_support | mock (URL param `?capture=` with unsupported, permission or failed) |
| meetingOpenCaptureSettings | meeting_open_capture_settings | no-op |

### Pure helpers in the bridge file (no Rust call)
`transcriptParagraphs`, `speakerName`, `speakerLabel`, `TRANSCRIPT_PARAGRAPH`, `MEDIA_EXTENSIONS`. `transcriptParagraphs` mirrors a Rust function and the two must stay equal.

### Tauri JS APIs used outside the bridge
- `@tauri-apps/api/app` `getVersion` (sidebar version).
- `@tauri-apps/api/window` `getCurrentWindow` (custom title bar: minimize, maximize, close).
- `@tauri-apps/api/webview` `onDragDropEvent` (Transcribe drop zone).
- `@tauri-apps/plugin-store` (settings and stats persistence).
- `@tauri-apps/plugin-updater` and `plugin-process` (`check`, download with progress, install, `relaunch`).
- `@tauri-apps/plugin-dialog` (file picker, inside the bridge).

### Events the frontend listens to
`pipeline://state`, `pipeline://result`, `pipeline://error`, `pipeline://level`, `mic://level`, `download://progress`, `transcript://update`, `tts://state`, `nav://to`, `proofread://add-vocab`, `notice://show`, `squiggle://set`, `squiggle://popup`, and `app-theme://changed` (sent by the main window, heard by the pill).

### Registered in Rust, no bridge function
`proofread_text`, `tts_read_selection`, `show_overlay`. The frontend never calls them.

## 4. Platform dependence

The repo does not gate any plugin with `cfg(desktop)` or `cfg(mobile)`, and there is no `src-tauri/gen/android` or `gen/ios`. So "supported on mobile" cannot be proven from the code for any plugin. Where I add a note from general Tauri knowledge, it is marked "per Tauri docs, not in the repo" and should be checked before relying on it.

What the repo does show: `lib.rs` has `#[cfg_attr(mobile, tauri::mobile_entry_point)]` and `Cargo.toml` builds `staticlib`, `cdylib` and `rlib`, so the crate is mobile-shaped. `capabilities/default.json` points at the desktop schema.

| Feature | Where | Mobile status |
|---|---|---|
| Tray icon and menu | Cargo feature `tray-icon`, `system/tray.rs` | Unsupported. Desktop concept; no tray on phones (per Tauri docs, not in the repo). |
| Close = hide window | `on_window_event` in `lib.rs` | Unsupported. Needs desktop windows. |
| Single instance | `tauri-plugin-single-instance`, registered first | Unknown from the repo. Per Tauri docs it is desktop-only, and it is registered with no `cfg`, so it would break a mobile build as written. It also carries the `--start/--stop/--toggle` flags. |
| Global shortcut | `tauri-plugin-global-shortcut`, handler in `lib.rs` | Unknown from the repo. Per Tauri docs it is desktop-only. The whole push-to-talk, paste-last and read-aloud key design depends on it. |
| Autostart | `system/autostart.rs` (Windows Run key, macOS LaunchAgent, Linux XDG file) | Unsupported. Hand-written for desktop OSes; no Android or iOS branch. |
| Updater | `tauri-plugin-updater`, pubkey and GitHub `latest.json` in `tauri.conf.json` | Unknown from the repo. Per Tauri docs it is desktop-only. Stores deliver mobile updates. |
| Process (relaunch) | `tauri-plugin-process` | Unknown. Only used after an update. |
| Store (settings.json) | `tauri-plugin-store` | Unknown from the repo. Per Tauri docs it supports mobile. |
| Dialog (open, save) | `tauri-plugin-dialog` | Unknown from the repo. Per Tauri docs it supports mobile. |
| Shell (sidecars) | `tauri-plugin-shell`, `externalBin: sidecars/whisper-cpp` | Unsupported for running helper programs: sidecars are separate executables (whisper, llama-server, piper), which phones do not allow in this form. |
| Custom title bar | `Titlebar.tsx`, `decorations: false` | Unsupported. Windows and Linux only. |
| Always-on-top pill window | `system/overlay.rs`, window label `overlay` | Unsupported. Needs desktop window control. |
| Underline and suggestion windows | `system/squiggle*.rs`, labels `squiggle`, `squiggle-popup` | Unsupported. Needs system accessibility APIs and click-through windows. |
| On-screen notice window | `system/notice.rs` | Unsupported as a window. A phone would use a notification. |
| Paste at cursor | `arboard`, `enigo` | Unsupported on phones. |
| Foreground app detection | `system/foreground.rs` | Unsupported. Powers per-app styles and the "where you speak" chart. |
| Updater signing and macOS private API | `macOSPrivateApi: true`, `tauri.macos.conf.json` | Desktop only. |
| Window drag and drop | `onDragDropEvent` in Transcribe | Unknown. Likely unavailable on phones. |
| App data folder | `dirs::config_dir()` in `registry.rs` and `logging.rs` | Unsupported as written: `dirs` has no config folder on Android or iOS, so the code would fall back to `.`. A mobile build needs Tauri's `app_data_dir`. |
| Bundled files folder | `system/paths.rs` `bundled_dir()` | Branches only for macOS and Linux; no Android or iOS branch. |
| Hardware detection | `sysinfo`, DXGI (Windows) | Unknown. |
| Microphone capture | `cpal` | Unknown. `cpal` can target Android and iOS, but the repo has never built there, and permission prompts are not wired. |
| Computer-sound capture | WASAPI, ScreenCaptureKit, PulseAudio | Unsupported. No phone equivalent. |
| CSP and dev URL | `tauri.conf.json` | Carries (a plain web CSP), but `devUrl` is localhost. |
| Bundle targets | `nsis` base; `app`/`dmg` macOS; `deb`/`appimage` Linux | No mobile target configured. |

## 5. Mobile portability

Layout facts that drive these rows: the dashboard is a fixed 224 px sidebar beside a scrolling page, the window minimum is 820 by 560, and many pages assume a mouse (hover actions, drag, hotkeys). A phone screen is about 390 px wide.

| Item | Verdict | Why |
|---|---|---|
| Home | Needs rework | One-screen two-column cockpit built around a hotkey; a phone needs a big record button and a stacked layout. The heatmap and Today stats carry over. |
| Progress | Needs rework | Logic carries (stats store is plain JS). The wide heatmap and by-hour charts need a narrow layout, and "where you speak" depends on desktop app names. |
| Transcribe | Needs rework | Backend carries (file in, transcript out) but drag and drop becomes a file picker, and the list-plus-detail layout must collapse to one pane. |
| Notetaker | Needs rework | Mic-only recording could work; the Others track is desktop-only (ADR 0001). Gear panel, wide paragraph and audio player layout need a phone redesign. |
| Model Store | Needs rework | Dense multi-column rows and a Hugging Face browser. The model list and engine choices differ on a phone (no whisper, llama or piper sidecars). Fit dots depend on desktop hardware data. |
| Writing styles | Needs rework | Style editor and prompt text carry. Local source relies on the bundled llama-server (desktop sidecar), so only cloud styles carry at first. Layout needs one column. |
| Theme | Needs rework | App accent and dark or light carry as CSS. The grammar-suggestion popup pickers are desktop-only (no inline proofreading on phones). |
| Settings | Needs rework | Rail plus content layout needs a drill-down list. Roughly half the rows are desktop-only: hotkeys, paste, autostart, pill, underlines, CPU cores, GPU, read-aloud key. |
| History | Needs rework | Search, edit, copy, delete, retention and audio carry. Layout and the hover actions need touch design. |
| Cloud providers | Carries over as-is | Plain forms and cards over HTTP; keys in the store. Needs only responsive spacing (the logic runs in Rust over `reqwest`). |
| How to use | Needs rework | Text is about holding a hotkey and the pill, which do not exist on a phone. |
| Onboarding | Needs rework | Model choice, mic check and test dictation carry; the push-to-talk key step and the sidebar-free rail layout do not. |
| Sidebar navigation (11 links) | Needs rework | Fixed 224 px sidebar; use a bottom tab bar or a menu on a phone. |
| Custom title bar | Desktop-only | Window buttons have no meaning on a phone. |
| Router, Zustand stores | Carries over as-is | Plain React; no desktop APIs. |
| Settings persistence (plugin-store, settings.json, migrations) | Carries over as-is | Same code path; confirm the plugin on Android and iOS. |
| Theme system (dark, light, system, accents) | Carries over as-is | CSS variables and `prefers-color-scheme`. |
| Screen-reader live region | Carries over as-is | Plain DOM. |
| Browser-preview mocks (`isTauri` guard) | Carries over as-is | Useful for building mobile UI without a device. |
| Bridge file `tauriBridge.ts` | Needs rework | The pattern carries, but each command must exist on mobile; many call desktop-only Rust (hotkeys, overlay, paste, accessibility, sidecars). |
| useRuntimeSync (push settings to Rust) | Needs rework | Pushes hotkeys, autostart, popup theme and app profiles that mobile does not have. Keep the model, language, vocabulary and behavior parts. |
| Recording pill window | Desktop-only | Always-on-top separate window. |
| Underline and suggestion windows | Desktop-only | System-wide accessibility and click-through windows. |
| On-screen notice window | Desktop-only | A phone would use an in-app banner or notification. |
| System tray and tray recording dot | Desktop-only | No tray on phones. |
| Main window close = hide | Desktop-only | Concept of a hidden window does not exist. |
| Single-instance and CLI flags | Desktop-only | One app instance by design on phones. |
| Global hotkeys (push-to-talk, paste last, read aloud) | Desktop-only | No global keys. A phone needs an on-screen button, keyboard extension or share action. |
| Autostart | Desktop-only | Not a phone concept. |
| Updater (sidebar button, Settings check) | Desktop-only | Phones update through the stores. |
| Package-managed check (`package_managed`) | Desktop-only | Linux packaging only. |
| Logs: copy diagnostics | Carries over as-is | Plain text to the clipboard (clipboard permission aside). |
| Logs: save to Downloads and reveal in file manager | Needs rework | Uses a Downloads folder and `explorer`, `open`, `xdg-open`. A phone would share the file. |
| Data paths (`registry.rs`, `logging.rs`, `paths.rs`) | Needs rework | Built on `dirs::config_dir()`; need Tauri's app data folder on mobile. |
| Tauri capabilities file | Needs rework | Desktop schema and desktop-only permissions (shortcut, shell, window). Needs per-platform capability files. |
| CI: `ci.yml` (tsc, vitest, cargo test on Windows) | Carries over as-is | The frontend checks apply unchanged. |
| CI: release, cross-platform build, distro smoke, AUR | Desktop-only | They build and test Windows, macOS and Linux installers. Mobile needs new jobs. |
| Single Hugging Face model browser | Needs rework | Heavy list UI and multi-GB downloads; likely replaced by a small curated list on a phone. |

## 6. Gaps: code versus HANDBOOK.md

Things the handbook gets wrong:
1. **Header.** "Current state v0.1.8 is released"; the app is 0.2.0. Section 1 says Windows only; macOS and Linux build and run.
2. **Section 3 tree is stale.** Missing: `statsStore`, `updateStore`, `announceStore`, `tauriStorage`; `services/updater.ts`, `appThemes.ts`; hooks `useAutoVoiceDetection`, `useResolvedTheme`; overlay views `SquiggleOverlay`, `SquigglePopup`, `NoticeCard`; the `dashboard/hf`, `providers` and `settings` folders; shared components (Titlebar, ThemeSwitch, TranscriptCard and others); Rust `system/paths.rs`, `notice.rs`; the three platform configs (`tauri.windows.conf.json`, `tauri.macos.conf.json`, `tauri.linux.conf.json`). It describes Onboarding as a 4-step wizard; the code is a left-rail flow with four steps plus an "all set" screen.
3. **Section 12 command table.** Lists six commands that do not exist: `ollama_status`, `ollama_generate`, `paste_text`, `set_overlay_opacity`, `speak_text` and `stop_tts` (the last two are now `tts_speak_text` and `tts_stop`). About 50 registered commands are missing from it, including all of `hf_*`, `meeting_retranscribe`, `meeting_set_people`, `meetings_search`, `meeting_audio_clip`, the helper-model commands (VAD, diarize, CoEdIT, GECToR), `copy_diagnostics`, `export_logs`, `open_logs_folder`, `package_managed`, `set_popup_theme`, `set_popup_surface`, `set_paste_last_hotkey`, the mic probe, the notice commands, pause and cancel download, and the audio clip commands.
4. **Section 13 events.** `tray://toggle-record`, `tray://toggle-listen`, `pipeline://partial` and `overlay://opacity` are listed but nothing in the code emits or listens to them (the tray calls Rust functions directly). Missing: `pipeline://level`, `mic://level`, `nav://to`, `notice://show`, `squiggle://set`, `squiggle://popup`, `app-theme://changed`, `proofread://add-vocab`. `tts://state` is described as "for future UI"; the pill already uses it.
5. **Section 7 and paths.** `system/paths.rs` only finds the bundled-files folder; the data root is in `models/registry.rs`. The handbook omits `data-location.json` (an override for models and history, read by Rust but nothing in the app writes it) and `overlay-pos.json`. With the override set, speech models go in `<root>\stt`, not `models`. The data and log folders come from `dirs::config_dir()`: that is `%APPDATA%` only on Windows (`~/.config` on Linux, `~/Library/Application Support` on macOS). `settings.json` is written by `tauri-plugin-store` into the plugin's default app folder (expected to be under the app identifier, not `SilentVoice`); the handbook names no path. Log export goes to Downloads.
6. **Autostart.** Section 16 says it writes the Windows Run key. The code also has a macOS LaunchAgent and a Linux XDG autostart file.
7. **Updater.** Handbook does not mention that the updater turns itself off for package-managed Linux installs (`package_managed`), that the sidebar button has a "not now" dismiss and a cancel button with a progress bar, or that cancel only works at the install step (the download itself cannot be stopped). It still says the sidebar footer holds an "offline-ready" label; the footer now holds the theme switch and the version.
8. **Settings page.** Handbook sends readers to "Settings, Performance" and "Custom vocabulary". Settings is now five topics with search and Advanced cards.
9. **Window lifecycle.** Handbook omits that closing the dashboard also shrinks WebView2 memory (`set_webview_memory_low`), and that macOS uses a native overlay title bar while Windows and Linux use the custom one.
10. **CI.** Only `release.yml` is described. Not covered: `ci.yml` (Windows: typecheck, vitest, cargo test; runs only on pushes and pull requests to `main`, so the `mobile` branch gets no run), `cross-platform.yml` (cargo check and unit tests on macOS and Linux, path-filtered), `cross-platform-build.yml` (builds installers and smoke-tests them, path-filtered or manual), `linux-distro-smoke.yml` (tests the published AppImage on several distros), `aur.yml` (publishes the AUR package; does nothing until its secrets exist). The release job also writes one `latest.json` for all platforms and refuses to publish a build missing its `.sig`.

Things in the code that look wrong or loose:
11. **`stt_preset`** is written by Onboarding and read by nothing.
12. **History in two places.** `historyStore` uses Zustand `persist` with the default storage (`localStorage`), so dictated text is also kept in the WebView's local storage in the desktop app, even though the comment says the file is the only source and `blur_history` suggests privacy matters. The file wins on load.
13. **Capabilities.** `capabilities/default.json` covers windows `main`, `overlay` and `notice` only. The `squiggle` and `squiggle-popup` webview windows (created on macOS and Linux) are not listed, yet they call `listen`. Worth testing on those OSes. It also grants `shell:allow-execute/spawn` and the global-shortcut permissions to the web side, though the frontend uses neither (sidecars and shortcuts are handled in Rust).
14. **Dead or unused.** `deleteVadModel` has no caller. `proofread_text`, `tts_read_selection` and `show_overlay` are registered but never called from the frontend. `retranscribeClip` has no browser guard. A comment above `previewDictationSound` in the bridge file describes the TTS function that follows it.
15. **Help text.** Guide and Writing styles refer to an "AI Processing tab" and a "Cloud providers tab"; the pages are called "Writing styles" and "Cloud providers" in the sidebar.
16. **Single-instance and plugins** are registered without `cfg(desktop)`, so the crate would not build for Android or iOS as it stands.
