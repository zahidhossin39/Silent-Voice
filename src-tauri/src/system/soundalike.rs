// Sound-alike fixer: after transcription, swap misheard versions of the user's
// vocabulary for the real spelling ("Aniton" / "N18" / "N A 10" → "n8n",
// "quad code" → "Claude Code", "Axe user tool" → "Ask user tool").
//
// Words are compared by a consonant skeleton of how they're SAID, so spelling
// differences don't matter. Two guards keep it from rewriting normal speech:
//   - a one-word vocab entry only replaces words that aren't real English
//     (so "cloud" / "could" never become "Claude"),
//   - a multi-word entry only replaces a run with the same number of words.
use harper_core::spell::{Dictionary, FstDictionary};

const LETTERS: [&str; 26] = [
    "ay", "bee", "see", "dee", "ee", "ef", "jee", "aitch", "eye", "jay", "kay", "el", "em", "en",
    "oh", "pee", "kyoo", "ar", "es", "tee", "yoo", "vee", "dubalyoo", "ex", "why", "zee",
];
const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn number_words(n: u32) -> String {
    match n {
        0..=19 => ONES[n as usize].into(),
        20..=99 => format!("{}{}", TENS[(n / 10) as usize], if n % 10 > 0 { ONES[(n % 10) as usize] } else { "" }),
        // ponytail: bigger numbers are read digit by digit; fine for names like "n8n".
        _ => n.to_string().chars().map(|d| ONES[d.to_digit(10).unwrap() as usize]).collect(),
    }
}

/// How a token is said. Letter+digit mixes ("n8n", "N18") and lone capital
/// letters ("N", "A") are spelled out; digit runs become number words.
fn spoken(token: &str) -> String {
    let has_digit = token.chars().any(|c| c.is_ascii_digit());
    let lone_cap = token.len() == 1 && token.chars().all(|c| c.is_ascii_uppercase());
    if !has_digit && !lone_cap {
        return token.to_lowercase();
    }
    let mut out = String::new();
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut String| {
        if !digits.is_empty() {
            out.push_str(&number_words(digits.parse().unwrap_or(0)));
            digits.clear();
        }
    };
    for c in token.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            flush(&mut digits, &mut out);
            if c.is_ascii_alphabetic() {
                out.push_str(LETTERS[(c.to_ascii_lowercase() as u8 - b'a') as usize]);
            }
        }
    }
    flush(&mut digits, &mut out);
    out
}

/// Consonant skeleton of spoken text: vowels and silent letters dropped,
/// voiced/unvoiced pairs merged, repeats collapsed. "Claude"→"klt", "quad"→"kt".
fn skeleton(s: &str) -> String {
    let s = s.replace("gh", "").replace("ph", "f").replace("ck", "k").replace("qu", "k");
    let b = s.as_bytes();
    let mut out = String::new();
    for (i, &c) in b.iter().enumerate() {
        let next = b.get(i + 1).copied().unwrap_or(b' ');
        let m = match c {
            b'a' | b'e' | b'i' | b'o' | b'u' | b'y' | b'h' | b'w' => continue,
            b'c' if matches!(next, b'e' | b'i' | b'y') => 's',
            b'c' | b'q' | b'g' | b'k' => 'k',
            b'x' => {
                out.push('k');
                's'
            }
            b'z' => 's',
            b'd' => 't',
            b'b' => 'p',
            b'v' => 'f',
            b'j' => 'j',
            c if c.is_ascii_lowercase() => c as char,
            _ => continue,
        };
        if !out.ends_with(m) {
            out.push(m);
        }
    }
    out
}

/// Optimal string alignment distance (Levenshtein + adjacent swaps), so
/// "ks" (axe) vs "sk" (ask) is 1.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        d[i][0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = (a[i - 1] != b[j - 1]) as usize;
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// Short skeletons must match exactly; longer ones may be off by a sound or two.
fn close_enough(heard: &str, want: &str) -> bool {
    let allowed = match want.len() {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    };
    !heard.is_empty() && distance(heard, want) <= allowed
}

/// A heard run matches a phrase when every word but one is exactly right and
/// that one sounds close: "quad code" → "Claude Code", "axe user tool" →
/// "Ask user tool". Requiring an exact anchor word keeps everyday pairs like
/// "could not" from turning into "Claude Code".
fn phrase_match(heard: &[&str], want: &[(String, String)]) -> bool {
    if heard.len() != want.len() {
        return false;
    }
    let mut off = 0;
    for (h, (wt, ws)) in heard.iter().zip(want) {
        if h.to_lowercase() == *wt {
            continue;
        }
        off += 1;
        let hs = skeleton(&spoken(h));
        // Two exact anchors earn more slack than one; a one-sound word ("the")
        // is too weak to stand in for anything.
        let slack = if want.len() >= 3 { 2 } else { 1 };
        // With a single anchor, a heard word with MORE sounds than the target
        // ("Claude create") is a different word, not a blurred one.
        let longer = want.len() < 3 && hs.len() > ws.len();
        if off > 1 || longer || hs.len() < 2 || distance(&hs, ws) > slack {
            return false;
        }
    }
    true
}

