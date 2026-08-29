use crate::llm::openai;
use crate::models::registry;
use tauri::{AppHandle, Emitter, Manager};
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
    let stt_state = app.state::<crate::AppState>();
    let _stt_gate = stt_state.stt_gate.lock().await;
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
    if let Ok(mut lock) = app.state::<crate::AppState>().last_stt_use.lock() {
        *lock = Some(std::time::Instant::now());
    }

    // Sherpa-onnx models (Moonshine, SenseVoice) are directories, not ggml .bin
    // files, and run through a different engine. Route them before the whisper
    // path. `vocabulary`/`use_gpu` don't apply (CPU, no prompt-biasing hook);
    // `language` is ignored too — Moonshine is English-only and SenseVoice
    // auto-detects per clip.
    if registry::stt_engine(model_id) != registry::SttEngine::Whisper {
        if !registry::sherpa_stt_installed(model_id) {
            return Err(format!("model '{model_id}' is not downloaded"));
        }
        // Say so out loud: this engine drops the language, so "I picked Bangla
        // and got English" has an answer in the log instead of being a mystery.
        if !language.is_empty() && language != "auto" && language != "en" {
            crate::logging::log_info(
                "stt",
                &format!(
                    "model '{model_id}' runs on sherpa-onnx, which ignores the language setting —                      '{language}' not applied"
                ),
            );
        }
        let app = app.clone();
        let audio_path = audio_path.to_string();
        let model_id = model_id.to_string();
        return tokio::task::spawn_blocking(move || {
            crate::system::sherpa_stt::transcribe_file(&app, &audio_path, &model_id, threads)
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    crate::logging::log_info(
        "stt",
        &format!("whisper: model '{model_id}', language '{language}'"),
    );
    let model_path = registry::model_path(model_id);
    if !model_path.exists() {
        return Err(format!(
            "model '{model_id}' is not downloaded ({})",
            model_path.display()
        ));
    }

    let lang = if language.is_empty() { "auto" } else { language };
    // Both the server and CLI paths below take the vocabulary from here, so the
    // script guard belongs here rather than at each of them.
    let vocabulary = prompt_for(lang, vocabulary);

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
        "-bs".into(),
        "1".into(), // greedy decoding — same speedup as the server path
        "-bo".into(),
        "1".into(), // best-of 1 candidate (default 2)
        "-fa".into(),
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

/// Whisper takes the custom vocabulary as an initial prompt, and the prompt's
/// script drags the decoder along with it. A Latin-script prompt against Bangla
/// speech returns romanised text — "pami bang lati kotaboli" instead of Bengali
/// — and once greedy decoding compounds that, pure noise. Measured, not
/// theorised: the same clip and model transcribe correctly the moment the
/// prompt is dropped.
///
/// So the prompt is only passed when it could plausibly belong to the target
/// language's script. An all-ASCII vocabulary is useless to a non-Latin
/// language anyway, which makes dropping it free.
fn prompt_for<'a>(language: &str, vocabulary: &'a str) -> &'a str {
    // Of the languages this app offers, the ones not written in Latin script.
    const NON_LATIN: [&str; 10] =
        ["zh", "hi", "ar", "bn", "ru", "ja", "ko", "th", "uk", "he"];
    let vocab = vocabulary.trim();
    if vocab.is_empty() {
        return "";
    }
    // `auto` is left alone: without a resolved language there is nothing to
    // compare against, and the prompt is what makes English dictation accurate.
    if NON_LATIN.contains(&language) && vocab.is_ascii() {
        return "";
    }
    vocab
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
    let ready = {
        let state = app.state::<crate::AppState>();
        let mut server = state.whisper_server.lock().map_err(|e| e.to_string())?;
        if !server.is_running(&key) {
            server.start(model_path, &key, threads, lang, vocabulary, use_gpu)?;
        }
        server.is_ready()
    };
    // First load of a big model takes a while; a warm server answers instantly.
    // A preloaded server might be running but still loading its model.
    let timeout = if !ready {
        std::time::Duration::from_secs(120)
    } else {
        std::time::Duration::from_secs(5)
    };
    super::server::wait_ready(timeout).await?;
    if !ready {
        if let Ok(mut server) = app.state::<crate::AppState>().whisper_server.lock() {
            server.mark_ready();
        }
    }
    super::server::transcribe(std::path::Path::new(audio_path)).await
}

/// whisper.cpp prints transcription lines; strip stray blank lines / markers,
/// and the ellipses Whisper hallucinates when it is handed a mid-dictation
/// pause (it was trained on subtitles, so near-silence becomes "..." / "…").
/// The chunked path drops these via `looks_hallucinated`; the normal push-to-
/// talk path lands here, so it needs the same guard or the dots reach the paste.
fn clean_output(raw: &str) -> String {
    let joined = raw
        .lines()
        .map(|l| l.trim().trim_start_matches("- ").trim())
        // Drop blank lines, whisper.cpp's `[markers]`, and any segment that is
        // nothing but punctuation/ellipsis — a pure pause rendered as dots.
        .filter(|l| !l.is_empty() && !l.starts_with('[') && !is_punct_only(l))
        .collect::<Vec<_>>()
        .join(" ");
    strip_wrapping_quotes(&strip_pause_ellipsis(&joined))
}

fn strip_wrapping_quotes(text: &str) -> String {
    let t = text.trim();
    let is_quote = |c: char| c == '"' || c == '\u{201c}' || c == '\u{201d}';
    if t.chars().filter(|&c| is_quote(c)).count() != 2 {
        return text.to_string();
    }
    let chars: Vec<char> = t.chars().collect();
    if chars.len() >= 2 && is_quote(chars[0]) && is_quote(chars[chars.len() - 1]) {
        return chars[1..chars.len() - 1].iter().collect::<String>().trim().to_string();
    }
    text.to_string()
}

/// True when every character is punctuation or whitespace (e.g. "...", "…",
/// "-", ". ,") — the shape of a pause Whisper hallucinated into a whole segment.
fn is_punct_only(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_punctuation() || c == '…' || c.is_whitespace())
}

