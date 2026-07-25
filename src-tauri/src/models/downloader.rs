use super::registry;
use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use tauri::{AppHandle, Emitter};

// CoEdIT-large grammar model, INT8 ONNX. Hosted on Hugging Face — upload the
// three files from coedit-work/coedit_onnx/ to this repo (see release notes).
// If the HF username differs from the GitHub one, update the repo path here.
const VAD_URL: &str =
    "https://huggingface.co/onnx-community/silero-vad/resolve/main/onnx/model.onnx";
const COEDIT_ENCODER_URL: &str = "https://huggingface.co/Zaid-Hossain/coedit-large-int8-onnx/resolve/main/encoder_model_int8.onnx";
const COEDIT_DECODER_URL: &str = "https://huggingface.co/Zaid-Hossain/coedit-large-int8-onnx/resolve/main/decoder_model_int8.onnx";
const COEDIT_TOKENIZER_URL: &str = "https://huggingface.co/Zaid-Hossain/coedit-large-int8-onnx/resolve/main/tokenizer.json";

#[derive(Serialize, Clone)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub status: String, // "downloading" | "downloaded" | "error"
    pub error: Option<String>,
}

/// Download a Whisper GGML model from `url` to the models directory.
pub async fn download_model(
    app: AppHandle,
    model_id: String,
    url: String,
    file_name: String,
) -> Result<(), String> {
    registry::ensure_dirs().map_err(|e| e.to_string())?;
    let dest = registry::models_dir().join(&file_name);
    download_to(app, model_id, url, dest).await
}

/// Download an LLM GGUF model to the llm directory (stored as <id>.gguf).
pub async fn download_llm_model(
    app: AppHandle,
    model_id: String,
    url: String,
) -> Result<(), String> {
    registry::ensure_dirs().map_err(|e| e.to_string())?;
    let dest = registry::llm_model_path(&model_id);
    download_to(app, model_id, url, dest).await
}

