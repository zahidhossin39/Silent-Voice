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
