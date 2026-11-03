use std::path::{Path, PathBuf};

/// Root that bundled sidecars and libraries live under.
///
/// Windows puts bundle.resources next to the executable. macOS puts the binary
/// in Contents/MacOS and the resources in Contents/Resources. Linux (.deb, and
/// the AppImage's usr/ tree) puts the binary in usr/bin and the resources in
/// usr/lib/<productName>. Dev builds have everything beside the binary on every
/// platform, so each branch falls back to the exe dir when its folder is not there.
pub fn bundled_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    resources_beside(&exe_dir)
}

fn resources_beside(exe_dir: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    if exe_dir.ends_with("MacOS") {
        if let Some(res) = exe_dir.parent().map(|c| c.join("Resources")) {
            if res.is_dir() {
                return res;
            }
        }
    }
    #[cfg(target_os = "linux")]
    if exe_dir.ends_with("bin") {
        // Must match productName in tauri.conf.json.
        if let Some(res) = exe_dir.parent().map(|u| u.join("lib").join("Silent Voice")) {
            if res.is_dir() {
                return res;
            }
        }
    }
    exe_dir.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_dir_not_empty() {
        let p = bundled_dir();
        assert!(!p.as_os_str().is_empty(), "bundled_dir should not be empty");
    }

    #[test]
    fn dev_build_uses_the_exe_dir() {
        let t = std::env::temp_dir().join("sv-paths-dev/target/debug");
        std::fs::create_dir_all(&t).unwrap();
        assert_eq!(resources_beside(&t), t);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn installed_linux_build_uses_usr_lib() {
        let usr = std::env::temp_dir().join("sv-paths-linux/usr");
        let lib = usr.join("lib/Silent Voice");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::create_dir_all(usr.join("bin")).unwrap();
        assert_eq!(resources_beside(&usr.join("bin")), lib);
    }
}
