use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

// Manages a bundled llama.cpp `llama-server` process. We keep one server alive
// and reuse it; switching models restarts it. The server exposes an
// OpenAI-compatible API at http://127.0.0.1:PORT/v1, so the generic openai
// client handles the actual chat call.

pub const LLAMA_PORT: u16 = 8088;

/// Directory holding llama-server.exe + its DLLs (next to the app executable).
fn engine_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default()
        .join("llama")
}

fn server_exe() -> PathBuf {
    let name = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    engine_dir().join(name)
}

pub fn base_url() -> String {
    format!("http://127.0.0.1:{LLAMA_PORT}/v1")
}

#[derive(Default)]
pub struct LlamaServer {
    child: Option<Child>,
    model_id: Option<String>,
}

impl LlamaServer {
    /// True if a server is already running for this exact model.
    pub fn is_running_model(&mut self, model_id: &str) -> bool {
        if self.model_id.as_deref() != Some(model_id) {
            return false;
        }
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => true, // still alive
                _ => {
                    self.child = None;
                    self.model_id = None;
                    false
                }
            },
            None => false,
        }
    }

    /// (Re)start llama-server with the given model. Returns immediately; call
    /// `wait_ready` afterwards to block until the model has loaded.
    pub fn start(
        &mut self,
        model_path: &Path,
        model_id: &str,
        threads: u32,
    ) -> Result<(), String> {
        self.stop();

        let exe = server_exe();
        if !exe.exists() {
            return Err(format!(
                "Local LLM engine not found at {}. Reinstall the app.",
                exe.display()
            ));
        }
        if !model_path.exists() {
            return Err(format!("Model '{model_id}' is not downloaded"));
        }

        let mut cmd = Command::new(&exe);
        cmd.arg("-m")
            .arg(model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(LLAMA_PORT.to_string())
            .arg("-c")
            .arg("4096")
            .arg("-t")
            .arg(threads.to_string())
            .current_dir(engine_dir()); // resolve sibling DLLs

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| e.to_string())?;
        self.child = Some(child);
        self.model_id = Some(model_id.to_string());
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.model_id = None;
    }
}

/// Poll the server's /health endpoint until the model is ready (or timeout).
pub async fn wait_ready(timeout: Duration) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{LLAMA_PORT}/health");
    let client = reqwest::Client::new();
    let start = Instant::now();
    loop {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        if start.elapsed() > timeout {
            return Err("Local LLM server did not become ready in time".into());
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}
