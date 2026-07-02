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
