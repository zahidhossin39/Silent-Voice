# AI text processing and proofreading

Scope: AI modes, the local LLM, cloud providers, per-app styles, grammar models,
inline proofreading (underlines in other apps), the correction popup, and
learn-from-edits. Read against the code on branch `mobile`. Where this file and
`docs/HANDBOOK.md` disagree, the code is right (see section 5).

Deep detail that the handbook already covers well: the inline proofreading
architecture, the editability gate, CRLF offset drift, the popup rendering rules
and the WebView2 accessibility trick are all in
[HANDBOOK section 16](../HANDBOOK.md) ("Inline proofreading"). The mode shape is in
[HANDBOOK section 10](../HANDBOOK.md). The "local means llama-server, not Ollama"
rule is in HANDBOOK section 8.5.

## 1. Feature list

| # | Feature | Controlled by |
|---|---|---|
| 1 | AI modes: 9 built-in (Raw, Clean Up, Whisper Flow Style, Formal, Casual, Email, Summary, Translate, Code Comment) plus custom modes | `settings.active_mode_id`, `settingsStore.modes` (Modes page) |
| 2 | Mode editor, per-mode Test button and sample previews | Modes page |
| 3 | Local AI (bundled llama-server, GGUF models from the Model Store) | mode `model_source = "local"`, `model_id`; `settings.use_gpu`, `high_performance`, `performance_threads` |
| 4 | Cloud AI providers (OpenAI-compatible: OpenAI, OpenRouter, Groq, Anthropic, Gemini, xAI, DeepSeek, Mistral, Perplexity, Together, Fireworks, DeepInfra, Custom). Local servers like Ollama or LM Studio work through "Custom" | API Keys page, `settingsStore.providers`; mode `model_source = "api"`, `provider_id`, `api_model` |
| 5 | Fallback to raw text when AI fails or returns nothing, plus a "Pasted without AI mode" on-screen card | automatic |
| 6 | Cleanup of chatty model output (preamble line, wrapping quotes) | automatic (`tidy_ai_output`) |
| 7 | Writing style per app (per-app profiles) | Settings > Writing help > "Writing style per app", `settingsStore.appProfiles` |
| 8 | Fix grammar before pasting (CoEdIT, on-device) | `settings.coedit_enabled` (needs an 818 MB download) |
| 9 | Inline proofreading: spelling, grammar and style underlines in other apps (Harper) | `settings.inline_proofread` (default OFF) |
| 10 | Smarter grammar checks (GECToR model, two variants) and its sensitivity | download in Settings; `settings.gector_sensitivity` (relaxed / balanced / aggressive); rule id "Gector" in `proofread_disabled_rules` |
| 11 | Rule toggles: Oxford comma, filler words; built-in repetition lint; code/JSON text is skipped | `settings.proofread_disabled_rules` ("OxfordComma", "Filler") |
| 12 | Don't check these apps (user list) plus a built-in ignore list (editors, terminals, password managers) | `settings.proofread_ignore_apps`; `IGNORE_APP_SUBSTRINGS` |
| 13 | Click-to-fix popup on an underline (hover, pick a suggestion) | automatic with feature 9 |
| 14 | Add to Dictionary and Dismiss in the popup | popup buttons; writes `settings.custom_vocabulary` |
| 15 | Learn-from-edits: after a dictated paste, a word you retyped gets an "add to dictionary" underline | automatic when feature 9 is ON |
| 16 | Popup look: accent palette and dark/light card | Theme page: `settings.popup_theme`, `settings.popup_surface` |
| 17 | Paste verification (did the text land?) and clipboard fallback card | automatic (Windows) |
| 18 | Password-field guard before auto-paste | automatic (Windows) |
| 19 | Custom vocabulary as the proofreader's personal dictionary | `settings.custom_vocabulary` |
| 20 | Sound-alike vocabulary fixer ("quad code" becomes "Claude Code") | `settings.use_custom_vocabulary` |
| 21 | Accessibility permission check and shortcut (macOS) | Onboarding / Settings, commands below |
| 22 | WebView2 app support for underlines (screen-reader flag + focus handler) | automatic while feature 9 is ON (Windows) |
| 23 | Meeting summary through the same LLM runtime (owned by Notetaker, listed because it shares the runtime) | Notetaker page |
| 24 | History squiggles (removed from the UI; backend command still exists) | none, dormant |

## 2. How it works

