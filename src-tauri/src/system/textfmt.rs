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

/// Spoken fillers. Whisper omits these on its own; Parakeet/NeMo transcribe them
/// verbatim, so the pipeline strips them to keep output the same across engines.
pub const FILLER_WORDS: &[&str] = &["um", "umm", "uh", "uhh", "erm"];

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

    tidy_numbers(out)
}

/// Second pass over format_numbers output, for shapes that span several
/// numbers so the word parser can't see them:
///   "1.2 point 3"   → "1.2.3"     (versions, IPs)
///   "5 5 5 1 2 3 4" → "5551234"   (phone numbers, codes)
///   "v 1.2.3"       → "v1.2.3"
///   "3 30 pm"       → "3:30 PM"   ("3 pm" → "3 PM")
fn tidy_numbers(toks: Vec<String>) -> String {
    let is_digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    let is_num = |t: &str| t.starts_with(|c: char| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_digit() || c == '.');
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].as_str();
        // Digit run: 4+ single digits (phone numbers, PINs), or 2+ ending in a dotted number
        // ("1 9 2.168" → "192.168").
        // Counting "1 2 3" stays separate. ponytail: a 4+ spoken list would join.
        let mut j = i;
        while j < toks.len() && toks[j].len() == 1 && is_digits(&toks[j]) { j += 1; }
        let dotted_tail = j - i >= 2 && toks.get(j).map_or(false, |n| is_num(split_punct(n).0));
        if j - i >= 4 || dotted_tail {
            let mut joined: String = toks[i..j].concat();
            if dotted_tail {
                joined.push_str(&toks[j]);
                j += 1;
            }
            out.push(joined);
            i = j;
            continue;
        }
        // "<decimal> point <num>" chains: "1.2 point 3" → "1.2.3".
        if t.eq_ignore_ascii_case("point") {
            if let (Some(prev), Some(next)) = (out.last(), toks.get(i + 1)) {
                if is_num(prev.trim_start_matches(['v', 'V'])) && prev.contains('.') && is_num(split_punct(next).0) {
                    let prev = out.pop().unwrap();
                    out.push(format!("{prev}.{next}"));
                    i += 2;
                    continue;
                }
            }
        }
        // Times: "<1-12> [<00-59>] am/pm".
        if let Some(h) = t.parse::<u32>().ok().filter(|h| (1..=12).contains(h) && is_digits(t)) {
            let (min, k) = match toks.get(i + 1).map(|m| m.as_str()) {
                Some(m) if m.len() == 2 && is_digits(m) && m < "60" => (Some(m.to_string()), i + 2),
                _ => (None, i + 1),
            };
            if let Some(mer) = toks.get(k) {
                let (core, punct) = split_punct(mer);
                let bare = core.to_lowercase().replace('.', "");
                if bare == "am" || bare == "pm" {
                    // Keep "a.m." if the STT wrote it that way; plain am/pm → AM/PM.
                    let mer = if core.contains('.') { mer.to_string() } else { format!("{}{punct}", bare.to_uppercase()) };
                    out.push(match min { Some(m) => format!("{h}:{m} {mer}"), None => format!("{h} {mer}") });
                    i = k + 1;
                    continue;
                }
            }
        }
        // "v 1.2.3" → "v1.2.3"
        if out.last().map_or(false, |p| p.eq_ignore_ascii_case("v")) && is_num(split_punct(t).0) {
            let prev = out.pop().unwrap();
            out.push(format!("{prev}{t}"));
            i += 1;
            continue;
        }
        out.push(t.to_string());
        i += 1;
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

