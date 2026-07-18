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

const FILLER_WORDS: &[&str] = &["um", "umm", "uh", "uhh", "erm"];

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
/// as correctly spelled. `disabled_rules` holds Harper rule ids the user
/// turned off in Settings (unknown ids are ignored by Harper).
pub fn check(text: &str, vocabulary: &str, disabled_rules: &[String]) -> Vec<ProofIssue> {
    let mut dict = MergedDictionary::new();
    dict.add_dictionary(FstDictionary::curated());

    let custom: Vec<&str> = vocabulary
        .split([',', '\n'])
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    let custom_lower: std::collections::HashSet<String> = custom.iter().map(|w| w.to_lowercase()).collect();
    if !custom.is_empty() {
        let mut user = MutableDictionary::new();
        for &w in &custom {
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
    linter.config.set_rule_enabled("LongSentences", false);
    for rule in disabled_rules {
        linter.config.set_rule_enabled(rule, false);
    }

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
                    Suggestion::InsertAfter(chars) => {
                        let mut s: String = text.chars().skip(l.span.start).take(l.span.end - l.span.start).collect();
                        s.extend(chars.iter());
                        Some(s)
                    }
                    Suggestion::Remove => Some(String::new()),
                })
                .take(3)
                .collect(),
        })
        .collect();
    // Scan text for consecutive duplicate words
    let chars: Vec<char> = text.chars().collect();
    struct Word {
        start: usize,
        end: usize,
        text: String,
    }
    let mut words = Vec::new();
    let mut in_word = false;
    let mut word_start = 0;
    for (idx, &c) in chars.iter().enumerate() {
        let is_word_char = c.is_alphanumeric() || c == '\'' || c == '-';
        if is_word_char {
            if !in_word {
                word_start = idx;
                in_word = true;
            }
        } else {
            if in_word {
                let word_text: String = chars[word_start..idx].iter().collect();
                words.push(Word {
                    start: word_start,
                    end: idx,
                    text: word_text,
                });
                in_word = false;
            }
        }
    }
    if in_word {
        let word_text: String = chars[word_start..chars.len()].iter().collect();
        words.push(Word {
            start: word_start,
            end: chars.len(),
            text: word_text,
        });
    }

    // Find duplicate pairs
    for i in 0..words.len().saturating_sub(1) {
        let w1 = &words[i];
        let w2 = &words[i + 1];
        if w1.text.to_lowercase() == w2.text.to_lowercase() {
            // Check if they are separated only by whitespace (spaces, tabs, newlines)
            let sep_slice = &chars[w1.end..w2.start];
            if !sep_slice.is_empty() && sep_slice.iter().all(|&c| c == ' ' || c == '\t' || c == '\n' || c == '\r') {
                let start = w1.start;
                let end = w2.end;
                // Skip adding it if any existing Harper issue already overlaps that exact range (avoid duplicates)
                let overlaps = issues.iter().any(|hi| hi.start < end && start < hi.end);
                if !overlaps {
                    issues.push(ProofIssue {
                        start,
                        end,
                        message: format!("Repeated word: '{}'", w1.text),
                        kind: "Repetition".to_string(),
                        suggestions: vec![w1.text.clone()],
                    });
                }
            }
        }
    }

    // --- GECToR Integration ---
    if !disabled_rules.iter().any(|r| r == "Gector") {
        let gector_edits = crate::gector::check(text);
        for edit in gector_edits {
            let mut start = edit.start;
            let end = edit.end;
            let mut suggestion = edit.replacement.clone();
            
            if start == end && edit.tag.starts_with("$APPEND_") {
                if let Some(prev_w) = words.iter().rev().find(|w| w.end <= start) {
                    start = prev_w.start;
                    suggestion = format!("{}{}", prev_w.text, suggestion);
                }
            }

            let overlaps = issues.iter().any(|hi| hi.start < end && start < hi.end);
            if !overlaps {
                let formatted_message = if edit.tag.starts_with("$REPLACE_") || edit.tag.starts_with("$TRANSFORM_") {
                    let original: String = chars[edit.start..edit.end].iter().collect();
                    format!("{}: '{}' -> '{}'", edit.message, original, edit.replacement)
                } else {
                    edit.message.clone()
                };

                issues.push(ProofIssue {
                    start,
                    end,
                    message: formatted_message,
                    kind: "Context".to_string(),
                    suggestions: vec![suggestion],
                });
            }
        }
    }
    // -------------------------

    // "Filler" is our own pseudo-rule id, honoring the same Settings toggles
    // as Harper's rules.
    let filler_enabled = !disabled_rules.iter().any(|r| r == "Filler");
    for word in &words {
        if !filler_enabled {
            break;
        }
        let text_lower = word.text.to_lowercase();
        if FILLER_WORDS.contains(&text_lower.as_str()) && !custom_lower.contains(&text_lower) {
            let mut start = word.start;
            let mut end = word.end;
            
            if end < chars.len() && chars[end] == ' ' {
                end += 1;
            } else if start > 0 && chars[start - 1] == ' ' {
                start -= 1;
            }
            
            // A filler beats whatever Harper said about the same span (e.g.
            // "capitalize um" at sentence start) — removing it is the fix.
            issues.retain(|hi| !(hi.start < end && start < hi.end));
            issues.push(ProofIssue {
                start,
                end,
                message: format!("Filler word: '{}'", word.text),
                kind: "Filler".to_string(),
                suggestions: vec![String::new()],
            });
        }
    }

    issues.sort_by_key(|i| i.start);
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_misspelling_and_respects_vocabulary() {
        let issues = check("This is a mispelled word.", "", &[]);
        assert!(
            issues.iter().any(|i| i.kind.contains("Spell")),
            "expected a spelling issue, got: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );

        // The same "word" whitelisted via vocabulary must not be flagged.
        let issues = check("Tauri and whisper.cpp are neat.", "Tauri, whisper.cpp", &[]);
        assert!(
            !issues.iter().any(|i| i.kind.contains("Spell")),
            "vocabulary words were still flagged: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn clean_text_has_no_issues() {
        let issues = check("This sentence is perfectly fine.", "", &[]);
        assert!(issues.is_empty(), "unexpected issues: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>());
    }

    #[test]
    fn test_repeated_word_the_the() {
        let issues = check("the the cat sat", "", &[]);
        let rep_issues: Vec<_> = issues.iter().filter(|i| i.kind == "Repetition").collect();
        assert_eq!(rep_issues.len(), 1);
        assert_eq!(rep_issues[0].start, 0);
        assert_eq!(rep_issues[0].end, 7);
        assert!(rep_issues[0].suggestions.contains(&"the".to_string()));
    }

    #[test]
    fn test_repeated_word_popup() {
        let issues = check("my pop-up pop-up window", "", &[]);
        let rep_issues: Vec<_> = issues.iter().filter(|i| i.kind == "Repetition").collect();
        assert_eq!(rep_issues.len(), 1);
        assert_eq!(rep_issues[0].start, 3);
        assert_eq!(rep_issues[0].end, 16);
        assert_eq!(rep_issues[0].message, "Repeated word: 'pop-up'");
        assert_eq!(rep_issues[0].suggestions, vec!["pop-up".to_string()]);
    }

    #[test]
    fn test_repeated_word_clean() {
        let issues = check("this sentence is clean", "", &[]);
        let rep_issues: Vec<_> = issues.iter().filter(|i| i.kind == "Repetition").collect();
        assert!(rep_issues.is_empty());
    }

    #[test]
    fn test_repeated_word_case_insensitive() {
        let issues = check("The the cat sat", "", &[]);
        let rep_issues: Vec<_> = issues.iter().filter(|i| i.kind == "Repetition").collect();
        assert_eq!(rep_issues.len(), 1);
        assert_eq!(rep_issues[0].start, 0);
        assert_eq!(rep_issues[0].end, 7);
        assert!(
            rep_issues[0].suggestions.contains(&"The".to_string())
                || rep_issues[0].suggestions.contains(&"the".to_string())
        );
    }

    #[test]
    fn long_sentences_do_not_produce_lint() {
        // A sentence with more than 50 words should not produce a "sentence is X words long" lint.
        let long_sentence = "This is a very long sentence that has a lot of words to ensure that it exceeds the default long sentence threshold of fifty words which would normally trigger the long sentence style lint from harper core but since we disabled it there should be no issues at all in this text.";
        let issues = check(long_sentence, "", &[]);
        let long_sentence_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.message.contains("words long"))
            .collect();
        assert!(
            long_sentence_issues.is_empty(),
            "expected no long sentence lint, got: {:?}",
            long_sentence_issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_insertion_suggestion() {
        // The Oxford comma lint uses InsertAfter.
        let issues = check("I like apples, oranges and bananas.", "", &[]);
        let comma_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.message.contains("Oxford comma"))
            .collect();
        
        assert!(!comma_issues.is_empty(), "expected an Oxford comma lint");
        assert!(!comma_issues[0].suggestions.is_empty(), "expected a suggestion for the insertion lint");
        // The span should be around 'oranges', so inserting ',' makes it 'oranges,'
        assert!(comma_issues[0].suggestions.iter().any(|s| s.contains(",")));
    }

    #[test]
    fn disabled_rules_are_respected() {
        // Same text as test_insertion_suggestion, but with the Oxford comma
        // rule disabled — the lint must disappear. This also pins the rule id:
        // if Harper renames "OxfordComma", this test fails loudly.
        let issues = check(
            "I like apples, oranges and bananas.",
            "",
            &["OxfordComma".to_string()],
        );
        assert!(
            !issues.iter().any(|i| i.message.contains("Oxford comma")),
            "OxfordComma rule was disabled but still fired"
        );
    }

    #[test]
    fn test_filler_words_trailing_space() {
        let issues = check("um I think we should go", "", &[]);
        let filler_issues: Vec<_> = issues.iter().filter(|i| i.kind == "Filler").collect();
        assert_eq!(filler_issues.len(), 1);
        assert_eq!(filler_issues[0].start, 0);
        assert_eq!(filler_issues[0].end, 3);
        assert_eq!(filler_issues[0].suggestions, vec!["".to_string()]);
    }

    #[test]
    fn test_filler_words_substring_safety() {
        let issues = check("the drum is loud", "", &[]);
        assert!(!issues.iter().any(|i| i.kind == "Filler"));
        
        let issues2 = check("give him a hug", "", &[]);
        assert!(!issues2.iter().any(|i| i.kind == "Filler"));
    }

    #[test]
    fn test_filler_words_whitelisted() {
        let issues = check("um", "um", &[]);
        assert!(!issues.iter().any(|i| i.kind == "Filler"));
    }

    #[test]
    fn test_filler_words_case_and_punctuation() {
        let issues = check("Um, let me think", "", &[]);
        let filler_issues: Vec<_> = issues.iter().filter(|i| i.kind == "Filler").collect();
        assert_eq!(filler_issues.len(), 1);
        assert_eq!(filler_issues[0].start, 0);
        assert_eq!(filler_issues[0].end, 2);
    }
}
