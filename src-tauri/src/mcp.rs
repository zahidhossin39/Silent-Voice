// Local MCP server: `silent-voice --mcp`. An AI agent (agy, Claude Code, Codex,
// Cursor...) starts it over stdio when it needs it; it exits when the agent
// closes the pipe. No window, no Tauri, no network port: it only reads and
// writes the meeting files in the data folder (transcripts.rs).
// Decision record: docs/adr/0003-mcp-server-over-stdio.md.

use std::path::{Path, PathBuf};

use crate::transcripts::{self, Kind, Line, Speaker, Status, Transcript};
use base64::Engine;
use rmcp::{
    handler::server::{router::prompt::PromptRouter, router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, PromptMessage, Role, ServerCapabilities, ServerConfig},
    prompt, prompt_handler, prompt_router, schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, Json, RoleServer, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};

/// The longest piece of audio one call returns: 5 min of 16 kHz mono 16-bit
/// WAV is ~9.6 MB, small enough for an agent to open or upload at once.
const MAX_PIECE_MS: u64 = 5 * 60_000;
/// Limits on what an agent may save (an hour-long meeting is ~700 lines).
const MAX_LINES: usize = 10_000;
const MAX_LINE_CHARS: usize = 4_000;
const MAX_SUMMARY_CHARS: usize = 50_000;
const MAX_PEOPLE: u32 = 50;
/// A line may end a little past the recorded length (rounding in the agent's timing).
const END_SLACK_MS: u64 = 5_000;
/// What a saved transcript's `model_id` starts with when an agent wrote it.
pub const AGENT_PREFIX: &str = "agent: ";

/// Runs the server until the agent disconnects. Called from main() before any
/// Tauri setup. stdout belongs to the protocol: nothing else may print there.
pub fn serve() {
    let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("silent-voice --mcp: could not start: {e}");
            std::process::exit(1);
        }
    };
    let server = Meetings::new(transcripts::dir());
    sweep_old_pieces(&server.pieces_dir());
    let code = rt.block_on(async {
        match server.serve(rmcp::transport::stdio()).await {
            Ok(service) => {
                let _ = service.waiting().await;
                0
            }
            Err(e) => {
                eprintln!("silent-voice --mcp: handshake failed: {e}");
                1
            }
        }
    });
    // A pending stdin read cannot be cancelled, so a normal runtime shutdown
    // could hang after the agent leaves. Exit right away instead.
    std::process::exit(code);
}