### Modes, prompts and sources (features 1 to 6)
- A mode is `{ id, name, system_prompt, model_source: none | local | api, model_id, provider_id, api_model, ... }`
  (`src/types/index.ts`, built-ins in `src/services/modes.ts`). Built-ins that the
  user never edited are refreshed from `BUILTIN_MODES` on load; edited ones carry
  `customized: true` and are left alone (`src/stores/settingsStore.ts`).
- `src/hooks/useRuntimeSync.ts` resolves the active mode into plain fields (prompt,
  model, base URL, API key) and calls the Tauri command `set_active_mode`. Rust only
  stores them in `RuntimeConfig` (`mode_source`, `mode_prompt`, `mode_model`,
  `mode_base_url`, `mode_api_key`). Rust never sees the mode or provider tables.
- Pipeline (`process_audio_pipeline` in `src-tauri/src/system/hotkey.rs`): after
  transcription and cleanup (fillers, repeated words, spoken symbols, sound-alike fix)
  the text goes to the LLM only if there is text, a non-empty prompt and the source is
  `local` or `api`. Raw mode has an empty prompt, so no LLM runs. After the LLM the
  deterministic passes still run on every mode: `structure_text`, `format_numbers`,
  then user replacements, then paste.
- `local`: `run_local_llm` in `lib.rs` takes a global lock, calls `ensure_local_llm`
  (starts `llama-server` on `127.0.0.1:8088` with `-c 4096`, waits up to 120 s for it),
  then posts to it with the same OpenAI client used for cloud. The model file is
  `%APPDATA%\SilentVoice\llm\<id>.gguf`. GPU offload is only used when `use_gpu` is on,
  otherwise `--device none`. Process management is in `src-tauri/src/llm/llama.rs`.
- `api`: `llm::openai::chat` (`src-tauri/src/llm/openai.rs`) posts a chat completion
  to `{base_url}/chat/completions` with a bearer key, temperature 0.3, frequency
  penalty 0.3 (left out for Gemini, which rejects it), 120 s timeout. The dictated
  text is wrapped in delimiters with "do not reply to it, only transform it", so a
  dictated question gets rewritten, not answered.
- Provider UI: `src/components/dashboard/ApiKeys.tsx` and `providers/` hold the
  presets, key entry, model list (`api_list_models`, with prices from OpenRouter) and
  Test connection (`api_generate`). The Modes page Test button calls `local_llm_generate`
  or `api_generate`.
- Failure: any error becomes `report_error(..., "ai-mode", ...)`, which logs it, emits
  `pipeline://error` and shows the notice window "Pasted without AI mode"
  (`system/notice.rs`). An empty reply also falls back. The raw text is pasted either way.
- `tidy_ai_output` strips a leading "Here is ...:" line and one pair of wrapping quotes.

### Per-app styles (feature 7)
- Settings rule: "app name contains X" gives mode Y. The frontend resolves each rule to a
  full mode config and calls `set_app_profiles`.
- On hotkey press `start_capture` stores the foreground app name in `AppState.active_app`
  (`system/foreground.rs::foreground_app`). In the pipeline, the first rule whose text is
  a substring of that name replaces the `mode_*` fields. A rule that points at a Raw mode
  turns AI off for that app.

### Fix grammar before pasting (feature 8)
- `src-tauri/src/coedit.rs`: CoEdIT-large int8, ONNX, run with the `ort` crate. Prompt is
  "Fix grammar: <text>". Skipped for one-word text, results cached by input.
- Runs only when no LLM mode is active (`else if coedit_enabled` in the pipeline).
- `set_behavior` triggers `coedit::prewarm` when the toggle turns on. A background sweep in
  `lib.rs` frees the model after 3 minutes idle.
- Download/delete: `download_coedit_model`, `coedit_installed`, `delete_coedit_model`
  (files from a Hugging Face repo, `models/downloader.rs`).

### Inline proofreading (features 9, 11, 12, 19)
- Windows: `system/inline_check.rs` (thread started in `lib.rs` setup). It uses UI Automation
  to read the focused field, runs `proofread::check`, maps each issue to screen rectangles,
  and sends them to `system/squiggle.rs`, which draws tiny click-through layered Win32
  windows. Details in HANDBOOK section 16.
- macOS and Linux: `system/inline_unix.rs` is the same loop over `system/ax.rs` (macOS
  Accessibility API) or `system/atspi.rs` (AT-SPI2 over D-Bus, loaded with dlopen).
  Underlines are drawn by `system/squiggle_overlay.rs`: one full-screen click-through Tauri
  window (`index.html?view=squiggle`, `src/components/overlay/SquiggleOverlay.tsx`) that
  receives `squiggle://set`.
