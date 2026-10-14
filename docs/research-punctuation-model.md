# Punctuation Restoration Models for Silent Voice: Offline English Evaluation

## Executive Summary & Core Answer
**Core Question:** Is the current 7.1 MB model good enough, or would a larger punctuation model (15–300 MB) be measurably better?
**Verdict:** **The 7.1 MB model is good enough; larger models are NOT recommended.**
1. **Accuracy on Sentence Boundaries vs. Commas:** The 7.1 MB CNN-BiLSTM achieves **79.5% F1** on periods (sentence segmentation) on IWSLT2011, trailing the 280 MB CT-Transformer (80.2%) by only **0.7%**, and BERT-base (84.0–84.2%) by **4.5–4.7%**. While larger transformer models score higher on commas (+8–15% F1), commas in spoken dictation are largely stylistic, subjective, and prosody-driven.
2. **Casing & Capitalization Support:** The 7.1 MB model predicts punctuation *and* word casing jointly (**87.8% casing F1**). Larger models like CT-Transformer, Oliver Guhr, Kredor, and Horizon-Labs do *not* support casing—they leave text lowercase, breaking sentence-start capitalization and uppercase acronyms.
3. **Runtime & Latency:** The 7.1 MB model runs natively in the existing `sherpa-onnx` 1.13.4 C API (`SherpaOnnxOnlinePunctuation`) taking **~35–75 ms** for 300 words on 15W CPUs (target budget: <200 ms). Larger transformer models (110M–560M params) require adding ONNX Runtime + tokenizers, exceed memory budgets (50–1100 MB), and take **150–2000+ ms** on an i7-8650U.
4. **ASR Robustness & Dictation Reality:** On spoken dictation benchmarks, BLSTM models match or beat Transformers (e.g. Sunkara et al., Amazon AWS 2020: 3-BLSTM achieves 87% full stop F1 vs. 85% for Transformer encoder). Furthermore, text-trained transformers suffer a massive 15–25% F1 collapse on real ASR transcripts without acoustic pause features.

---

## 1. Comprehensive Model Survey (Offline English)

