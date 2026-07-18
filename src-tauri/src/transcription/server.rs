use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

// Persistent whisper.cpp `whisper-server` process: keeps the STT model loaded
// between dictations, cutting the multi-second model reload out of every
// hotkey release. Mirrors llm/llama.rs. Any failure at any point falls back
// to the one-shot whisper-cli sidecar (whisper.rs), so dictation never breaks.

pub const WHISPER_PORT: u16 = 8090;

/// whisper-server.exe sits next to the app executable, beside the whisper
/// DLLs (bundle.resources maps sidecars/whisper-server.exe → app root).
fn server_exe() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default()
        .join("whisper-server.exe")
}

#[derive(Default)]
pub struct WhisperServer {
    child: Option<Child>,
    /// model|language|vocab|gpu|threads — any change requires a restart.
    key: Option<String>,
}

impl WhisperServer {
    pub fn is_running(&mut self, key: &str) -> bool {
        if self.key.as_deref() != Some(key) {
            return false;
        }
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => true,
                _ => {
                    self.child = None;
                    self.key = None;
                    false
                }
            },
            None => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        model_path: &Path,
        key: &str,
        threads: u32,
        language: &str,
        vocabulary: &str,
        use_gpu: bool,
    ) -> Result<(), String> {
        self.stop();

        let exe = server_exe();
        if !exe.exists() {
            return Err(format!("whisper-server not found at {}", exe.display()));
        }

        let mut cmd = Command::new(&exe);
        cmd.arg("-m")
            .arg(model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(WHISPER_PORT.to_string())
            .arg("-t")
            .arg(threads.to_string())
            .arg("-l")
            .arg(if language.is_empty() { "auto" } else { language })
            .current_dir(exe.parent().unwrap_or_else(|| Path::new(".")));
        let vocab = vocabulary.trim();
        if !vocab.is_empty() {
            cmd.arg("--prompt").arg(vocab);
        }
        // whisper-server has no -ng flag; hiding all Vulkan devices via an
        // invalid index forces the CPU backend (verified: "0 devices" + CPU
        // fallback, harmless warning on stderr).
        if !use_gpu {
            cmd.env("GGML_VK_VISIBLE_DEVICES", "-1");
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| e.to_string())?;
        self.child = Some(child);
        self.key = Some(key.to_string());
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.key = None;
    }
}

/// whisper-server binds its socket only AFTER the model has loaded, so any
/// HTTP response at all (even 404) means it is ready for /inference.
pub async fn wait_ready(timeout: Duration) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{WHISPER_PORT}/");
    let client = reqwest::Client::new();
    let start = Instant::now();
    loop {
        if client.get(&url).send().await.is_ok() {
            return Ok(());
        }
        if start.elapsed() > timeout {
            return Err("whisper-server did not become ready in time".into());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

/// POST the WAV to /inference and return the transcribed text.
pub async fn transcribe(audio_path: &Path) -> Result<String, String> {
    let bytes = tokio::fs::read(audio_path).await.map_err(|e| e.to_string())?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("response_format", "json");
    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{WHISPER_PORT}/inference"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("whisper-server inference failed: {}", resp.status()));
    }
    #[derive(serde::Deserialize)]
    struct Inference {
        text: String,
    }
    let out: Inference = resp.json().await.map_err(|e| e.to_string())?;
    Ok(out.text)
}
