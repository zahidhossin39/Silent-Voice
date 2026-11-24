import { describe, expect, it } from "vitest";
import { agentSetup } from "./agentSetup";

const WIN = "C:\\Program Files\\Silent Voice\\silent-voice.exe";

describe("agentSetup", () => {
  it("quotes a path with spaces for agy and Claude Code, with --mcp after the command", () => {
    expect(agentSetup("agy", WIN)).toBe('agy mcp add silent-voice "C:\\Program Files\\Silent Voice\\silent-voice.exe" -- --mcp');
    expect(agentSetup("claude", WIN)).toBe('claude mcp add --scope user silent-voice -- "C:\\Program Files\\Silent Voice\\silent-voice.exe" --mcp');
  });

  it("writes Codex TOML with a literal string so backslashes stay single", () => {
    expect(agentSetup("codex", WIN)).toBe("[mcp_servers.silent-voice]\ncommand = 'C:\\Program Files\\Silent Voice\\silent-voice.exe'\nargs = [\"--mcp\"]");
  });

  it("falls back to an escaped TOML string when the path holds a single quote", () => {
    expect(agentSetup("codex", "C:\\Users\\O'Neil\\sv.exe")).toContain('command = "C:\\\\Users\\\\O\'Neil\\\\sv.exe"');
  });

  it("writes JSON that parses back to the exact path", () => {
    const parsed = JSON.parse(agentSetup("json", WIN));
    expect(parsed.mcpServers["silent-voice"]).toEqual({ command: WIN, args: ["--mcp"] });
  });
});
