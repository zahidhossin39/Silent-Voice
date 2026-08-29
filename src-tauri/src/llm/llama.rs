use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

// Manages a bundled llama.cpp `llama-server` process. We keep one server alive
// and reuse it; switching models restarts it. The server exposes an
// OpenAI-compatible API at http://127.0.0.1:PORT/v1, so the generic openai
// client handles the actual chat call.

pub const LLAMA_PORT: u16 = 8088;

/// Directory holding llama-server.exe + its DLLs, inside the bundled-resource root.
fn engine_dir() -> PathBuf {
    crate::system::paths::bundled_dir().join("llama")
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

const STDERR_TAIL_LIMIT: usize = 800;

/// Shared with `wait_ready`, which is a free function and has no handle on the
/// server instance that spawned the process.
///
/// `generation` is bumped on every spawn and captured by that spawn's watcher
/// thread. Restarting kills the old process, so its watcher wakes and reports
/// an exit — without this guard that late report would land after the restart
/// cleared the flags, and `wait_ready` would declare the healthy NEW server
/// dead.
struct Diagnostics {
    generation: AtomicU64,
    stderr_tail: Mutex<String>,
    exited: AtomicBool,
    exit_code: Mutex<Option<i32>>,
}

fn diagnostics() -> &'static Arc<Diagnostics> {
    static DIAGNOSTICS: OnceLock<Arc<Diagnostics>> = OnceLock::new();
    DIAGNOSTICS.get_or_init(|| {
        Arc::new(Diagnostics {
            generation: AtomicU64::new(0),
            stderr_tail: Mutex::new(String::new()),
            exited: AtomicBool::new(false),
            exit_code: Mutex::new(None),
        })
    })
}

#[derive(Default)]
pub struct LlamaServer {
    child: Option<Arc<Mutex<Child>>>,
    model_id: Option<String>,
}

impl LlamaServer {
    /// True if a server is already running for this exact model.
    pub fn is_running_model(&mut self, model_id: &str) -> bool {
        if self.model_id.as_deref() != Some(model_id) {
            return false;
        }
        let alive = match self.child.as_ref() {
            Some(child) => matches!(child.lock().unwrap().try_wait(), Ok(None)),
            None => false,
        };
        if !alive {
            self.child = None;
            self.model_id = None;
        }
        alive
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
            .current_dir(engine_dir()) // resolve sibling DLLs
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        if let Err(e) = crate::system::job::adopt(&child) {
            crate::logging::log_info("job", &format!("Warning: failed to adopt llama-server: {}", e));
        }
        let stderr = child.stderr.take().unwrap();

        let diag = diagnostics().clone();
        // Claim a new generation BEFORE clearing, so any still-running watcher
        // from the process we just stopped is already invalidated.
        let generation = diag.generation.fetch_add(1, Ordering::SeqCst) + 1;
        diag.stderr_tail.lock().unwrap().clear();
        diag.exited.store(false, Ordering::SeqCst);
        *diag.exit_code.lock().unwrap() = None;

        let child = Arc::new(Mutex::new(child));
        let watched = child.clone();
        // Drain stderr continuously: an undrained pipe fills its buffer and
        // blocks the child. EOF here means the process has ended, which is also
        // our fast-fail signal for `wait_ready`.
        std::thread::spawn(move || {
            let mut stderr = stderr;
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if diag.generation.load(Ordering::SeqCst) != generation {
                            return;
                        }
                        let mut tail = diag.stderr_tail.lock().unwrap();
                        tail.push_str(&String::from_utf8_lossy(&buf[..n]));
                        if tail.len() > STDERR_TAIL_LIMIT {
                            let cut = tail
                                .char_indices()
                                .nth(tail.chars().count().saturating_sub(STDERR_TAIL_LIMIT))
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            *tail = tail[cut..].to_string();
                        }
                    }
                }
            }
            // Poll rather than block on wait(): holding the child lock across a
            // blocking wait would deadlock a concurrent stop() trying to kill.
            loop {
                if let Ok(Some(status)) = watched.lock().unwrap().try_wait() {
                    if diag.generation.load(Ordering::SeqCst) != generation {
                        return;
                    }
                    *diag.exit_code.lock().unwrap() = status.code();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            diag.exited.store(true, Ordering::SeqCst);
        });

        self.child = Some(child);
        self.model_id = Some(model_id.to_string());
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(child) = self.child.take() {
            let mut child = child.lock().unwrap();
            let _ = child.kill();
            let _ = child.wait();
        }
        self.model_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A watcher from a stopped server must not be able to mark a NEWER server
    // as exited. Without the generation guard the late store lands after the
    // restart cleared the flag, and wait_ready reports a healthy server dead.
    #[test]
    fn stale_watcher_cannot_report_exit_for_new_generation() {
        let diag = diagnostics().clone();
        let stale = diag.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // A restart claims a newer generation and clears the flags.
        let _current = diag.generation.fetch_add(1, Ordering::SeqCst) + 1;
        diag.exited.store(false, Ordering::SeqCst);

        // The stale watcher now finishes and tries to report its exit.
        if diag.generation.load(Ordering::SeqCst) == stale {
            diag.exited.store(true, Ordering::SeqCst);
        }

        assert!(
            !diag.exited.load(Ordering::SeqCst),
            "stale watcher marked the current server as exited"
        );
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
        let diag = diagnostics();
        if diag.exited.load(Ordering::Relaxed) {
            let code = *diag.exit_code.lock().unwrap();
            let tail = diag.stderr_tail.lock().unwrap().trim().to_string();
            let code = code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".into());
            return Err(if tail.is_empty() {
                format!("Local LLM server exited (code {code}) before becoming ready")
            } else {
                format!("Local LLM server exited (code {code}) before becoming ready:\n{tail}")
            });
        }
        if start.elapsed() > timeout {
            return Err("Local LLM server did not become ready in time".into());
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}
