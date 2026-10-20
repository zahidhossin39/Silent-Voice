//! The Overview of a meeting: summary, key points, decisions, action items,
//! written by an LLM (the bundled llama-server or a cloud provider).
//! Long meetings are noted in pieces, the notes are combined until they fit,
//! then one last call writes the Overview.

use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::transcripts::{self, Transcript};

/// Input tokens per call. The bundled llama-server runs with a 4096-token
/// context, which also has to hold the prompt and the answer.
pub const LOCAL_TOKENS: usize = 2_000;
/// Cloud models take far more; this keeps a 1-hour meeting to one or two calls.
pub const CLOUD_TOKENS: usize = 16_000;
/// Combining rounds before giving up on making the notes fit.
const MAX_ROUNDS: usize = 4;

pub const SUMMARY_PROMPT: &str = "You write meeting notes from a transcript. \
\"You\" is the person who recorded it; \"Others\" is everyone else on the call. \
Write Markdown with exactly these sections:\n\
## Summary\n2-4 sentences: what the meeting was about and how it ended.\n\
## Key points\nShort bullets.\n\
## Decisions\nBullets, or \"None\".\n\
## Action items\nBullets as \"- [You] task\" or \"- [Others] task\" when the owner is clear, else \"- task\". Or \"None\".\n\
Only use what was said. Do not invent names, dates, numbers or tasks. Skip small talk. \
Write in the language of the transcript. Output only the notes.";

pub const PIECE_PROMPT: &str = "You take notes on one part of a longer meeting transcript. \
\"You\" is the person who recorded it; \"Others\" is everyone else. \
Write short Markdown bullets of what matters: topics, facts, decisions, tasks with their owner (You/Others). \
Only use what was said; do not invent anything. Skip small talk. Output only the bullets.";

pub const COMBINE_PROMPT: &str = "You are given notes on consecutive parts of one meeting. \
Combine them into one shorter list of Markdown bullets: keep every decision and task with its owner (You/Others), \
merge repeats, drop detail that does not matter. Do not invent anything. Output only the bullets.";

/// Put before notes so the final call knows it is not reading the transcript.
pub const NOTES_HEADER: &str = "These are notes taken on consecutive parts of one meeting, in order.";

/// Meetings being summarized: a second Summarize on the same one is refused.
static BUSY: Mutex<BTreeSet<i64>> = Mutex::new(BTreeSet::new());

pub struct Busy(i64);

impl Drop for Busy {
    fn drop(&mut self) {
        if let Ok(mut b) = BUSY.lock() {
            b.remove(&self.0);
        }
    }
}

/// Marks a meeting as being summarized until the guard drops.
pub fn claim(id: i64) -> Result<Busy, String> {
    let mut b = BUSY.lock().map_err(|e| e.to_string())?;
    if !b.insert(id) {
        return Err("This meeting is already being summarized.".into());
    }
    Ok(Busy(id))
}

/// Rough token count: about 4 ASCII characters per token; every other
/// character (accents, CJK, …) counted as a whole token to stay on the safe side.
pub fn tokens(s: &str) -> usize {
    let (ascii, other) = s.chars().fold((0usize, 0usize), |(a, o), c| if c.is_ascii() { (a + 1, o) } else { (a, o + 1) });
    ascii.div_ceil(4) + other
}

/// The transcript as speaker-labelled paragraphs, skipping empty ones.
pub fn paragraphs(t: &Transcript) -> Vec<String> {
    transcripts::paragraphs(&t.lines)
        .into_iter()
        .filter_map(|r| {
            let p = &t.lines[r];
            let text = transcripts::join(p);
            (!text.trim().is_empty()).then(|| format!("{}{}", transcripts::label(&p[0]), text.trim()))
        })
        .collect()
}