/// Remove ellipses Whisper inserts mid-sentence on a pause (unicode "…" or a
/// run of 2+ ASCII dots), then tidy the spacing the removal leaves behind. A
/// single "." (a real sentence end) is untouched.
fn strip_pause_ellipsis(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '…' {
            out.push(' ');
        } else if c == '.' && chars.peek() == Some(&'.') {
            // Start of a "..".."..." run — consume every following dot.
            while chars.peek() == Some(&'.') {
                chars.next();
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    // Collapse the gaps left by removed ellipses, and pull punctuation back
    // against the previous word (" ," -> ",").
    let mut result = String::with_capacity(out.len());
    let mut prev_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            prev_space = true;
            continue;
        }
        if !result.is_empty() {
            if matches!(c, ',' | '.' | '!' | '?' | ';' | ':') {
                // no space before trailing punctuation
            } else if prev_space {
                result.push(' ');
            }
        }
        result.push(c);
        prev_space = false;
    }
    result
}

pub async fn preload(app: &AppHandle) -> Result<(), String> {
    let (model_id, language, vocabulary, use_gpu, high_performance, performance_threads, stt_source) = {
        let state = app.state::<crate::AppState>();
        let Ok(cfg) = state.config.lock() else {
            return Ok(());
        };
        (
            cfg.model_id.clone(),
            cfg.language.clone(),
            cfg.vocabulary.clone(),
            cfg.use_gpu,
            cfg.high_performance,
            cfg.performance_threads,
            cfg.stt_source.clone(),
        )
    };
    if stt_source != "local" {
        return Ok(());
    }

    let threads = crate::system::hotkey::resolve_thread_count(high_performance, performance_threads);
    let lang = if language.is_empty() { "auto" } else { &language };

    if registry::stt_engine(&model_id) == registry::SttEngine::Whisper {
        let model_path = registry::model_path(&model_id);
        if !model_path.exists() {
            return Ok(());
        }
        let key = format!("{model_id}|{lang}|{}|{use_gpu}|{threads}", vocabulary.trim());
        let ready = {
            let state = app.state::<crate::AppState>();
            let Ok(mut server) = state.whisper_server.lock() else {
                return Ok(());
            };
            if !server.is_running(&key) {
                let _ = server.start(&model_path, &key, threads, lang, &vocabulary, use_gpu);
            }
            server.is_ready()
        };
        if !ready
            && super::server::wait_ready(std::time::Duration::from_secs(120))
                .await
                .is_ok()
        {
            if let Ok(mut server) = app.state::<crate::AppState>().whisper_server.lock() {
                server.mark_ready();
            }
        }
    } else {
        if !registry::sherpa_stt_installed(&model_id) {
            return Ok(());
        }
        let app_clone = app.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = crate::system::sherpa_stt::ensure_engine(&app_clone, &model_id, threads);
        })
        .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The bug this guards: an ASCII vocabulary romanised Bangla dictation.
    #[test]
    fn ascii_prompt_is_dropped_for_non_latin_languages() {
        assert_eq!(prompt_for("bn", "claude,"), "");
        assert_eq!(prompt_for("hi", "Zaid, Tauri"), "");
        // English and other Latin-script languages still get their vocabulary.
        assert_eq!(prompt_for("en", "claude,"), "claude,");
        assert_eq!(prompt_for("de", "claude,"), "claude,");
        // Unresolved language: nothing to compare against, so keep it.
        assert_eq!(prompt_for("auto", "claude,"), "claude,");
        // A vocabulary actually written in the target script is what the prompt
        // is for, so it survives.
        assert_eq!(prompt_for("bn", "কথা"), "কথা");
        assert_eq!(prompt_for("bn", "   "), "");
    }

    #[test]
    fn strips_leading_dialogue_dashes() {
        assert_eq!(clean_output("- Hey there.\n- How are you?"), "Hey there. How are you?");
    }

    #[test]
    fn drops_standalone_ellipsis_segment() {
        assert_eq!(clean_output("Hello there\n...\nfriend"), "Hello there friend");
        assert_eq!(clean_output("…"), "");
        assert_eq!(clean_output("Okay.\n[BLANK_AUDIO]\n…"), "Okay.");
    }

    #[test]
    fn strips_inline_pause_ellipsis() {
        assert_eq!(
            clean_output("I've written… A sentence"),
            "I've written A sentence"
        );
        assert_eq!(clean_output("wait... what"), "wait what");
        assert_eq!(clean_output("trailing off…"), "trailing off");
    }

    #[test]
    fn keeps_real_periods_and_words() {
        assert_eq!(clean_output("Hello world."), "Hello world.");
        assert_eq!(clean_output("One. Two. Three."), "One. Two. Three.");
        assert_eq!(clean_output("It cost $3.50 today."), "It cost $3.50 today.");
    }

    #[test]
    fn strips_quotes_wrapping_whole_utterance() {
        assert_eq!(clean_output("\"Hello there.\""), "Hello there.");
        assert_eq!(clean_output("\"a\" and \"b\""), "\"a\" and \"b\"");
        assert_eq!(clean_output("\"Hey?\" Is it ok?"), "\"Hey?\" Is it ok?");
    }
}
