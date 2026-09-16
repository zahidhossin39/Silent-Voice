# GECToR Rust Implementation Plan

## 1. `ort` Crate & Cargo Features
- **Version**: We will use `ort = "2.0"` (or a stable 1.x version depending on requirements).
- **Features**: Enable the `load-dynamic` feature. This prevents static linking and allows the app to load an external `onnxruntime.dll` at runtime.
- **Version Compat**: Since the app already ships an `onnxruntime.dll` for Sherpa, we must ensure the `ort` crate version is compatible with the shared library's ORT C API version. (Usually, ORT guarantees backward compatibility, but the specific `ort` crate version should be tested against the ONNX Runtime DLL version shipped with Sherpa, ideally v1.14+).

## 2. Tokenization with `tokenizers` Crate
- Use the `tokenizers` crate to load `tokenizer.json` (RoBERTa tokenizer).
- **Whole-Sentence vs Per-Word Encoding Tradeoffs**:
  - *Per-Word (`add_special_tokens=false`)*: It trivially yields the first subtoken for alignment. However, RoBERTa relies on prefix spaces (the `Ġ` character) for BPE matching. Encoding words individually strips their contextual prefix space, leading to incorrect subtokens compared to the Python `transformers` training data.
  - *Whole-Sentence*: **(Recommended)** Encode the entire sentence. The `tokenizers` crate returns an `Encoding` object that contains `.word_ids()`. This maps each subtoken back to its original word index! We can simply scan the subtokens and pick the first subtoken that corresponds to each `word_id`.
- Remember to handle the `<s>` and `</s>` special tokens appropriately, ignoring them during tag extraction.

## 3. CPU Inference Latency
- **Realistic Latency (roberta-base INT8)**: On a modern Windows CPU, a single forward pass of a quantized `roberta-base` INT8 ONNX model on a short sentence (15-30 tokens) typically takes **15ms - 40ms**.
- For 3 iterations (an average correction loop), the total latency would be roughly **45ms - 120ms**, which is well within interactive/real-time thresholds for a background spelling/grammar checker.

## 4. Pitfalls & Watch-outs
- **Tokenization Alignment Mismatch**: The biggest pitfall. GECToR requires the exact tokenization alignment (the first subtoken of each word). If your word splitting logic (regex/whitespaces) differs from the original python implementation, the tagging indices will shift, destroying the model's accuracy.
- **RoBERTa's `Ġ` (Space) character**: When decoding or applying subtokens, remember that RoBERTa's tokenizer uses `Ġ` to denote spaces.
- **Iterative Offset Drift**: If you try to track character offsets manually across 5 iterations of insertions and deletions, the indices can easily become corrupted. It's safer to apply the string edits to form the final corrected sentence, and then run a standard diff (like the `similar` crate) against the original string to get char-level spans.
- **Softmax vs Logits**: The confidence bias (`0.2`) must be added to the `$KEEP` index **BEFORE** the softmax normalization. Adding it after softmax is mathematically incorrect and will ruin the min-error probability thresholding.