fn split_words(s: &str) -> Vec<&str> {
    s.split(|c: char| c.is_whitespace() || c == '-').filter(|w| !w.is_empty()).collect()
}

struct Entry {
    text: String,
    words: usize,
    skel: String,
    // Per-word lowercase text and skeletons, for phrase matching.
    parts: Vec<(String, String)>,
}

fn entries(vocabulary: &str) -> Vec<Entry> {
    vocabulary
        .split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|t| {
            let words = split_words(t);
            Entry {
                text: t.to_string(),
                words: words.len(),
                skel: skeleton(&words.iter().map(|w| spoken(w)).collect::<String>()),
                parts: words.iter().map(|w| (w.to_lowercase(), skeleton(&spoken(w)))).collect(),
            }
        })
        // Very short skeletons ("ask" → "sk") would match too much on their own.
        .filter(|e| e.skel.len() >= 2 && (e.words > 1 || e.skel.len() >= 3))
        .collect()
}

fn core(token: &str) -> &str {
    token.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Replace sound-alike mishearings of `vocabulary` entries in `text`.
pub fn fix(text: &str, vocabulary: &str) -> String {
    let list = entries(vocabulary);
    if list.is_empty() || text.trim().is_empty() {
        return text.to_string();
    }
    let dict = FstDictionary::curated();
    // A word that's real English (or a lowercase single letter like "a")
    // can't stand in for a one-word name.
    let is_real = |w: &str| {
        let lone_cap = w.len() == 1 && w.chars().all(|c| c.is_ascii_uppercase());
        let has_digit = w.chars().any(|c| c.is_ascii_digit());
        !lone_cap && !has_digit && dict.contains_word_str(w)
    };

    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    'outer: while i < tokens.len() {
        // Try longer runs first so "N A 10" wins over "N".
        for len in (1..=4.min(tokens.len() - i)).rev() {
            let run = &tokens[i..i + len];
            let cores: Vec<&str> = run.iter().map(|t| core(t)).collect();
            if cores.iter().any(|c| c.is_empty()) {
                continue;
            }
            // Punctuation inside the run means it crosses a phrase boundary.
            if run[..len - 1].iter().any(|t| t.ends_with([',', '.', '?', '!', ';', ':'])) {
                continue;
            }
            let joined = cores.join(" ");
            let words = split_words(&joined);
            let skel = skeleton(&cores.iter().map(|w| spoken(w)).collect::<String>());
            for e in &list {
                let matched = if joined.eq_ignore_ascii_case(&e.text) {
                    // Already right; only restore the user's casing for names.
                    if joined == e.text || !e.text.chars().any(|c| c.is_uppercase() || c.is_ascii_digit()) {
                        continue;
                    }
                    true
                } else if e.words == 1 {
                    cores.iter().all(|c| !is_real(c)) && close_enough(&skel, &e.skel)
                } else {
                    phrase_match(&words, &e.parts)
                };
                if matched {
                    let first = run[0];
                    let last = run[len - 1];
                    let lead = &first[..first.find(|c: char| c.is_alphanumeric()).unwrap_or(0)];
                    let tail_at = last.rfind(|c: char| c.is_alphanumeric()).map_or(last.len(), |p| {
                        p + last[p..].chars().next().map_or(1, char::len_utf8)
                    });
                    out.push(format!("{lead}{}{}", e.text, &last[tail_at..]));
                    i += len;
                    continue 'outer;
                }
            }
        }
        out.push(tokens[i].to_string());
        i += 1;
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOCAB: &str = "Claude, discord, task, Anti-gravity, n8n, Claude Code, Gemini, Ask user tool";

    #[test]
    fn fixes_the_reported_mishearings() {
        let cases = [
            ("I didn't use the Axe user tool.", "I didn't use the Ask user tool."),
            ("He's saying that the axe user tool actually is working.", "He's saying that the Ask user tool actually is working."),
            ("worked well like N18, Claude, Gemini", "worked well like n8n, Claude, Gemini"),
            ("making automation with N A 10 for a few months.", "making automation with n8n for a few months."),
            ("Sometimes Aniton works, sometimes it doesn't.", "Sometimes n8n works, sometimes it doesn't."),
            ("sometimes I use quad code, anti-gravity", "sometimes I use Claude Code, Anti-gravity"),
            ("The access user tool is very hard to fix.", "The Ask user tool is very hard to fix."),
        ];
        for (heard, want) in cases {
            assert_eq!(fix(heard, VOCAB), want, "input: {heard}");
        }
    }

    #[test]
    fn leaves_normal_speech_alone() {
        let keep = [
            "I could not find the file.",
            "The cloud looks dark today.",
            "Could you call me back in an hour?",
            "I went into an office and then on to lunch.",
            "Ask a question about the budget.",
            "My next task is to update the app.",
            "Send it on Discord please.",
            "The gravity of the situation was clear.",
            "He said the code was ready.",
            "Then the tasks and sometimes I ask some questions.",
            "Other than that I think others were working pretty well.",
            "I need ten more minutes and a new token.",
            "You Claude create a file.",
        ];
        for s in keep {
            assert_eq!(fix(s, VOCAB), s, "changed: {s}");
        }
    }
}

