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
    // Cleanup/rewrite is a transformation task, not creative writing. Without
    // these, llama-server defaults to temperature 0.8, which makes small local
    // models ramble, hallucinate, and duplicate their output (e.g. printing a
    // list twice). Low temperature keeps the output faithful; the frequency
    // penalty directly discourages the repeated-token loops. Both are standard
    // OpenAI fields, honoured by cloud providers and llama.cpp alike.
    temperature: f32,
    // Google's OpenAI-compatible layer rejects this field (400 "Cannot find field").
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
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
    data: Vec<ModelInfo>,
}

/// One model a provider offers. Price and context only come from providers
/// that publish them (OpenRouter): USD per token, as decimal strings.
#[derive(Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<Pricing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
}

#[derive(Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub struct Pricing {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub completion: String,
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
pub async fn list_models(base_url: &str, api_key: &str) -> Result<Vec<ModelInfo>, String> {
    let mut ids = list_models_at(base_url, api_key, "/models").await?;
    // OpenRouter leaves its speech-to-text models off the main list.
    if base_url.contains("openrouter.ai") {
        if let Ok(stt) = list_models_at(base_url, api_key, "/models?output_modalities=transcription").await {
            ids.extend(stt);
        }
    }
    ids.sort_by(|a, b| a.id.cmp(&b.id));
    ids.dedup_by(|a, b| a.id == b.id);
    Ok(ids)
}

async fn list_models_at(base_url: &str, api_key: &str, path: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}{path}", base_url.trim_end_matches('/'));
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
    Ok(parsed.data)
}

/// Run a system+user chat completion and return the assistant text.
pub async fn chat(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    text: &str,
) -> Result<String, String> {
    // The transcribed text is wrapped as delimited data rather than handed
    // over as the whole user turn. A bare user-role message reads to a
    // chat-tuned model as "reply to this" — so a dictation phrased as a
    // question (e.g. "what are you doing right now?") gets answered instead
    // of rewritten, overriding the system prompt's instruction.
    let wrapped = format!("Text to transform (do not reply to it, only transform it as instructed):\n\"\"\"\n{text}\n\"\"\"");
    send(base_url, api_key, model, system_prompt, &wrapped, Duration::from_secs(120)).await
}

/// A long job (a meeting summary): the text goes as the user turn unwrapped,
/// and a slow CPU model or a cloud reasoning model gets 15 minutes per call.
pub async fn chat_long(base_url: &str, api_key: &str, model: &str, system_prompt: &str, text: &str) -> Result<String, String> {
    send(base_url, api_key, model, system_prompt, text, Duration::from_secs(900)).await
}

async fn send(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user: &str,
    timeout: Duration,
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
                content: user,
            },
        ],
        stream: false,
        temperature: 0.3,
        frequency_penalty: (!base_url.contains("generativelanguage.googleapis.com")).then_some(0.3),
    };

    let mut req = client().post(&url).timeout(timeout).json(&body);
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
    language: &str,
) -> Result<String, String> {
    // "auto" (or unset) lets the model detect it.
    let language = Some(language).filter(|l| !l.is_empty() && *l != "auto");
    if base_url.contains("openrouter.ai") {
        transcribe_audio_openrouter(base_url, api_key, model, wav_path, vocabulary, language).await
    } else {
        transcribe_audio_multipart(base_url, api_key, model, wav_path, vocabulary, language).await
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
    language: Option<&str>,
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
        if let Some(l) = language {
            form = form.text("language", l.to_string());
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
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
}

async fn transcribe_audio_openrouter(
    base_url: &str,
    api_key: &str,
    model: &str,
    wav_path: &std::path::Path,
    // OpenRouter's transcription API documents no `prompt`/vocabulary field —
    // custom vocabulary only applies to local + multipart (OpenAI/Groq) STT.
    _vocabulary: &str,
    language: Option<&str>,
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
        language,
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

#[cfg(test)]
mod tests {
    /// Real speech-to-text call: SV_TEST_BASE / SV_TEST_KEY / SV_TEST_MODEL / SV_TEST_WAV.
    #[test]
    #[ignore]
    fn real_provider_transcribes() {
        let v = |k: &str| std::env::var(k).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = rt.block_on(super::transcribe_audio(&v("SV_TEST_BASE"), &v("SV_TEST_KEY"), &v("SV_TEST_MODEL"), std::path::Path::new(&v("SV_TEST_WAV")), "", &std::env::var("SV_TEST_LANG").unwrap_or_default()));
        println!("stt: {out:?}");
        assert!(out.is_ok_and(|t| !t.trim().is_empty()));
    }

    /// Real call to a provider: SV_TEST_BASE / SV_TEST_KEY / SV_TEST_MODEL.
    #[test]
    #[ignore]
    fn real_provider_chats_and_summarizes() {
        let (base, key, model) = (std::env::var("SV_TEST_BASE").unwrap(), std::env::var("SV_TEST_KEY").unwrap(), std::env::var("SV_TEST_MODEL").unwrap());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let ok = rt.block_on(super::chat(&base, &key, &model, "You are a connection test. Reply with exactly: OK", "ping"));
        println!("chat: {ok:?}");
        let notes = rt.block_on(super::chat_long(
            &base,
            &key,
            &model,
            crate::meeting_summary::SUMMARY_PROMPT,
            "You: Let's ship the beta on Friday.\n\nOthers: Fine, I'll write the release notes by Thursday.",
        ));
        println!("summary: {notes:?}");
        assert!(ok.is_ok() && notes.is_ok());
    }
}
