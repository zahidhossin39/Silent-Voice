// Smart number formatting for dictated text.
//
// Product decision: every spelled-out number becomes digits, including
// zero–nine ("one" → "1", "three cats" → "3 cats"). Additional rules:
//   • multi-word numbers combine           → "twenty five" → "25"
//   • percent merges                       → "five percent" → "5%"
//   • years as digits                      → "twenty twenty six" → "2026"
//   • decimals as digits                   → "three point five" → "3.5"
//
// Digit tokens are never rewritten, and adjacent spelled digits stay separate
// numbers ("one two three" → "1 2 3", not "123"). Known trade-off, accepted:
// idioms like "no one knows" become "no 1 knows".

use std::collections::HashMap;

fn units_map() -> HashMap<&'static str, u64> {
    HashMap::from([
        ("zero", 0), ("one", 1), ("two", 2), ("three", 3), ("four", 4),
        ("five", 5), ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9),
        ("ten", 10), ("eleven", 11), ("twelve", 12), ("thirteen", 13),
        ("fourteen", 14), ("fifteen", 15), ("sixteen", 16), ("seventeen", 17),
        ("eighteen", 18), ("nineteen", 19),
    ])
}

fn tens_map() -> HashMap<&'static str, u64> {
    HashMap::from([
        ("twenty", 20), ("thirty", 30), ("forty", 40), ("fifty", 50),
        ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90),
    ])
}

/// One whitespace-delimited source token, split into punctuation shell + core.
struct Tok {
    lead: String,  // leading punctuation, e.g. "("
    core: String,  // the word itself
    trail: String, // trailing punctuation, e.g. ","
}

fn split_token(raw: &str) -> Tok {
    let lead_end = raw
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(i, _)| i)
        .unwrap_or(raw.len());
    let trail_start = raw
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(lead_end);
    Tok {
        lead: raw[..lead_end].to_string(),
        core: raw[lead_end..trail_start].to_string(),
        trail: raw[trail_start..].to_string(),
    }
}

/// Expand "twenty-five" into ["twenty","five"]; leave other words as one part.
fn number_parts(core: &str) -> Vec<String> {
    core.split('-').map(|p| p.to_lowercase()).collect()
}

fn is_number_word(w: &str, units: &HashMap<&str, u64>, tens: &HashMap<&str, u64>) -> bool {
    units.contains_key(w) || tens.contains_key(w) || matches!(w, "hundred" | "thousand" | "million")
}

/// Greedy parse of a spelled-out integer starting at `i` over lowercase words.
/// Returns (value, words_consumed). Stops before anything that would make the
/// sequence invalid English number grammar (so "five three" parses as two
/// separate numbers, not 53).
fn parse_int(words: &[String], i: usize, units: &HashMap<&str, u64>, tens: &HashMap<&str, u64>) -> Option<(u64, usize)> {
    let mut total: u64 = 0;
    let mut current: u64 = 0;
    let mut consumed = 0;
    let mut last_unit_val: Option<u64> = None; // guards "five three"
    let mut j = i;

    while j < words.len() {
        let w = words[j].as_str();
        if let Some(&v) = tens.get(w) {
            if last_unit_val.is_some() {
                break; // "five twenty" → stop after "five"
            }
            current += v;
            consumed = j - i + 1;
            j += 1;
            // A tens word may be followed by a unit digit ("twenty five").
            if j < words.len() {
                if let Some(&uv) = units.get(words[j].as_str()) {
                    if uv < 10 {
                        current += uv;
                        consumed = j - i + 1;
                        j += 1;
                    }
                }
            }
            last_unit_val = Some(current);
        } else if let Some(&v) = units.get(w) {
            if last_unit_val.is_some() {
                break; // "five three" → two separate numbers
            }
            current += v;
            last_unit_val = Some(v);
            consumed = j - i + 1;
            j += 1;
        } else if w == "hundred" {
            if current == 0 {
                break;
            }
            current *= 100;
            last_unit_val = None;
            consumed = j - i + 1;
            j += 1;
            // allow "one hundred and five"
            if j < words.len() && words[j] == "and" && j + 1 < words.len()
                && is_number_word(&words[j + 1], units, tens) && words[j + 1] != "hundred"
            {
                j += 1; // skip "and" (not counted as consumed unless number follows — it does)
                consumed = j - i;
            }
        } else if w == "thousand" || w == "million" {
            if current == 0 {
                break;
            }
            let scale = if w == "thousand" { 1_000 } else { 1_000_000 };
            total += current * scale;
            current = 0;
            last_unit_val = None;
            consumed = j - i + 1;
            j += 1;
        } else {
            break;
        }
    }

    if consumed == 0 {
        None
    } else {
        Some((total + current, consumed))
    }
}

