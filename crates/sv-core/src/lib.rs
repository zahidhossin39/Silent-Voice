//! Silent Voice text logic shared with the mobile apps.
//!
//! Kotlin and Swift call this through UniFFI-generated bindings. The clean-up
//! steps and their order match the desktop pipeline (`system/hotkey.rs`).

#[allow(dead_code)]
mod textfmt;

uniffi::setup_scaffolding!();

/// Which clean-up steps to run. Each maps to one desktop step.
#[derive(uniffi::Record, Clone, Copy)]
pub struct CleanOptions {
    /// Drop "um", "uh" and similar.
    pub fillers: bool,
    /// "the the cat" -> "the cat".
    pub repeats: bool,
    /// "slash", "at sign" and similar spoken as symbols.
    pub symbols: bool,
    /// "new line", "new paragraph", spoken lists.
    pub structure: bool,
    /// "twenty five" -> "25".
    pub numbers: bool,
}

/// Steps that run on the raw transcript, before any AI rewrite.
#[uniffi::export]
pub fn clean_before_ai(text: String, opts: CleanOptions) -> String {
    let mut t = text;
    if opts.fillers {
        t = textfmt::strip_fillers(&t);
    }
    if opts.repeats {
        t = textfmt::collapse_repeated_words(&t);
    }
    if opts.symbols {
        t = textfmt::spoken_symbols(&t);
    }
    t
}

/// Steps that run last, on the raw or AI-rewritten text.
#[uniffi::export]
pub fn clean_after_ai(text: String, opts: CleanOptions) -> String {
    let mut t = text;
    if opts.structure {
        t = textfmt::structure_text(&t);
    }
    if opts.numbers {
        // Per line: format_numbers joins on spaces and would flatten line breaks.
        t = t.lines().map(textfmt::format_numbers).collect::<Vec<_>>().join("\n");
    }
    t
}

/// A "say this, get that" rule. Whole words, case-insensitive.
#[derive(uniffi::Record, Clone)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

/// Applies replacements last, as desktop does.
#[uniffi::export]
pub fn apply_replacements(text: String, rules: Vec<Replacement>) -> String {
    let mut out = text;
    for r in rules {
        let from = r.from.trim();
        if from.is_empty() {
            continue;
        }
        out = replace_whole(&out, from, &r.to);
    }
    out
}

fn replace_whole(text: &str, from: &str, to: &str) -> String {
    let lower = text.to_lowercase();
    let needle = from.to_lowercase();
    // Lowercasing can change byte lengths outside ASCII; then offsets would not line up.
    if lower.len() != text.len() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&needle) {
        let start = i + pos;
        let end = start + needle.len();
        let before_ok = text[..start].chars().last().map_or(true, |c| !c.is_alphanumeric());
        let after_ok = text[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        out.push_str(&text[i..start]);
        if before_ok && after_ok {
            out.push_str(to);
        } else {
            out.push_str(&text[start..end]);
        }
        i = end;
    }
    out.push_str(&text[i..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: CleanOptions = CleanOptions { fillers: true, repeats: true, symbols: true, structure: true, numbers: true };

    #[test]
    fn cleans_like_desktop() {
        let t = clean_before_ai("Um, I think we need need twenty five of them.".into(), ALL);
        let t = clean_after_ai(t, ALL);
        assert_eq!(t, "I think we need 25 of them.");
    }

    #[test]
    fn options_off_leave_text_alone() {
        let off = CleanOptions { fillers: false, repeats: false, symbols: false, structure: false, numbers: false };
        let s = "Um, twenty five the the";
        assert_eq!(clean_after_ai(clean_before_ai(s.into(), off), off), s);
    }

    #[test]
    fn replacements_match_whole_words() {
        let rules = vec![Replacement { from: "cloud code".into(), to: "Claude Code".into() }];
        assert_eq!(apply_replacements("I use Cloud Code daily".into(), rules.clone()), "I use Claude Code daily");
        assert_eq!(apply_replacements("icloud codex".into(), rules), "icloud codex");
    }
}
