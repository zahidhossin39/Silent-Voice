// Data shared between the inline-proofreading watchers and whatever draws the
// underlines. Windows draws them with Win32 layered windows (squiggle.rs);
// macOS draws them in a transparent Tauri window (squiggle_mac.rs). Both
// consume the same list, so the type lives here rather than inside either
// renderer.

pub(crate) const MAX_SQUIGGLES: usize = 64;

/// One flagged word occurrence on screen.
///
/// Units differ by platform, deliberately: Windows works in physical pixels
/// because GDI does, macOS works in points because both the Accessibility API
/// and Tauri's window positioning do. Neither renderer ever converts, so the
/// numbers are always already in the unit its own drawing code expects.
#[derive(Clone, PartialEq, serde::Serialize)]
pub struct SquiggleInfo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub spelling: bool,
    pub message: String,
    pub suggestions: Vec<String>,
    /// Char range + exact current text of the flagged span, so the fix can
    /// verify nothing changed before replacing.
    pub start: usize,
    pub end: usize,
    pub expected: String,
}

/// Overlay → watcher: actions representing either fixing a word, dismissing,
/// or adding to the vocabulary.
pub enum OverlayAction {
    Fix { start: usize, end: usize, expected: String, replacement: String },
    Dismiss { word: String },
    AddToVocab { word: String },
}

/// Apps where proofreading English prose is always wrong: code editors and
/// IDEs, terminals, and password managers.
///
/// Code is not prose. `console.log(calculateTotal(20,10))` gets flagged for a
/// missing "the", and every identifier looks like a spelling mistake — the
/// underlines are pure noise in an editor, and a "fix" applied there would
/// corrupt working source.
///
/// Matched as lowercase substrings against the process/app name, so one entry
/// covers `Code.exe`, `code`, and `Visual Studio Code` across platforms.
/// Entries must be distinctive enough not to catch unrelated apps — which is
/// why this says "sublime_text" and "studio64" rather than "text" or "studio".
pub const IGNORE_APP_SUBSTRINGS: &[&str] = &[
    // Editors and IDEs
    "code",          // VS Code, VSCodium, Code - Insiders
    "cursor",
    "antigravity",
    "windsurf",
    "zed",
    "devenv",        // Visual Studio
    "rider",
    "idea",          // IntelliJ IDEA
    "pycharm",
    "webstorm",
    "phpstorm",
    "rubymine",
    "clion",
    "goland",
    "datagrip",
    "androidstudio",
    "studio64",      // Android Studio launcher
    "xcode",
    "sublime_text",
    "notepad++",
    "geany",
    "eclipse",
    "netbeans",
    "neovim",
    "nvim",
    "gvim",
    "emacs",
    // Terminals — squiggling scrollback is noise
    "windowsterminal",
    "conhost",
    "cmd.exe",
    "powershell",
    "pwsh",
    "alacritty",
    "wezterm",
    "kitty",
    "gnome-terminal",
    "konsole",
    "iterm",
    "terminal",
    // Password managers are none of our business
    "keepass",
    "1password",
    "bitwarden",
    "dashlane",
    "lastpass",
];

/// True when the focused app is one we never proofread.
pub fn is_ignored_app(name: &str) -> bool {
    let lower = name.to_lowercase();
    IGNORE_APP_SUBSTRINGS.iter().any(|a| lower.contains(a))
}

#[cfg(test)]
mod tests {
    use super::is_ignored_app;

    #[test]
    fn ignores_editors_and_terminals_across_platforms() {
        // Windows exe names, macOS/Linux app names — same list covers both.
        for name in [
            "Code.exe", "code", "Visual Studio Code", "Cursor.exe",
            "Antigravity IDE", "idea64.exe", "pycharm64.exe", "nvim",
            "WindowsTerminal.exe", "gnome-terminal", "1Password.exe",
        ] {
            assert!(is_ignored_app(name), "{name} should be ignored");
        }
        // Ordinary places people actually write prose must stay proofread.
        for name in [
            "chrome.exe", "WhatsApp.exe", "Slack", "notion", "Mail",
            "winword.exe", "Obsidian.exe", "Discord.exe",
        ] {
            assert!(!is_ignored_app(name), "{name} should NOT be ignored");
        }
    }
}
