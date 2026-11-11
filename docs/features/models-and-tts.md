# Models and text-to-speech

Area: model catalogs, Model Store, downloads, bundled engines (sidecars and FFI), GPU use, and read-aloud.
Deep detail already written elsewhere: [HANDBOOK section 2](../HANDBOOK.md) (stack), [section 7](../HANDBOOK.md) (data folders), [section 15](../HANDBOOK.md) (STT catalog rule), [section 16](../HANDBOOK.md) (Read aloud, second TTS engine, pinning), [section 17](../HANDBOOK.md) (sherpa pin), [ADR 0002](../adr/0002-sherpa-onnx-one-pinned-version.md) (one pinned sherpa-onnx version).

## 1. Feature list

| Feature | Controlled by |
|---|---|
| Speech-to-text model catalog (about 25 curated entries: Whisper family, Parakeet, Moonshine, SenseVoice, Qwen3-ASR, language fine-tunes) | `settings.active_stt_model` |
| Local AI (LLM) model catalog (8 GGUF models: Llama, Gemma, Phi, Qwen) | A mode's `model_source = "local"` and `model_id` |
| Voice (TTS) catalog: 19 curated Piper voices, 3 Kokoro/Melo packs, 2 Bangla voices, plus the full live Piper voice list | `settings.active_tts_voice` |
| Model Store page with three tabs (Speech, AI, Voices) | Sidebar page |
| Hugging Face browser: live search, sort by downloads / likes / newest / trending, repo details | `settings.hf_show_incompatible` |
| Quant picker: pick a GGUF or GGML file from a repo; best fit is pre-selected | Per download |
| Device-fit check: models grouped "Runs well / Runs, but may be slow / Too heavy", and a confirm step before a heavy download | Detected hardware |
| Speed and accuracy bars (speed scaled to this CPU) | Detected hardware |
| Language ranking: models for your system language and time zone float up; other languages sit under "More languages" | OS locale (offline) |
| Pin (star) models per tab | `pinned_stt`, `pinned_llm`, `pinned_tts` |
| Downloads: progress, pause, resume, cancel, retry, size check | Model Store buttons |
| Remove a model (confirm dialog) | Model Store button |
| Models that left the store stay listed, usable and removable | Automatic |
| Startup cleanup of the old speaker model (`embedding.onnx`) | Automatic |
| Extra helper models: punctuation repair (7 MB), voice activity (Silero), speaker labels (CAM++ plus pyannote), CoEdIT grammar, GECToR grammar (int8 or fp32) | Settings toggles; each has download and delete |
| Moving the data folder | Settings, data location |
| GPU use for Whisper and local AI | `settings.use_gpu`, auto-set once (`gpu_auto_checked`) |
| Free the speech model after idle | `settings.model_unload_minutes` (0 = never) |
| Performance threads | `high_performance`, `performance_threads` |
| Custom vocabulary biases every engine that supports it | `custom_vocabulary`, `use_vocabulary` |
| Cloud fallback to local STT | `stt_cloud_provider_id` |
| Read aloud: select text, press hotkey, hear it | `tts_enabled`, `tts_hotkey` (default Ctrl+Alt+S), `active_tts_voice` |
| Read aloud pause, resume, stop from the pill | Overlay buttons |
| Voice preview ("Play sample") in Settings and Model Store | Button |
| Kokoro: pick one of 11 voices inside one pack | Voice id `kokoro-...#sid` |

## 2. How it works

**Catalogs.** `src/services/catalog.ts` holds `STT_MODELS`, `LLM_MODELS`, `TTS_MODELS`. STT rule: `file` must equal `ggml-<id>.bin` for Whisper entries ([HANDBOOK 15](../HANDBOOK.md)). Sherpa-family STT entries (`engine` = transducer, moonshine, sense-voice) download as `.tar.bz2` into a folder; `llama-asr` (Qwen3-ASR) downloads two GGUF files (model plus `mmproj`). Rust finds the engine by id prefix in `models/registry.rs::stt_engine`.