/// Groups paragraphs into pieces of at most `budget` tokens, never splitting
/// a paragraph unless it alone is too long.
pub fn split(paras: &[String], budget: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let (mut cur, mut n) = (Vec::<String>::new(), 0);
    for p in paras {
        let size = tokens(p);
        if n + size > budget && !cur.is_empty() {
            pieces.push(cur.join("\n\n"));
            (cur, n) = (Vec::new(), 0);
        }
        if size > budget {
            pieces.extend(cut(p, budget));
            continue;
        }
        n += size;
        cur.push(p.clone());
    }
    if !cur.is_empty() {
        pieces.push(cur.join("\n\n"));
    }
    pieces
}

/// Cuts one overlong paragraph into pieces of at most `budget` tokens, at a
/// space when there is one (CJK text has none: cut anywhere).
fn cut(p: &str, budget: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = p;
    while !rest.is_empty() {
        let mut end = rest.len();
        let (mut ascii, mut other) = (0usize, 0usize);
        for (i, c) in rest.char_indices() {
            if c.is_ascii() { ascii += 1 } else { other += 1 }
            if ascii.div_ceil(4) + other > budget {
                end = i;
                break;
            }
        }
        if end < rest.len() {
            if let Some(sp) = rest[..end].rfind(char::is_whitespace).filter(|&sp| sp > end / 2) {
                end = sp;
            }
        }
        let end = end.max(rest.chars().next().map_or(1, char::len_utf8));
        let (head, tail) = rest.split_at(end);
        if !head.trim().is_empty() {
            out.push(head.trim().to_string());
        }
        rest = tail;
    }
    out
}

/// Runs the whole summary through `chat(system_prompt, text)`: one call for a
/// short meeting; for a long one, notes per piece, combined in rounds until
/// they fit in `budget`, then the final call.
pub async fn summarize<F, Fut>(t: &Transcript, budget: usize, mut chat: F) -> Result<String, String>
where
    F: FnMut(&'static str, String) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let mut ask = async |prompt: &'static str, text: String| -> Result<String, String> {
        let out = strip_think(&chat(prompt, text).await?).trim().to_string();
        if out.is_empty() {
            return Err("The model returned nothing.".into());
        }
        Ok(out)
    };
    let pieces = split(&paragraphs(t), budget);
    let text = match pieces.len() {
        0 => return Err("There is nothing to summarize yet.".into()),
        1 => pieces.into_iter().next().unwrap(),
        _ => {
            let mut notes = Vec::new();
            for p in pieces {
                notes.push(ask(PIECE_PROMPT, p).await?);
            }
            for _ in 0..MAX_ROUNDS {
                if notes.len() == 1 || tokens(&notes.join("\n\n")) <= budget {
                    break;
                }
                let mut next = Vec::new();
                for group in split(&notes, budget) {
                    next.push(ask(COMBINE_PROMPT, group).await?);
                }
                notes = next;
            }
            format!("{NOTES_HEADER}\n\n{}", notes.join("\n\n"))
        }
    };
    ask(SUMMARY_PROMPT, text).await
}

