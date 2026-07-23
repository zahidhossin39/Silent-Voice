use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use ndarray::{Array2, Array3};
use ort::init_from;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

struct Coedit {
    encoder: std::sync::Mutex<Session>,
    decoder: std::sync::Mutex<Session>,
    tokenizer: Tokenizer,
}

static COEDIT: Mutex<Option<Arc<Coedit>>> = Mutex::new(None);
static LAST_USED: Mutex<Option<Instant>> = Mutex::new(None);
static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn cache_get(key: &str) -> Option<String> {
    CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|m| m.get(key).cloned())
}

fn cache_put(key: String, value: String) {
    if let Ok(mut m) = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        // Crude bound: repeated dictation rarely exceeds this; clear wholesale
        // rather than track LRU.
        if m.len() >= 256 {
            m.clear();
        }
        m.insert(key, value);
    }
}

#[cfg(test)]
fn is_loaded() -> bool {
    COEDIT.lock().map(|g| g.is_some()).unwrap_or(false)
}

pub fn unload_if_idle(max_idle: Duration) {
    let idle = LAST_USED
        .lock()
        .ok()
        .and_then(|t| *t)
        .map(|t| t.elapsed() >= max_idle)
        .unwrap_or(false);
    if idle {
        if let Ok(mut slot) = COEDIT.lock() {
            if slot.take().is_some() {
                crate::logging::log_info("coedit", "unloaded idle model");
            }
        }
        if let Ok(mut t) = LAST_USED.lock() {
            *t = None;
        }
    }
}

fn init_coedit() -> Option<Coedit> {
    let base_dir = crate::models::registry::coedit_dir();

    let encoder_path = base_dir.join("encoder_model_int8.onnx");
    let decoder_path = base_dir.join("decoder_model_int8.onnx");
    let tokenizer_path = base_dir.join("tokenizer.json");

    if !encoder_path.exists() || !decoder_path.exists() || !tokenizer_path.exists() {
        return None;
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))?;
    let dll_path = [Some(exe_dir.as_path()), exe_dir.parent()]
        .into_iter()
        .flatten()
        .map(|d| d.join("sherpa").join("onnxruntime.dll"))
        .find(|p| p.exists())?;
    let _ = init_from(dll_path.display().to_string()).commit();

    // Using ALL logical cores thrashes BLAS on hyperthreaded CPUs — use half.
    let cores = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4);
    let threads = (cores / 2).max(2);

    let encoder = match Session::builder()
        .and_then(|b| b.with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3))
        .and_then(|b| b.with_intra_threads(threads))
        .and_then(|b| b.with_inter_threads(1))
        .and_then(|b| b.commit_from_file(&encoder_path))
    {
        Ok(s) => s,
        Err(e) => {
            crate::logging::log_error("coedit", &format!("Failed to load ONNX encoder session: {}", e));
            return None;
        }
    };

    let decoder = match Session::builder()
        .and_then(|b| b.with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3))
        .and_then(|b| b.with_intra_threads(threads))
        .and_then(|b| b.with_inter_threads(1))
        .and_then(|b| b.commit_from_file(&decoder_path))
    {
        Ok(s) => s,
        Err(e) => {
            crate::logging::log_error("coedit", &format!("Failed to load ONNX decoder session: {}", e));
            return None;
        }
    };

    let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
        Ok(t) => t,
        Err(e) => {
            crate::logging::log_error("coedit", &format!("Failed to load tokenizer: {}", e));
            return None;
        }
    };

    Some(Coedit {
        encoder: std::sync::Mutex::new(encoder),
        decoder: std::sync::Mutex::new(decoder),
        tokenizer,
    })
}

pub fn prewarm() {
    let mut slot = match COEDIT.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    if slot.is_none() {
        *slot = init_coedit().map(Arc::new);
    }
}

