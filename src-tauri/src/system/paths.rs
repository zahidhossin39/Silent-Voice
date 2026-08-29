use std::path::PathBuf;

/// Root that bundled sidecars and libraries live under.
///
/// Windows and Linux put bundle.resources next to the executable; macOS puts
/// the binary in Contents/MacOS and the resources in Contents/Resources. Dev
/// builds have everything beside the binary on every platform, so the macOS
/// branch falls back to the exe dir when Resources is not there.
pub fn bundled_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    #[cfg(target_os = "macos")]
    if exe_dir.ends_with("MacOS") {
        if let Some(res) = exe_dir.parent().map(|c| c.join("Resources")) {
            if res.is_dir() {
                return res;
            }
        }
    }
    exe_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_dir_not_empty() {
        let p = bundled_dir();
        assert!(!p.as_os_str().is_empty(), "bundled_dir should not be empty");
    }
}
