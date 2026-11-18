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

/// Applies replacements last, as desktop does. Longer phrases go first (also as
/// desktop does), so "my email work" is not cut short by a "my email" rule
/// listed before it.
#[uniffi::export]
pub fn apply_replacements(text: String, rules: Vec<Replacement>) -> String {
    let mut rules: Vec<(&str, &str)> = rules
        .iter()
        .map(|r| (r.from.trim(), r.to.as_str()))
        .filter(|(from, _)| !from.is_empty())
        .collect();
    rules.sort_by_key(|(from, _)| std::cmp::Reverse(from.chars().count())); // stable: ties keep list order
    let mut out = text;
    for (from, to) in rules {
        out = replace_whole(&out, from, to);
    }
    out
}

/// Replaces every whole-word, case-insensitive `from` in `text`. Matching walks
/// the original text one character at a time, so positions always land on real
/// character boundaries. (Searching a lowercased copy breaks when lowercasing
/// changes byte lengths: "İ" grows, the ohm sign shrinks, and slicing the
/// original at the copy's offsets panicked.)
fn replace_whole(text: &str, from: &str, to: &str) -> String {
    let needle: Vec<char> = from.chars().flat_map(char::to_lowercase).collect();
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if prev.map_or(true, |c| !c.is_alphanumeric()) {
            if let Some(len) = match_len(rest, &needle) {
                if rest[len..].chars().next().map_or(true, |c| !c.is_alphanumeric()) {
                    out.push_str(to);
                    prev = rest[..len].chars().last();
                    i += len;
                    continue;
                }
            }
        }
        let c = rest.chars().next().expect("i is on a char boundary below text.len()");
        out.push(c);
        prev = Some(c);
        i += c.len_utf8();
    }
    out
}

/// Byte length of the start of `rest` that equals `needle` (already lowercased),
/// ignoring case; None when it doesn't. A match must end on a whole character.
fn match_len(rest: &str, needle: &[char]) -> Option<usize> {
    let mut k = 0;
    for (at, c) in rest.char_indices() {
        if k == needle.len() {
            return Some(at);
        }
        for lc in c.to_lowercase() {
            if needle.get(k) != Some(&lc) {
                return None;
            }
            k += 1;
        }
    }
    (k == needle.len() && k > 0).then_some(rest.len())
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

    fn rule(from: &str, to: &str) -> Replacement {
        Replacement { from: from.into(), to: to.into() }
    }

    #[test]
    fn longer_phrase_wins_whatever_the_list_order() {
        let rules = vec![rule("my email", "home@x"), rule("my email work", "work@x")];
        assert_eq!(apply_replacements("send it to my email work".into(), rules.clone()), "send it to work@x");
        assert_eq!(apply_replacements("send it to my email".into(), rules), "send it to home@x");
    }

    #[test]
    fn letters_that_change_size_when_lowercased_do_not_panic() {
        // 'İ' is 2 bytes and 3 once lowercased; the ohm sign is 3 bytes and 2 once
        // lowercased. Together the lengths cancel out, which misled the old
        // byte-offset search into slicing inside a character.
        let rules = vec![rule("my email", "X")];
        assert_eq!(apply_replacements("\u{130}my email\u{2126}x".into(), rules.clone()), "\u{130}my email\u{2126}x");
        assert_eq!(apply_replacements("\u{130}\u{2126} my email".into(), rules.clone()), "\u{130}\u{2126} X");
        assert_eq!(apply_replacements("\u{130} My Email \u{2126}".into(), rules), "\u{130} X \u{2126}");
    }

    #[test]
    fn non_latin_text_and_shortcuts_work() {
        assert_eq!(apply_replacements("আমার my email".into(), vec![rule("my email", "X")]), "আমার X");
        assert_eq!(apply_replacements("ঠিকানা দাও".into(), vec![rule("ঠিকানা", "12 Road")]), "12 Road দাও");
        // Case-insensitive beyond ASCII too, which the old code skipped.
        assert_eq!(apply_replacements("Ask ZOË now".into(), vec![rule("zoë", "Zoë")]), "Ask Zoë now");
    }

    #[test]
    fn symbols_and_edges() {
        assert_eq!(apply_replacements("I write C++ daily".into(), vec![rule("c++", "C plus plus")]), "I write C plus plus daily");
        assert_eq!(apply_replacements("brb".into(), vec![rule("brb", "be right back")]), "be right back");
        assert_eq!(apply_replacements("brbx xbrb".into(), vec![rule("brb", "!")]), "brbx xbrb");
        assert_eq!(apply_replacements("".into(), vec![rule("a", "b")]), "");
        assert_eq!(apply_replacements("hi".into(), vec![rule("  ", "b")]), "hi");
        // Replacement text is not searched again within the same rule.
        assert_eq!(apply_replacements("ha".into(), vec![rule("ha", "ha ha")]), "ha ha");
    }
}
