// Local MCP server: `silent-voice --mcp`. An AI agent (Gemini CLI, Claude Code,
// Codex, Cursor...) starts it over stdio when it needs it; it exits when the
// agent closes the pipe. No window, no Tauri, no network port: it only reads and
// writes the meeting files in the data folder (transcripts.rs).
// Decision record: docs/adr/0003-mcp-server-over-stdio.md.

use crate::transcripts::{self, Kind, Speaker, Status, Transcript};
use base64::Engine;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router, Json, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};

/// The longest piece of audio one call returns: 5 min of 16 kHz mono 16-bit
/// WAV is ~9.6 MB (~12.8 MB as base64), small enough for every MCP client.
const MAX_PIECE_MS: u64 = 5 * 60_000;

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
    sweep_old_pieces();
    let code = rt.block_on(async {
        match Meetings::new().serve(rmcp::transport::stdio()).await {
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
    tool_router: ToolRouter<Self>,
}

impl Default for Meetings {
    fn default() -> Self {
        Self::new()
    }
}

// ── Inputs and outputs (their JSON schemas are what the agent sees) ──

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListMeetingsArgs {
    /// Only meetings with this status: "recorded" (waiting for an agent to
    /// transcribe it), "done", "running", "stopped" or "failed".
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
    pub name: String,
    /// Start time, Unix ms.
    pub created: i64,
    pub duration_ms: Option<u64>,
    /// "recorded" means audio only, waiting to be transcribed.
    pub status: String,
    /// Who or what wrote the transcript ("agent: gemini-cli", a model id...).
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

/// Where audio pieces for agents are written: inside the data folder, so they
/// sit next to the meetings and never in a shared temp folder.
fn pieces_dir() -> std::path::PathBuf {
    transcripts::dir().join("agent-pieces")
}

/// Pieces older than a day are leftovers from earlier agent sessions.
fn sweep_old_pieces() {
    let day = std::time::Duration::from_secs(24 * 3600);
    for e in std::fs::read_dir(pieces_dir()).into_iter().flatten().flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > day);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

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

fn parse_track(track: &str) -> Result<Speaker, String> {
    match track.trim().to_ascii_lowercase().as_str() {
        "you" => Ok(Speaker::You),
        "others" => Ok(Speaker::Others),
        other => Err(format!("track must be \"you\" or \"others\", not \"{other}\"")),
    }
}

/// A recorded meeting by id, or a message the agent can act on.
fn meeting(id: i64) -> Result<Transcript, String> {
    let t = transcripts::load(id).map_err(|_| format!("No meeting with id {id}. Call list_meetings for valid ids."))?;
    if t.kind != Kind::Meeting {
        return Err(format!("{id} is a transcribed file, not a recorded meeting."));
    }
    Ok(t)
}

#[tool_router]
impl Meetings {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    #[tool(
        description = "List recorded meetings, newest first. Meetings with status \"recorded\" have audio but no transcript yet: transcribe them with get_meeting_audio and save_transcript.",
        annotations(title = "List meetings", read_only_hint = true, open_world_hint = false)
    )]
    pub async fn list_meetings(&self, Parameters(args): Parameters<ListMeetingsArgs>) -> Result<Json<MeetingList>, String> {
        let limit = args.limit.unwrap_or(20).clamp(1, 100);
        let want = args.status.map(|s| s.trim().to_ascii_lowercase());
        // list() drops the lines to stay light; count them from the full file only for the ones returned.
        let meetings = transcripts::list()
            .into_iter()
            .filter(|t| t.kind == Kind::Meeting)
            .filter(|t| want.as_deref().is_none_or(|w| status_name(t.status) == w))
            .take(limit)
            .map(|t| {
                let lines = transcripts::load(t.id).map(|full| full.lines.len()).unwrap_or(0);
                info(&t, lines)
            })
            .collect();
        Ok(Json(MeetingList { meetings }))
    }

    #[tool(
        description = "Get a piece (at most 5 minutes) of one track of a meeting's audio, saved as a 16 kHz mono WAV file. Open `piece_file` with your own file tool to listen to it. Call again from `next_start_ms` until it is absent to cover the whole meeting. Track \"you\" is the user's microphone; \"others\" is everyone else on the call.",
        annotations(title = "Get meeting audio", read_only_hint = true, open_world_hint = false)
    )]
    pub async fn get_meeting_audio(&self, Parameters(args): Parameters<AudioArgs>) -> Result<CallToolResult, String> {
        let t = meeting(args.id)?;
        let track = parse_track(&args.track)?;
        let track_name = if track == Speaker::You { "you" } else { "others" };
        if !transcripts::audio_path(t.id, track).exists() {
            return Err(format!("Meeting {} has no saved {track_name} audio.", t.id));
        }
        let total = t.duration_ms.unwrap_or(0);
        if total > 0 && args.start_ms >= total {
            return Err(format!("start_ms {} is past the end of the meeting ({total} ms).", args.start_ms));
        }
        let length = args.length_ms.unwrap_or(MAX_PIECE_MS).clamp(1_000, MAX_PIECE_MS);
        let end = if total > 0 { (args.start_ms + length).min(total) } else { args.start_ms + length };
        let wav = transcripts::audio_clip(t.id, Some(track), args.start_ms, end)?;

        let dir = pieces_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Could not save the audio piece: {e}"))?;
        let file = dir.join(format!("{}-{track_name}-{}-{end}.wav", t.id, args.start_ms));
        std::fs::write(&file, &wav).map_err(|e| format!("Could not save the audio piece: {e}"))?;

        let piece = AudioPiece {
            id: t.id,
            track: track_name.into(),
            start_ms: args.start_ms,
            end_ms: end,
            duration_ms: total,
            piece_file: file.to_string_lossy().into_owned(),
            next_start_ms: (total > 0 && end < total).then_some(end),
            file_you: transcripts::audio_path(t.id, Speaker::You).to_string_lossy().into_owned(),
            file_others: transcripts::audio_path(t.id, Speaker::Others).to_string_lossy().into_owned(),
        };
        let mut result = CallToolResult::structured(serde_json::to_value(&piece).map_err(|e| e.to_string())?);
        if args.inline {
            result.content.push(ContentBlock::audio(base64::engine::general_purpose::STANDARD.encode(&wav), "audio/wav"));
        }
        Ok(result)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Meetings {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("silent-voice", env!("CARGO_PKG_VERSION")).with_title("Silent Voice"))
            .with_instructions(
            "Silent Voice meeting recordings on this computer. Each meeting has two audio tracks: \
             \"you\" (the user's microphone) and \"others\" (everyone else, from the computer's sound). \
             Meetings with status \"recorded\" are waiting to be transcribed.",
        )
    }
}
