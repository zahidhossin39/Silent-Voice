# Security Review Report: Silent Voice

## Overview

A security-focused code review was conducted on the Silent Voice local-first Tauri v2 desktop application. The review focused on the requested areas: archive extraction, model downloads, local HTTP sidecar servers, Tauri command endpoints, and the frontend. 

The review identified **critical path traversal vulnerabilities** allowing arbitrary file write and arbitrary file deletion from the Tauri frontend. 

Below are the detailed findings.

## 1. Archive extraction (tar.bz2 / zip)

### [HIGH] Zip-Slip / Path Traversal in Archive Extraction
- **File:** `src-tauri/src/models/downloader.rs:192`
- **Category:** Zip-Slip / Path Traversal
- **Concrete Exploit Path:** The application downloads `.tar.bz2` archives (e.g., Moonshine/Sherpa models) and extracts them in `extract_tar_bz2()`. To prevent Zip-Slip, the code verifies if `target_path.starts_with(&dest_path)`. However, Rust's `std::path::Path::starts_with()` checks **components**, not canonicalized strings. For example, if a malicious tar archive contains an entry `../evil.exe`, `target_path` evaluates to `[dest_path_components, "..", "evil.exe"]`. This path's first components *exactly match* `dest_path_components`, so `starts_with()` evaluates to **TRUE**. The extraction bypasses the check, resolves the `../` natively via the OS during unpacking, and writes `evil.exe` outside the intended directory, enabling arbitrary file write and potential RCE.
- **Fix:** Do not use `starts_with()` on uncanonicalized `PathBuf` joins. Instead, explicitly check for directory traversal components in the archive entry path before joining:
  ```rust
  if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
      continue;
  }
  ```

## 2. Model download (downloader.rs and hf.rs)

### [HIGH] Arbitrary File Write / Path Traversal via Tauri Download Commands
- **File:** `src-tauri/src/lib.rs:818` (and `833`, `782`), `src-tauri/src/models/downloader.rs:35`
- **Category:** Path Traversal / Arbitrary File Write
- **Concrete Exploit Path:** The Tauri commands `download_model`, `download_llm_model`, `download_stt_archive`, and `download_tts_model` take `model_id` and/or `file_name` parameters directly from the untrusted frontend and pass them to `PathBuf::join()`. For example, `models_dir().join(&file_name)`. If a compromised frontend (or malicious HF model config) passes an absolute path (`C:/Windows/System32/malicious.dll`) or a relative traversal path (`../../../../../malicious.dll`) as `file_name` / `model_id`, the HTTP client in `fetch_to_file` will fetch content from a provided URL and write it to that location on disk. `PathBuf::join` replaces the entire path if given an absolute path, so directory restrictions are bypassed entirely.
- **Fix:** Sanitize all `file_name` and `model_id` strings received in Tauri commands. Ensure they contain no path separators or traversal sequences before passing them to the backend file system. A strict regex (e.g. `^[a-zA-Z0-9_.-]+$`) or checking `file_name.contains('/') || file_name.contains('\\') || file_name.contains("..")` (similar to how it is done correctly in `retranscribe_clip`) must be enforced.

## 3. Local HTTP servers (server.rs and llama.rs)

### [INFO] Missing Authentication on Local Sidecar Servers
- **File:** `src-tauri/src/transcription/server.rs:73`, `src-tauri/src/llm/llama.rs:109`
- **Category:** Unauthenticated Local API
- **Concrete Exploit Path:** Both `whisper-server` and `llama-server` are explicitly bound to `127.0.0.1`. While this successfully blocks network/remote access, the servers do not require authentication, meaning any other non-privileged application running on the local machine can query the STT and LLM servers over ports 8090 and 8088 respectively. Since the servers do not expose model-loading or system-modifying endpoints (models are passed via CLI arguments during spawn), this cannot easily escalate to RCE. However, it does allow arbitrary local software to consume system resources or leak context through prompts.
- **Fix:** Explicit fix isn't strictly required as this is standard behavior for local inference servers. However, binding to ephemeral ports (instead of hardcoded ports) or passing a dynamically generated API key to the sidecar and enforcing it on requests would mitigate cross-app usage.

## 4. Tauri commands (lib.rs)

### [HIGH] Arbitrary Directory & File Deletion via Path Traversal
- **File:** `src-tauri/src/lib.rs:845` (`delete_model`, `delete_llm_model`, `delete_tts_model`), `src-tauri/src/models/downloader.rs:622`
- **Category:** Path Traversal / Arbitrary File Deletion
- **Concrete Exploit Path:** Similar to the download commands, `delete_model` and its variants receive the `model_id` from the frontend and use it to construct a path: e.g., `models_dir().join(model_id)`. If an attacker passes `../../../../Users/Zaid Hossain/Documents` as `model_id`, `stt_model_dir` returns that exact path, and `std::fs::remove_dir_all(&dir)` executes, deleting the user's files.
- **Fix:** Apply the same sanitization as in the download fixes. Ensure `model_id` only contains safe characters and rejects `..`, `/`, and `\`.

### [NO VULNERABILITIES FOUND] Subprocess Spawns and Command Injection
- **File:** `src-tauri/src/transcription/whisper.rs:194`, `src-tauri/src/system/*.rs`
- **Status:** **Secure**.
- **Reasoning:** Subprocesses are spawned using Tauri's Sidecar API (`app.shell().sidecar()`) or `std::process::Command::new()`. Unsanitized frontend variables are passed directly as process arguments (`.args()`) rather than concatenated into a shell string. This completely eliminates the risk of shell metacharacter injection.

## 5. Frontend (React/TS and Tauri Config)

### [NO VULNERABILITIES FOUND] XSS / DOM Clobbering 
- **File:** `src/components/dashboard/hf/SimpleMarkdown.tsx`, `src-tauri/tauri.conf.json`
- **Status:** **Secure**.
- **Reasoning:** 
  1. In `SimpleMarkdown.tsx`, the `markdownToHtml` function strictly calls an `escapeHtml` function (converting `<`, `>`, `&` to entities) *before* processing any Markdown inline styles. Therefore, an attacker cannot insert malicious HTML tags.
  2. The application enforces a strict Content Security Policy (CSP) in `tauri.conf.json` (`script-src 'self'`). This effectively mitigates any potential DOM-based XSS because `unsafe-inline` and `unsafe-eval` are disallowed. Even if a `dangerouslySetInnerHTML` bypass were discovered, the payload would be blocked by the CSP.
