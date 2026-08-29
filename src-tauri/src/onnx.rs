use ort::init_from;

/// Reuse sherpa's onnxruntime.dll and pass an absolute path (see system/sherpa.rs for why);
/// ort PANICS if the dylib cannot be loaded so existence is verified first, and a panic here
/// would kill the inline-check watcher thread; test executables live in target\debug\deps\ which
/// is one level below the DLL directory, which is why the exe's parent directory is also probed.
///
/// Note that ort's environment is a process-wide OnceLock, so calling this from several modules
/// is harmless — only the first call actually initialises it.
pub fn ensure_runtime() -> Option<()> {
    let exe_dir = crate::system::paths::bundled_dir();
    let dll_path = [Some(exe_dir.as_path()), exe_dir.parent()]
        .into_iter()
        .flatten()
        .map(|d| d.join("sherpa").join("onnxruntime.dll"))
        .find(|p| p.exists())?;
    let _ = init_from(dll_path.display().to_string()).commit();
    Some(())
}