#[derive(Clone)]
pub struct Meetings {
    /// The transcripts folder; a temp folder in tests.
    dir: PathBuf,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

// ── Inputs and outputs (their JSON schemas are what the agent sees) ──

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListMeetingsArgs {
    /// Only meetings with this status: "recorded" (audio only, waiting to be
    /// transcribed), "done", "running", "stopped" or "failed".
    #[serde(default)]
    pub status: Option<String>,
    /// How many to return, newest first (default 20, max 100).
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MeetingInfo {
    /// Pass this to the other tools.
    pub id: i64,
    /// Empty until the user names it.
    pub name: String,
    /// Start time, Unix ms.
    pub created: i64,
    pub duration_ms: Option<u64>,
    /// "recorded" means audio only, waiting to be transcribed.
    pub status: String,
    /// What wrote the transcript: "agent: <name>", a speech model id, or "record only".
    pub transcribed_by: String,
    pub has_summary: bool,
    pub lines: usize,
    /// How many other people the user said were on the call, if they said.
    pub others: Option<u32>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MeetingList {
    pub meetings: Vec<MeetingInfo>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct IdArgs {
    /// The meeting id from list_meetings.
    pub id: i64,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct LineData {
    /// From the start of the meeting.
    pub start_ms: u64,
    pub end_ms: u64,
    /// "you" (the user, from the microphone track) or "others" (everyone else,
    /// from the computer's sound track).
    pub speaker: String,
    /// For "others" only: which person (1, 2, ...), the same number for the same
    /// voice throughout. Leave it out when unsure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<u32>,
    pub text: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MeetingDetail {
    #[serde(flatten)]
    pub info: MeetingInfo,
    /// The transcript, in time order.
    pub transcript: Vec<LineData>,
    /// The Overview (Markdown), if one was written.
    pub summary: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AudioArgs {
    /// The meeting id from list_meetings.
    pub id: i64,
    /// "you" = the user's microphone, "others" = the computer's sound (everyone
    /// else on the call).
    pub track: String,
    /// Where the piece starts, in ms from the start of the meeting (default 0).
    #[serde(default)]
    pub start_ms: u64,
    /// Piece length in ms (default and max 300000 = 5 minutes).
    #[serde(default)]
    pub length_ms: Option<u64>,
    /// Also attach the piece as an MCP audio block. Most agents drop audio
    /// blocks, so the default is off: open `piece_file` with your own file tool.
    #[serde(default)]
    pub inline: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AudioPiece {
    pub id: i64,
    pub track: String,
    pub start_ms: u64,
    pub end_ms: u64,
    /// The whole meeting's length.
    pub duration_ms: u64,
    /// A WAV of just this piece (16 kHz mono). Open or upload it to listen.
    /// Pieces are deleted after a day.
    pub piece_file: String,
    /// Where the next piece starts; absent when this piece reaches the end.
    pub next_start_ms: Option<u64>,
    /// The full recordings, for agents that can upload a long file at once.
    pub file_you: String,
    pub file_others: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveTranscriptArgs {
    /// The meeting id from list_meetings.
    pub id: i64,
    /// The lines to save. Times are from the start of the meeting, not of a piece.
    pub lines: Vec<LineData>,
    /// "replace" (default): these lines become the whole transcript.
    /// "append": add them to what is saved, for saving piece by piece.
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema, Debug)]
pub struct Saved {
    pub id: i64,
    /// Lines in the transcript after this save.
    pub lines: usize,
    /// The user can undo the agent's work in the app.
    pub undo_available: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveSummaryArgs {
    /// The meeting id from list_meetings.
    pub id: i64,
    /// The Overview in Markdown: "## " headings and "- " bullets (key points,
    /// decisions, action items with owners). Replaces any earlier Overview.
    pub markdown: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TranscribePromptArgs {
    /// A meeting id; leave empty for the newest meeting waiting to be transcribed.
    #[serde(default)]
    pub id: Option<String>,
}

// ── Helpers ──

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Recorded => "recorded",
        Status::Running => "running",
        Status::Done => "done",
        Status::Stopped => "stopped",
        Status::Failed => "failed",
    }
}

fn info(t: &Transcript, lines: usize) -> MeetingInfo {
    MeetingInfo {
        id: t.id,
        name: t.name.clone(),
        created: t.created,
        duration_ms: t.duration_ms,
        status: status_name(t.status).into(),
        transcribed_by: t.model_id.clone(),
        has_summary: t.summary.is_some(),
        lines,
        others: t.others,
    }
}

fn parse_speaker(s: &str) -> Result<Speaker, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "you" => Ok(Speaker::You),
        "others" => Ok(Speaker::Others),
        other => Err(format!("speaker/track must be \"you\" or \"others\", not \"{other}\"")),
    }
}

fn speaker_name(s: Speaker) -> &'static str {
    if s == Speaker::You {
        "you"
    } else {
        "others"
    }
}

/// Checks one line from an agent and turns it into a stored line. `n` is its
/// position, so the message points at the bad one.
fn to_line(n: usize, l: &LineData, duration_ms: Option<u64>) -> Result<Line, String> {
    let speaker = parse_speaker(&l.speaker).map_err(|e| format!("line {n}: {e}"))?;
    let text = l.text.trim();
    if text.is_empty() {
        return Err(format!("line {n}: text is empty"));
    }
    if text.chars().count() > MAX_LINE_CHARS {
        return Err(format!("line {n}: text is longer than {MAX_LINE_CHARS} characters; split it"));
    }
    if l.end_ms < l.start_ms {
        return Err(format!("line {n}: end_ms {} is before start_ms {}", l.end_ms, l.start_ms));
    }
    if let Some(total) = duration_ms.filter(|d| *d > 0) {
        if l.end_ms > total + END_SLACK_MS {
            return Err(format!(
                "line {n}: end_ms {} is past the end of the meeting ({total} ms). Times are from the start of the meeting, not of a piece.",
                l.end_ms
            ));
        }
    }
    let who = match (speaker, l.person) {
        (_, None) => None,
        (Speaker::You, Some(_)) => return Err(format!("line {n}: person is only for \"others\" lines")),
        (Speaker::Others, Some(p)) if (1..=MAX_PEOPLE).contains(&p) => Some(p),
        (Speaker::Others, Some(p)) => return Err(format!("line {n}: person must be 1 to {MAX_PEOPLE}, not {p}")),
    };
    Ok(Line { start_ms: l.start_ms, end_ms: l.end_ms, text: text.to_string(), speaker: Some(speaker), who })
}

/// The agent's name from the MCP handshake ("agy", "claude-code"...).
fn client_name(ctx: &RequestContext<RoleServer>) -> String {
    ctx.peer
        .peer_info()
        .map(|p| p.client_info.name.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "AI agent".into())
}

/// Pieces older than a day are leftovers from earlier agent sessions.
fn sweep_old_pieces(dir: &Path) {
    let day = std::time::Duration::from_secs(24 * 3600);
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > day);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

impl Meetings {
    /// Where audio pieces for agents go: inside the data folder, next to the
    /// meetings, never a shared temp folder.
    fn pieces_dir(&self) -> PathBuf {
        self.dir.join("agent-pieces")
    }

    /// A recorded meeting by id, or a message the agent can act on.
    fn meeting(&self, id: i64) -> Result<Transcript, String> {
        let t = transcripts::load_in(&self.dir, id).map_err(|_| format!("No meeting with id {id}. Call list_meetings for valid ids."))?;
        if t.kind != Kind::Meeting {
            return Err(format!("{id} is a transcribed file, not a recorded meeting."));
        }
        Ok(t)
    }

    /// A meeting the agent may write to: not being recorded or transcribed now.
    fn writable(&self, id: i64) -> Result<Transcript, String> {
        let t = self.meeting(id)?;
        if t.status == Status::Running {
            return Err(format!("Meeting {id} is being recorded or transcribed in Silent Voice right now. Try again when it has finished."));
        }
        Ok(t)
    }

    /// Before an agent's first change, keep the old version so the user can undo it.
    fn keep_undo(&self, t: &Transcript) -> Result<(), String> {
        let prev = transcripts::prev_in(&self.dir, t.id);
        if t.model_id.starts_with(AGENT_PREFIX) && prev.exists() {
            return Ok(()); // already kept from before the agent started
        }
        std::fs::write(&prev, serde_json::to_vec(t).map_err(|e| e.to_string())?).map_err(|e| format!("Could not keep a copy for undo: {e}"))
    }

    fn save_lines(&self, args: SaveTranscriptArgs, agent: &str) -> Result<Saved, String> {
        let mut t = self.writable(args.id)?;
        let append = match args.mode.as_deref().map(|m| m.trim().to_ascii_lowercase()) {
            None => false,
            Some(m) if m.is_empty() || m == "replace" => false,
            Some(m) if m == "append" => true,
            Some(m) => return Err(format!("mode must be \"replace\" or \"append\", not \"{m}\"")),
        };
        if args.lines.is_empty() && append {
            return Err("No lines given.".into());
        }
        let mut lines = args.lines.iter().enumerate().map(|(n, l)| to_line(n, l, t.duration_ms)).collect::<Result<Vec<_>, _>>()?;
        let kept = if append { t.lines.len() } else { 0 };
        if kept + lines.len() > MAX_LINES {
            return Err(format!("A transcript can hold at most {MAX_LINES} lines."));
        }
        self.keep_undo(&t)?;
        if append {
            let mut all = std::mem::take(&mut t.lines);
            all.append(&mut lines);
            lines = all;
        }
        lines.sort_by_key(|l| (l.start_ms, l.speaker.map(|s| s as u8)));
        t.lines = lines;
        t.model_id = format!("{AGENT_PREFIX}{agent}");
        t.status = Status::Done;
        t.error = None;
        t.redo = false;
        t.done_ms = t.duration_ms.unwrap_or(t.done_ms);
        transcripts::save_in(&self.dir, &t)?;
        Ok(Saved { id: t.id, lines: t.lines.len(), undo_available: transcripts::prev_in(&self.dir, t.id).exists() })
    }

    fn save_overview(&self, args: SaveSummaryArgs) -> Result<Saved, String> {
        let mut t = self.writable(args.id)?;
        let md = args.markdown.trim();
        if md.is_empty() {
            return Err("markdown is empty.".into());
        }
        if md.chars().count() > MAX_SUMMARY_CHARS {
            return Err(format!("The Overview is longer than {MAX_SUMMARY_CHARS} characters."));
        }
        t.summary = Some(md.to_string());
        transcripts::save_in(&self.dir, &t)?;
        Ok(Saved { id: t.id, lines: t.lines.len(), undo_available: transcripts::prev_in(&self.dir, t.id).exists() })
    }
}

#[tool_router]
impl Meetings {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, tool_router: Self::tool_router(), prompt_router: Self::prompt_router() }
    }

    #[tool(
        description = "List recorded meetings, newest first. Status \"recorded\" means audio only, waiting to be transcribed: use get_meeting_audio and save_transcript.",
        annotations(title = "List meetings", read_only_hint = true, open_world_hint = false)
    )]
    pub async fn list_meetings(&self, Parameters(args): Parameters<ListMeetingsArgs>) -> Result<Json<MeetingList>, String> {
        let limit = args.limit.unwrap_or(20).clamp(1, 100);
        let want = args.status.map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty());
        let meetings = transcripts::all_in(&self.dir)
            .into_iter()
            .filter(|t| t.kind == Kind::Meeting)
            .filter(|t| want.as_deref().is_none_or(|w| status_name(t.status) == w))
            .take(limit)
            .map(|t| info(&t, t.lines.len()))
            .collect();
        Ok(Json(MeetingList { meetings }))
    }