**Model Store.** `src/components/dashboard/ModelStore.tsx` hosts the Voices tab; `hf/HfBrowser.tsx` renders the Speech and AI tabs (staff picks plus Hugging Face search). Search calls `hf_search_models(query, sort, limit, track)`; details call `hf_model_details(repo_id, track)`, both in `models/hf.rs`. STT search keeps only ggml repos, LLM search keeps GGUF. Voice tab loads the Piper list with `hf_piper_voices` (reads `rhasspy/piper-voices/voices.json`) and merges it under the curated list. Fit logic is in `hf/fit.tsx`, `services/recommend.ts` and `services/modelMetrics.ts`; hardware comes from `get_hardware_info` (`system/hardware.rs`, sysinfo plus DXGI / system_profiler / lspci).

**Downloads.** Commands in `lib.rs`: `download_model`, `download_stt_archive`, `download_llama_asr`, `download_llm_model`, `download_tts_model`, `download_vad_model`, `download_diarize_model`, `download_coedit_model`, `download_gector_model`, plus `pause_download`, `cancel_download`, `delete_model`, `delete_llm_model`, `delete_tts_model`, and `list_downloaded_models / _llm / _tts`. All stream through `models/downloader.rs::fetch_to_file` into `<name>.part`, send a `Range` header to resume, retry 4 times, check the final size, then rename. Pause sets a stop flag (keeps `.part`); cancel also deletes it. Progress arrives as the `download://progress` event (`status` = downloading, downloaded, paused, cancelled, error). State lives in `src/stores/modelStore.ts`. Ids are checked by `safe_id` and tar paths by `rel_is_safe` so a bad name cannot escape the data folder.

**Left the store but still usable.** Rust finds a model by id and file, not by catalog. `retiredSttModels` and `retiredLlmModels` in `catalog.ts` build stand-in rows ("no longer in the store") so the user can still select or remove them. TTS has no stand-in: `list_downloaded_tts` lists whatever is on disk, but the Voices tab only shows catalog and Piper-list voices (see gaps).

**Startup cleanup.** In `lib.rs` setup: `ensure_dirs()`, delete `diarize/embedding.onnx` (old 40 MB speaker model), then a background thread loads sherpa-onnx so the version check logs once. A 10-minute idle reaper frees the CoEdIT session; `model_unload_minutes` frees the speech model.

**Engines and loading.**
- Whisper: resident `whisper-server.exe` (`transcription/server.rs`, localhost HTTP on port 8090, `-bs 1 -bo 1 -fa`). One-shot `whisper-cpp` CLI sidecar is the fallback in `transcription/whisper.rs`. Vulkan build; GPU off means env `GGML_VK_VISIBLE_DEVICES=-1` (server) or `-ng` (CLI).
- Local AI: `llm/llama.rs` starts `llama-server.exe` on port 8088, context 4096, `-ngl 99` with GPU or `--device none`; restarts when the model changes. Also runs Qwen3-ASR with `--mmproj`. Orphaned sidecars are reaped by `system/job.rs`.
- Sherpa-onnx: `system/sherpa.rs` (TTS) and `system/sherpa_stt.rs` (Parakeet, Moonshine, SenseVoice, punctuation) load `sherpa-onnx-c-api.dll` with libloading after pre-loading our `onnxruntime.dll` by absolute path. Version must equal `sidecars/sherpa/VERSION` (1.13.4) or load fails with a clear error ([ADR 0002](../adr/0002-sherpa-onnx-one-pinned-version.md)). Layout tests: `system/sherpa_layout_tests.rs`, `scripts/sherpa-layout.c`.
- ONNX Runtime via the `ort` crate (`onnx.rs`) reuses sherpa's `onnxruntime.dll` for Silero VAD (`audio/vad.rs`), CoEdIT (`coedit.rs`) and GECToR (`gector.rs`).
- Piper: `tts.rs::synthesize` spawns `piper.exe --model ... --output_file ...`, text on stdin.

