use super::registry;
use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use tauri::{AppHandle, Emitter};

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
        let tokens = reqwest::get(&url_json)
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
    let json = reqwest::get(&url_json)
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

/// Stream `url` to `dest`, emitting `download://progress` events. §4 / §14.
async fn download_to(
    app: AppHandle,
    model_id: String,
    url: String,
    dest: std::path::PathBuf,
) -> Result<(), String> {
    let tmp = dest.with_extension("part");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();

    let mut last_emit = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        // Throttle events to ~every 1 MB to avoid flooding the UI.
        if downloaded - last_emit > 1_000_000 {
            last_emit = downloaded;
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
        }
    }

    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;

    emit(
        &app,
        DownloadProgress {
            model_id,
            downloaded_bytes: downloaded,
            total_bytes: total.max(downloaded),
            status: "downloaded".into(),
            error: None,
        },
    );
    Ok(())
}

pub fn delete_model(model_id: &str) -> Result<(), String> {
    let path = registry::model_path(model_id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn emit(app: &AppHandle, payload: DownloadProgress) {
    let _ = app.emit("download://progress", payload);
}