    #[tool(
        description = "Get one meeting: its details, the transcript so far (time, speaker, person, text) and the Overview.",
        annotations(title = "Get meeting", read_only_hint = true, open_world_hint = false)
    )]
    pub async fn get_meeting(&self, Parameters(args): Parameters<IdArgs>) -> Result<Json<MeetingDetail>, String> {
        let t = self.meeting(args.id)?;
        let transcript = t
            .lines
            .iter()
            .map(|l| LineData {
                start_ms: l.start_ms,
                end_ms: l.end_ms,
                speaker: speaker_name(l.speaker.unwrap_or(Speaker::Others)).into(),
                person: l.who,
                text: l.text.clone(),
            })
            .collect();
        Ok(Json(MeetingDetail { info: info(&t, t.lines.len()), transcript, summary: t.summary.clone() }))
    }

    #[tool(
        description = "Get a piece (at most 5 minutes) of one track of a meeting's audio, saved as a 16 kHz mono WAV file. Open `piece_file` with your own file tool to listen to it. Call again from `next_start_ms` until it is absent to cover the whole meeting. Track \"you\" is the user's microphone; \"others\" is everyone else on the call.",
        annotations(title = "Get meeting audio", read_only_hint = true, open_world_hint = false)
    )]
    pub async fn get_meeting_audio(&self, Parameters(args): Parameters<AudioArgs>) -> Result<CallToolResult, String> {
        let t = self.meeting(args.id)?;
        let track = parse_speaker(&args.track)?;
        let track_name = speaker_name(track);
        if !transcripts::audio_path_in(&self.dir, t.id, track).exists() {
            return Err(format!("Meeting {} has no saved {track_name} audio.", t.id));
        }
        let total = t.duration_ms.unwrap_or(0);
        if total > 0 && args.start_ms >= total {
            return Err(format!("start_ms {} is past the end of the meeting ({total} ms).", args.start_ms));
        }
        let length = args.length_ms.unwrap_or(MAX_PIECE_MS).clamp(1_000, MAX_PIECE_MS);
        let end = if total > 0 { (args.start_ms + length).min(total) } else { args.start_ms + length };
        let wav = transcripts::audio_clip_in(&self.dir, t.id, Some(track), args.start_ms, end)?;

        let dir = self.pieces_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Could not save the audio piece: {e}"))?;
        let file = dir.join(format!("{}-{track_name}-{}-{end}.wav", t.id, args.start_ms));
        std::fs::write(&file, &wav).map_err(|e| format!("Could not save the audio piece: {e}"))?;

        let path = |s| transcripts::audio_path_in(&self.dir, t.id, s).to_string_lossy().into_owned();
        let piece = AudioPiece {
            id: t.id,
            track: track_name.into(),
            start_ms: args.start_ms,
            end_ms: end,
            duration_ms: total,
            piece_file: file.to_string_lossy().into_owned(),
            next_start_ms: (total > 0 && end < total).then_some(end),
            file_you: path(Speaker::You),
            file_others: path(Speaker::Others),
        };
        let mut result = CallToolResult::structured(serde_json::to_value(&piece).map_err(|e| e.to_string())?);
        if args.inline {
            result.content.push(ContentBlock::audio(base64::engine::general_purpose::STANDARD.encode(&wav), "audio/wav"));
        }
        Ok(result)
    }

    #[tool(
        description = "Save a meeting's transcript. mode \"replace\" (default) makes these lines the whole transcript; \"append\" adds them, for saving piece by piece. Times are from the start of the meeting. The user can undo the agent's changes in Silent Voice.",
        annotations(title = "Save transcript", read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    pub async fn save_transcript(&self, Parameters(args): Parameters<SaveTranscriptArgs>, ctx: RequestContext<RoleServer>) -> Result<Json<Saved>, String> {
        self.save_lines(args, &client_name(&ctx)).map(Json)
    }

    #[tool(
        description = "Save a meeting's Overview (Markdown summary: key points, decisions, action items). Replaces the earlier Overview.",
        annotations(title = "Save summary", read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn save_summary(&self, Parameters(args): Parameters<SaveSummaryArgs>) -> Result<Json<Saved>, String> {
        self.save_overview(args).map(Json)
    }
}

#[prompt_router]
impl Meetings {
    #[prompt(name = "transcribe_meeting", description = "Step by step: transcribe a recorded meeting, label who spoke, and write its Overview.")]
    pub async fn transcribe_meeting(&self, Parameters(args): Parameters<TranscribePromptArgs>) -> Vec<PromptMessage> {
        let which = match args.id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(id) => format!("meeting {id}"),
            None => "the newest meeting with status \"recorded\" (list_meetings with status \"recorded\")".into(),
        };
        vec![PromptMessage::new_text(Role::User, transcribe_steps(&which))]
    }
}