**Read aloud.** Hotkey -> `tts::read_selection` copies the selection, `resolve_voice` picks Piper (`<id>.onnx` + `.onnx.json`), Kokoro (folder with `voices.bin`, voice index after `#`) or generic sherpa VITS (folder), `synthesize` writes `audio/tts.wav`, rodio plays it. Commands: `set_tts`, `tts_read_selection`, `tts_speak_text`, `tts_pause`, `tts_resume`, `tts_stop`. Event `tts://state` ("synthesizing" | "speaking" | "idle") drives the pill buttons. Sherpa synthesis uses the C API, never the CLI ([CLAUDE.md rule 8](../../CLAUDE.md)).

**GPU.** `recommend_device_defaults` sets `use_gpu` only for NVIDIA or AMD with 4 GB or more of own memory (`fast_gpu`); weaker cards were slower than CPU. `useRuntimeSync.ts` applies it once. Qwen3-ASR is marked `needs_fast_gpu` and is ranked down without one.

## 3. Engine inventory

| Engine | How it runs | Model sizes on disk | Used for |
|---|---|---|---|
| whisper.cpp v1.9.1 (Vulkan) | Sidecar exes `whisper-server.exe` (resident, HTTP) and `whisper-cpp` (CLI fallback), plus ggml DLLs | 55 MB to 1.6 GB (tiny 75, base 142, small 466, large-v3-turbo 1638) | Dictation, file transcribe, meetings |
| Parakeet TDT 0.6B v2/v3 | sherpa-onnx C API (FFI DLL), transducer | 640 MB each | Best-accuracy dictation |
| Moonshine tiny/base | sherpa-onnx FFI | 108 / 251 MB | Fast English dictation |
| SenseVoice Small | sherpa-onnx FFI | 156 MB | Fast multilingual dictation |
| Qwen3-ASR 1.7B | llama-server sidecar (model GGUF plus mmproj) | 2.4 GB | Multilingual dictation, needs strong GPU |
| llama.cpp b9830 (Vulkan) | Sidecar `llama-server.exe`, HTTP on 8088 | 0.77 to 4.9 GB (8 models) | Mode rewrites, meeting summaries |
| Cloud LLM / STT | HTTP, OpenAI-compatible (`llm/openai.rs`) | none | Optional modes, cloud STT |
| Punctuation repair (sherpa online punct) | sherpa-onnx FFI | about 7 MB | Fix punctuation on background transcription |
| Silero VAD | ort (ONNX Runtime) in process | about 2 MB | Silence gate, segmenting |
| CAM++ plus pyannote segmentation | sherpa-onnx FFI (`system/diarize.rs`) | about 30 MB | Speaker labels in meetings |
| CoEdIT-large int8 | ort in process | not stated in code or HANDBOOK (int8 encoder and decoder) | Generative grammar pass |
| GECToR RoBERTa | ort in process (int8 or fp32, user picks) | int8 smaller than fp32; both together would be about 630 MB (per downloader comment) | Inline proofreading and grammar |
| Piper (release 2023.11.14-2) | Sidecar `piper.exe` plus espeak-ng-data, own onnxruntime 1.17 | voices 20 to 116 MB each | Read aloud, about 19 curated plus all of Piper's list |
| Kokoro v0.19 (int8 / full) | sherpa-onnx FFI, 11 voices per pack | 98 / 305 MB | Read aloud (English) |
| MeloTTS English | sherpa-onnx FFI (VITS config) | 155 MB | Read aloud |
| Bangla VITS (Coqui) and Meta MMS | sherpa-onnx FFI (VITS config) | 103 / 109 MB | Read aloud (Bangla) |

## 4. Mobile portability