pub fn strip_fillers(text: &str) -> String {
    // Untouched when there is nothing to strip — Whisper output has no fillers,
    // and rebuilding the string would flatten line breaks the way format_numbers
    // does, which structure_text depends on later in the pipeline.
    if !text.split_whitespace().any(is_filler_token) {
        return text.to_string();
    }
    text.split('\n')
        .map(strip_fillers_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_fillers_line(line: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    let mut cap_next = false;
    for token in line.split_whitespace() {
        if is_filler_token(token) {
            // Only a filler that opened a sentence leaves a gap worth fixing;
            // capitalizing anywhere else would turn "e.g. here" into "e.g. Here".
            if kept
                .last()
                .map_or(true, |prev| prev.ends_with(['.', '!', '?']))
            {
                cap_next = true;
            }
            continue;
        }
        if cap_next {
            kept.push(capitalize_first(token));
            cap_next = false;
        } else {
            kept.push(token.to_string());
        }
    }
    kept.join(" ")
}

fn is_filler_token(token: &str) -> bool {
    let bare = token
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase();
    FILLER_WORDS.contains(&bare.as_str())
}

fn capitalize_first(token: &str) -> String {
    let mut out = String::with_capacity(token.len());
    let mut done = false;
    for c in token.chars() {
        if !done && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

// ── Spoken symbols: file extensions, domains, emails ─────────────────────────
// "readme dot md" → "readme.md", "the dot txt file" → "the .txt file",
// "google dot com" → "google.com", "john dot smith at gmail dot com" →
// "john.smith@gmail.com". Also repairs STT that already wrote the period but
// split it: "readme. MD" / "gmail. Com" → "readme.md" / "gmail.com".
// ponytail: fixed suffix list, not a general "dot <anything>" rule — "dot"
// before an unknown word is left alone so "a red dot appeared" survives.
const SUFFIXES: &[&str] = &[
    "md", "txt", "json", "js", "ts", "tsx", "jsx", "py", "rs", "html", "htm", "css", "csv",
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "png", "jpg", "jpeg", "gif", "svg", "webp",
    "mp3", "mp4", "wav", "mov", "zip", "rar", "exe", "msi", "dll", "yaml", "yml", "toml",
    "xml", "sh", "bat", "ps1", "log", "ini", "env", "sql", "db", "go", "java", "cpp", "c",
    "h", "kt", "swift", "rb", "php", "lua", "vue", "svelte", "ipynb", "lock", "cfg", "conf",
    "com", "org", "net", "io", "dev", "ai", "app", "co", "edu", "gov", "uk", "in", "us",
    "me", "info", "xyz", "tech", "bd", "ca", "de", "au",
];
// Suffixes that are also ordinary words: only joined when "dot" was spoken,
// never when repairing a "word. Word" sentence break.
const AMBIGUOUS: &[&str] = &[
    "in", "us", "me", "go", "c", "h", "app", "co", "ai", "doc", "log", "lock", "env", "db",
    "dev", "info", "ca", "de", "au", "conf",
];
// Words after which "dot md" means the bare extension, not "<word>.md".
const STANDALONE_BEFORE: &[&str] = &[
    "a", "an", "the", "my", "your", "our", "this", "that", "these", "those", "any", "every",
    "all", "some", "each", "open", "in", "into", "to", "of", "as", "and", "or", "with", "for",
    "save", "create", "make", "new", "is", "are", "it's", "its", "use", "from", "on",
];

fn split_punct(tok: &str) -> (&str, &str) {
    let core = tok.trim_end_matches(|c: char| matches!(c, ',' | '.' | '?' | '!' | ';' | ':'));
    (core, &tok[core.len()..])
}

// Spoken symbol words → (symbol, glue to previous word, glue to next word).
// Longer phrases first so "colon slash slash" wins over "slash".
// ponytail: bare "dash" and "colon" are left out — both are ordinary English
// ("a quick dash", "colon cancer"); say "hyphen", or "colon slash slash".
const SYMBOL_WORDS: &[(&str, &str, bool, bool)] = &[
    ("colon slash slash", "://", true, true),
    ("forward slash", "/", true, true),
    ("back slash", "\\", true, true),
    ("backslash", "\\", true, true),
    ("slash", "/", true, true),
    ("underscore", "_", true, true),
    ("hyphen", "-", true, true),
    ("at sign", "@", true, true),
    ("hash tag", "#", false, true),
    ("hashtag", "#", false, true),
    ("percent sign", "%", true, false),
];

// "camel case user name" → "userName". Takes the next words up to a
// punctuation mark, a stop word, or 4 words.
// ponytail: 4-word cap + stop list; add a spoken "end case" if names run longer.
const CASE_COMMANDS: &[&str] = &["camel case", "pascal case", "snake case", "kebab case", "constant case", "all caps"];
const CASE_STOP: &[&str] = &[
    "is", "are", "was", "were", "the", "a", "an", "and", "or", "to", "in", "of", "for", "with",
    "on", "at", "from", "should", "will", "then", "but", "it", "that", "which", "as", "into",
];

fn apply_case(cmd: &str, words: &[String]) -> String {
    let lw: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    match cmd {
        "camel case" => lw.iter().enumerate().map(|(i, w)| if i == 0 { w.clone() } else { capitalize_first(w) }).collect(),
        "pascal case" => lw.iter().map(|w| capitalize_first(w)).collect(),
        "snake case" => lw.join("_"),
        "kebab case" => lw.join("-"),
        "constant case" => lw.join("_").to_uppercase(),
        _ => lw.join(" ").to_uppercase(), // all caps
    }
}

/// If `phrase` starts at toks[i], returns (word count, trailing punctuation
/// on its last word). Punctuation inside the phrase breaks the match.
fn phrase_at(toks: &[&str], i: usize, phrase: &str) -> Option<(usize, String)> {
    let words: Vec<&str> = phrase.split(' ').collect();
    if i + words.len() > toks.len() { return None; }
    for (k, w) in words.iter().enumerate() {
        let (core, punct) = split_punct(toks[i + k]);
        if !core.eq_ignore_ascii_case(w) || (k + 1 < words.len() && !punct.is_empty()) { return None; }
    }
    Some((words.len(), split_punct(toks[i + words.len() - 1]).1.to_string()))
}

fn symbol_words(line: &str) -> String {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut glue_next = false;
    let mut i = 0;
    'outer: while i < toks.len() {
        for &cmd in CASE_COMMANDS {
            if let Some((n, punct)) = phrase_at(&toks, i, cmd) {
                if !punct.is_empty() { break; }
                let mut words = Vec::new();
                let mut tail = String::new();
                let mut k = i + n;
                while k < toks.len() && words.len() < 4 {
                    let (core, p) = split_punct(toks[k]);
                    if core.is_empty() || CASE_STOP.contains(&core.to_lowercase().as_str()) { break; }
                    words.push(core.to_string());
                    k += 1;
                    if !p.is_empty() { tail = p.to_string(); break; }
                }
                if words.is_empty() { break; }
                out.push(format!("{}{tail}", apply_case(cmd, &words)));
                glue_next = false;
                i = k;
                continue 'outer;
            }
        }
        for &(phrase, sym, gl, gr) in SYMBOL_WORDS {
            if let Some((n, punct)) = phrase_at(&toks, i, phrase) {
                match out.last_mut() {
                    Some(prev) if gl && !prev.ends_with(|c: char| ",;:!?".contains(c)) => prev.push_str(sym),
                    _ => out.push(sym.to_string()),
                }
                out.last_mut().unwrap().push_str(&punct);
                glue_next = gr && punct.is_empty();
                i += n;
                continue 'outer;
            }
        }
        match out.last_mut() {
            Some(prev) if glue_next => prev.push_str(toks[i]),
            _ => out.push(toks[i].to_string()),
        }
        glue_next = false;
        i += 1;
    }
    out.join(" ")
}

pub fn spoken_symbols(text: &str) -> String {
    let lower = text.to_lowercase();
    let has_command = SYMBOL_WORDS.iter().any(|(p, ..)| lower.contains(p.split(' ').next().unwrap()))
        || CASE_COMMANDS.iter().any(|c| lower.contains(c));
    if !(has_command || lower.contains("dot") || lower.contains(". ") || lower.contains(" at ")) {
        return text.to_string();
    }
    text.split('\n').map(spoken_symbols_line).collect::<Vec<_>>().join("\n")
}

fn spoken_symbols_line(line: &str) -> String {
    let line = symbol_words(line);
    let toks: Vec<&str> = line.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let (core, _) = split_punct(toks[i]);
        let next = toks.get(i + 1).map(|t| split_punct(t));
        let next_sfx = next.map(|(c, p)| (c.to_lowercase(), p));
        // Inside a web address ("www dot google", "https://www dot google")
        // "dot" joins any word, not just known suffixes.
        if core.eq_ignore_ascii_case("dot") && toks[i] == core {
            let in_url = out.last().map_or(false, |p| {
                let p = p.to_lowercase();
                p.ends_with("www") || (p.contains("://") && !p.ends_with(|c: char| c.is_ascii_punctuation()))
            });
            if let (true, Some((w, punct))) = (in_url, next) {
                let prev = out.pop().unwrap();
                out.push(format!("{prev}.{}{punct}", w.to_lowercase()));
                i += 2;
                continue;
            }
        }
        // "dot <suffix>"
        if core.eq_ignore_ascii_case("dot") && toks[i] == core {
            if let Some((sfx, punct)) = next_sfx.as_ref().filter(|(c, _)| SUFFIXES.contains(&c.as_str())) {
                // Join onto the previous word unless it closed a clause or is
                // a word like "the"/"a" ("the dot md file" → "the .md file").
                let attach = out.last().map_or(false, |prev| {
                    !prev.ends_with(|c: char| c.is_ascii_punctuation())
                        && !STANDALONE_BEFORE.contains(&prev.to_lowercase().as_str())
                });
                if attach {
                    let prev = out.pop().unwrap();
                    out.push(format!("{prev}.{sfx}{punct}"));
                } else {
                    out.push(format!(".{sfx}{punct}"));
                }
                i += 2;
                continue;
            }
        }
        // "readme. MD" → "readme.md" (STT already wrote the period)
        if toks[i].ends_with('.') && !toks[i].ends_with("..") && core.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c)) && !core.is_empty() {
            if let Some((sfx, punct)) = next_sfx.as_ref().filter(|(c, _)| SUFFIXES.contains(&c.as_str()) && !AMBIGUOUS.contains(&c.as_str())) {
                out.push(format!("{core}.{sfx}{punct}"));
                i += 2;
                continue;
            }
        }
        out.push(toks[i].to_string());
        i += 1;
    }
    join_emails(out)
}