/// Detect a spoken year like "twenty twenty six" / "nineteen ninety nine".
/// Returns (year, words_consumed) for values 1900–2099 only.
fn parse_year(words: &[String], i: usize, units: &HashMap<&str, u64>, tens: &HashMap<&str, u64>) -> Option<(u64, usize)> {
    let first = words.get(i)?;
    let century = match first.as_str() {
        "nineteen" => 19u64,
        "twenty" => 20u64,
        _ => return None,
    };
    // Parse the remainder as 0–99 from the following one or two words.
    let (rest, used) = {
        let w1 = words.get(i + 1)?;
        if let Some(&t) = tens.get(w1.as_str()) {
            if let Some(w2) = words.get(i + 2) {
                if let Some(&u) = units.get(w2.as_str()) {
                    if u < 10 {
                        (t + u, 2)
                    } else {
                        (t, 1)
                    }
                } else {
                    (t, 1)
                }
            } else {
                (t, 1)
            }
        } else if let Some(&u) = units.get(w1.as_str()) {
            // "twenty eleven" (2011) — teens allowed; bare digits ("twenty five")
            // are ambiguous with the number 25, so only accept 10–19 here.
            if (10..20).contains(&u) {
                (u, 1)
            } else {
                return None;
            }
        } else {
            return None;
        }
    };
    let year = century * 100 + rest;
    if (1900..=2099).contains(&year) {
        Some((year, 1 + used))
    } else {
        None
    }
}

/// Apply smart number formatting to a final transcript.
pub fn format_numbers(text: &str) -> String {
    let units = units_map();
    let tens = tens_map();

    let raw_tokens: Vec<&str> = text.split_whitespace().collect();
    if raw_tokens.is_empty() {
        return text.to_string();
    }

    // Flatten into word list, remembering which raw token each word came from.
    // A hyphenated token only expands when every part is a number word
    // ("twenty-five" → ["twenty","five"]); otherwise it stays one word — the
    // non-number fallback below emits the whole token once per word, so
    // expanding "post-writing" would paste it twice.
    let toks: Vec<Tok> = raw_tokens.iter().map(|r| split_token(r)).collect();
    let mut words: Vec<String> = Vec::new();
    let mut word_tok: Vec<usize> = Vec::new(); // word index → token index
    for (ti, t) in toks.iter().enumerate() {
        let parts = number_parts(&t.core);
        if parts.len() > 1 && parts.iter().all(|p| is_number_word(p, &units, &tens)) {
            for p in parts {
                words.push(p);
                word_tok.push(ti);
            }
        } else {
            words.push(t.core.to_lowercase());
            word_tok.push(ti);
        }
    }

    let mut out: Vec<String> = Vec::new();
    let mut wi = 0; // word index

    while wi < words.len() {
        let ti = word_tok[wi];
        let tok = &toks[ti];

        // Try year first (most specific), then general integer.
        let year = parse_year(&words, wi, &units, &tens);
        let num = parse_int(&words, wi, &units, &tens);

        let (value, consumed, is_year) = match (year, num) {
            (Some((y, yc)), Some((_, nc))) if yc >= nc => (y, yc, true),
            (_, Some((v, nc))) => (v, nc, false),
            (Some((y, yc)), None) => (y, yc, true),
            (None, None) => {
                out.push(format!("{}{}{}", tok.lead, tok.core, tok.trail));
                wi += 1;
                continue;
            }
        };

        // Optional decimal tail: "<int> point <digit> <digit>…"
        let mut decimal_digits = String::new();
        let mut dec_consumed = 0;
        if !is_year {
            let mut k = wi + consumed;
            if words.get(k).map(|w| w == "point").unwrap_or(false) {
                let mut digits = String::new();
                let mut kk = k + 1;
                while let Some(w) = words.get(kk) {
                    match units.get(w.as_str()) {
                        Some(&d) if d < 10 => {
                            digits.push_str(&d.to_string());
                            kk += 1;
                        }
                        _ => break,
                    }
                }
                if !digits.is_empty() {
                    decimal_digits = digits;
                    k = kk;
                    dec_consumed = k - (wi + consumed);
                }
            }
        }
        let total_consumed = consumed + dec_consumed;
        let end_wi = wi + total_consumed;

        // Next word (for the "N percent" → "N%" merge).
        let next_word = words.get(end_wi).map(|s| s.as_str());

        // Every spelled-out number becomes digits — "one" → "1", "twenty
        // five" → "25". (Per product decision: dictated numbers should always
        // paste as digits, even below ten.)
        let last_tok = &toks[word_tok[end_wi - 1]];
        let mut rendered = value.to_string();
        if !decimal_digits.is_empty() {
            rendered = format!("{rendered}.{decimal_digits}");
        }

        // "25 percent" → "25%": swallow the following "percent" word.
        let mut extra_consumed = 0;
        if next_word == Some("percent") {
            rendered.push('%');
            extra_consumed = 1;
        }

        let trail_tok = if extra_consumed > 0 {
            &toks[word_tok[end_wi + extra_consumed - 1]]
        } else {
            last_tok
        };
        out.push(format!("{}{}{}", tok.lead, rendered, trail_tok.trail));
        wi = end_wi + extra_consumed;
    }

    out.join(" ")
}