pub fn delete_llm_model(model_id: &str) -> Result<(), String> {
    let path = registry::llm_model_path(model_id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Download a TTS voice.
///
/// Piper voice (url_json non-empty): the .onnx model (with progress events)
/// plus its small .onnx.json config. Progress is reported against the .onnx
/// size — the JSON is a few KB and fetched after.
///
/// Sherpa voice (url_json EMPTY): url_onnx is a .tar.bz2 archive whose top-level
/// folder name equals `voice_id` (that's how k2-fsa distributes them). We
/// download it, extract into the tts dir with the system `tar` (bsdtar ships
/// with Windows 10+ and auto-detects bzip2), and delete the archive.
pub async fn download_tts_model(
    app: AppHandle,
    voice_id: String,
    url_onnx: String,
    url_json: String,
) -> Result<(), String> {
    registry::ensure_dirs().map_err(|e| e.to_string())?;

    if url_json.ends_with("tokens.txt") {
        // Sherpa two-file voice (e.g. MMS conversions): url_onnx → dir/model.onnx,
        // url_json → dir/tokens.txt. No archive involved.
        let dir = registry::sherpa_voice_dir(&voice_id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .user_agent("SilentVoice/0.1.6 (+https://github.com/zahidhossin39/Silent-Voice)")
            .build()
            .map_err(|e| e.to_string())?;
        let tokens = client.get(&url_json)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("tokens.txt"), &tokens).map_err(|e| e.to_string())?;
        return download_to(app, voice_id, url_onnx, dir.join("model.onnx")).await;
    }

    if url_json.is_empty() {
        // Sherpa archive path.
        let archive = registry::tts_models_dir().join(format!("{voice_id}.tar.bz2"));
        download_to(app.clone(), voice_id.clone(), url_onnx, archive.clone()).await?;

        let out = tokio::task::spawn_blocking({
            let archive = archive.clone();
            let dest = registry::tts_models_dir();
            move || {
                let mut cmd = std::process::Command::new("tar");
                cmd.arg("-xf").arg(&archive).arg("-C").arg(&dest);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                    cmd.creation_flags(CREATE_NO_WINDOW);
                }
                cmd.output()
            }
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("could not run tar: {e}"))?;
        let _ = std::fs::remove_file(&archive);
        if !out.status.success() {
            return Err(format!(
                "voice archive extraction failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        if registry::sherpa_voice_model(&voice_id).is_none() {
            return Err("voice archive did not contain the expected files".into());
        }
        return Ok(());
    }

    // Piper pair path. Config first (tiny) — if it fails we haven't wasted a
    // big download.
    let client = reqwest::Client::builder()
        .user_agent("SilentVoice/0.1.6 (+https://github.com/zahidhossin39/Silent-Voice)")
        .build()
        .map_err(|e| e.to_string())?;
    let json = client.get(&url_json)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    std::fs::write(registry::tts_config_path(&voice_id), &json).map_err(|e| e.to_string())?;

    download_to(app, voice_id.clone(), url_onnx, registry::tts_model_path(&voice_id)).await
}

pub fn delete_tts_model(voice_id: &str) -> Result<(), String> {
    // Sherpa voice = a whole directory.
    let dir = registry::sherpa_voice_dir(voice_id);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    for path in [
        registry::tts_model_path(voice_id),
        registry::tts_config_path(voice_id),
    ] {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub async fn download_vad_model(app: AppHandle) -> Result<(), String> {
    registry::ensure_dirs().map_err(|e| e.to_string())?;
    let dir = registry::vad_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    download_to(
        app,
        "vad".into(),
        VAD_URL.into(),
        dir.join("silero_vad.onnx"),
    )
    .await
}

pub fn delete_vad_model() -> Result<(), String> {
    let dir = registry::vad_dir();
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn download_coedit_model(app: AppHandle) -> Result<(), String> {
    registry::ensure_dirs().map_err(|e| e.to_string())?;
    let dir = registry::coedit_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (name, url) in [
        ("encoder_model_int8.onnx", COEDIT_ENCODER_URL),
        ("decoder_model_int8.onnx", COEDIT_DECODER_URL),
        ("tokenizer.json", COEDIT_TOKENIZER_URL),
    ] {
        download_to(app.clone(), "coedit".into(), url.into(), dir.join(name)).await?;
    }
    Ok(())
}

pub fn delete_coedit_model() -> Result<(), String> {
    let dir = registry::coedit_dir();
    if dir.is_dir() { std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?; }
    Ok(())
}

/// Stream `url` to `dest` with resume support, emitting `download://progress` events. §4 / §14.
async fn download_to(
    app: AppHandle,
    model_id: String,
    url: String,
    dest: std::path::PathBuf,
) -> Result<(), String> {
    fetch_to_file(&url, &dest, |downloaded, total| {
        emit(
            &app,
            DownloadProgress {
                model_id: model_id.clone(),
                downloaded_bytes: downloaded,
                total_bytes: total,
                status: "downloading".into(),
                error: None,
            },
        );
    })
    .await?;

    let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    emit(
        &app,
        DownloadProgress {
            model_id,
            downloaded_bytes: size,
            total_bytes: size,
            status: "downloaded".into(),
            error: None,
        },
    );
    Ok(())
}

/// The actual transport: resume-aware, retrying, size-verified fetch of `url`
/// into `dest`. Kept free of `AppHandle` so it is testable without a Tauri
/// runtime — progress is reported through `on_progress(downloaded, total)`.
async fn fetch_to_file(
    url: &str,
    dest: &std::path::Path,
    on_progress: impl Fn(u64, u64),
) -> Result<(), String> {
    // Append .part to dest filename so it cannot collide with a differently-named real model file.
    let mut tmp_name = dest.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".part");
    let tmp = dest.with_file_name(tmp_name);

    let client = reqwest::Client::builder()
        .user_agent("SilentVoice/0.1.6 (+https://github.com/zahidhossin39/Silent-Voice)")
        .build()
        .map_err(|e| e.to_string())?;

    for attempt in 1..=4 {
        let resume_from = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);

        let mut req = client.get(url);
        if resume_from > 0 {
            req = req.header("Range", format!("bytes={resume_from}-"));
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                if attempt < 4 {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                    continue;
                }
                return Err(e.to_string());
            }
        };

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 4 {
            tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
            continue;
        }

        // 416 means our .part is longer than the file the server now has (it was
        // replaced, or the leftover belongs to a different build). Resuming can
        // never recover from that, and keeping the .part would make every future
        // attempt fail the same way — so drop it and restart from scratch.
        if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && resume_from > 0 {
            let _ = std::fs::remove_file(&tmp);
            if attempt < 4 {
                continue;
            }
            return Err("stale partial download; please try again".into());
        }

        let resp = resp.error_for_status().map_err(|e| e.to_string())?;

        let status = resp.status();
        let (mut file, mut downloaded) =
            if resume_from > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT {
                let f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&tmp)
                    .map_err(|e| e.to_string())?;
                (f, resume_from)
            } else {
                // A 200 reply to a Range request means the server ignored it, so appending would corrupt the file by duplicating the leading bytes.
                let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
                (f, 0)
            };

        let total = if status == reqwest::StatusCode::PARTIAL_CONTENT {
            resume_from + resp.content_length().unwrap_or(0)
        } else {
            resp.content_length().unwrap_or(0)
        };

        let mut stream = resp.bytes_stream();
        let mut stream_err: Option<String> = None;
        let mut last_emit = downloaded;

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    stream_err = Some(e.to_string());
                    break;
                }
            };

            if let Err(e) = file.write_all(&chunk) {
                stream_err = Some(e.to_string());
                break;
            }

            downloaded += chunk.len() as u64;

            // Throttle events to ~every 1 MB to avoid flooding the UI.
            if downloaded - last_emit > 1_000_000 {
                last_emit = downloaded;
                on_progress(downloaded, total);
            }
        }

        if stream_err.is_none() {
            if let Err(e) = file.flush() {
                stream_err = Some(e.to_string());
            }
        }
        drop(file);

        if let Some(err) = stream_err {
            if attempt < 4 {
                tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                continue;
            }
            return Err(err);
        }

        let actual_len = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
        if total > 0 && actual_len != total {
            if attempt < 4 {
                tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                continue;
            } else {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!(
                    "download truncated: expected {total} bytes, got {actual_len}"
                ));
            }
        }

        std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Unreachable: the final attempt always either returns Ok or Err above.
    Err("download failed".into())
}

