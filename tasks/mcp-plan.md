# Implementation Plan: Record-only meetings + local MCP server

## Overview

A "Record only" choice in the Notetaker. The meeting is recorded (You and
Others tracks, as today), and no speech-to-text runs. Any MCP-capable AI agent
(Gemini CLI, Claude Code, Codex, Cursor) connects to a local MCP server built
into Silent Voice. The agent reads the meeting, gets its audio, and saves back
a transcript and a summary. The Notetaker shows that transcript like any other.
The app's own transcription stays the default.

Research: `rmcp` 3.11.0 (checked on crates.io 2026-11-24), MCP spec 2026-07-28.
Agents on this PC: Gemini CLI 0.64, Claude Code 2.1. agy's notes are in the
session scratchpad.

## Architecture decisions

- **No separate program: the app's own exe in a special mode, `silent-voice --mcp`.**
  The agent starts it over stdio when it needs it, and it exits when the agent
  closes the pipe. It costs nothing when no agent is using it.
  - `main.rs` checks for `--mcp` before Tauri, the single-instance plugin or a
    window start, so no webview loads: just a small server. Target: under
    15 MB of RAM and under 0.5 s to start, measured in Task 1.
  - Why not a separate `silent-voice-mcp` exe: no extra file to bundle and sign
    on 3 OSes, and it reuses `transcripts.rs` directly, with no copy of it in a
    second crate.
  - Fallback if Task 1 shows that the Windows GUI build can't use stdio: a
    small second binary in the same crate.
- **stdio only, no network port.** Nothing on the network can reach it. The
  agent owns the process.
- **Official SDK `rmcp` 3.11** (features `server`, `macros`, `transport-io`),
  with `schemars` 1.x for tool input/output schemas. Tools declare
  `outputSchema` and return `structuredContent`, so agents get typed data, not
  prose.
- **Tool annotations:** reads are `readOnlyHint`. `save_transcript` is
  `destructiveHint`, because it replaces lines, so agents ask before using it.
- **Audio goes to the agent in pieces.** The cloud model can't read a local
  path (agy, Gemini docs). `get_meeting_audio` returns one track, a time window
  of at most 5 minutes, as an MCP `audio` content block (16 kHz mono WAV, about
  9.6 MB), plus the full file paths for agents that can upload files with their
  own tools. A one-hour meeting is 12 calls per track.
- **Record-only = a new status `Recorded`** on the transcript, with no lines
  yet. The "Transcribe here" button reuses the existing `retranscribe`, so
  changing your mind costs nothing.
- **Safe writes:**
  - Saves use the existing temp-file + rename.
  - The previous version is kept as `{id}.json.prev`, so an agent's save can
    always be undone.
  - The server refuses to save while the app is transcribing that meeting
    (`status == Running`).
  - Every input is checked: the id is an existing meeting, times fall inside
    the recording, the speaker is `you`/`others`, and text and line counts
    have limits.
- **Who wrote it:** the transcript's `model_id` becomes `agent: <client name>`,
  taken from the MCP handshake (for example `agent: gemini-cli`). The page shows
  "Transcribed by Gemini CLI".
- **The app sees changes:** the MCP process is separate, so it can't send app
  events. The Notetaker reloads its list when the window gets focus. That is
  cheap and needs no file watcher.

## Task list

### Phase 1: Prove the risky parts first
- [ ] **Task 1: `--mcp` spike.** Do the release Windows build and the
  agents actually talk over stdio, and does Gemini CLI pass MCP audio to its
  model?
  - Build `--mcp` with one tool (`list_meetings`) and a test
    `get_meeting_audio` (a 10 s clip).
  - Register it in Gemini CLI and Claude Code.
  - Measure RAM and start time, and check the process exits when the agent quits.
  - Ask Gemini "what is said in this clip" to prove that audio reaches the model.

### Checkpoint A (stop and show you)
- [ ] Spike results. If audio does not reach the model, choose: paths only
  (agents upload the files themselves) or something else.

### Phase 2: Record only
- [ ] **Task 2: Record-only mode.**
  - A Notetaker choice: "Transcribe: on this device / Record only".
  - `Status::Recorded`, and `meeting_job` records with no STT.
  - The meeting card says "Waiting for an AI agent" and has a "Transcribe here"
    button.

### Phase 3: The MCP tools
- [ ] **Task 3: Read tools.**
  - `list_meetings` (newest first; filter by status; limit), `get_meeting`
    (details, lines, summary), and `get_meeting_audio` (5-minute pieces + paths).
  - Typed output, and unit tests on a temporary data folder.
- [ ] **Task 4: Write tools.**
  - `save_transcript` (replace or append lines) and `save_summary` (Markdown).
  - Checks, `.json.prev` backup, the Running guard, and `agent: <name>`.
  - Unit tests: bad id, a time past the end, a wrong speaker, a save during
    Running, and the backup restoring.
- [ ] **Task 5: The `transcribe_meeting` prompt + server instructions.** Step
  by step for the agent: get pieces in order, label You/Others, number the
  Others, save, then summarise.

### Checkpoint B
- [ ] `cargo test` and clippy are clean. A full test with Gemini CLI on a real
  2-minute recorded meeting: transcript and summary show up in the Notetaker.

### Phase 4: In the app
- [ ] **Task 6: The Notetaker** reloads on focus, shows "Transcribed by
  <agent>", and has Undo when a `.json.prev` exists.
- [ ] **Task 7: Settings → "Connect an AI agent".** The exe path, plus
  ready-to-copy setup for Claude Code (`claude mcp add`), Gemini CLI
  (`settings.json`), Codex (`config.toml`) and Cursor (`mcp.json`). Says which
  agents can listen to audio.
- [ ] **Task 8: Docs and cross-platform.**
  - An ADR (0003: MCP over stdio in the main exe) and the feature doc.
  - Update `docs/FUTURE.md`, `CLAUDE.md` and HANDBOOK.
  - Check that the Mac and Linux CI builds run `--mcp` (a smoke test that
    sends `initialize` + `tools/list`).

### Checkpoint C: complete
- [ ] Every acceptance check passes and the dev app is tested by you, then a
  version bump only when you say so.

## Risks and mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| A GUI-subsystem exe can't use the agent's stdio pipes on Windows | High | Task 1 tests the release build first. The fallback is a second small binary |
| Gemini CLI doesn't pass MCP audio blocks to the model | High | Task 1 tests it. The fallback is paths + the agent's own upload, documented per agent |
| Claude can't hear audio | Med | Settings and the prompt say so. A Claude agent needs its own speech-to-text tool |
| The app and the MCP process write the same meeting at once | Med | The Running guard, atomic rename, and the app reloads from disk before it edits |
| A big WAV makes MCP messages heavy | Med | 5-minute pieces (about 9.6 MB). Opus/FLAC encoding later, only if needed |
| rmcp breaks its API again (v2 → v3 within 2026) | Low | Pin `rmcp = "3.11"`. All MCP code lives in one `mcp.rs` |

## Testing the dev app

This work is on branch `mcp` in the worktree `D:\Vibe-coding\sv-mac-install`,
so the main checkout (branch `mobile`) is untouched. For your live testing the
dev app has to run this branch: switch the main checkout to `mcp` at
Checkpoint B.
