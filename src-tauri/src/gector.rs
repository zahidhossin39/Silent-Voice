use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use ort::init_from;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

struct Gector {
    session: std::sync::Mutex<Session>,
    tokenizer: Tokenizer,
    labels: Vec<String>,
    verb_map: HashMap<(String, String), String>,
}

static GECTOR: OnceLock<Option<Gector>> = OnceLock::new();

fn parse_verb_vocab(text: &str) -> HashMap<(String, String), String> {
    let mut verb_map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((words, tags)) = line.split_once(':') {
            if let Some((source, target)) = words.split_once('_') {
                verb_map.insert((source.to_string(), tags.to_string()), target.to_string());
            }
        }
    }
    verb_map
}

fn init_gector() -> Option<Gector> {
    let base_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("SilentVoice")
        .join("models")
        .join("gector");

    let model_path = base_dir.join("gector-int8.onnx");
    let tokenizer_path = base_dir.join("tokenizer.json");
    let labels_path = base_dir.join("labels.txt");
    let verbs_path = base_dir.join("verb-form-vocab.txt");

    if !model_path.exists() || !tokenizer_path.exists() || !labels_path.exists() || !verbs_path.exists() {
        return None;
    }

    // Reuse sherpa's onnxruntime.dll (see sherpa.rs on why absolute paths).
    // ort PANICS if the dylib can't be loaded, so verify existence first —
    // a panic here would kill the inline-check watcher thread.
    // Test exes live in target\debug\deps\, one level below the DLL dir.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))?;
    let dll_path = [Some(exe_dir.as_path()), exe_dir.parent()]
        .into_iter()
        .flatten()
        .map(|d| d.join("sherpa").join("onnxruntime.dll"))
        .find(|p| p.exists())?;
    let _ = init_from(dll_path.display().to_string()).commit();

    let session = match Session::builder()
        .and_then(|b| b.with_intra_threads(1))
        .and_then(|b| b.commit_from_file(&model_path))
    {
        Ok(s) => s,
        Err(e) => {
            crate::logging::log_error("gector", &format!("Failed to load ONNX session: {}", e));
            return None;
        }
    };

    let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
        Ok(t) => t,
        Err(e) => {
            crate::logging::log_error("gector", &format!("Failed to load tokenizer: {}", e));
            return None;
        }
    };

    let labels_text = match std::fs::read_to_string(&labels_path) {
        Ok(t) => t,
        Err(e) => {
            crate::logging::log_error("gector", &format!("Failed to read labels: {}", e));
            return None;
        }
    };
    let labels: Vec<String> = labels_text.lines().map(|s| s.trim().to_string()).collect();

    let verbs_text = match std::fs::read_to_string(&verbs_path) {
        Ok(t) => t,
        Err(e) => {
            crate::logging::log_error("gector", &format!("Failed to read verbs: {}", e));
            return None;
        }
    };

    let verb_map = parse_verb_vocab(&verbs_text);

    Some(Gector {
        session: std::sync::Mutex::new(session),
        tokenizer,
        labels,
        verb_map,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct GectorEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub tag: String,
    pub message: String,
}

struct WordSpan {
    text: String,
    start: usize,
    end: usize,
}

static CACHE: OnceLock<std::sync::Mutex<HashMap<String, Vec<GectorEdit>>>> = OnceLock::new();

fn get_cache() -> &'static std::sync::Mutex<HashMap<String, Vec<GectorEdit>>> {
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// The tokenizers crate reports BYTE offsets into the encoded string, while
/// ProofIssue (and everything downstream) uses CHAR indices. Non-boundary or
/// out-of-range values clamp to the containing char.
fn resolve_char_offsets(text: &str, offsets: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut byte_to_char = vec![0usize; text.len() + 1];
    let mut char_idx = 0;
    for (byte_idx, ch) in text.char_indices() {
        for b in byte_idx..byte_idx + ch.len_utf8() {
            byte_to_char[b] = char_idx;
        }
        char_idx += 1;
    }
    byte_to_char[text.len()] = char_idx;
    offsets
        .iter()
        .map(|&(s, e)| (byte_to_char[s.min(text.len())], byte_to_char[e.min(text.len())]))
        .collect()
}

/// Naive s/es pluralization mangles short function words ("is" -> "ises");
/// agreement transforms only make sense on content nouns.
fn is_agreement_blocked(word: &str) -> bool {
    const BLOCK: &[&str] = &[
        "is", "as", "was", "has", "his", "its", "this", "thus", "does", "goes", "yes", "us",
        "plus", "ours", "hers", "yours", "theirs", "whose", "less", "unless", "perhaps",
        "always", "besides", "of", "off",
    ];
    word.chars().count() < 3 || BLOCK.contains(&word.to_lowercase().as_str())
}

pub fn check(text: &str, sensitivity: &str) -> Vec<GectorEdit> {
    let (label_thresh, gate_thresh) = match sensitivity {
        "relaxed" => (0.60, 0.60),
        "aggressive" => (0.30, 0.40),
        _ => (0.45, 0.50), // "balanced" default
    };

    let mut edits = Vec::new();
    let gector = GECTOR.get_or_init(init_gector);

    let Some(g) = gector else {
        return edits;
    };

    let keep_idx = g.labels.iter().position(|l| l == "$KEEP").unwrap_or(0);

    let mut sentence_start = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = Vec::new();

    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '.' || chars[i] == '!' || chars[i] == '?' {
            let mut end = i + 1;
            while end < chars.len() && chars[end].is_whitespace() {
                end += 1;
            }
            sentences.push((sentence_start, end));
            sentence_start = end;
            i = end;
        } else {
            i += 1;
        }
    }
    if sentence_start < chars.len() {
        sentences.push((sentence_start, chars.len()));
    }

    for (s_start, s_end) in sentences {
        if s_end - s_start > 350 {
            continue;
        }

        let sentence_chars = &chars[s_start..s_end];
        let sentence_text: String = sentence_chars.iter().collect();

        {
            let mut cache = get_cache().lock().unwrap();
            if let Some(cached_edits) = cache.get(&sentence_text) {
                for mut edit in cached_edits.clone() {
                    edit.start += s_start;
                    edit.end += s_start;
                    edits.push(edit);
                }
                continue;
            }
        }

        let encoded_text = format!("$START {}", sentence_text);

        let encoding = match g.tokenizer.encode(encoded_text.clone(), true) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let word_ids = encoding.get_word_ids();
        let raw_offsets = encoding.get_offsets();
        let char_offsets = resolve_char_offsets(&encoded_text, raw_offsets);

        let mut word_ranges: std::collections::BTreeMap<usize, (usize, usize)> = std::collections::BTreeMap::new();
        for (tok_idx, &word_id_opt) in word_ids.iter().enumerate() {
            if let Some(w_id) = word_id_opt {
                let w_id = w_id as usize;
                if w_id > 0 { // skip $START
                    let (c_start, c_end) = char_offsets[tok_idx];
                    let entry = word_ranges.entry(w_id).or_insert((c_start, c_end));
                    entry.0 = entry.0.min(c_start);
                    entry.1 = entry.1.max(c_end);
                }
            }
        }

        let mut words = Vec::new();
        let mut max_w_id = 0;
        if let Some(&m) = word_ranges.keys().max() {
            max_w_id = m;
        }
        
        for w_id in 1..=max_w_id {
            if let Some(&(mut c_start, mut c_end)) = word_ranges.get(&w_id) {
                if c_start >= 7 { c_start -= 7; } else { c_start = 0; }
                if c_end >= 7 { c_end -= 7; } else { c_end = 0; }
                c_start = c_start.min(sentence_chars.len());
                c_end = c_end.min(sentence_chars.len());
                
                while c_start < c_end && c_start < sentence_chars.len() && sentence_chars[c_start] == ' ' {
                    c_start += 1;
                }
                
                let w_text: String = sentence_chars[c_start..c_end].iter().collect();
                words.push(WordSpan {
                    text: w_text,
                    start: c_start,
                    end: c_end,
                });
            } else {
                words.push(WordSpan {
                    text: String::new(),
                    start: 0,
                    end: 0,
                });
            }
        }

        if words.is_empty() || words.len() > 60 {
            continue;
        }

        let mut first_subtokens = vec![None; words.len() + 1];
        for (tok_idx, &word_id_opt) in word_ids.iter().enumerate() {
            if let Some(w_id) = word_id_opt {
                let w_id = w_id as usize;
                if w_id < first_subtokens.len() && first_subtokens[w_id].is_none() {
                    first_subtokens[w_id] = Some(tok_idx);
                }
            }
        }

        let mut sentence_edits = Vec::new();

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let seq_len = input_ids.len();

        let input_ids_arr = match ndarray::Array2::from_shape_vec(
            (1, seq_len),
            input_ids.iter().map(|&x| x as i64).collect(),
        ) {
            Ok(a) => a,
            Err(_) => continue,
        };
        let attention_mask_arr = match ndarray::Array2::from_shape_vec(
            (1, seq_len),
            attention_mask.iter().map(|&x| x as i64).collect(),
        ) {
            Ok(a) => a,
            Err(_) => continue,
        };

        let (Ok(input_ids_t), Ok(attention_mask_t)) = (
            Tensor::from_array(input_ids_arr),
            Tensor::from_array(attention_mask_arr),
        ) else {
            continue;
        };
        let inputs = ort::inputs![
            "input_ids" => input_ids_t,
            "attention_mask" => attention_mask_t,
        ];

        let Ok(mut session) = g.session.lock() else {
            continue;
        };
        let outputs = match session.run(inputs) {
            Ok(o) => o,
            Err(_) => continue,
        };

        // rc.10 returns (shape, flat data); batch is 1 so the layout is
        // [seq][num_labels] row-major.
        let (shape, logit_data) = match outputs["label_logits"].try_extract_tensor::<f32>() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let num_labels = g.labels.len();
        if shape.len() != 3 || shape[2] as usize != num_labels {
            continue;
        }

        // Detect-head gate (original GECToR postprocess_batch): skip the whole
        // sentence unless the max per-token INCORRECT probability clears a
        // threshold. This is the model's own false-positive filter — without
        // it, borderline label-head tags fire on clean sentences.
        // dtags order: 0 = $CORRECT, 1 = $INCORRECT (2-class softmax).
        if let Ok((d_shape, d_data)) = outputs["detect_logits"].try_extract_tensor::<f32>() {
            if d_shape.len() == 3 && d_shape[2] == 2 {
                let mut max_incorrect = 0.0f32;
                for (tok_idx, &word_id_opt) in word_ids.iter().enumerate() {
                    if word_id_opt.is_none() {
                        continue; // specials/padding
                    }
                    let row = tok_idx * 2;
                    if let Some(pair) = d_data.get(row..row + 2) {
                        let m = pair[0].max(pair[1]);
                        let e0 = (pair[0] - m).exp();
                        let e1 = (pair[1] - m).exp();
                        max_incorrect = max_incorrect.max(e1 / (e0 + e1));
                    }
                }
                if max_incorrect < gate_thresh {
                    get_cache().lock().map(|mut c| c.insert(sentence_text.clone(), Vec::new())).ok();
                    continue;
                }
            }
        }

        for w_id in 0..=words.len() {
            if let Some(sub_idx) = first_subtokens[w_id] {
                if sub_idx >= seq_len {
                    continue;
                }

                let row = sub_idx * num_labels;
                let Some(logits) = logit_data.get(row..row + num_labels) else {
                    continue;
                };
                let logits: Vec<f32> = logits.to_vec();

                let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let mut exp_sum = 0.0;
                let mut probs: Vec<f32> = logits
                    .iter()
                    .map(|&l| {
                        let e = (l - max_logit).exp();
                        exp_sum += e;
                        e
                    })
                    .collect();

                for p in probs.iter_mut() {
                    *p /= exp_sum;
                }

                let mut best_idx = 0;
                let mut best_prob = -1.0;
                for (idx, &p) in probs.iter().enumerate() {
                    if p > best_prob {
                        best_prob = p;
                        best_idx = idx;
                    }
                }

                let tag = &g.labels[best_idx];
                // 0.45 threshold tuned against this checkpoint (README's 0.2
                // bias + 0.5 was tuned for grammarly's own weights and
                // rejects real errors like "go"->"goes" at 0.47).
                if best_idx == keep_idx || tag == "<OOV>" || best_prob < label_thresh {
                    continue;
                }

                if w_id == 0 {
                    if let Some(t) = tag.strip_prefix("$APPEND_") {
                        if !words.is_empty() {
                            let first_word = &words[0];
                            let is_punct = t.chars().all(|c| c.is_ascii_punctuation() || !c.is_alphanumeric());
                            let repl = if is_punct {
                                format!("{}", t)
                            } else {
                                format!("{} ", t)
                            };
                            let current_text: String = sentence_chars[first_word.start..first_word.start].iter().collect();
                            if repl != current_text {
                                sentence_edits.push(GectorEdit {
                                    start: first_word.start,
                                    end: first_word.start,
                                    replacement: repl,
                                    tag: tag.clone(),
                                    message: "Possibly missing word".to_string(),
                                });
                            }
                        }
                    }
                    continue;
                }

                let word_idx = w_id - 1;
                let w = &words[word_idx];

                let mut replacement = None;
                let mut message = "Grammar or style suggestion".to_string();
                let mut edit_start = w.start;
                let mut edit_end = w.end;

                if tag == "$DELETE" {
                    replacement = Some("".to_string());
                    message = "Possibly redundant word".to_string();
                } else if let Some(t) = tag.strip_prefix("$APPEND_") {
                    edit_start = w.end;
                    edit_end = w.end;
                    // Contractions ('s, n't, 'll) attach directly, like punctuation.
                    let is_punct = t.starts_with('\'')
                        || t.chars().all(|c| c.is_ascii_punctuation() || !c.is_alphanumeric());
                    if is_punct {
                        replacement = Some(t.to_string());
                    } else {
                        replacement = Some(format!(" {}", t));
                    }
                    message = "Possibly missing word".to_string();
                } else if let Some(t) = tag.strip_prefix("$REPLACE_") {
                    replacement = Some(t.to_string());
                    message = "Possible word confusion".to_string();
                } else if tag == "$MERGE_SPACE" {
                    if word_idx + 1 < words.len() {
                        let next = &words[word_idx + 1];
                        edit_start = w.end;
                        edit_end = next.start;
                        replacement = Some("".to_string());
                        message = "Should be a single word".to_string();
                    }
                } else if tag == "$MERGE_HYPHEN" {
                    if word_idx + 1 < words.len() {
                        let next = &words[word_idx + 1];
                        edit_start = w.end;
                        edit_end = next.start;
                        replacement = Some("-".to_string());
                        message = "Should be hyphenated".to_string();
                    }
                } else if tag == "$TRANSFORM_SPLIT_HYPHEN" {
                    replacement = Some(w.text.replace('-', " "));
                    message = "Should be split".to_string();
                } else if tag == "$TRANSFORM_CASE_CAPITAL" {
                    let mut c = w.text.chars();
                    if let Some(first) = c.next() {
                        replacement = Some(format!("{}{}", first.to_uppercase(), c.as_str().to_lowercase()));
                    }
                    message = "Capitalization".to_string();
                } else if tag == "$TRANSFORM_CASE_CAPITAL_1" {
                    let mut c = w.text.chars();
                    if let Some(first) = c.next() {
                        replacement = Some(format!("{}{}", first.to_uppercase(), c.as_str()));
                    }
                    message = "Capitalization".to_string();
                } else if tag == "$TRANSFORM_CASE_LOWER" {
                    replacement = Some(w.text.to_lowercase());
                    message = "Capitalization".to_string();
                } else if tag == "$TRANSFORM_CASE_UPPER" {
                    replacement = Some(w.text.to_uppercase());
                    message = "Capitalization".to_string();
                } else if tag == "$TRANSFORM_AGREEMENT_SINGULAR" {
                    if !is_agreement_blocked(&w.text) {
                        let lower = w.text.to_lowercase();
                        if lower.ends_with("ies") {
                            replacement = Some(format!("{}y", &w.text[..w.text.len() - 3]));
                        } else if lower.ends_with('s') && !lower.ends_with("ss") {
                            replacement = Some(w.text[..w.text.len() - 1].to_string());
                        } else {
                            replacement = Some(w.text.clone());
                        }
                        message = "Grammar: agreement".to_string();
                    }
                } else if tag == "$TRANSFORM_AGREEMENT_PLURAL" {
                    if !is_agreement_blocked(&w.text) {
                        let lower = w.text.to_lowercase();
                        if lower.ends_with('y') && !lower.ends_with("ay") && !lower.ends_with("ey") && !lower.ends_with("oy") && !lower.ends_with("uy") {
                            replacement = Some(format!("{}ies", &w.text[..w.text.len() - 1]));
                        } else if lower.ends_with('s') || lower.ends_with("sh") || lower.ends_with("ch") || lower.ends_with('x') || lower.ends_with('z') {
                            replacement = Some(format!("{}es", w.text));
                        } else {
                            replacement = Some(format!("{}s", w.text));
                        }
                        message = "Grammar: agreement".to_string();
                    }
                } else if let Some(t) = tag.strip_prefix("$TRANSFORM_VERB_") {
                    let lower = w.text.to_lowercase();
                    if let Some(target) = g.verb_map.get(&(lower, t.to_string())) {
                        let mut c = w.text.chars();
                        if let Some(first) = c.next() {
                            if first.is_uppercase() {
                                let mut tc = target.chars();
                                if let Some(tfirst) = tc.next() {
                                    replacement = Some(format!("{}{}", tfirst.to_uppercase(), tc.as_str()));
                                }
                            } else {
                                replacement = Some(target.clone());
                            }
                        }
                    }
                    message = "Grammar: verb form".to_string();
                }

                if let Some(repl) = replacement {
                    let current_text: String = sentence_chars[edit_start..edit_end].iter().collect();
                    if repl != current_text {
                        if !tag.starts_with("$MERGE_") || edit_start != edit_end {
                            sentence_edits.push(GectorEdit {
                                start: edit_start,
                                end: edit_end,
                                replacement: repl,
                                tag: tag.clone(),
                                message,
                            });
                        }
                    }
                }
            }
        }

        {
            let mut cache = get_cache().lock().unwrap();
            if cache.len() >= 256 {
                cache.clear();
            }
            cache.insert(sentence_text, sentence_edits.clone());
        }

        for mut edit in sentence_edits {
            edit.start += s_start;
            edit.end += s_start;
            edits.push(edit);
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_verb_vocab() {
        let sample = "abandon_abandoned:VB_VBD\nabandon_abandons:VB_VBZ";
        let map = parse_verb_vocab(sample);
        assert_eq!(map.get(&("abandon".to_string(), "VB_VBD".to_string())), Some(&"abandoned".to_string()));
        assert_eq!(map.get(&("abandon".to_string(), "VB_VBZ".to_string())), Some(&"abandons".to_string()));
    }

    #[test]
    fn test_gector_inference() {
        let edits = check("I have alot of work .", "balanced");
        if GECTOR.get().is_some() && GECTOR.get().unwrap().is_some() {
            assert!(!edits.is_empty(), "expected some edit for alot");
        } else {
            println!("test_gector_inference skip: model missing");
        }

        let clean = check("This sentence is perfectly fine.", "balanced");
        if GECTOR.get().is_some() && GECTOR.get().unwrap().is_some() {
            assert!(clean.is_empty(), "expected no edits for clean text");
        }
    }

    #[test]
    fn test_punctuation_alignment() {
        // Mid-sentence punctuation used to shift every following tag onto the
        // wrong word ("is" -> "ises" garbage). With tokenizer-derived words,
        // the flagged range must exactly cover the offending word.
        let text = "I know, I have alot of work.";
        let edits = check(text, "balanced");
        if GECTOR.get().map(|g| g.is_some()) != Some(true) {
            println!("test_punctuation_alignment skip: model missing");
            return;
        }
        let chars: Vec<char> = text.chars().collect();
        let alot = edits.iter().find(|e| {
            chars.get(e.start..e.end).map(|c| c.iter().collect::<String>()) == Some("alot".into())
        });
        assert!(alot.is_some(), "expected 'alot' flagged at exact offsets, got: {:?}",
            edits.iter().map(|e| (e.start, e.end, &e.replacement)).collect::<Vec<_>>());

        // The user's real sentence that produced "is" -> "ises".
        let text2 = "the underlying is not appearing smoothly, I mean it should be smooth.";
        let edits2 = check(text2, "balanced");
        assert!(
            !edits2.iter().any(|e| e.replacement.contains("ises")),
            "agreement garbage returned: {:?}",
            edits2.iter().map(|e| (&e.tag, &e.replacement)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_resolve_char_offsets() {
        let text = "I have alot of cafe\u{301} work.";
        let byte_offsets = vec![(15, 21)];
        let char_offsets = resolve_char_offsets(text, &byte_offsets);
        assert_eq!(char_offsets, vec![(15, 20)]);
        
        let chars: Vec<char> = text.chars().collect();
        let extracted: String = chars[char_offsets[0].0..char_offsets[0].1].iter().collect();
        assert_eq!(extracted, "cafe\u{301}");
    }
}
