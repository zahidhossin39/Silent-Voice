// Setup text for connecting an AI agent to Silent Voice's local MCP server
// (`silent-voice --mcp`, src-tauri/src/mcp.rs). Each agent stores the command
// in its own format, and a Windows path needs different escaping in each.

export type AgentId = "agy" | "claude" | "codex" | "json";

export const AGENTS: { id: AgentId; label: string; hears: boolean; where: string }[] = [
  { id: "agy", label: "Antigravity (agy)", hears: true, where: "Run this once in a terminal:" },
  { id: "claude", label: "Claude Code", hears: false, where: "Run this once in a terminal:" },
  { id: "codex", label: "Codex", hears: false, where: "Add this to ~/.codex/config.toml:" },
  { id: "json", label: "Cursor, Gemini CLI, others", hears: false, where: "Add this to the agent's mcp.json or settings.json:" },
];

/** A path inside double quotes for a shell (PowerShell, cmd, bash): `"` doubled is never valid in a path, so only quote it. */
const shellQuote = (p: string) => `"${p}"`;

/** TOML literal string ('...'): backslashes stay as they are, so Windows paths need no escaping. */
const tomlLiteral = (p: string) => (p.includes("'") ? JSON.stringify(p) : `'${p}'`);

export function agentSetup(agent: AgentId, exe: string): string {
  switch (agent) {
    case "agy":
      return `agy mcp add silent-voice ${shellQuote(exe)} -- --mcp`;
    case "claude":
      return `claude mcp add --scope user silent-voice -- ${shellQuote(exe)} --mcp`;
    case "codex":
      return `[mcp_servers.silent-voice]\ncommand = ${tomlLiteral(exe)}\nargs = ["--mcp"]`;
    case "json":
      return JSON.stringify({ mcpServers: { "silent-voice": { command: exe, args: ["--mcp"] } } }, null, 2);
  }
}