#[derive(Debug, Clone, PartialEq)]
enum RepeatToken {
    Word(String),
    Whitespace(String),
    Other(String),
}

fn is_potential_word_char(c: char) -> bool {
    c.is_alphabetic() || c == '-' || c == '\'' || c == '’'
}

fn is_collapsible_word(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        return false;
    }
    // Starts and ends with a letter.
    if !chars[0].is_alphabetic() || !chars[chars.len() - 1].is_alphabetic() {
        return false;
    }
    // Contains only letters, hyphens, and apostrophes.
    for &c in &chars {
        if !c.is_alphabetic() && c != '-' && c != '\'' && c != '’' {
            return false;
        }
    }
    true
}

fn tokenize_repeated(text: &str) -> Vec<RepeatToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if is_potential_word_char(c) {
            let start = i;
            while i < chars.len() && is_potential_word_char(chars[i]) {
                i += 1;
            }
            let word_str: String = chars[start..i].iter().collect();
            if is_collapsible_word(&word_str) {
                tokens.push(RepeatToken::Word(word_str));
            } else {
                tokens.push(RepeatToken::Other(word_str));
            }
        } else if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let ws_str: String = chars[start..i].iter().collect();
            tokens.push(RepeatToken::Whitespace(ws_str));
        } else {
            let start = i;
            while i < chars.len() && !is_potential_word_char(chars[i]) && !chars[i].is_whitespace() {
                i += 1;
            }
            let other_str: String = chars[start..i].iter().collect();
            tokens.push(RepeatToken::Other(other_str));
        }
    }
    tokens
}