/// "<local> at <domain.tld>" → "local@domain.tld". The local part may be
/// spoken in pieces: "john dot smith" / "john underscore smith".
fn join_emails(toks: Vec<String>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let is_at = toks[i].eq_ignore_ascii_case("at");
        let domain = toks.get(i + 1).map(|t| split_punct(t));
        let is_domain = domain.map_or(false, |(c, _)| {
            c.rsplit_once('.').map_or(false, |(host, tld)| {
                !host.is_empty() && host.chars().all(|ch| ch.is_alphanumeric() || "-.".contains(ch))
                    && SUFFIXES.contains(&tld.to_lowercase().as_str())
            })
        });
        let local_ok = out.last().map_or(false, |p| p.chars().all(|c| c.is_alphanumeric() || "._-".contains(c)));
        if is_at && is_domain && local_ok {
            let mut local = out.pop().unwrap();
            // Pull "john dot" / "john underscore" pieces back into the local part.
            while out.len() >= 2 {
                let sep = match out[out.len() - 1].to_lowercase().as_str() {
                    "dot" => ".",
                    "underscore" => "_",
                    "dash" | "hyphen" => "-",
                    _ => break,
                };
                if !out[out.len() - 2].chars().all(|c| c.is_alphanumeric()) { break; }
                out.pop();
                local = format!("{}{sep}{local}", out.pop().unwrap());
            }
            let (d, punct) = domain.unwrap();
            out.push(format!("{}@{}{punct}", local.to_lowercase(), d.to_lowercase()));
            i += 2;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out.join(" ")
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

pub fn structure_text(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }

    let commands = [
        ("new paragraph", "\n\n"),
        ("new line", "\n"),
        ("bullet point", "\n- "),
        ("bullet", "\n- "),
        ("number one", "\n1. "),
        ("number 1", "\n1. "),
        ("number two", "\n2. "),
        ("number 2", "\n2. "),
        ("number three", "\n3. "),
        ("number 3", "\n3. "),
        ("number four", "\n4. "),
        ("number 4", "\n4. "),
        ("number five", "\n5. "),
        ("number 5", "\n5. "),
        ("number six", "\n6. "),
        ("number 6", "\n6. "),
        ("number seven", "\n7. "),
        ("number 7", "\n7. "),
        ("number eight", "\n8. "),
        ("number 8", "\n8. "),
        ("number nine", "\n9. "),
        ("number 9", "\n9. "),
        ("number ten", "\n10. "),
        ("number 10", "\n10. "),
    ];

    let mut result = text.to_string();

    for &(phrase, replacement) in &commands {
        let mut new_result = String::new();
        let chars: Vec<(usize, char)> = result.char_indices().collect();
        let mut i = 0;
        let mut last_idx = 0;
        
        while i < chars.len() {
            let mut matched = false;
            let mut match_len_bytes = 0;
            let mut match_len_chars = 0;
            
            let mut j = i;
            let mut p_match = true;
            for p_c in phrase.chars() {
                if j < chars.len() {
                    let mut lc = chars[j].1.to_lowercase();
                    if lc.next() == Some(p_c) && lc.next().is_none() {
                        j += 1;
                    } else {
                        p_match = false;
                        break;
                    }
                } else {
                    p_match = false;
                    break;
                }
            }
            
            if p_match {
                let prev_char = if i > 0 { Some(chars[i - 1].1) } else { None };
                let next_char = if j < chars.len() { Some(chars[j].1) } else { None };
                
                let is_start_boundary = prev_char.map_or(true, |c| !c.is_alphanumeric());
                let is_end_boundary = next_char.map_or(true, |c| !c.is_alphanumeric());
                
                if is_start_boundary && is_end_boundary {
                    matched = true;
                    match_len_bytes = if j < chars.len() {
                        chars[j].0 - chars[i].0
                    } else {
                        result.len() - chars[i].0
                    };
                    match_len_chars = j - i;
                }
            }
            
            if matched {
                new_result.push_str(&result[last_idx..chars[i].0]);
                new_result.push_str(replacement);
                last_idx = chars[i].0 + match_len_bytes;
                i += match_len_chars;
            } else {
                i += 1;
            }
        }
        new_result.push_str(&result[last_idx..]);
        result = new_result;
    }

    let chars: Vec<char> = result.chars().collect();
    
    let mut temp1 = Vec::new();
    for &c in &chars {
        if c == '\n' {
            while let Some(&last) = temp1.last() {
                if last == ' ' || last == '\t' {
                    temp1.pop();
                } else {
                    break;
                }
            }
        }
        temp1.push(c);
    }
    
    let mut temp2 = Vec::new();
    let mut after_newline = false;
    for &c in &temp1 {
        if after_newline {
            if c == ' ' || c == '\t' {
                continue;
            } else {
                after_newline = false;
            }
        }
        if c == '\n' {
            after_newline = true;
        }
        temp2.push(c);
    }
    
    let mut temp3 = Vec::new();
    let mut nl_count = 0;
    for &c in &temp2 {
        if c == '\n' {
            nl_count += 1;
            if nl_count <= 2 {
                temp3.push(c);
            }
        } else {
            nl_count = 0;
            temp3.push(c);
        }
    }
    
    let mut temp4 = Vec::new();
    let mut sp_count = 0;
    for &c in &temp3 {
        if c == ' ' || c == '\t' {
            if sp_count == 0 {
                temp4.push(c);
            }
            sp_count += 1;
        } else {
            sp_count = 0;
            temp4.push(c);
        }
    }
    
    result = temp4.into_iter().collect::<String>();
    result = result.trim().to_string();

    let mut cap_result = String::new();
    let chars2: Vec<char> = result.chars().collect();
    let mut cap_next_alpha = true;
    
    let mut i = 0;
    while i < chars2.len() {
        let c = chars2[i];
        
        if (c == 'i' || c == 'I') && (i == 0 || !chars2[i - 1].is_alphanumeric()) {
            let mut j = i + 1;
            while j < chars2.len() && (chars2[j].is_alphabetic() || chars2[j] == '\'' || chars2[j] == '’') {
                j += 1;
            }
            let is_end_boundary = j == chars2.len() || !chars2[j].is_alphanumeric();
            if is_end_boundary {
                let word: String = chars2[i..j].iter().collect();
                let word_lower = word.to_lowercase();
                // Explanation: The pronoun 'I' must always be capitalized in English.
                if matches!(word_lower.as_str(), "i" | "i'm" | "i've" | "i'll" | "i'd" | "i’m" | "i’ve" | "i’ll" | "i’d") {
                    cap_result.push('I');
                    for k in i + 1..j {
                        cap_result.push(chars2[k]);
                    }
                    if cap_next_alpha {
                        cap_next_alpha = false;
                    }
                    i = j;
                    continue;
                }
            }
        }
        
        if c == '\n' {
            cap_next_alpha = true;
            cap_result.push(c);
        } else if c == '.' || c == '?' || c == '!' {
            // Only a real sentence end (punctuation then a space/newline/end)
            // starts a new capital. Guards decimals ("3.5 stars") and
            // abbreviations ("U.S.") from capitalising the following word.
            let next_is_break = i + 1 >= chars2.len() || chars2[i + 1].is_whitespace();
            if next_is_break {
                cap_next_alpha = true;
            }
            cap_result.push(c);
        } else if c.is_alphabetic() {
            // Don't capitalise a sentence-opening email, URL, file name or
            // code identifier: "john@gmail.com", "readme.md", "userName".
            if cap_next_alpha && (i == 0 || chars2[i - 1].is_whitespace()) {
                let tok: String = chars2[i..].iter().take_while(|c| !c.is_whitespace()).collect();
                let core = tok.trim_end_matches(|c: char| ",.?!;:".contains(c));
                let literal = core.contains('@')
                    || core.contains('/')
                    || core.contains('_')
                    || core.contains('.')
                    || core.chars().skip(1).any(|c| c.is_uppercase());
                if literal {
                    cap_result.push(c);
                    cap_next_alpha = false;
                    i += 1;
                    continue;
                }
            }
            if cap_next_alpha {
                for uc in c.to_uppercase() {
                    cap_result.push(uc);
                }
                cap_next_alpha = false;
            } else {
                cap_result.push(c);
            }
        } else {
            cap_result.push(c);
        }
        
        i += 1;
    }
    
    cap_result
}

#[cfg(test)]
mod tests {
    use super::{collapse_repeated_words, format_numbers, spoken_symbols, strip_fillers, structure_text};

    #[test]
    fn test_spoken_symbols() {
        assert_eq!(spoken_symbols("open readme dot md please"), "open readme.md please");
        assert_eq!(spoken_symbols("save it as a dot txt file"), "save it as a .txt file");
        assert_eq!(spoken_symbols("Dot JSON files are fine"), ".json files are fine");
        assert_eq!(spoken_symbols("go to google dot com."), "go to google.com.");
        assert_eq!(spoken_symbols("Check notes. MD now"), "Check notes.md now");
        assert_eq!(spoken_symbols("email john dot smith at gmail dot com today"), "email john.smith@gmail.com today");
        assert_eq!(spoken_symbols("mail Zaid at Gmail.com"), "mail zaid@gmail.com");
        assert_eq!(spoken_symbols("a red dot appeared"), "a red dot appeared");
        assert_eq!(spoken_symbols("I was home. In the morning"), "I was home. In the morning");
        assert_eq!(spoken_symbols("meet me at noon"), "meet me at noon");
        assert_eq!(spoken_symbols("line one\nfile dot py"), "line one\nfile.py");
        // symbol words
        assert_eq!(spoken_symbols("src slash main dot rs"), "src/main.rs");
        assert_eq!(spoken_symbols("users back slash zaid"), "users\\zaid");
        assert_eq!(spoken_symbols("my underscore file dot txt"), "my_file.txt");
        assert_eq!(spoken_symbols("hashtag rust is great"), "#rust is great");
        assert_eq!(spoken_symbols("fifty percent sign off"), "fifty% off");
        assert_eq!(spoken_symbols("and slash or"), "and/or");
        assert_eq!(spoken_symbols("a quick dash home"), "a quick dash home");
        assert_eq!(spoken_symbols("zaid at sign home"), "zaid@home");
        // web addresses
        assert_eq!(spoken_symbols("go to https colon slash slash www dot google dot com slash docs"), "go to https://www.google.com/docs");
        assert_eq!(spoken_symbols("visit www dot example dot org."), "visit www.example.org.");
        // case commands
        assert_eq!(spoken_symbols("rename it camel case user name is wrong"), "rename it userName is wrong");
        assert_eq!(spoken_symbols("snake case max retry count, then"), "max_retry_count, then");
        assert_eq!(spoken_symbols("pascal case http client"), "HttpClient");
        assert_eq!(spoken_symbols("kebab case main nav bar"), "main-nav-bar");
        assert_eq!(spoken_symbols("constant case api key"), "API_KEY");
        assert_eq!(spoken_symbols("all caps warning"), "WARNING");
    }

    #[test]
    fn test_tidy_numbers() {
        assert_eq!(format_numbers("v one point two point three"), "v1.2.3");
        assert_eq!(format_numbers("version one point two point three"), "version 1.2.3");
        assert_eq!(format_numbers("one nine two point one six eight point one point one"), "192.168.1.1");
        assert_eq!(format_numbers("call five five five one two three four"), "call 5551234");
        assert_eq!(format_numbers("three thirty pm"), "3:30 PM");
        assert_eq!(format_numbers("meet at three pm."), "meet at 3 PM.");
        assert_eq!(format_numbers("at seven forty five a.m. ok"), "at 7:45 a.m. ok");
        assert_eq!(format_numbers("three point five stars"), "3.5 stars");
        assert_eq!(format_numbers("I am fine"), "I am fine");
    }

    #[test]
    fn test_no_capital_on_literals() {
        assert_eq!(structure_text("john@gmail.com is mine"), "john@gmail.com is mine");
        assert_eq!(structure_text("readme.md is here. userName too"), "readme.md is here. userName too");
        assert_eq!(structure_text("hello there. my_var works"), "Hello there. my_var works");
        assert_eq!(structure_text("hello. world"), "Hello. World");
    }

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
            "call 555123"
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

    #[test]
    fn test_structure_text() {
        assert_eq!(structure_text(""), "");
        assert_eq!(structure_text("hello world"), "Hello world");
        assert_eq!(structure_text("first line new line second line"), "First line\nSecond line");
        assert_eq!(structure_text("intro new paragraph body text"), "Intro\n\nBody text");
        assert_eq!(structure_text("shopping bullet point milk bullet point eggs bullet point bread"), "Shopping\n- Milk\n- Eggs\n- Bread");
        assert_eq!(structure_text("steps number one plan number two build number three ship"), "Steps\n1. Plan\n2. Build\n3. Ship");
        assert_eq!(structure_text("i think i'm ready"), "I think I'm ready");
        assert_eq!(structure_text("hello new line world"), "Hello\nWorld");
        assert_eq!(structure_text("the victorian period was long"), "The victorian period was long"); // "period" NOT a command
        assert_eq!(structure_text("done. next thing"), "Done. Next thing");
        // A decimal must not capitalise the following word (abbreviations like
        // "u.s. team" stay ambiguous and are accepted as-is).
        assert_eq!(structure_text("it is 3.5 stars now"), "It is 3.5 stars now");
    }

    #[test]
    fn test_strip_fillers() {
        assert_eq!(strip_fillers("Um it's been a while."), "It's been a while.");
        assert_eq!(strip_fillers("Um yeah, that's all. Um we can also uh make it"), "Yeah, that's all. We can also make it");
        assert_eq!(strip_fillers("The drummer went rat-a-tat."), "The drummer went rat-a-tat.");
        assert_eq!(strip_fillers("um uh"), "");
    }

    #[test]
    fn test_strip_fillers_keeps_line_breaks() {
        let input = "Um first line
second um line";
        assert_eq!(strip_fillers(input), "First line
second line");
    }

    #[test]
    fn test_strip_fillers_leaves_clean_text_byte_identical() {
        // No filler present -> returned untouched, including line breaks and
        // any leading lowercase the pipeline may rely on.
        let input = "hello world
second line";
        assert_eq!(strip_fillers(input), input);
    }

    #[test]
    fn test_strip_fillers_does_not_break_abbreviations() {
        assert_eq!(strip_fillers("um use e.g. here"), "Use e.g. here");
    }

    #[test]
    fn test_strip_fillers_real_parakeet_transcript() {
        // Verbatim output from the Parakeet model, as reported.
        let got = strip_fillers(
            "Um it's been a while. Um yeah, that's all. Sometimes we do a lot of things. Um we can also uh make it",
        );
        assert_eq!(
            got,
            "It's been a while. Yeah, that's all. Sometimes we do a lot of things. We can also make it"
        );
    }
}
