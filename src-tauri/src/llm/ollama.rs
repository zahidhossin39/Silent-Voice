use serde::{Deserialize, Serialize};
use std::time::Duration;

const OLLAMA_BASE: &str = "http://localhost:11434";

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Serialize, Clone)]
pub struct OllamaStatus {
    pub running: bool,
    pub models: Vec<String>,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_default()
}

/// Check whether Ollama is running locally and which models are installed.
pub async fn status() -> OllamaStatus {
    let url = format!("{OLLAMA_BASE}/api/tags");
    match client().get(&url).send().await {
        Ok(resp) => match resp.json::<TagsResponse>().await {
            Ok(tags) => OllamaStatus {
                running: true,
                models: tags.models.into_iter().map(|m| m.name).collect(),
            },
            Err(_) => OllamaStatus {
                running: true,
                models: vec![],
            },
        },
        Err(_) => OllamaStatus {
            running: false,
            models: vec![],
        },
    }
}

/// Run a prompt through a local Ollama model and return the generated text.
/// Build plan §13 — Ollama API Call.
pub async fn generate(
    model: &str,
    system_prompt: &str,
    text: &str,
) -> Result<String, String> {
    let url = format!("{OLLAMA_BASE}/api/generate");
    let body = GenerateRequest {
        model,
        prompt: text,
        system: system_prompt,
        stream: false,
    };

    let resp = client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                "Ollama is not running. Start it with `ollama serve`.".to_string()
            } else {
                e.to_string()
            }
        })?;

    if !resp.status().is_success() {
        let code = resp.status();
        let msg = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama error {code}: {msg}"));
    }

    let parsed: GenerateResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.response.trim().to_string())
}