/// Collapse immediate consecutive duplicate words.
/// Only collapses when the word is >= 2 chars and consists of letters (plus internal hyphens/apostrophes).
/// Preserves casing of the first occurrence and spacing.
pub fn collapse_repeated_words(text: &str) -> String {
    let tokens = tokenize_repeated(text);
    let mut result: Vec<RepeatToken> = Vec::new();

    for token in tokens {
        match token {
            RepeatToken::Word(ref w) => {
                let mut last_word_idx = None;
                let mut only_whitespace = true;
                for (idx, t) in result.iter().enumerate().rev() {
                    match t {
                        RepeatToken::Word(_) => {
                            last_word_idx = Some(idx);
                            break;
                        }
                        RepeatToken::Whitespace(_) => {}
                        RepeatToken::Other(_) => {
                            only_whitespace = false;
                            break;
                        }
                    }
                }

                let mut is_duplicate = false;
                if let Some(idx) = last_word_idx {
                    if only_whitespace {
                        if let RepeatToken::Word(ref last_w) = result[idx] {
                            if last_w.to_lowercase() == w.to_lowercase() {
                                is_duplicate = true;
                                result.truncate(idx + 1);
                            }
                        }
                    }
                }

                if !is_duplicate {
                    result.push(token);
                }
            }
            _ => {
                result.push(token);
            }
        }
    }

    let mut out = String::new();
    for t in result {
        match t {
            RepeatToken::Word(w) => out.push_str(&w),
            RepeatToken::Whitespace(ws) => out.push_str(&ws),
            RepeatToken::Other(oth) => out.push_str(&oth),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{collapse_repeated_words, format_numbers};

    #[test]
    fn small_numbers_become_digits_too() {
        assert_eq!(format_numbers("I have three cats"), "I have 3 cats");
        assert_eq!(format_numbers("one of them left"), "1 of them left");
        assert_eq!(format_numbers("one two three"), "1 2 3");
    }

    #[test]
    fn converts_ten_and_above() {
        assert_eq!(format_numbers("twenty five people came"), "25 people came");
        assert_eq!(format_numbers("about a hundred people"), "about a hundred people"); // "a" not a number word
        assert_eq!(format_numbers("one hundred and five items"), "105 items");
        assert_eq!(format_numbers("three thousand users"), "3000 users");
    }

    #[test]
    fn converts_units() {
        assert_eq!(format_numbers("five percent growth"), "5% growth");
        assert_eq!(format_numbers("two kilometers away"), "2 kilometers away");
        assert_eq!(format_numbers("eight gb of ram"), "8 gb of ram");
    }

    #[test]
    fn converts_years() {
        assert_eq!(format_numbers("back in twenty twenty six"), "back in 2026");
        assert_eq!(format_numbers("since nineteen ninety nine"), "since 1999");
    }

    #[test]
    fn converts_decimals() {
        assert_eq!(format_numbers("three point five stars"), "3.5 stars");
        assert_eq!(format_numbers("zero point five percent"), "0.5%");
    }

    #[test]
    fn keeps_punctuation() {
        assert_eq!(format_numbers("we sold twenty five."), "we sold 25.");
        assert_eq!(format_numbers("(twenty five items)"), "(25 items)");
    }

    #[test]
    fn digit_runs_convert_separately() {
        assert_eq!(
            format_numbers("call five five five one two three"),
            "call 5 5 5 1 2 3"
        );
    }

    #[test]
    fn hyphenated_numbers() {
        assert_eq!(format_numbers("twenty-five people"), "25 people");
    }

    #[test]
    fn hyphenated_words_not_duplicated() {
        // Regression: expanding hyphen parts made the non-number fallback
        // emit the whole token once per part ("post-writing post-writing").
        assert_eq!(format_numbers("the follow-up message"), "the follow-up message");
        assert_eq!(format_numbers("post-writing AI agent"), "post-writing AI agent");
        assert_eq!(format_numbers("a well-known long-term plan"), "a well-known long-term plan");
        assert_eq!(format_numbers("forty-ish people showed"), "forty-ish people showed");
    }

    #[test]
    fn digits_untouched() {
        assert_eq!(format_numbers("already 25 people"), "already 25 people");
        assert_eq!(format_numbers("v2.0 release"), "v2.0 release");
    }

    #[test]
    fn empty_and_plain() {
        assert_eq!(format_numbers(""), "");
        assert_eq!(format_numbers("hello world"), "hello world");
    }

    #[test]
    fn test_collapse_repeated_words() {
        assert_eq!(collapse_repeated_words("follow-up follow-up"), "follow-up");
        assert_eq!(collapse_repeated_words("the the cat"), "the cat");
        assert_eq!(collapse_repeated_words("New York New York"), "New York New York");
        assert_eq!(collapse_repeated_words("I I am"), "I I am");
        assert_eq!(collapse_repeated_words("5 5"), "5 5");
        assert_eq!(collapse_repeated_words("The the dog"), "The dog");
        assert_eq!(collapse_repeated_words("no no no"), "no");
        assert_eq!(collapse_repeated_words("no no no cat"), "no cat");
        assert_eq!(collapse_repeated_words("no, no"), "no, no");
    }
}
