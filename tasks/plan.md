# Meeting summary (Meeting Recorder, step 3): plan

## Overview
After a meeting, an AI writes the Overview: a short summary, key points, decisions and action items.
It runs on any provider the user already set up: a cloud provider (OpenAI-shaped API) or a local
model through the bundled llama-server. The user tests with a cloud key later; no large local model is run here.

## Decisions
- Provider is picked on the Notetaker page (cloud providers with the AI use, plus downloaded local models), remembered per device.
- Long meetings: map-reduce. Split into pieces (about 2,000 words for local, 4096 ctx; about 12,000 for cloud),
  write notes per piece, then one final pass. Short meetings: one call.
- Output is Markdown with fixed headings; stored on the transcript as `summary`, shown in Overview, included in Copy/exports.
- Manual "Summarize" button (and "Redo"); no auto-run, since the provider may be unset or cost money.
- A failure shows the error in the Overview; the transcript is never touched.

## Tasks
1. Rust: `summary` field, piece splitting + prompts (tested), `meeting_summarize` command.
2. UI: Overview shows the summary, provider picker, Summarize/Redo, busy and error states.
3. Exports include the summary.

## Risks
- Small local models invent action items: prompt says "only what was said; write None".
- Cost on cloud: one call per piece plus one; shown nowhere yet.