pub fn correct(text: &str) -> String {
    let text_trim = text.trim();
    if text_trim.is_empty() {
        return text.to_string();
    }

    // Gate: single-word utterances ("yes", "okay", a command) get no useful
    // grammar correction and risk being mangled — skip the model.
    if text_trim.split_whitespace().count() < 2 {
        return text.to_string();
    }

    // Cache: identical prior input returns instantly, no model call.
    if let Some(hit) = cache_get(text_trim) {
        return hit;
    }

    let coedit = {
        let mut slot = match COEDIT.lock() {
            Ok(s) => s,
            Err(_) => return text.to_string(),
        };
        if slot.is_none() {
            *slot = init_coedit().map(Arc::new);
        }
        slot.clone()
    };

    let Some(c) = coedit else {
        return text.to_string();
    };
    if let Ok(mut t) = LAST_USED.lock() {
        *t = Some(Instant::now());
    }

    let prompt = format!("Fix grammar: {}", text_trim);
    let encoding = match c.tokenizer.encode(prompt, true) {
        Ok(e) => e,
        Err(_) => return text.to_string(),
    };

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let attn: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
    let enc_len = input_ids.len();

    let input_ids_arr = match Array2::from_shape_vec((1, enc_len), input_ids) {
        Ok(a) => a,
        Err(_) => return text.to_string(),
    };
    let attn_arr = match Array2::from_shape_vec((1, enc_len), attn.clone()) {
        Ok(a) => a,
        Err(_) => return text.to_string(),
    };

    let (Ok(input_ids_t), Ok(attention_mask_t)) = (
        Tensor::from_array(input_ids_arr),
        Tensor::from_array(attn_arr),
    ) else {
        return text.to_string();
    };

    let encoder_inputs = ort::inputs![
        "input_ids" => input_ids_t,
        "attention_mask" => attention_mask_t,
    ];

    let hidden_shape;
    let hidden_data: Vec<f32>;
    {
        let Ok(mut encoder) = c.encoder.lock() else {
            return text.to_string();
        };
        let outputs = match encoder.run(encoder_inputs) {
            Ok(o) => o,
            Err(_) => return text.to_string(),
        };
        let (shape, data) = match outputs["last_hidden_state"].try_extract_tensor::<f32>() {
            Ok(t) => t,
            Err(_) => return text.to_string(),
        };
        hidden_shape = shape.to_vec();
        hidden_data = data.to_vec();
    }

    if hidden_shape.len() != 3 || hidden_shape[0] != 1 || hidden_shape[1] as usize != enc_len || hidden_shape[2] != 1024 {
        return text.to_string();
    }

    let mut dec_ids: Vec<i64> = vec![0];
    let max_steps = (enc_len + 16).min(128);

    let vocab = 32100;

    for _ in 0..max_steps {
        let cur = dec_ids.len();
        
        let dec_input = match Array2::from_shape_vec((1, cur), dec_ids.clone()) {
            Ok(a) => a,
            Err(_) => return text.to_string(),
        };
        let enc_hidden = match Array3::from_shape_vec((1, enc_len, 1024), hidden_data.clone()) {
            Ok(a) => a,
            Err(_) => return text.to_string(),
        };
        let enc_mask = match Array2::from_shape_vec((1, enc_len), attn.clone()) {
            Ok(a) => a,
            Err(_) => return text.to_string(),
        };

        let (Ok(dec_input_t), Ok(enc_hidden_t), Ok(enc_mask_t)) = (
            Tensor::from_array(dec_input),
            Tensor::from_array(enc_hidden),
            Tensor::from_array(enc_mask),
        ) else {
            return text.to_string();
        };

        let decoder_inputs = ort::inputs![
            "input_ids" => dec_input_t,
            "encoder_hidden_states" => enc_hidden_t,
            "encoder_attention_mask" => enc_mask_t,
        ];

        let logits_data;
        {
            let Ok(mut decoder) = c.decoder.lock() else {
                return text.to_string();
            };
            let outputs = match decoder.run(decoder_inputs) {
                Ok(o) => o,
                Err(_) => return text.to_string(),
            };
            let (shape, data) = match outputs["logits"].try_extract_tensor::<f32>() {
                Ok(t) => t,
                Err(_) => return text.to_string(),
            };
            if shape.len() != 3 || shape[0] != 1 || shape[1] as usize != cur || shape[2] as usize != vocab {
                return text.to_string();
            }
            logits_data = data.to_vec();
        }

        let last = cur - 1;
        let base = last * vocab;
        
        let next = logits_data[base..base+vocab]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i as i64)
            .unwrap();

        if next == 1 {
            break;
        }
        dec_ids.push(next);
    }

    let out_ids: Vec<u32> = dec_ids[1..].iter().map(|&x| x as u32).collect();
    let corrected = match c.tokenizer.decode(&out_ids, true) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return text.to_string(),
    };

    if corrected.is_empty() {
        return text.to_string();
    }

    if corrected.chars().count() > text.chars().count() * 3 + 30 {
        return text.to_string();
    }

    // Digit-preservation: the model must never alter numbers the user dictated
    // (format_numbers handles formatting later). If the digit stream changed,
    // reject the whole correction.
    let in_digits: String = text_trim.chars().filter(|c| c.is_ascii_digit()).collect();
    let out_digits: String = corrected.chars().filter(|c| c.is_ascii_digit()).collect();
    if in_digits != out_digits {
        cache_put(text_trim.to_string(), text.to_string());
        return text.to_string();
    }

    // Over-rewrite guard: a real grammar fix keeps most of the original words.
    // If the output dropped more than half the input words, it is a hallucinated
    // rewrite — reject it.
    let in_words: Vec<&str> = text_trim.split_whitespace().collect();
    if in_words.len() >= 4 {
        let out_lower: std::collections::HashSet<String> =
            corrected.split_whitespace().map(|w| w.to_lowercase()).collect();
        let kept = in_words
            .iter()
            .filter(|w| out_lower.contains(&w.to_lowercase()))
            .count();
        if (kept as f32) < 0.5 * in_words.len() as f32 {
            cache_put(text_trim.to_string(), text.to_string());
            return text.to_string();
        }
    }

    cache_put(text_trim.to_string(), corrected.clone());
    corrected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corrects_homophones() {
        let text = "their going to they're house over there.";
        let corrected = correct(text);
        if !is_loaded() {
            return;
        }
        assert!(corrected.contains("They're"));
        assert!(corrected.contains("their"));
        assert_ne!(corrected, text);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(correct(""), "");
    }

    #[test]
    fn test_single_word_skipped() {
        // Gate: one-word input returns unchanged without touching the model.
        assert_eq!(correct("hello"), "hello");
        assert_eq!(correct("  yes  "), "  yes  ");
    }

    #[test]
    fn test_already_correct() {
        let text = "This is a correct sentence.";
        let corrected = correct(text);
        if is_loaded() {
            assert!(!corrected.is_empty());
        }
    }
}