pub fn delete_model(model_id: &str) -> Result<(), String> {
    let path = registry::model_path(model_id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn emit<R: tauri::Runtime>(app: &AppHandle<R>, payload: DownloadProgress) {
    let _ = app.emit("download://progress", payload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const TOTAL: usize = 1000;

    fn body() -> Vec<u8> {
        (0..TOTAL).map(|i| (i % 251) as u8).collect()
    }

    /// How the fake server should answer one request.
    #[derive(Clone, Copy)]
    enum Reply {
        /// Honour Range with a 206, or send the whole body with a 200.
        Serve,
        /// Claim the full length but send half the bytes and hang up — this is
        /// what a dropped connection mid-download looks like to the client.
        Truncated,
        /// Reject the Range (what a real server does when our .part is longer
        /// than the file it currently has).
        Range416,
    }

    /// Serve `replies.len()` sequential requests on an ephemeral port, then stop.
    async fn spawn_server(replies: Vec<Reply>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            for reply in replies {
                let Ok((mut sock, _)) = listener.accept().await else { return };

                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&req).to_lowercase();
                let start: usize = text
                    .split("range: bytes=")
                    .nth(1)
                    .and_then(|r| r.split('-').next())
                    .and_then(|n| n.trim().parse().ok())
                    .unwrap_or(0);

                let full = body();
                let _ = match reply {
                    Reply::Range416 if start > 0 => {
                        sock.write_all(
                            format!(
                                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{TOTAL}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            )
                            .as_bytes(),
                        )
                        .await
                    }
                    Reply::Truncated => {
                        let remaining = TOTAL - start;
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {remaining}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
                        );
                        let _ = sock.write_all(head.as_bytes()).await;
                        sock.write_all(&full[start..start + remaining / 2]).await
                    }
                    _ if start > 0 && start < TOTAL => {
                        let last = TOTAL - 1;
                        let head = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{last}/{TOTAL}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                            TOTAL - start
                        );
                        let _ = sock.write_all(head.as_bytes()).await;
                        sock.write_all(&full[start..]).await
                    }
                    _ => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {TOTAL}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
                        );
                        let _ = sock.write_all(head.as_bytes()).await;
                        sock.write_all(&full).await
                    }
                };
                let _ = sock.shutdown().await;
            }
        });
        port
    }

    /// A unique dest path per test so they can run in parallel.
    fn dest_for(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sv-dl-test-{name}.bin"));
        let _ = std::fs::remove_file(&p);
        let mut part = p.file_name().unwrap().to_os_string();
        part.push(".part");
        let _ = std::fs::remove_file(p.with_file_name(part));
        p
    }

    fn part_of(dest: &std::path::Path) -> std::path::PathBuf {
        let mut n = dest.file_name().unwrap().to_os_string();
        n.push(".part");
        dest.with_file_name(n)
    }

    async fn run(replies: Vec<Reply>, dest: &std::path::Path) -> Result<(), String> {
        let port = spawn_server(replies).await;
        let url = format!("http://127.0.0.1:{port}/model.bin");
        fetch_to_file(&url, dest, |_, _| {}).await
    }

    #[tokio::test]
    async fn full_download_writes_every_byte() {
        let dest = dest_for("full");
        run(vec![Reply::Serve], &dest).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body());
        assert!(!part_of(&dest).exists(), ".part should be renamed away");
        let _ = std::fs::remove_file(&dest);
    }

    #[tokio::test]
    async fn resumes_from_an_existing_part_file() {
        let dest = dest_for("resume");
        // Pretend a previous run already fetched the first 400 bytes.
        std::fs::write(part_of(&dest), &body()[..400]).unwrap();

        run(vec![Reply::Serve], &dest).await.unwrap();

        // Exactly the original bytes — not 1400, which is what appending a full
        // second body onto the partial file would produce.
        assert_eq!(std::fs::read(&dest).unwrap(), body());
        let _ = std::fs::remove_file(&dest);
    }

    #[tokio::test]
    async fn recovers_from_a_midstream_drop() {
        let dest = dest_for("midstream");
        // First reply dies halfway; the retry must resume, not start over.
        run(vec![Reply::Truncated, Reply::Serve], &dest).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body());
        let _ = std::fs::remove_file(&dest);
    }

    #[tokio::test]
    async fn stale_oversized_part_is_discarded() {
        let dest = dest_for("stale");
        // A leftover .part LONGER than the remote file makes the server reject
        // the Range with 416. Without the 416 branch this download could never
        // succeed again, on this run or any future one.
        std::fs::write(part_of(&dest), vec![9u8; TOTAL + 400]).unwrap();

        run(vec![Reply::Range416, Reply::Serve], &dest).await.unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), body());
        let _ = std::fs::remove_file(&dest);
    }
}
