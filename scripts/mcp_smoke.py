"""Smoke test for the local MCP server: run `<exe> --mcp` the way an agent does.

    python scripts/mcp_smoke.py path/to/silent-voice[.exe]

Checks the handshake, that every tool and the prompt are listed with their
hints, that list_meetings answers, and that the process exits when stdin
closes. Exit code 0 = pass. Used by CI on Windows, macOS and Linux.
"""
import json
import os
import subprocess
import sys
import time

TOOLS = {"list_meetings", "get_meeting", "get_meeting_audio", "save_transcript", "save_summary"}
READ_ONLY = {"list_meetings", "get_meeting", "get_meeting_audio"}


def main(exe: str) -> int:
    exe = os.path.abspath(exe)  # Windows won't launch a relative path with forward slashes
    started = time.time()
    p = subprocess.Popen([exe, "--mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def rpc(i, method, params=None):
        msg = {"jsonrpc": "2.0", "id": i, "method": method}
        if params is not None:
            msg["params"] = params
        p.stdin.write((json.dumps(msg) + "\n").encode())
        p.stdin.flush()
        while True:
            line = p.stdout.readline()
            if not line:
                raise SystemExit("FAIL: the server closed stdout. stderr: " + p.stderr.read().decode(errors="replace"))
            reply = json.loads(line)  # anything that is not JSON on stdout breaks every agent
            if reply.get("id") == i:
                if "error" in reply:
                    raise SystemExit(f"FAIL: {method}: {reply['error']}")
                return reply["result"]

    init = rpc(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "mcp-smoke", "version": "1"}})
    print(f"initialize: {init['serverInfo']} protocol {init['protocolVersion']} in {(time.time() - started) * 1000:.0f} ms")
    assert init["serverInfo"]["name"] == "silent-voice", init["serverInfo"]
    p.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    p.stdin.flush()

    tools = {t["name"]: t for t in rpc(2, "tools/list")["tools"]}
    assert set(tools) == TOOLS, f"tools: {sorted(tools)}"
    for name in READ_ONLY:
        assert tools[name]["annotations"]["readOnlyHint"] is True, name
    assert tools["save_transcript"]["annotations"]["destructiveHint"] is True
    print("tools:", ", ".join(sorted(tools)))

    prompts = [x["name"] for x in rpc(3, "prompts/list")["prompts"]]
    assert prompts == ["transcribe_meeting"], prompts

    listed = rpc(4, "tools/call", {"name": "list_meetings", "arguments": {"limit": 5}})
    assert "meetings" in listed["structuredContent"], listed
    print(f"list_meetings: {len(listed['structuredContent']['meetings'])} meeting(s)")

    bad = rpc(5, "tools/call", {"name": "get_meeting", "arguments": {"id": 1}})
    assert bad.get("isError") is True, bad

    p.stdin.close()
    closed = time.time()
    code = p.wait(timeout=10)
    print(f"exit code {code}, {(time.time() - closed) * 1000:.0f} ms after stdin closed")
    assert code == 0
    print("PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
