use serde::{Deserialize, Serialize};
use std::time::Duration;

// Generic OpenAI-compatible chat client. Works with cloud providers (OpenAI,
// OpenRouter, Groq, Together, …) and local servers (LM Studio, llama.cpp
// server) — they all expose POST {base_url}/chat/completions.

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_default()
}

/// Normalize a base URL: trim trailing slash. Caller's URL should include the
/// API version path (e.g. ".../v1").
fn endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

/// Fetch the full list of model ids a provider offers (GET {base_url}/models).
/// Works for OpenRouter, OpenAI, Groq, Together, LM Studio, etc.
pub async fn list_models(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = client().get(&url);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Could not reach {base_url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Provider returned {}", resp.status()));
    }
    let parsed: ModelsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}

/// Run a system+user chat completion and return the assistant text.
pub async fn chat(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    text: &str,
) -> Result<String, String> {
    let url = endpoint(base_url);
    let body = ChatRequest {
        model,
        messages: vec![
            Message {
                role: "system",
                content: system_prompt,
            },
            Message {
                role: "user",
                content: text,
            },
        ],
        stream: false,
    };

    let mut req = client().post(&url).json(&body);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    // Recommended attribution headers for OpenRouter (ignored by others).
    req = req
        .header("HTTP-Referer", "https://silent-voice.app")
        .header("X-Title", "Silent Voice");

    let resp = req.send().await.map_err(|e| {
        if e.is_connect() {
            format!("Could not reach {base_url}. Is the server running?")
        } else {
            e.to_string()
        }
    })?;

    if !resp.status().is_success() {
        let code = resp.status();
        let msg = resp.text().await.unwrap_or_default();
        return Err(format!("API error {code}: {msg}"));
    }

    let parsed: ChatResponse = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content.trim().to_string())
        .ok_or_else(|| "empty response from model".to_string())
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Turn a cloud STT HTTP failure into a message a user can act on, instead of
/// raw status + JSON. The raw body is appended for debugging (it also lands in
/// the log file via report_error).
fn stt_http_error(code: reqwest::StatusCode, body: &str) -> String {
    let hint = match code.as_u16() {
        401 | 403 => "The API key was rejected — re-check it in API Keys.",
        402 => "The provider says you're out of credits — add credits to your account.",
        404 => "The provider doesn't offer this endpoint or model — check the STT model name.",
        429 => "The model's server is overloaded or rate-limited right now (already retried twice). On OpenRouter, whisper models have a single upstream provider — when it's saturated there is no alternative route regardless of your credits. Try again shortly, or use Groq/OpenAI for STT.",
        500..=599 => "The provider had a server error — try again shortly.",
        _ => "",
    };
    if hint.is_empty() {
        format!("Cloud STT error {code}: {body}")
    } else {
        format!("Cloud STT: {hint} ({code}: {body})")
    }
}

/// Transcribe a WAV file via a cloud provider's Whisper endpoint. Dispatches
/// to one of two known-real request shapes based on the host in `base_url`:
///  - OpenRouter: POST {base_url}/audio/transcriptions, JSON body with
///    base64-encoded audio (`input_audio: { data, format }`) — confirmed
///    against OpenRouter's own docs (openrouter.ai/docs/guides/overview/multimodal/stt).
///  - Everyone else: the standard OpenAI Whisper API shape — POST
///    {base_url}/audio/transcriptions, multipart/form-data with `file` +
///    `model` fields. Confirmed working for OpenAI and Groq. Providers that
///    implement neither shape (most of them) will fail here with whatever
///    error they return — this function does not fabricate support.
pub async fn transcribe_audio(
    base_url: &str,
    api_key: &str,
    model: &str,
    wav_path: &std::path::Path,
    vocabulary: &str,
) -> Result<String, String> {
    if base_url.contains("openrouter.ai") {
        transcribe_audio_openrouter(base_url, api_key, model, wav_path, vocabulary).await
    } else {
        transcribe_audio_multipart(base_url, api_key, model, wav_path, vocabulary).await
    }
}

