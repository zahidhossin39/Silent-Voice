// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // An AI agent starting the local MCP server (src/mcp.rs): no window, no
    // single-instance hand-off, just the server on stdin/stdout.
    if std::env::args().skip(1).any(|a| a == "--mcp") {
        silent_voice_lib::mcp::serve();
    }
    silent_voice_lib::run()
}