/// Reasoning models (Qwen3, DeepSeek R1) put their thinking in <think>…</think>.
/// An answer cut off inside the thinking has no notes in it at all.
fn strip_think(s: &str) -> &str {
    match (s.find("<think>"), s.find("</think>")) {
        (_, Some(i)) => &s[i + "</think>".len()..],
        (Some(_), None) => "",
        (None, None) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcripts::{Kind, Line, Speaker};

    /// `n` ASCII words of 3 letters + a space: exactly one token each.
    fn words(n: usize) -> String {
        vec!["www"; n].join(" ") + " "
    }

    #[test]
    fn paragraphs_that_exactly_fill_the_budget_share_a_piece() {
        assert_eq!(split(&[words(50), words(50)], 100).len(), 1);
        assert_eq!(split(&[words(50), words(50), words(1)], 100).len(), 2);
    }

    #[test]
    fn an_overlong_paragraph_is_cut_and_no_word_is_lost() {
        let paras = vec![words(10), words(250), words(10)];
        let pieces = split(&paras, 100);
        let total: usize = pieces.iter().map(|p| p.split_whitespace().count()).sum();
        assert_eq!(total, 270);
        assert!(pieces.iter().all(|p| tokens(p) <= 100), "{:?}", pieces.iter().map(|p| tokens(p)).collect::<Vec<_>>());
    }

    #[test]
    fn text_without_spaces_is_cut_by_size() {
        // A long run of Chinese has no spaces: it must still come out in pieces.
        let zh = "会议".repeat(300);
        let pieces = split(&[zh.clone()], 100);
        assert!(pieces.len() >= 6);
        assert!(pieces.iter().all(|p| tokens(p) <= 100));
        assert_eq!(pieces.concat(), zh);
    }

    fn meeting(text_words: usize, lines: usize) -> Transcript {
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        for i in 0..lines {
            let speaker = Some(if i % 2 == 0 { Speaker::You } else { Speaker::Others });
            t.lines.push(Line { start_ms: i as u64 * 1000, end_ms: i as u64 * 1000 + 900, text: words(text_words), speaker, who: None });
        }
        t
    }

    fn run(t: &Transcript, budget: usize, reply: &str) -> (Result<String, String>, Vec<(&'static str, String)>) {
        let mut calls = Vec::new();
        let r = tokio::runtime::Runtime::new().unwrap().block_on(summarize(t, budget, |p, text| {
            calls.push((p, text));
            let reply = reply.to_string();
            async move { Ok(reply) }
        }));
        (r, calls)
    }

    #[test]
    fn a_short_meeting_is_one_labelled_call() {
        let (r, calls) = run(&meeting(5, 2), 1000, "<think>hmm</think>\n## Summary\nok");
        assert_eq!(r.unwrap(), "## Summary\nok", "thinking must be stripped");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SUMMARY_PROMPT);
        assert!(calls[0].1.starts_with("You: ") && calls[0].1.contains("\n\nOthers: "));
    }

    #[test]
    fn a_long_meeting_is_noted_per_piece_then_merged_without_thinking() {
        let (r, calls) = run(&meeting(60, 4), 100, "<think>long thoughts</think>- note");
        assert!(r.is_ok());
        let prompts: Vec<_> = calls.iter().map(|c| c.0).collect();
        assert_eq!(prompts, [PIECE_PROMPT, PIECE_PROMPT, PIECE_PROMPT, PIECE_PROMPT, SUMMARY_PROMPT]);
        assert!(calls[4].1.starts_with(NOTES_HEADER));
        assert!(!calls[4].1.contains("think"), "piece notes must lose their thinking");
    }

    #[test]
    fn notes_too_long_for_one_call_are_combined_first() {
        // Each piece's notes are 80 tokens: 4 of them don't fit a 100-token call.
        let (r, calls) = run(&meeting(60, 4), 100, &words(80));
        assert!(r.is_ok());
        let prompts: Vec<_> = calls.iter().map(|c| c.0).collect();
        assert!(prompts.contains(&COMBINE_PROMPT), "{prompts:?}");
        assert_eq!(*prompts.last().unwrap(), SUMMARY_PROMPT);
    }

    #[test]
    fn an_answer_that_is_only_thinking_is_an_error() {
        let (r, _) = run(&meeting(5, 2), 1000, "<think>x</think>");
        assert!(r.is_err());
        let (r, _) = run(&meeting(5, 2), 1000, "<think>cut off mid-tho");
        assert!(r.is_err(), "unfinished thinking must not become the summary");
    }

    #[test]
    fn an_empty_meeting_makes_no_call() {
        let (r, calls) = run(&meeting(5, 0), 100, "x");
        assert!(r.is_err());
        assert!(calls.is_empty());
        let mut blank = meeting(5, 1);
        blank.lines[0].text = " ".into();
        assert!(run(&blank, 100, "x").1.is_empty(), "blank lines are not a meeting");
    }

    #[test]
    fn one_summary_per_meeting_at_a_time() {
        let a = claim(42).unwrap();
        assert!(claim(42).is_err());
        drop(a);
        assert!(claim(42).is_ok());
    }
}