| Item | Verdict | Why |
|---|---|---|
| STT catalog and model metadata | Carries over as-is | Plain TypeScript data; trim to small models |
| LLM and TTS catalogs | Carries over as-is | Same, with a smaller list |
| Model Store UI and HF browser | Needs rework | React UI fine, but the fit rules assume desktop RAM and GGML files |
| Hugging Face search and details (`hf.rs`) | Needs rework | Rust HTTP; move to JS fetch or keep in Rust core |
| Device-fit and speed bars | Needs rework | Thresholds and hardware probe (DXGI, lspci) are desktop |
| Downloader (resume, pause, cancel, verify) | Needs rework | Logic is sound; needs background download service and OS storage rules |
| Models that left the store, startup cleanup | Carries over as-is | Pure id and file logic |
| Data-folder move | Desktop-only | Phones have fixed app storage |
| Pinning, language ranking | Carries over as-is | Settings plus locale |
| GPU toggle and `fast_gpu` | Needs rework | Vulkan on desktop; phones use Metal, NNAPI or CPU |
| Idle model unload | Needs rework | Needed more on mobile, tied to desktop AppState |
| whisper.cpp (server and CLI) | Needs rework | Sidecar exes cannot run on phones; link the library directly |
| llama.cpp `llama-server` | Needs rework | Sidecar exe plus localhost HTTP; use an in-process build |
| Parakeet, Moonshine, SenseVoice (sherpa FFI) | Needs rework | Sherpa has Android and iOS builds, but the hand-written Windows FFI loader and the pin need redoing |
| Qwen3-ASR | Desktop-only | 2.4 GB and wants a strong GPU |
| Punctuation, Silero VAD, CAM++ | Needs rework | Small and portable, but loaded via desktop DLL paths |
| CoEdIT and GECToR (ort) | Needs rework | Heavy on phones; reuse needs an ort mobile build |
| Cloud LLM and cloud STT | Carries over as-is | Plain HTTP |
| Piper (`piper.exe`) | Needs rework | Sidecar exe; use sherpa Piper support or native TTS |
| Kokoro, MeloTTS, Bangla VITS (sherpa FFI) | Needs rework | Works on mobile via sherpa; loader must change |
| Read aloud hotkey and selection copy | Desktop-only | Global hotkey and simulated copy; mobile would use a share or select action |
| Read aloud playback (rodio) and pill controls | Needs rework | Replace with OS audio and a notification control |
| Sidecar reaper (`system/job.rs`) | Desktop-only | No child processes on mobile |
| Sherpa version pin and layout tests | Carries over as-is | The idea and CI checks still apply |

## 5. Gaps (code versus HANDBOOK)

- HANDBOOK section 2 and 3 say whisper runs only as a bundled sidecar CLI; the main path is a resident `whisper-server.exe` over HTTP, with the CLI only as fallback. Section 2 also says "GPU paths exist but untested" and lists only Whisper and llama; Parakeet, Moonshine, SenseVoice, Qwen3-ASR, CoEdIT, GECToR, Silero are missing from the stack table.
- Section 15 says "~49 STT models"; `STT_MODELS` has about 25. Section 16 says 29 Piper voices; the curated list has 19 (the live Piper list adds many more).
- Section 16 says sherpa TTS uses `sherpa-onnx-offline-tts.exe`; the code uses the C-API FFI only (CLAUDE.md rule 8 is right, the section is stale). It also says `Voice::Piper | Voice::Sherpa`; there is now a third, `Voice::Kokoro`.
- Section 12 lists `stop_tts`; the real commands are `tts_stop`, `tts_pause`, `tts_resume`, `tts_speak_text`, `tts_read_selection`. Section 13 says `tts://state` is "for future UI"; the pill uses it now.
- Section 7 lists only `tts/<id>.onnx` pairs, not sherpa voice folders, `models/<id>/` folders for sherpa STT, `diarize`, `vad`, `punct`, `coedit`, `gector` dirs, or the movable data location (`registry.rs::load_data_location`).
- CLAUDE.md says "device-fit dots"; `fit.tsx` says colored dots were replaced by grouped headings plus a pre-download check.
- Sherpa TTS config mirrors Matcha, Kitten, ZipVoice, Pocket and Supertonic structs, but `synthesize` fills only Kokoro and VITS; other families are not wired.
- `sidecars/parakeet.dll` sits in the sidecars folder but nothing in `src-tauri/src` references it.
- A downloaded TTS voice that leaves the catalog is not listed in the Voices tab (no retired-voice stand-in, unlike STT and LLM) unless it is in Piper's live list.
- The User-Agent string `SilentVoice/0.1.6` in the downloader is stale (still says 0.1.6).
