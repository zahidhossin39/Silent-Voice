---
status: accepted (2026-11-24; built on branch experiment)
---

# The local MCP server: the app's own exe over stdio, audio handed over as files

"Record only" meetings are transcribed by an AI agent the user already runs
(Antigravity/agy, Claude Code, Codex, Cursor...). The agent reaches the meetings
through an MCP server built into the app: **`silent-voice --mcp`**, started by
the agent over **stdio** and gone when the agent closes the pipe. `main()` checks
for the flag before Tauri, the single-instance plugin or any window, so the
process is only the server: about 18 MB of memory (debug build), and it exits
4 ms after stdin closes. The code is `src-tauri/src/mcp.rs` on the official Rust
SDK (`rmcp` 3.11). It reads and writes the same JSON and WAV files as the app
(`transcripts.rs`), so the app does not need to be running.

## Considered options

- **A second, small exe (`silent-voice-mcp`).** It is what we first planned. It
  would need bundling as a Tauri sidecar and signing on three OSes, and the
  transcript code would have to move to a shared crate. The main exe in a
  special mode reuses `transcripts.rs` directly. A test showed that a Windows
  GUI-subsystem exe reads and writes the pipes its parent gives it, which was
  the one risk.
- **Streamable HTTP on localhost inside the running app.** It is always on
  while the app runs, opens a port that any local process can reach, and needs
  the app open. stdio has none of that: the agent owns the process.
- **Audio as MCP `audio` content blocks.** Tested with agy: it drops audio blocks
  and passes only text to its model, and Claude models take no audio at all.
  agy **can** listen to a local WAV with its own file tool, and transcribed real
  Bangla/English correctly. So `get_meeting_audio` writes each piece (at most
  5 minutes, 16 kHz mono) to `transcripts/agent-pieces/` and returns its path.
  `inline: true` still attaches the block, for agents that pass it on.
- **Gemini CLI as the reference agent.** It now refuses individual accounts
  ("migrate to Antigravity"), so agy is the one we test with.

## Consequences

- Agents write with `save_transcript` and `save_summary`. Every input is
  checked:
  - the id is a meeting;
  - the speaker is you/others;
  - person is 1–50, for others only;
  - times fall inside the recording;
  - text and line counts have limits.
- A meeting being recorded or transcribed (`Running`) is refused.
- The first agent save keeps the meeting as it was in `{id}.json.prev`; the
  Notetaker's Undo puts it back. `model_id` becomes `agent: <client name>` from
  the MCP handshake.
- The MCP process can't send app events, so the Notetaker re-reads meetings
  when its window regains focus.
- Pieces older than a day are deleted when the server starts.
- **Windows locks a running exe:** while an agent has the server open, the
  exe can't be replaced. The dev build fails to rebuild ("Access is denied"),
  and the updater's installer has to close the server process like the app
  itself. The agent then reconnects on its next call.
- Not every agent can listen. Settings → Connect an AI agent says which ones
  can, and suggests a speech-to-text tool for the rest.