- `proofread::check` (`src-tauri/src/proofread.rs`) does the shared logic: skip text that
  looks like code or JSON, run Harper (`harper-core`, English, American) with the custom
  vocabulary merged into its dictionary, add our own "Repetition" and "Filler" lints,
  then add GECToR edits that do not overlap an existing issue. Issues are char ranges, with
  up to 3 suggestions each.
- Guards before reading a field: own process, password fields (Windows), the built-in
  ignore list, the user list, an editability check, and a size cap of 6000 chars.
- `set_behavior` carries `inline_proofread`, `proofread_disabled_rules`, `gector_sensitivity`
  and `proofread_ignore_apps` into `RuntimeConfig`. The watcher re-reads them every cycle.

### GECToR (feature 10)
- `src-tauri/src/gector.rs`: a RoBERTa tagger exported to ONNX, run with `ort`. It splits text
  into sentences (skips ones over 350 chars), turns tags into edits, applies the sensitivity
  thresholds, and caches by sentence. It catches correctly spelled wrong words
  (their/there). Freed after 10 minutes idle.
- Two variants: int8 "Balanced" (122 MB) and fp32 "Best quality" (512 MB). Commands:
  `download_gector_model(variant)`, `gector_installed`, `delete_gector_model`.
  Download progress goes out on `download://progress` with id `gector`.

### Popup, dictionary, dismiss (features 13, 14, 16)
- Windows: hovering an underline for about 250 ms shows a native GDI card
  (`SVSuggestPopup` in `squiggle.rs`). Clicking a suggestion sends `OverlayAction::Fix` to the
  watcher thread, which re-checks the text, selects the range with UIA and types the
  replacement (enigo). The popup also has "Add to Dictionary" and "Dismiss"
  (`OverlayAction::AddToVocab` / `Dismiss`). Dismiss is kept in memory for the session.
  Add emits `proofread://add-vocab`, which `useRuntimeSync` turns into an edit of
  `custom_vocabulary`. Fixes never add to the vocabulary on their own.
- macOS and Linux: `inline_unix` polls the pointer against the underline rectangles and asks
  `squiggle_overlay::show_popup` to emit `squiggle://popup` to a second window
  (`SquigglePopup.tsx`). Clicking a row calls the command `inline_fix`, which runs
  `inline_unix::apply_pending_fix`. This popup has suggestions only.
- Look: `set_popup_theme` (5 accent palettes) and `set_popup_surface` (dark or light) change
  statics in `squiggle.rs`. They are Windows-only; other platforms ignore them.

### Learn-from-edits (feature 15)
- After a paste lands, `learn_from_edits` (`hotkey.rs`) re-reads the field every 3 s for
  about a minute. `textfmt::corrected_phrases` diffs the pasted text against the field and
  keeps small 1 to 3 word swaps. A phrase that is not already in the vocabulary and looks like
  a name or jargon (`sherpa_stt::hotword_phrases`) goes to `inline_check::suggest_learned`.
  It then shows as an underline for 10 minutes. Its only action is "Add to Dictionary".
- Needs inline proofreading ON (it draws with the same watcher). Windows only.

### Paste verification and guards (features 17, 18)
- `verify_paste` (Windows) re-reads the focused field 300 ms and 900 ms after paste. If the field
  is readable and lacks the end of the text, it puts the text on the clipboard and shows
  "Text copied instead". An unreadable field counts as "unknown" and is trusted.
- `secure_field::focused_is_password` (UIA `IsPassword`) stops auto-paste into password
  fields. It fails open.

### Sound-alike fixer (feature 20)
- `system/soundalike.rs`: compares how words sound (consonant skeleton) against vocabulary
  entries and swaps misheard words. Pure Rust, runs in the pipeline before the LLM.

### macOS accessibility (feature 21)
- `accessibility_granted` and `open_accessibility_settings` (`system/accessibility.rs`,
  `AXIsProcessTrusted`). Always true on other platforms.

### WebView2 support (feature 22)
- While inline proofreading is ON the watcher sets the system screen-reader flag
  (`SPI_SETSCREENREADER`) and registers a UIA focus handler so Chromium builds its
  accessibility tree. Reset on exit (`reset_screen_reader`).

### History squiggles (feature 24)
- `proofread_text` is still a registered command. Nothing in `src/` calls it. The History
  card dropped squiggles (note in `src/components/shared/TranscriptCard.tsx`).

## 3. Platform dependence