/// 429s and 5xx from cloud STT are usually transient upstream-capacity blips
/// (e.g. OpenRouter's whisper models have a single upstream provider — when
/// it's briefly saturated the next attempt often succeeds). Retry twice with
/// short backoff before giving up.
const STT_RETRY_DELAYS_MS: [u64; 2] = [800, 2000];

fn stt_status_is_retryable(code: reqwest::StatusCode) -> bool {
    code.as_u16() == 429 || code.is_server_error()
}

async fn transcribe_audio_multipart(
    base_url: &str,
    api_key: &str,
    model: &str,
    wav_path: &std::path::Path,
    vocabulary: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/audio/transcriptions",
        base_url.trim_end_matches('/')
    );

    let bytes = tokio::fs::read(wav_path)
        .await
        .map_err(|e| format!("could not read recording: {e}"))?;

    let mut attempt = 0usize;
    loop {
        // multipart forms aren't reusable — rebuild per attempt (clip is small).
        let file_part = reqwest::multipart::Part::bytes(bytes.clone())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", model.to_string());

        let vocab = vocabulary.trim();
        if !vocab.is_empty() {
            form = form.text("prompt", vocab.to_string());
        }

        let mut req = client().post(&url).multipart(form);
        if !api_key.is_empty() {
            req = req.bearer_auth(api_key);
        }

        let resp = req.send().await.map_err(|e| {
            if e.is_connect() {
                format!("Could not reach {base_url}. Is the server running?")
            } else {
                e.to_string()
            }
        })?;

        if !resp.status().is_success() {
            let code = resp.status();
            let msg = resp.text().await.unwrap_or_default();
            if stt_status_is_retryable(code) && attempt < STT_RETRY_DELAYS_MS.len() {
                tokio::time::sleep(Duration::from_millis(STT_RETRY_DELAYS_MS[attempt])).await;
                attempt += 1;
                continue;
            }
            return Err(stt_http_error(code, &msg));
        }

        let parsed: TranscriptionResponse = resp
            .json()
            .await
            .map_err(|e| format!("Unexpected response shape from provider: {e}"))?;
        return Ok(parsed.text.trim().to_string());
    }
}

#[derive(Serialize)]
struct OpenRouterInputAudio<'a> {
    data: String,
    format: &'a str,
}

#[derive(Serialize)]
struct OpenRouterTranscriptionRequest<'a> {
    model: &'a str,
    input_audio: OpenRouterInputAudio<'a>,
}

async fn transcribe_audio_openrouter(
    base_url: &str,
    api_key: &str,
    model: &str,
    wav_path: &std::path::Path,
    // OpenRouter's transcription API documents no `prompt`/vocabulary field —
    // custom vocabulary only applies to local + multipart (OpenAI/Groq) STT.
    _vocabulary: &str,
) -> Result<String, String> {
    use base64::Engine;

    let url = format!(
        "{}/audio/transcriptions",
        base_url.trim_end_matches('/')
    );

    let bytes = tokio::fs::read(wav_path)
        .await
        .map_err(|e| format!("could not read recording: {e}"))?;
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let body = OpenRouterTranscriptionRequest {
        model,
        input_audio: OpenRouterInputAudio { data, format: "wav" },
    };

    let mut attempt = 0usize;
    loop {
        let mut req = client().post(&url).json(&body);
        if !api_key.is_empty() {
            req = req.bearer_auth(api_key);
        }
        req = req
            .header("HTTP-Referer", "https://silent-voice.app")
            .header("X-Title", "Silent Voice");

        let resp = req.send().await.map_err(|e| {
            if e.is_connect() {
                format!("Could not reach {base_url}. Is the server running?")
            } else {
                e.to_string()
            }
        })?;

        if !resp.status().is_success() {
            let code = resp.status();
            let msg = resp.text().await.unwrap_or_default();
            if stt_status_is_retryable(code) && attempt < STT_RETRY_DELAYS_MS.len() {
                tokio::time::sleep(Duration::from_millis(STT_RETRY_DELAYS_MS[attempt])).await;
                attempt += 1;
                continue;
            }
            return Err(stt_http_error(code, &msg));
        }

        let parsed: TranscriptionResponse = resp
            .json()
            .await
            .map_err(|e| format!("Unexpected response shape from provider: {e}"))?;
        return Ok(parsed.text.trim().to_string());
    }
}