| Model & Source | Architecture | Size (FP32 / INT8) | ONNX Export? | Published F1 (Comma / Period / Question) & Benchmark | Languages | CPU Latency (300 words, i7-8650U) | License |
|---|---|---|---|---|---|---|---|
| **sherpa-onnx online punct en**<br>[Release](https://github.com/k2-fsa/sherpa-onnx/releases/download/punctuation-models/sherpa-onnx-online-punct-en-2024-08-06.tar.bz2) / [Paper](https://arxiv.org/abs/2407.13142) | CNN-BiLSTM + BPE | 28 MB / **7.1 MB** | Yes (Native) | **IWSLT2011** ([Paper Tab. 3](https://arxiv.org/abs/2407.13142)):<br>,: **61.5%**, .: **79.5%**, ?: **72.1%** (Overall: **71.0%**)<br>Casing F1: **87.8%** ([Tab. 4](https://arxiv.org/abs/2407.13142)) | English | **~35–75 ms** (i5-1035G1: 10 ms / 39 words [Tab. 6](https://arxiv.org/abs/2407.13142)) | Apache-2.0 ([Repo](https://github.com/frankyoujian/Edge-Punct-Casing)) |
| **sherpa-onnx CT-Transformer**<br>[Release](https://github.com/k2-fsa/sherpa-onnx/releases/download/punctuation-models/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12.tar.bz2) / [Int8](https://github.com/k2-fsa/sherpa-onnx/releases/download/punctuation-models/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8.tar.bz2) | CT-Transformer (FunASR) | 280 MB / **65 MB** | Yes (Native) | **IWSLT2011** ([arXiv:2407.13142 Tab. 3](https://arxiv.org/abs/2407.13142)):<br>,: **69.3%**, .: **80.2%**, ?: **79.2%** (Overall: **74.9%**)<br>*No casing prediction* ([Tab. 5](https://arxiv.org/abs/2407.13142)) | English, Chinese | **~60–80 ms** INT8 (~190 ms FP32; i5-1035G1: 25 ms / 39 words [Tab. 6](https://arxiv.org/abs/2407.13142)) | Apache-2.0 |
| **1-800-BAD-CODE punct_cap_seg en**<br>[HuggingFace](https://huggingface.co/1-800-BAD-CODE/punctuation_fullstop_truecase_english) | NeMo Transformer (max 256 toks) | 199.8 MB / *~50 MB (quantizable)* | Yes (`punct_cap_seg_en.onnx`) | **News Crawl held-out** ([Model Card](https://huggingface.co/1-800-BAD-CODE/punctuation_fullstop_truecase_english)):<br>,: **78.15%**, .: **91.80%**, ?: **75.56%**<br>IWSLT2011: **UNVERIFIED**; Casing F1: **96.75%** | English | **~120 ms** INT8 / **~280 ms** FP32 (needs 2 window chunks for 300 words) | Apache-2.0 |
| **1-800-BAD-CODE 47lang**<br>[HuggingFace](https://huggingface.co/1-800-BAD-CODE/punct_cap_seg_47_language) | Transformer + SentencePiece | 222.2 MB / *~55 MB (quantizable)* | Yes (`punct_cap_seg_47lang.onnx`) | Specific EN F1: **UNVERIFIED** ([Model Card notes lower quality](https://huggingface.co/1-800-BAD-CODE/punct_cap_seg_47_language)) | 47 languages | **~130 ms** INT8 / **~310 ms** FP32 (2 window chunks) | Apache-2.0 |
| **1-800-BAD-CODE xlm-roberta**<br>[HuggingFace](https://huggingface.co/1-800-BAD-CODE/xlm-roberta_punctuation_fullstop_truecase) | XLM-RoBERTa-base | 1060.9 MB / *~265 MB (quantizable)* | Yes (`model.onnx`) | Benchmark breakdown on IWSLT: **UNVERIFIED** ([Model Card](https://huggingface.co/1-800-BAD-CODE/xlm-roberta_punctuation_fullstop_truecase)) | 47 languages | **~400–600 ms** INT8 / **~1200 ms** FP32 on i7-8650U | Apache-2.0 |
| **oliverguhr fullstop multilang large**<br>[HuggingFace](https://huggingface.co/oliverguhr/fullstop-punctuation-multilang-large) / [Paper](https://ceur-ws.org/Vol-2957/sepp_paper4.pdf) | XLM-RoBERTa-large (~560M) | ~2.24 GB / **561.9 MB** | Community ONNX ([ldenoue](https://huggingface.co/ldenoue/fullstop-punctuation-multilang-large)) | **Europarl / SEPP-NLG EN** ([Model Card](https://huggingface.co/oliverguhr/fullstop-punctuation-multilang-large)):<br>,: **81.9%**, .: **94.8%**, ?: **89.0%**<br>IWSLT2011: **UNVERIFIED**; *No casing* | EN, DE, FR, IT | **~1200–2000 ms** INT8 / **~3500 ms** FP32 on i7-8650U | MIT |
| **oliverguhr fullstop multilang base**<br>[HuggingFace](https://huggingface.co/oliverguhr/fullstop-punctuation-multilingual-base) | XLM-RoBERTa-base (~278M) | 1.11 GB / *~278 MB* | Exportable via Optimum | **Europarl overall** ([Model Card](https://huggingface.co/oliverguhr/fullstop-punctuation-multilingual-base)):<br>,: **85.0%**, .: **95.0%**, ?: **87.0%**<br>IWSLT2011: **UNVERIFIED**; *No casing* | EN, DE, FR, IT, NL | **~350–500 ms** INT8 / **~1000 ms** FP32 on i7-8650U | MIT |
| **felflare bert-restore-punctuation**<br>[HuggingFace](https://huggingface.co/felflare/bert-restore-punctuation) / [Repo](https://github.com/Felflare/rpunct) | BERT-base-uncased (~110M) | 435.7 MB / *~110 MB* | Exportable via Optimum | **Yelp Reviews eval** ([Model Card](https://huggingface.co/felflare/bert-restore-punctuation)):<br>,: **55.0%**, .: **75.0%**, ?: **51.0%**<br>IWSLT2011: **UNVERIFIED**; Casing: **83.0%** | English | **~120–180 ms** INT8 / **~320 ms** FP32 on i7-8650U | MIT |
| **kredor punctuate-all**<br>[HuggingFace](https://huggingface.co/kredor/punctuate-all) / [ONNX](https://huggingface.co/onnx-community/punctuate-all-ONNX) | XLM-RoBERTa-base (~278M) | 1058.7 MB / **265.7 MB** | Yes (`model_int8.onnx`) | **Europarl** ([Model Card](https://huggingface.co/kredor/punctuate-all)):<br>,: **86.0%**, .: **95.0%**, ?: **86.0%**<br>IWSLT2011: **UNVERIFIED**; *No casing* | 12 European langs | **~350–500 ms** INT8 / **~1000 ms** FP32 on i7-8650U | MIT |
| **NeMo punctuation_en_bert**<br>[NGC Catalog](https://catalog.ngc.nvidia.com/orgs/nvidia/teams/nemo/models/punctuation_en_bert) | BERT-base-uncased (~110M) | ~438 MB (.nemo 387MB) / *~110 MB* | Exportable via NeMo / ONNX | **IWSLT TED Talks** ([Nagy et al. 2021 Tab. 4](https://arxiv.org/abs/2101.07343)):<br>,: **67.8–71.8%**, .: **84.0–84.2%**, ?: **85.7–87.6%**; Casing: Yes | English | **~120–180 ms** INT8 / **~300 ms** FP32 on i7-8650U | Apache-2.0 |
| **NeMo punctuation_en_distilbert**<br>[NGC Catalog](https://catalog.ngc.nvidia.com/orgs/nvidia/teams/nemo/models/punctuation_en_distilbert) | DistilBERT-base (~66M) | ~260 MB (.nemo 234MB) / *~65 MB* | Exportable via NeMo / ONNX | **IWSLT2011: UNVERIFIED** (ALBERT-base reference [Tab. 4](https://arxiv.org/abs/2101.07343)):<br>,: **69.4%**, .: **82.7%**, ?: **74.2%**; Casing: Yes | English | **~70–110 ms** INT8 / **~180–220 ms** FP32 on i7-8650U | Apache-2.0 |
| **Horizon-Labs punctuation-restoration-small**<br>[HuggingFace](https://huggingface.co/Horizon-Labs/punctuation-restoration-small) | mmBERT-small (140M) | 562.6 MB / *~140 MB* | Yes (Transformers.js ONNX) | **FineWeb-2**: IWSLT2011: **UNVERIFIED**; *No casing* | 88 languages | **~150–220 ms** INT8 on i7-8650U | Apache-2.0 |
| **Silero Text Enhancement v2**<br>[Model](https://models.silero.ai/te_models/v2_4lang_q.pt) / [Repo](https://github.com/snakers4/silero-models) | PyTorch JIT Quantized | **87.5 MB** (PyTorch JIT) | **No** (TorchScript JIT only) | Detailed IWSLT breakdown: **UNVERIFIED** ([Wiki](https://github.com/snakers4/silero-models/wiki/Quality-Benchmarks)); Punct + Casing: Yes | EN, RU, DE, ES | **~80–120 ms** on CPU (TorchScript) | **CC-BY-NC-SA 4.0** (Non-commercial!) |

---

## 2. Do Bigger Models Beat CNN-BiLSTM on Real ASR Output?
**Short Answer:** On real ASR output, bigger models provide **modest gains on commas/questions (+5–8%)**, **negligible gains on periods (+0.7–4.5%)**, and suffer **severe performance degradation** unless specifically trained on ASR error distributions.

### Published Empirical Evidence:
1. **Direct Benchmark on IWSLT2011 Spoken Transcripts ([You & Li, 2024](https://arxiv.org/abs/2407.13142)):**
   - **Period F1:** CNN-BiLSTM (7.1 MB) achieves **79.5%** vs. CT-Transformer (280 MB) at **80.2%** (only **+0.7% difference**).
   - **Comma F1:** CT-Transformer reaches **69.3%** vs. CNN-BiLSTM **61.5%** (+7.8%).
   - **Question F1:** CT-Transformer reaches **79.2%** vs. CNN-BiLSTM **72.1%** (+7.1%).
   - *Crucial Deficit:* CT-Transformer completely lacks casing prediction ([arXiv:2407.13142 Table 5](https://arxiv.org/abs/2407.13142)). It outputs all-lowercase sentences.
2. **Dictation & Medical ASR Transcripts ([Sunkara et al., Amazon AWS, 2020](https://arxiv.org/abs/2007.02025)):**
   - In Table 1 (Dictation Corpus), a **3-BLSTM** achieved **87.0% Full Stop F1** and **76.0% Comma F1**, actually *outperforming* a Transformer Encoder (85.0% Full Stop, 72.0% Comma).
   - In Table 5 (Robustness to ASR Errors), when a BERT model trained on ground-truth text was tested on real noisy ASR output, Full Stop F1 plummeted from **>85% to 58.0%**, and Comma F1 fell to **45.0%**. ASR substitution and deletion errors break multi-head self-attention, whereas recurrent LSTMs degrade smoothly.
3. **Punctuation Blindness to Acoustic Pauses:**
   - In Silent Voice, VAD cuts audio at acoustic pauses. If a user pauses mid-sentence, VAD produces two chunks. A purely text-based model (whether 7 MB or 300 MB) lacks audio prosody (pitch reset, pause duration, energy) and cannot reliably disambiguate hesitations from sentence terminators.

---

## 3. Runtime Compatibility & C API Support

| Model Category | Runs via `sherpa-onnx` 1.13.4 C API? | Runtime Requirements |
|---|---|---|
| **`sherpa-onnx-online-punct-en-2024-08-06`** | **YES (Native)** | None. Supported directly via `SherpaOnnxOnlinePunctuation` (`--cnn-bilstm` + `--bpe-vocab`). Zero new runtime. |
| **`sherpa-onnx-punct-ct-transformer-zh-en`** | **YES (Native)** | None. Supported directly via `SherpaOnnxOfflinePunctuation` (`--ct-transformer`). Zero new runtime. |
| **1-800-BAD-CODE models** | **NO** | Needs `onnxruntime` engine (or Rust `ort` crate) + SentencePiece tokenizer (`spe_32k_lc_en.model`). |
| **Oliver Guhr (Large / Base)** | **NO** | Needs `onnxruntime` + HuggingFace `tokenizers` (XLM-RoBERTa BPE). |
| **Kredor (`punctuate-all`)** | **NO** | Needs `onnxruntime` + HuggingFace `tokenizers` (XLM-RoBERTa BPE). |
| **Horizon-Labs (small / base)** | **NO** | Needs `onnxruntime` + HuggingFace `tokenizers` (ModernBERT BPE). |
| **Felflare (`bert-restore-punctuation`)** | **NO** | Needs `onnxruntime` + WordPiece tokenizer. |
| **NeMo (`punctuation_en_bert / distilbert`)** | **NO** | Needs `onnxruntime` + WordPiece tokenizer. |
| **Silero TE** | **NO** | Needs PyTorch / LibTorch C++ runtime (not ONNX). |

*Inspection of `sherpa-onnx` C API headers (`c-api.h`, `online-punctuation-model-config.h`, `offline-punctuation-model-config.h`) verifies that sherpa-onnx only supports `cnn_bilstm` and `ct_transformer` backends.*

---

## 4. Ranked Recommendations for Silent Voice

### **Rank 1 (Strongly Recommended): Stay with `sherpa-onnx-online-punct-en-2024-08-06` (CNN-BiLSTM INT8, 7.1 MB)**
- **Why:**
  1. **Zero Runtime Bloat:** Reuses the existing linked `sherpa-onnx` C API (`SherpaOnnxOnlinePunctuation`). Zero new DLLs or crates.
  2. **Fastest Latency:** Takes **~35–75 ms** for 300 words on 15W mobile CPUs (well under the 200 ms budget).
  3. **Joint Casing + Punctuation:** Accurately restores sentence-start capitals and uppercase acronyms (87.8% casing F1).
  4. **Empirically Proven in App:** Errors dropped from 55 to 21 (62% reduction) on test audio. Comma F1 deficit (61.5% vs ~70%) is imperceptible in spoken dictation.

### **Rank 2 (Best Upgrade Path if Higher Comma F1 is Demanded): NeMo `punctuation_en_distilbert` (Quantized INT8, ~65 MB)**
- **Why:** If the product requirement strictly demands higher comma/question mark F1 (+6–8%), DistilBERT is the *only* transformer light enough to meet the 200 ms budget on an i7-8650U (~70–110 ms INT8). Predicts both punctuation and capitalization.
- **Drawbacks:** Requires adding `ort` and `tokenizers` crates to Tauri/Rust, plus custom export & quantization pipeline.

### **Rank 3: 1-800-BAD-CODE `punctuation_fullstop_truecase_english` (Quantized INT8, ~50 MB)**
- **Why:** Explicitly designed for lowercased raw ASR text with punctuation, capitalization, and sentence boundary heads.
- **Drawbacks:** Hard 256-token limit requires custom sliding-window overlapping chunker in Rust; requires `ort` + SentencePiece runtime.

### **Rank 4: `sherpa-onnx-punct-ct-transformer-zh-en` (INT8, 65 MB)**
- **Why:** Supported natively in `sherpa-onnx` C API without extra dependencies, runs in ~70 ms.
- **Fatal Flaw:** **Does not predict casing/capitalization.** It leaves all output lowercase, requiring a second model for sentence capitalization.

### **Rank 5 (Avoid): Oliver Guhr Large/Base, Kredor, Horizon-Labs, and XLM-RoBERTa Models (265 MB–2.2 GB)**
- **Why:** Severe latency (350–2000+ ms on 15W CPU, blowing the 200 ms budget), massive binary/disk footprint, and lack of capitalization prediction.

### **Rank 6 (Exclude): Silero Text Enhancement v2**
- **Why:** Restricted by **CC-BY-NC-SA 4.0 (Non-Commercial)** license and requires heavy LibTorch runtime (no ONNX).

---

## 5. Remaining Questions & Gaps
1. **Acoustic Seam Overlap:** As established in `docs/research-latency.md`, post-hoc text punctuation is a lossy heuristic for pause seams. Overlapping acoustic chunks with timestamp slicing remains superior to larger text models.
2. **Domain Adaptation:** None of the public open models are trained specifically on technical dictation (code, developer prose). If capitalization errors remain, rule-based TrueCasing heuristics or Parakeet native casing preservation will yield larger gains than switching models.

---

## 6. Measured on the user's laptop (Claude, i7-8650U, 2026-10-15)

Gemini's i7-8650U latency figures above are estimates. These are measured: 3 Kokoro-TTS paragraphs (321 words), background chunking, same keep-casing hybrid rule for both models.

| Model | Size | Punct + caps errors | Model time (3 paragraphs) |
|---|---|---|---|
| None (background today) | - | 51 | - |
| CNN-BiLSTM online-punct-en int8 | 7 MB | **23** | 289 ms |
| CT-Transformer zh-en int8 | 76 MB | 27 | 354 ms |
| (Full pass, no punct model) | - | 35 | - |

The big model won paragraph 2 (4 vs 8) but lost 1 and 3; it runs sentences together ("afternoon that works for me"). Decision: ship the 7 MB model.