| Part | Pure logic (portable) | Runs a separate program | Calls OS APIs |
|---|---|---|---|
| Mode data, prompts, provider list, `resolveMode`, store refresh | TypeScript, `modes.ts`, `settingsStore.ts`, `useRuntimeSync.ts` | | |
| `tidy_ai_output`, fallback logic, notice text | Rust, `hotkey.rs` | | notice window is a Tauri window |
| Cloud chat client | Rust `llm/openai.rs` (reqwest, HTTPS) | | |
| Local LLM | HTTP client part is portable | `llama-server` exe from `sidecars/llama/` (Vulkan build on Windows) on port 8088 | process spawn |
| Per-app profiles | matching logic is portable | `xprop` on Linux, `osascript` on macOS (spawned commands) | Windows: `GetForegroundWindow`, `OpenProcess`, `QueryFullProcessImageNameW` |
| CoEdIT, GECToR | Rust + `ort` + `tokenizers` | none (in-process ONNX Runtime, loaded from `sherpa/onnxruntime.dll`) | native library load |
| Harper, repetition and filler lints, code skip | `proofread.rs`, pure Rust | | |
| Underline watcher, Windows | | | UI Automation (COM `IUIAutomation`, TextPattern, ValuePattern), `SystemParametersInfoW` |
| Underline drawing, Windows | | | Win32 layered windows, GDI, `UpdateLayeredWindow`, `SetWindowPos` |
| Typing the fix, Windows | enigo | | UIA `Select()`, synthetic input |
| Underline watcher, macOS | | | Accessibility API (`AXUIElement`), `AXIsProcessTrusted` |
| Underline watcher, Linux | | | AT-SPI2 over D-Bus (dlopen), GLib main context |
| Underline drawing, macOS and Linux | React (`SquiggleOverlay.tsx`, `SquigglePopup.tsx`) | | Tauri transparent always-on-top click-through window |
| Paste verification, learn-from-edits | `paste_verdict`, `corrected_phrases` are portable | | Windows only: UIA text read, `GetForegroundWindow` |
| Password guard | | | UIA `IsPassword` (Windows) |
| API keys | | | stored as plain text in `settings.json` (`tauri-plugin-store`) |

## 4. Mobile portability

| Feature | Verdict | Why |
|---|---|---|
| 1 AI modes (data, prompts, picker) | Carries over as-is | Plain TypeScript and JSON. Only the mode picker UI needs a phone layout. |
| 2 Mode editor and Test | Needs rework | Logic carries over, but the page is built for a wide desktop window. |
| 3 Local AI (llama-server) | Needs rework | A separate exe cannot run on iOS or Android. Needs llama.cpp linked as a library, or Core ML / NNAPI. A 1B model also costs real RAM and battery on a phone. |
| 4 Cloud AI providers | Carries over as-is | HTTPS calls from Rust or JS. Keys should move to the Keychain / Keystore instead of plain `settings.json`. |
| 5 Raw-text fallback and notice | Carries over as-is | Logic is portable. The notice window becomes an in-app toast. |
| 6 Output tidy-up | Carries over as-is | Pure string code. |
| 7 Per-app styles | Desktop-only | Phones have no reliable way to read the foreground app, and a keyboard or share sheet knows its host differently. Could become "per keyboard context" later. |
| 8 Fix grammar before pasting (CoEdIT) | Needs rework | ONNX model is 818 MB and loads `onnxruntime.dll`. Needs a mobile ONNX Runtime build and a smaller model. |
| 9 Inline proofreading in other apps | Desktop-only | Needs to read other apps' text fields and draw over them. iOS forbids it. Android would need an Accessibility Service plus overlay permission, which is a rewrite and a store-policy risk. A keyboard extension could underline only inside its own field. |
| 10 GECToR smarter grammar | Needs rework | Same ONNX loading issue as 8. 122 MB int8 is plausible on a phone. |
| 11 Rule toggles, repetition, code skip | Carries over as-is | All in `proofread.rs` (pure Rust). Harper compiles for mobile targets. |
| 12 Ignore lists | Desktop-only | Matches desktop process names. Not meaningful on mobile. |
| 13 Click-to-fix popup | Desktop-only | Native Win32 popup on Windows, floating window on macOS and Linux. A mobile version would be a different design (suggestion bar). |
| 14 Add to Dictionary and Dismiss | Needs rework | The vocabulary edit (`proofread://add-vocab`) carries over; the popup does not. |
| 15 Learn-from-edits | Desktop-only | Re-reads another app's field after paste. Could be redone inside the app's own editor. |
| 16 Popup theme and surface | Desktop-only | Settings for the desktop popup only. |
| 17 Paste verification | Desktop-only | Mobile inserts text through the keyboard or share API, which reports success itself. |
| 18 Password-field guard | Desktop-only | Mobile keyboards are told the field type by the OS. |
| 19 Vocabulary as personal dictionary | Carries over as-is | A string list merged into Harper. |
| 20 Sound-alike fixer | Carries over as-is | Pure Rust over Harper's dictionary. |
| 21 macOS accessibility permission | Desktop-only | Not applicable. |
| 22 WebView2 accessibility trick | Desktop-only | Windows-specific. |
| 23 Meeting summary via LLM | Carries over as-is | Same cloud client. Local path has the same limits as row 3. |
| 24 History squiggles / `proofread_text` | Carries over as-is | A pure function if it is ever used again. Nothing calls it today. |

