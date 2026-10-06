use std::io::Write;
use std::path::PathBuf;

// Simple append-only error/info log so silent failures leave a trace.
// Lives next to the app's other data: %APPDATA%/SilentVoice/logs/silent-voice.log
// Rotated once past ~2 MB (renamed to .old, fresh file started).

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

fn log_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("SilentVoice")
        .join("logs")
}

fn log_path() -> PathBuf {
    log_dir().join("silent-voice.log")
}

fn timestamp() -> String {
    // Seconds since epoch — no chrono dependency; grep-friendly and sortable.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn write_line(level: &str, context: &str, msg: &str) {
    let dir = log_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return; // never let logging failures affect the app
    }
    let path = log_path();

    // Rotate when the file gets big.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::rename(&path, dir.join("silent-voice.old.log"));
        }
    }

    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        // One line per event; strip newlines from the message so the file stays line-oriented.
        let clean = msg.replace(['\n', '\r'], " ");
        let _ = writeln!(f, "{} [{}] {}: {}", timestamp(), level, context, clean);
    }
}

/// Record an error with a short context tag, e.g. `log_error("stt", &e)`.
pub fn log_error(context: &str, msg: &str) {
    write_line("ERROR", context, msg);
}

/// Record a notable non-error event (app start, config milestones).
pub fn log_info(context: &str, msg: &str) {
    write_line("INFO", context, msg);
}

/// The last `max_lines` lines of the current log file, for the diagnostics
/// export. Empty if the log doesn't exist yet.
pub fn recent(max_lines: usize) -> String {
    let content = std::fs::read_to_string(log_path()).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

/// Write `header` plus the full log (previous rotated file first) to
/// Downloads/silent-voice-logs-<time>.txt, the file a user sends with a bug
/// report. Returns its path.
pub fn export(header: &str) -> Result<PathBuf, String> {
    let dir = dirs::download_dir()
        .or_else(dirs::desktop_dir)
        .ok_or("no Downloads folder found")?;
    let path = dir.join(format!("silent-voice-logs-{}.txt", timestamp()));
    let old = std::fs::read_to_string(log_dir().join("silent-voice.old.log")).unwrap_or_default();
    let cur = std::fs::read_to_string(log_path()).unwrap_or_default();
    std::fs::write(&path, format!("{header}\n--- full log ---\n{old}{cur}"))
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

/// Show `path` in the system file manager (selected, where supported).
pub fn reveal(path: &std::path::Path) {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open")
        .arg(if path.is_dir() { path } else { path.parent().unwrap_or(path) })
        .spawn();
    if let Err(e) = r {
        log_error("logs", &format!("could not open file manager: {e}"));
    }
}

pub fn logs_folder() -> PathBuf {
    log_dir()
}

/// Human-readable message from a caught panic payload.
pub fn panic_msg(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
