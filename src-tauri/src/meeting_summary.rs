//! The Overview of a meeting: summary, key points, decisions, action items,
//! written by an LLM (the bundled llama-server or a cloud provider).
//! Long meetings are summarized in pieces, then the piece notes are merged.

use crate::transcripts::{self, Transcript};

/// Words per piece. The bundled llama-server runs with a 4096-token context,
/// which has to hold the prompt, the piece and the answer.
pub const LOCAL_WORDS: usize = 2_000;
/// Cloud models take far more; this keeps a 1-hour meeting to one or two calls.
pub const CLOUD_WORDS: usize = 12_000;

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

pub const MERGE_PROMPT_NOTE: &str = "These are notes taken on consecutive parts of one meeting, in order.";

/// The transcript as speaker-labelled paragraphs.
pub fn paragraphs(t: &Transcript) -> Vec<String> {
    transcripts::paragraphs(&t.lines)
        .into_iter()
        .map(|r| {
            let p = &t.lines[r];
            let label = match p[0].speaker {
                Some(transcripts::Speaker::You) => "You: ",
                Some(transcripts::Speaker::Others) => "Others: ",
                None => "",
            };
            format!("{label}{}", p.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" "))
        })
        .collect()
}

/// Groups paragraphs into pieces of at most `max_words` words, never splitting
/// a paragraph unless it alone is too long.
pub fn split(paras: &[String], max_words: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let (mut cur, mut n) = (Vec::<String>::new(), 0);
    let mut push = |cur: &mut Vec<String>, n: &mut usize| {
        if !cur.is_empty() {
            pieces.push(cur.join("\n\n"));
            cur.clear();
            *n = 0;
        }
    };
    for p in paras {
        let words: Vec<&str> = p.split_whitespace().collect();
        if words.len() > max_words {
            push(&mut cur, &mut n);
            for chunk in words.chunks(max_words) {
                cur.push(chunk.join(" "));
                n = chunk.len();
                push(&mut cur, &mut n);
            }
            continue;
        }
        if n + words.len() > max_words {
            push(&mut cur, &mut n);
        }
        n += words.len();
        cur.push(p.clone());
    }
    push(&mut cur, &mut n);
    pieces
}

/// Runs the whole summary through `chat(system_prompt, text)`: one call for a
/// short meeting; for a long one, notes per piece, then one merging call.
pub async fn summarize<F, Fut>(t: &Transcript, max_words: usize, mut chat: F) -> Result<String, String>
where
    F: FnMut(&'static str, String) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let pieces = split(&paragraphs(t), max_words);
    let text = match pieces.len() {
        0 => return Err("There is nothing to summarize yet.".into()),
        1 => pieces.into_iter().next().unwrap(),
        _ => {
            let mut notes = vec![MERGE_PROMPT_NOTE.to_string()];
            for (i, p) in pieces.into_iter().enumerate() {
                notes.push(format!("Part {}:\n{}", i + 1, chat(PIECE_PROMPT, p).await?.trim()));
            }
            // ponytail: one merge pass; the notes of a very long meeting on a
            // local model could overflow the context; merge in rounds if it does.
            notes.join("\n\n")
        }
    };
    let out = chat(SUMMARY_PROMPT, text).await?;
    let out = strip_think(&out).trim().to_string();
    if out.is_empty() {
        return Err("The model returned nothing.".into());
    }
    Ok(out)
}

/// Reasoning models (Qwen3, DeepSeek R1) put their thinking in <think>…</think>.
fn strip_think(s: &str) -> &str {
    match s.find("</think>") {
        Some(i) => &s[i + "</think>".len()..],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcripts::{Kind, Line, Speaker};

    fn words(n: usize) -> String {
        vec!["w"; n].join(" ")
    }

    #[test]
    fn short_paragraphs_share_a_piece_until_it_is_full() {
        let paras = vec![words(40), words(40), words(40)];
        let pieces = split(&paras, 100);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].split_whitespace().count(), 80);
        assert_eq!(pieces[1].split_whitespace().count(), 40);
    }

    #[test]
    fn an_overlong_paragraph_is_cut_and_no_word_is_lost() {
        let paras = vec![words(10), words(250), words(10)];
        let pieces = split(&paras, 100);
        let total: usize = pieces.iter().map(|p| p.split_whitespace().count()).sum();
        assert_eq!(total, 270);
        assert!(pieces.iter().all(|p| p.split_whitespace().count() <= 100));
    }

    fn meeting(text_words: usize, lines: usize) -> Transcript {
        let mut t = Transcript::new(1, Kind::Meeting, String::new(), String::new(), "m".into());
        for i in 0..lines {
            let speaker = Some(if i % 2 == 0 { Speaker::You } else { Speaker::Others });
            t.lines.push(Line { start_ms: i as u64 * 1000, end_ms: i as u64 * 1000 + 900, text: words(text_words), speaker });
        }
        t
    }

    fn run(t: &Transcript, max: usize) -> (Result<String, String>, Vec<(&'static str, String)>) {
        let mut calls = Vec::new();
        let r = tokio::runtime::Runtime::new().unwrap().block_on(summarize(t, max, |p, text| {
            calls.push((p, text));
            async { Ok("<think>hmm</think>\n## Summary\nok".to_string()) }
        }));
        (r, calls)
    }

    #[test]
    fn a_short_meeting_is_one_labelled_call() {
        let (r, calls) = run(&meeting(5, 2), 1000);
        assert_eq!(r.unwrap(), "## Summary\nok", "thinking must be stripped");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SUMMARY_PROMPT);
        assert!(calls[0].1.starts_with("You: ") && calls[0].1.contains("\n\nOthers: "));
    }

    #[test]
    fn a_long_meeting_is_noted_per_piece_then_merged() {
        let (r, calls) = run(&meeting(60, 4), 100);
        assert!(r.is_ok());
        let prompts: Vec<_> = calls.iter().map(|c| c.0).collect();
        assert_eq!(prompts, [PIECE_PROMPT, PIECE_PROMPT, PIECE_PROMPT, PIECE_PROMPT, SUMMARY_PROMPT]);
        assert!(calls[4].1.contains("Part 4:"));
    }

    #[test]
    fn an_empty_meeting_makes_no_call() {
        let (r, calls) = run(&meeting(5, 0), 100);
        assert!(r.is_err());
        assert!(calls.is_empty());
    }
}