## 5. Gaps: what the code does that the handbook misses or gets wrong

1. **Ollama.** HANDBOOK section 3 and section 12 list `llm/ollama.rs`, `ollama_status` and
   `ollama_generate`. None of them exist. `src-tauri/src/llm/` has only `llama.rs`, `openai.rs`
   and `mod.rs`. Ollama is reachable only as a Custom OpenAI-compatible provider
   (`http://localhost:11434/v1`).
2. **Providers are undocumented.** Section 10 only mentions `provider_id`. It does not describe the
   API Keys page, the preset list, per-mode `api_model`, model pricing lookups, the
   `uses: ["stt","llm"]` flag, or that keys sit in plain text in `settings.json` and are copied into
   Rust memory (also inside per-app profiles).
3. **Mode shape is out of date.** Section 10 omits `description`, `api_model`, `pinned`,
   `customized` and the built-in refresh rule. The `hotkey` field on a mode exists in the type but
   nothing uses it.
4. **LLM call details missing.** Delimited-text wrapper, temperature 0.3, frequency penalty 0.3 (not
   for Gemini), 120 s timeout, llama-server on port 8088, 4096 context, CPU unless `use_gpu`.
5. **GECToR, CoEdIT and the ONNX runtime are not in the handbook at all.** No mention of
   `gector.rs`, `coedit.rs`, `onnx.rs`, the two GECToR variants, sensitivity levels, the idle unload
   times (10 min and 3 min), or that CoEdIT is skipped when an LLM mode is active.
6. **Inline proofreading default is wrong.** Section 16 says the toggle defaults ON. The code
   default is OFF on every platform (`INLINE_PROOFREAD_DEFAULT = false` in `settingsStore.ts`).
7. **Watcher timing.** Section 16 says it polls every 400 ms. The code is event-driven with 60 ms
   (underlines showing), 250 ms, 800 ms and 3000 ms steps, wakes on UIA focus events, and lints
   only the visible part of the text.
8. **Popup look.** Section 16 says a white card with an orange border. The code has a dark default
   surface, a light option, and five accent palettes (`set_popup_theme`, `set_popup_surface`).
9. **macOS and Linux inline path is missing from the handbook** (it is in `PORTING.md` and
   `CLAUDE.md`): `inline_unix.rs`, `ax.rs`, `atspi.rs`, `squiggle_overlay.rs`, the
   `squiggle://set` / `squiggle://popup` events and the `inline_fix` command. These underlines are wavy
   (SVG), unlike the straight Windows lines. The popup has no Dismiss, no Add to Dictionary, no
   learn-from-edits and no theme.
10. **Learn-from-edits, paste verification and the notice card are not in section 16.** Also missing:
    the code/JSON skip (`looks_like_code`), the Filler lint, the built-in ignore list
    (`IGNORE_APP_SUBSTRINGS`) and the user ignore list `proofread_ignore_apps`.
11. **Per-app profiles are not Windows-only.** Section 16 says "Windows APIs". macOS uses `osascript`
    (app name) and Linux uses `xprop` WM_CLASS, which only works on X11. On Wayland the lookup
    returns nothing, so profiles silently never match.
12. **History squiggles** are described as shipped. They were removed from the UI; `proofread_text`
    now has no caller.
13. **Likely bug, not verified at runtime:** `onnx::ensure_runtime` only looks for
    `sherpa/onnxruntime.dll`. On macOS and Linux that file does not exist, so GECToR, CoEdIT and the
    VAD model probably never load there. Inline proofreading would then run on Harper alone.
14. **Local LLM never unloads.** "Free model after N minutes" (`model_unload_minutes`) only stops the
    speech model. `llama-server` stays running until the model changes or the app exits.
15. **Section 10 says `tidy_ai_output` lives in hotkey.rs.** True, but it also omits the second
    output path: the notice "Pasted without AI mode" shown when the AI step fails.
