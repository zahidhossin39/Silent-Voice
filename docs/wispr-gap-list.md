# Silent Voice vs Wispr Flow: gap list

Source: `D:\Vibe-coding\REA AppS\Wispr Flow\docs` (Wispr Flow 1.6.1147 teardown) compared with Silent Voice's code on 2026-10-14.
Work through it top to bottom, one item per branch. Size: S = hours, M = a day or two, L = bigger.

**Status (2026-10-15, branch `experiment`):** done #1, #2 (hotkey changeable), #3 (only "press enter" kept, cancel phrases dropped), #4 (sound packs, Chorus default), #10, #11 (background mode + 7 MB punctuation repair, on by default for English), #12, #13, #14. Skipped #9. **Deferred for later: #5, #6** (only matter with AI writing styles, which the user doesn't use yet).

## A. Missing: worth building (local-only, keeps the app simple)

| # | Feature | What Wispr does | Our version | Size |
|---|---|---|---|---|
| 1 | **Esc cancels a dictation** | `dismiss` shortcut throws the recording away | Esc while recording → drop audio, pill back to idle, nothing pasted | S |
| 2 | **Paste last / copy last** | Shift+Alt+Z / Shift+Alt+X recover a paste that landed in the wrong place | Two hotkeys + tray items that reuse the newest history entry | S |
| 3 | **"Scratch that" / "press enter"** | Spoken "scratch that", "never mind", "cancel that" cancel the dictation; "press enter" sends Enter | Detect at the end of the transcript in `textfmt`; cancel or press Enter after the paste | S |
| 4 | **Start/stop sounds** | Optional sounds on start, stop, and paste | Two short bundled sounds, one toggle (default off) | S |
| 5 | **Context-aware AI cleanup** | Sends the app name/type, ~500 chars before the cursor, the selected text and the dictionary to the formatter | We already read the focused field via UIA (`inline_check`). Add app name + text before the cursor + vocabulary to the AI-mode prompt | M |
| 6 | **Command mode** | Select text, hold a key, say "make it shorter" or "reply to this"; the LLM rewrites the selection | Separate hotkey: copy the selection, speak an instruction, local/API LLM rewrites, paste over it | M |
| 7 | **Learn words from your edits** | After pasting, watches the field; a word you corrected becomes a dictionary suggestion | Re-read the field ~20 s after the paste via UIA and diff it. Changed proper nouns appear as "Add to vocabulary?" (always the user's click, per the existing product rule) | M |
| 8 | **Per-app styles out of the box** | Built-in map of hundreds of apps → email / work chat / personal chat / AI prompt / code, with a tone per category | Ship a small default app→category map so profiles work without typing exe names | M |
| 9 | **Pause media while dictating** | `shouldPauseAudio` / `shouldMuteAudio` | Send the media play/pause key on start and stop (only if something is playing) | S–M |

## B. Exists but works less well than Wispr

| # | Area | Problem in our code | Fix | Size |
|---|---|---|---|---|
| 10 | **Overlapping dictations** (suspected bug, verify first) | `finalize_recording` frees the recorder slot before processing, so a new recording can start while the last is still transcribing. Both pipelines write the same `last.wav` and can paste out of order. Wispr uses a start/stop mutex and a queue | One pipeline at a time: a per-clip WAV name, plus a queue or "still processing" notice | S |
| 11 | **Release-to-paste latency** | Wispr: ~720 ms median, flat for any clip length (streams while you talk). Ours transcribes everything after release; background chunking only runs with `chunk_on_silence` + the VAD model + local STT | Measure with the existing `pipeline total` log line, then consider chunking on by default once VAD is installed | M |
| 12 | **Cold model at key-down** | After `model_unload_minutes` the first dictation pays the model load time. Wispr warms its backend on key-down | Start loading the model on key-down, not on release | S |
| 13 | **Clipboard restore loses images/files** | `paste.rs` saves and restores text only, so a copied image or file is wiped by every dictation | Save and restore the other clipboard formats too (or skip the restore when the original wasn't text) | S–M |
| 14 | **Focus change → clipboard only** | If the window changed during processing we only copy. Wispr refocuses the stored window and still pastes | Try `SetForegroundWindow(target)` first; fall back to copy if it fails | S |
| 15 | **No paste verification** | We trust Ctrl+V. Wispr re-reads the field and reports "copied instead" on failure | Re-read the focused field via UIA after the paste; if the text isn't there, keep it on the clipboard and say so | M |

## C. Bigger, optional (only if you want them later)

- **Scratchpad:** a small floating notes window you dictate into.
- **Polish hotkeys:** select text → hotkey → saved prompt 1–9 rewrites it (overlaps with #6).
- **Insights:** words per minute vs typing, time saved (we already have the heatmap).
- **Meeting recorder:** mic + system audio, transcript and summary after the call. Large.

## Deliberately skipped

Cloud ASR ensemble/streaming servers, accounts, sync, billing, teams, telemetry, MCP server, auto intent classifier (adds latency to every dictation), screen OCR. These are cloud-product features that work against "free, local, simple".

## Where Silent Voice is already ahead

Fully offline and free; several local STT engines; system-wide inline proofreading; read-aloud TTS; Model Store; and Linux/Wayland CLI control (Wispr's deep links do the same job).