fn transcribe_steps(which: &str) -> String {
    format!(
        "Transcribe {which} from Silent Voice.\n\n\
         1. Call get_meeting for its duration_ms and how many others were on the call (`others`, if known).\n\
         2. For each track, \"you\" then \"others\": call get_meeting_audio from start_ms 0, open the returned piece_file \
         with your own file tool and listen, then continue from next_start_ms until it is absent.\n\
         3. Write lines of one speaker turn each (a sentence or a few), word for word in the language spoken, \
         keeping mixed languages as spoken. Times are from the start of the meeting (piece start_ms + offset in the piece).\n\
         4. \"you\" lines come from the you track. On the others track, number each distinct voice as person 1, 2, ... \
         consistently. The microphone can also pick up the others from the speakers: leave out a you line that repeats \
         an others line at the same time.\n\
         5. Save with save_transcript (mode \"append\" after each piece is fine).\n\
         6. Then write a short Overview in Markdown (## Summary, ## Decisions, ## Action items with owners) in the \
         meeting's main language, and save it with save_summary.\n\
         7. Tell the user it is done; they can undo your changes in Silent Voice."
    )
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for Meetings {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_prompts().build())
            .with_server_info(Implementation::new("silent-voice", env!("CARGO_PKG_VERSION")).with_title("Silent Voice"))
            .with_instructions(
                "Silent Voice meeting recordings on this computer. Each meeting has two audio tracks: \
                 \"you\" (the user's microphone) and \"others\" (everyone else, from the computer's sound). \
                 Meetings with status \"recorded\" are waiting to be transcribed; the transcribe_meeting prompt \
                 has the steps. Audio pieces come back as WAV files: open them with your own file tool.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp transcripts folder holding one meeting of `secs` seconds with
    /// silent 16 kHz WAVs for both tracks.
    fn folder(name: &str, secs: u32, status: Status) -> (PathBuf, Meetings, i64) {
        let dir = std::env::temp_dir().join(format!("sv-mcp-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = 1_700_000_000_000;
        let mut t = Transcript::new(id, Kind::Meeting, String::new(), String::new(), "record only".into());
        t.status = status;
        t.duration_ms = Some(secs as u64 * 1000);
        transcripts::save_in(&dir, &t).unwrap();
        let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        for s in [Speaker::You, Speaker::Others] {
            let mut w = hound::WavWriter::create(transcripts::audio_path_in(&dir, id, s), spec).unwrap();
            for _ in 0..secs * 16_000 {
                w.write_sample(0i16).unwrap();
            }
            w.finalize().unwrap();
        }
        (dir.clone(), Meetings::new(dir), id)
    }

    fn line(start_ms: u64, speaker: &str, person: Option<u32>, text: &str) -> LineData {
        LineData { start_ms, end_ms: start_ms + 1_000, speaker: speaker.into(), person, text: text.into() }
    }

    fn save(m: &Meetings, id: i64, lines: Vec<LineData>, mode: Option<&str>) -> Result<Saved, String> {
        m.save_lines(SaveTranscriptArgs { id, lines, mode: mode.map(Into::into) }, "agy")
    }

    #[tokio::test]
    async fn a_recorded_meeting_is_listed_and_filtered_by_status() {
        let (_d, m, id) = folder("list", 2, Status::Recorded);
        let all = m.list_meetings(Parameters(ListMeetingsArgs { status: None, limit: None })).await.unwrap().0;
        assert_eq!(all.meetings.len(), 1);
        assert_eq!((all.meetings[0].id, all.meetings[0].status.as_str()), (id, "recorded"));
        let done = m.list_meetings(Parameters(ListMeetingsArgs { status: Some("done".into()), limit: None })).await.unwrap().0;
        assert!(done.meetings.is_empty());
    }

    #[tokio::test]
    async fn audio_comes_in_pieces_that_end_at_the_meeting_end() {
        let (_d, m, id) = folder("audio", 7, Status::Recorded);
        let get = |start_ms| m.get_meeting_audio(Parameters(AudioArgs { id, track: "others".into(), start_ms, length_ms: Some(5_000), inline: false }));
        let first = get(0).await.unwrap().structured_content.unwrap();
        assert_eq!((first["end_ms"].as_u64(), first["next_start_ms"].as_u64()), (Some(5_000), Some(5_000)));
        let wav = std::fs::read(first["piece_file"].as_str().unwrap()).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 44 + 5 * 16_000 * 2, "5 s of 16 kHz mono 16-bit");
        let last = get(5_000).await.unwrap().structured_content.unwrap();
        assert_eq!((last["end_ms"].as_u64(), last["next_start_ms"].as_u64()), (Some(7_000), None));
        assert!(get(7_000).await.is_err(), "starting at the end is refused");
        let bad = m.get_meeting_audio(Parameters(AudioArgs { id, track: "mic".into(), start_ms: 0, length_ms: None, inline: false })).await;
        assert!(bad.unwrap_err().contains("\"you\" or \"others\""));
    }

    #[test]
    fn saving_marks_the_meeting_done_by_the_agent_sorted_and_undoable() {
        let (dir, m, id) = folder("save", 60, Status::Recorded);
        save(&m, id, vec![line(5_000, "others", Some(1), "Hello there."), line(1_000, "you", None, " Hi. ")], None).unwrap();
        let t = transcripts::load_in(&dir, id).unwrap();
        assert_eq!(t.status, Status::Done);
        assert_eq!(t.model_id, "agent: agy");
        assert_eq!(t.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Hi.", "Hello there."], "sorted by time, trimmed");
        assert_eq!(t.lines[1].who, Some(1));
        let before: Transcript = serde_json::from_slice(&std::fs::read(transcripts::prev_in(&dir, id)).unwrap()).unwrap();
        assert_eq!(before.status, Status::Recorded, "the undo copy is the meeting before the agent");
    }

    #[test]
    fn appending_piece_by_piece_keeps_the_first_undo_copy() {
        let (dir, m, id) = folder("append", 60, Status::Recorded);
        save(&m, id, vec![line(1_000, "you", None, "One.")], Some("append")).unwrap();
        save(&m, id, vec![line(2_000, "others", None, "Two.")], Some("append")).unwrap();
        assert_eq!(transcripts::load_in(&dir, id).unwrap().lines.len(), 2);
        let before: Transcript = serde_json::from_slice(&std::fs::read(transcripts::prev_in(&dir, id)).unwrap()).unwrap();
        assert!(before.lines.is_empty() && before.status == Status::Recorded, "undo goes back to before the agent, not to its first piece");
    }

    #[test]
    fn bad_lines_are_refused_and_nothing_is_written() {
        let (dir, m, id) = folder("bad", 10, Status::Recorded);
        let cases = [
            (line(0, "me", None, "x"), "\"you\" or \"others\""),
            (line(0, "you", None, "   "), "empty"),
            (line(0, "you", Some(1), "x"), "only for \"others\""),
            (line(0, "others", Some(0), "x"), "1 to 50"),
            (line(60_000, "others", None, "x"), "past the end"),
            (LineData { start_ms: 5_000, end_ms: 4_000, speaker: "you".into(), person: None, text: "x".into() }, "before start_ms"),
        ];
        for (l, want) in cases {
            let err = save(&m, id, vec![line(0, "you", None, "fine"), l], None).unwrap_err();
            assert!(err.starts_with("line 1:") && err.contains(want), "{err}");
        }
        assert!(save(&m, id, vec![], Some("sideways")).unwrap_err().contains("\"replace\" or \"append\""));
        assert_eq!(transcripts::load_in(&dir, id).unwrap().status, Status::Recorded, "untouched");
        assert!(!transcripts::prev_in(&dir, id).exists(), "no undo copy for a refused save");
    }

    #[test]
    fn a_meeting_in_progress_is_not_written() {
        let (_d, m, id) = folder("running", 10, Status::Running);
        assert!(save(&m, id, vec![line(0, "you", None, "x")], None).unwrap_err().contains("right now"));
        assert!(m.save_overview(SaveSummaryArgs { id, markdown: "## Summary".into() }).unwrap_err().contains("right now"));
    }

    #[test]
    fn the_summary_is_saved_and_an_empty_one_refused() {
        let (dir, m, id) = folder("summary", 10, Status::Done);
        assert!(m.save_overview(SaveSummaryArgs { id, markdown: "  ".into() }).is_err());
        m.save_overview(SaveSummaryArgs { id, markdown: "## Summary\n- Shipped it.".into() }).unwrap();
        assert_eq!(transcripts::load_in(&dir, id).unwrap().summary.as_deref(), Some("## Summary\n- Shipped it."));
    }

    #[test]
    fn unknown_ids_and_files_get_a_clear_message() {
        let (dir, m, _) = folder("ids", 1, Status::Done);
        assert!(m.meeting(42).unwrap_err().contains("list_meetings"));
        let file = Transcript::new(7, Kind::File, "a.mp3".into(), "a.mp3".into(), "x".into());
        transcripts::save_in(&dir, &file).unwrap();
        assert!(m.meeting(7).unwrap_err().contains("not a recorded meeting"));
    }
}
