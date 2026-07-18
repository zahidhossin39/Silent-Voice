use crate::llm::openai;
use crate::models::registry;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::ShellExt;

/// Runs transcription via either the local whisper.cpp sidecar or a cloud
/// provider's Whisper-shaped endpoint, based on `stt_source` ("local" | "cloud").
/// Both hotkey dictation and the manual start/stop-recording command go
/// through this so they stay in sync.
#[allow(clippy::too_many_arguments)]
pub async fn transcribe_dispatch(
    app: &AppHandle,
    audio_path: &std::path::Path,
    model_id: &str,
    threads: u32,
    language: &str,
    vocabulary: &str,
    use_gpu: bool,
    stt_source: &str,
    stt_base_url: &str,
    stt_api_key: &str,
    stt_cloud_model: &str,
) -> Result<String, String> {
    if stt_source == "cloud" {
        if stt_base_url.is_empty() || stt_cloud_model.is_empty() {
            return Err(
                "Cloud STT is selected but the provider's base URL or STT model is empty — check API Keys.".into(),
            );
        }
        match openai::transcribe_audio(
            stt_base_url,
            stt_api_key,
            stt_cloud_model,
            audio_path,
            vocabulary,
        )
        .await
        {
            Ok(t) => Ok(t),
            // Cloud failed (rate limit, outage, bad key…). If a local model is
            // downloaded, transcribe with it instead so the user never loses
            // their words — and tell them what happened.
            Err(cloud_err) => {
                if registry::model_path(model_id).exists() {
                    crate::logging::log_error(
                        "stt",
                        &format!("cloud STT failed, falling back to local '{model_id}': {cloud_err}"),
                    );
                    let _ = app.emit(
                        "pipeline://error",
                        format!("Cloud STT failed — used local model '{model_id}' instead. ({cloud_err})"),
                    );
                    transcribe(
                        app,
                        audio_path.to_string_lossy().as_ref(),
                        model_id,
                        threads,
                        language,
                        vocabulary,
                        use_gpu,
                    )
                    .await
                } else {
                    Err(cloud_err)
                }
            }
        }
    } else {
        transcribe(
            app,
            audio_path.to_string_lossy().as_ref(),
            model_id,
            threads,
            language,
            vocabulary,
            use_gpu,
        )
        .await
    }
}

/// Run the bundled whisper.cpp sidecar over a 16 kHz WAV file and return the
/// transcribed text. Build plan §13 — whisper.cpp Sidecar Invocation.
///
/// `model_id` is e.g. "small.en"; `language` is an ISO code or "auto".
#[allow(clippy::too_many_arguments)]
pub async fn transcribe(
    app: &AppHandle,
    audio_path: &str,
    model_id: &str,
    threads: u32,
    language: &str,
    vocabulary: &str,
    use_gpu: bool,
) -> Result<String, String> {
    let model_path = registry::model_path(model_id);
    if !model_path.exists() {
        return Err(format!(
            "model '{model_id}' is not downloaded ({})",
            model_path.display()
        ));
    }

    let lang = if language.is_empty() { "auto" } else { language };

    // Fast path: persistent whisper-server keeps the model loaded between
    // dictations. Any failure falls through to the one-shot CLI below.
    match transcribe_via_server(
        app, audio_path, &model_path, model_id, threads, lang, vocabulary, use_gpu,
    )
    .await
    {
        Ok(text) => return Ok(clean_output(&text)),
        Err(e) => {
            crate::logging::log_error("stt", &format!("whisper-server path failed, using CLI: {e}"));
        }
    }

    let mut args: Vec<String> = vec![
        "-m".into(),
        model_path.to_string_lossy().into_owned(),
        "-f".into(),
        audio_path.into(),
        "-t".into(),
        threads.to_string(),
        "-l".into(),
        lang.into(),
        "--no-timestamps".into(), // -nt: text only, no [timestamps]
    ];

    // Custom vocabulary: fed to whisper.cpp as an initial prompt, which biases
    // the decoder toward recognizing these words/names correctly. Only helps
    // for roughly the first 30s of audio (whisper's prompt window) — fine for
    // push-to-talk dictation, which is normally shorter than that anyway.
    let vocab = vocabulary.trim();
    if !vocab.is_empty() {
        args.push("--prompt".into());
        args.push(vocab.into());
    }

    // GPU toggle: whisper.cpp uses its GPU backend by default when the binary
    // was built with one; `-ng` forces CPU. (Boolean flag — no value, see §8.3.)
    if !use_gpu {
        args.push("-ng".into());
    }

    let output = app
        .shell()
        .sidecar("whisper-cpp")
        .map_err(|e| e.to_string())?
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("whisper.cpp failed: {stderr}"));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(clean_output(&text))
}

/// Ensure the persistent whisper-server is running with the current settings
/// (restarting it if any of them changed) and run one inference against it.
#[allow(clippy::too_many_arguments)]
async fn transcribe_via_server(
    app: &AppHandle,
    audio_path: &str,
    model_path: &std::path::Path,
    model_id: &str,
    threads: u32,
    lang: &str,
    vocabulary: &str,
    use_gpu: bool,
) -> Result<String, String> {
    use tauri::Manager;
    let key = format!("{model_id}|{lang}|{}|{use_gpu}|{threads}", vocabulary.trim());
    let started = {
        let state = app.state::<crate::AppState>();
        let mut server = state.whisper_server.lock().map_err(|e| e.to_string())?;
        if server.is_running(&key) {
            false
        } else {
            server.start(model_path, &key, threads, lang, vocabulary, use_gpu)?;
            true
        }
    };
    // First load of a big model takes a while; a warm server answers instantly.
    let timeout = if started {
        std::time::Duration::from_secs(120)
    } else {
        std::time::Duration::from_secs(5)
    };
    super::server::wait_ready(timeout).await?;
    super::server::transcribe(std::path::Path::new(audio_path)).await
}

/// whisper.cpp prints transcription lines; strip stray blank lines / markers.
fn clean_output(raw: &str) -> String {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('['))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}
