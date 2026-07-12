// Grammarly-style proofreading of transcriptions via Harper (Apache-2.0,
// pure Rust — compiles straight into the binary, no sidecar).
//
// Scope: English only, spelling + grammar + basic style. Offsets in the
// returned issues are CHAR indices (Harper's Span<char>), NOT UTF-16 units —
// the frontend must index with Array.from(text), not text.slice().
//
// The user's custom vocabulary (Settings → Custom vocabulary, same list that
// primes Whisper) doubles as the personal dictionary: those words are merged
// into Harper's dictionary so names/jargon are never flagged.

use harper_core::linting::{LintGroup, Linter, Suggestion};
use harper_core::spell::{FstDictionary, MergedDictionary, MutableDictionary};
use harper_core::{Dialect, DictWordMetadata, Document};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize, Clone)]
pub struct ProofIssue {
    /// Char-index range into the checked text (exclusive end).
    pub start: usize,
    pub end: usize,
    /// Human-readable problem description.
    pub message: String,
    /// Lint category (e.g. "Spelling", "Grammar") for possible filtering.
    pub kind: String,
    /// Replacement suggestions (best first), already rendered as strings.
    pub suggestions: Vec<String>,
}

/// Check `text`, treating each word of `vocabulary` (comma/newline separated)
/// as correctly spelled.
pub fn check(text: &str, vocabulary: &str) -> Vec<ProofIssue> {
    let mut dict = MergedDictionary::new();
    dict.add_dictionary(FstDictionary::curated());

    let custom: Vec<&str> = vocabulary
        .split([',', '\n'])
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    if !custom.is_empty() {
        let mut user = MutableDictionary::new();
        for w in custom {
            // Harper tokenizes on punctuation, so "whisper.cpp" is checked as
            // "whisper" + "cpp" — whitelist each sub-token too, plus a
            // lowercase variant so sentence position doesn't re-flag it.
            for part in w
                .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
                .filter(|p| !p.is_empty())
                .chain(std::iter::once(w))
            {
                user.append_word_str(part, DictWordMetadata::default());
                user.append_word_str(&part.to_lowercase(), DictWordMetadata::default());
            }
        }
        dict.add_dictionary(Arc::new(user));
    }

    let dict = Arc::new(dict);
    let doc = Document::new_plain_english(text, &*dict);
    let mut linter = LintGroup::new_curated(dict, Dialect::American);

    let mut issues: Vec<ProofIssue> = linter
        .lint(&doc)
        .into_iter()
        .map(|l| ProofIssue {
            start: l.span.start,
            end: l.span.end,
            message: l.message,
            kind: format!("{:?}", l.lint_kind),
            suggestions: l
                .suggestions
                .iter()
                .filter_map(|s| match s {
                    Suggestion::ReplaceWith(chars) => Some(chars.iter().collect::<String>()),
                    _ => None,
                })
                .take(3)
                .collect(),
        })
        .collect();
    issues.sort_by_key(|i| i.start);
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_misspelling_and_respects_vocabulary() {
        let issues = check("This is a mispelled word.", "");
        assert!(
            issues.iter().any(|i| i.kind.contains("Spell")),
            "expected a spelling issue, got: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );

        // The same "word" whitelisted via vocabulary must not be flagged.
        let issues = check("Tauri and whisper.cpp are neat.", "Tauri, whisper.cpp");
        assert!(
            !issues.iter().any(|i| i.kind.contains("Spell")),
            "vocabulary words were still flagged: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn clean_text_has_no_issues() {
        let issues = check("This sentence is perfectly fine.", "");
        assert!(issues.is_empty(), "unexpected issues: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>());
    }
}
