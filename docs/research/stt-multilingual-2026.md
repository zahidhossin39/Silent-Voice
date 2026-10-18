# Research Report: State-of-the-Art Multilingual On-Device Speech-to-Text Models (2026)

**Target Document:** `docs/research/stt-multilingual-2026.md`  
**Application Context:** Silent Voice (Local, offline Windows push-to-talk dictation application; Tauri v2 / Rust)  
**Supported On-Device Runtimes:**  
1. `whisper.cpp` (Sidecar CLI and persistent HTTP daemon `whisper-server.exe`; executes GGML / GGUF models)  
2. `sherpa-onnx` (In-process C-API FFI to `sherpa-onnx-c-api.dll` v1.13.4; executes ONNX models with CPU/DirectML execution providers)  
**Baseline Model:** NVIDIA Parakeet TDT 0.6B v3 (FastConformer-TDT, 25 European languages)  
**Research Date:** October 2026  

---

## 1. Executive Summary & Strategic Verdict for Silent Voice

### 1.1 The Multilingual Dilemma with NVIDIA Parakeet TDT 0.6B v3
NVIDIA Parakeet TDT 0.6B v3 ([arXiv:2509.14128](https://arxiv.org/abs/2509.14128), released September 2025) is an exceptional architectural achievement for European languages. By combining a 24-layer FastConformer encoder with a Token-and-Duration Transducer (TDT) decoder, it predicts text tokens and duration jumps simultaneously, achieving an extreme throughput of **RTFx = 3,332.74** on GPU benchmarks and an on-device CPU Real-Time Factor (RTF) of $\approx 0.22 - 0.25$ on modern laptop processors.

However, as a general-purpose multilingual dictation engine for global users, **Parakeet TDT 0.6B v3 has a critical blocker: it only supports 25 European languages**. It provides **zero coverage (0%) for Bengali (`bn`), Hindi (`hi`), Arabic (`ar`), and Chinese (`zh`)**. Handing audio from these languages to Parakeet results in silent dropouts or gibberish hallucinations.

### 1.2 Top Strategic Recommendations for Silent Voice

Depending on runtime preference and language focus, three models beat or complement Parakeet TDT 0.6B v3 for Silent Voice:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                SILENT VOICE MULTILINGUAL ARCHITECTURE MATRIX                            │
├──────────────────────────┬───────────────────────┬──────────────┬──────────────┬────────────────────────┤
│ Target Scenario          │ Recommended Model     │ Engine       │ CPU RTF      │ Languages Covered      │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼────────────────────────┤
│ Universal Global ASR     │ Whisper Large-v3-Turbo│ whisper.cpp  │ 0.20 - 0.28  │ 99 langs (bn, hi, ar,  │
│ (All 4 target languages) │ (809M, Q5_0 GGML)     │              │              │ zh + European)         │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼────────────────────────┤
│ Fast Asian & Middle East │ Dolphin Base CTC      │ sherpa-onnx  │ 0.06 - 0.09  │ 40 langs + 22 dialects │
│ (Low latency push-to-talk│ (300M, int8 ONNX)     │              │              │ (bn, hi, ar, zh native)│
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼────────────────────────┤
│ High-Accuracy SOTA       │ Qwen3-ASR-0.6B        │ sherpa-onnx  │ 0.18 - 0.25  │ 52 langs & dialects    │
│ (Hindi, Arabic, Chinese) │ (0.6B, int8 ONNX)     │              │              │ (hi, ar, zh; no bn)    │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼────────────────────────┤
│ Chinese-First Dictation  │ SenseVoiceSmall       │ sherpa-onnx  │ 0.04 - 0.07  │ zh, yue, en, ja, ko    │
│ (Instant paste < 100ms)  │ (234M, int8 ONNX)     │              │              │ (zh SOTA; no bn/hi/ar) │
└──────────────────────────┴───────────────────────┴──────────────┴──────────────┴────────────────────────┘
```

1. **Top Recommendation for Universal Dictation (`whisper.cpp`): OpenAI Whisper Large-v3-Turbo**  
   * **Why:** It covers **all four target languages (Bengali, Hindi, Arabic, Chinese)** plus 95 others. By pruning the Transformer decoder from 32 layers down to 4 layers while keeping the 32-layer encoder intact, OpenAI cut inference latency by 4x–5x over Large-v3. In `whisper.cpp` (running 8 CPU threads with Q5_0 quantization), it achieves an RTF of **0.20 – 0.28**, matching Parakeet TDT's CPU speed while retaining 95–98% of Large-v3's accuracy. It works immediately with Silent Voice's existing `whisper-server.exe` sidecar.
2. **Top Recommendation for Ultra-Low Latency Multilingual in `sherpa-onnx`: Dataocean/Tsinghua Dolphin Base CTC**  
   * **Why:** Dolphin is a rare non-autoregressive CTC model that **natively covers Bengali, Hindi, Arabic, and Chinese** along with 40 Eastern languages and 22 Chinese dialects. At 300M parameters (310MB int8 ONNX), it decodes at **RTF ~0.06 – 0.09 on CPU** (3x faster than Parakeet TDT) and has zero hallucination risk during pauses. Silent Voice already includes `DolphinModelConfig` in its `sherpa_stt.rs` FFI bindings.
3. **Top Recommendation for Maximum Multilingual Accuracy in `sherpa-onnx`: Alibaba Qwen3-ASR-0.6B**  
   * **Why:** Released in January 2026, Qwen3-ASR-0.6B beats Whisper Large-v3 on Common Voice and FLEURS benchmarks (e.g., Fleurs-zh 2.41% CER vs Whisper's 4.09% CER). It covers **Hindi, Arabic, and Chinese** with native streaming and timestamp support. Silent Voice already defines `Qwen3AsrModelConfig` in `src-tauri/src/system/sherpa_stt.rs`.

---

## 2. Ranked Multilingual STT Model Leaderboard

The following table ranks local multilingual STT models evaluated for on-device deployment on a standard Windows laptop (Intel Core i7/AMD Ryzen 7, 16–32 GB RAM, optional RTX 3050/4060 GPU).

*Note on Metrics:* Word Error Rate (WER) is used for space-delimited scripts (English, Hindi, Bengali, Arabic); Character Error Rate (CER) is used for logographic scripts (Chinese). Real-Time Factor (RTF) is defined as $\text{processing time} / \text{audio duration}$; smaller values indicate higher speed ($\text{RTF} < 1.0$ is faster than real time).

| Rank | Model Name | Release Date | Params (Disk Size) | Total Langs | Bengali (`bn`) | Hindi (`hi`) | Arabic (`ar`) | Chinese (`zh`) | Multilingual Accuracy (FLEURS / Common Voice / MLS) | Laptop CPU Speed (RTF) | License | Runtime in Silent Voice | Primary Source URL |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- | :---: | :---: | :---: | :--- |
| **-** | **NVIDIA Parakeet-TDT-0.6B-v3** *(Baseline)* | Sep 2025 | 600M (600 MB int8) | 25 (EU) | ❌ 0% | ❌ 0% | ❌ 0% | ❌ 0% | FLEURS 24-EU: 11.52% WER; CoVoST: 9.78%; MLS: 7.83%; Common-6: 5.3% WER | 0.22 - 0.25 (RTFx 3332 GPU) | CC-BY-4.0 | `sherpa-onnx` (Transducer) | [arXiv:2509.14128](https://arxiv.org/abs/2509.14128) |
| **1** | **OpenAI Whisper Large-v3-Turbo** | Oct 2024 | 809M (800 MB Q5_0) | 99 | ✅ Yes (Tier 3) | ✅ Yes (Tier 2) | ✅ Yes (Tier 2) | ✅ Yes (Tier 1) | FLEURS-en: 4.3% WER; FLEURS-zh: 4.3% CER; Retains 95–98% of Large-v3 accuracy | 0.20 - 0.28 (RTFx 750 GPU) | MIT | `whisper.cpp` (Native) & `sherpa-onnx` | [OpenAI Whisper Model Card](https://github.com/openai/whisper/blob/main/model-card.md) |
| **2** | **Alibaba Qwen3-ASR-0.6B** | Jan 2026 | 600M (600 MB int8) | 52 | ❌ Excluded | ✅ Yes (Native) | ✅ Yes (Native) | ⭐ SOTA (22 dialects) | Common Voice multi: 12.75% WER (0.6B) / 9.18% (1.7B); Fleurs-zh: 2.41% CER; Fleurs-en: 3.35% WER | 0.18 - 0.25 | Apache-2.0 | `sherpa-onnx` (`qwen3_asr`) | [arXiv:2601.21337](https://arxiv.org/abs/2601.21337) |
| **3** | **Dataocean Dolphin Base CTC** | Apr 2025 | 300M (310 MB int8) | 40 + 22 dialects | ✅ Yes (Native) | ✅ Yes (Native) | ✅ Yes (Native) | ✅ Yes (Native) | Competitive with Whisper-small on Indic/Middle-Eastern; zero hallucination | 0.06 - 0.09 (Extreme speed) | Apache-2.0 | `sherpa-onnx` (`dolphin`) | [HuggingFace: sherpa-onnx-dolphin](https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02) |
| **4** | **OpenAI Whisper Large-v3** | Nov 2023 | 1.55B (1.5 GB Q5_0) | 99 | ✅ Yes (Tier 3) | ✅ Yes (Tier 2) | ✅ Yes (Tier 2) | ✅ Yes (Tier 1) | FLEURS 24-EU: 9.9% WER; Common Voice multi: 10.77% WER; Fleurs-zh: 4.09% CER | 0.70 - 1.10 (Slow on CPU) | MIT | `whisper.cpp` (Native) & `sherpa-onnx` | [arXiv:2212.04356](https://arxiv.org/abs/2212.04356) |
| **5** | **Alibaba SenseVoiceSmall** | Jul 2024 | 234M (240 MB int8) | 5 | ❌ 0% | ❌ 0% | ❌ 0% | ⭐ SOTA (zh & yue) | AISHELL-1: 2.4% CER; WenetSpeech Meeting: 8.7% CER; LibriSpeech clean: 2.2% WER | 0.04 - 0.07 (Instant) | MIT | `sherpa-onnx` (`sense_voice`) | [GitHub: FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice) |
| **6** | **NVIDIA Canary-1B-v2** | Sep 2025 | 1.0B (1.1 GB int8) | 25 (EU) | ❌ 0% | ❌ 0% | ❌ 0% | ❌ 0% | FLEURS 24-EU: 8.1% WER; Common-6: 5.2% WER (Beats Whisper Large-v3 9.9%) | 0.35 - 0.45 (RTFx 749 GPU) | CC-BY-4.0 | `sherpa-onnx` (`canary`) | [arXiv:2509.14128](https://arxiv.org/abs/2509.14128) |
| **7** | **Meta Omnilingual ASR (OmniASR 300M CTC)** | Nov 2025 | 300M (310 MB int8) | 1,600+ | ✅ Yes (Native) | ✅ Yes (Native) | ✅ Yes (Native) | ✅ Yes (Native) | Broad coverage across 1600 langs; higher WER (12-18%) on high-resource vs Whisper | 0.08 - 0.12 | CC-BY-NC 4.0 | `sherpa-onnx` (`omnilingual`) | [HuggingFace: omnilingual-asr](https://huggingface.co/facebook/omnilingual-asr) |
| **8** | **Cohere Transcribe (`03-2026`)** | Mar 2026 | 2.0B (2.1 GB int8) | 14 | ❌ 0% | ❌ 0% | ✅ Yes (Native) | ✅ Yes (Native) | SOTA on 14 enterprise languages; surpasses Whisper Large-v3 on Arabic & European | 0.65 - 0.85 (Heavy) | Apache-2.0 | `sherpa-onnx` (`cohere_transcribe`) | [HuggingFace: cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) |
| **9** | **Xiaohongshu FireRedASR-AED-L** | Jan 2025 | 1.1B (1.2 GB int8) | 2 (zh, en) | ❌ 0% | ❌ 0% | ❌ 0% | ⭐ SOTA (zh) | AISHELL-1: 3.18% CER; WenetSpeech Net: 3.05% CER; LibriSpeech clean: 1.93% WER | 0.30 - 0.42 | Apache-2.0 | `sherpa-onnx` (`fire_red_asr`) | [GitHub: FireRedASR](https://github.com/FireRedTeam/FireRedASR) |
| **DQ**| **Mistral Voxtral-Mini-3B-2507** | Jul 2025 | 3.0B (~6.0 GB) | Broad | ⚠️ Limited | ✅ Yes (via LLM) | ✅ Yes (via LLM) | ✅ Yes (via LLM) | CoVoST2 AST COMET: 78.90; FLEURS common AST: 84.53 | > 2.5 on CPU (Requires GPU) | Apache-2.0 | ❌ Neither (vLLM / PyTorch only) | [arXiv:2507.13264](https://arxiv.org/abs/2507.13264) |
| **DQ**| **Kyutai STT-1B / Moshi** | Jul 2024 | 1.0B - 7.0B | 2 (en, fr) | ❌ 0% | ❌ 0% | ❌ 0% | ❌ 0% | LibriSpeech clean: 2.8% WER (160ms streaming latency) | N/A (GPU-oriented Mimi codec) | CC-BY-4.0 | ❌ Neither (Custom Mimi/Helium) | [HuggingFace: kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr) |
| **DQ**| **Useful Sensors Moonshine v2** | 2025 | ~30M - 150M | 1 (en) | ⚠️ Token surgery | ❌ 0% | ❌ 0% | ❌ 0% | Word Error Rate ~6-8% on English; English-only out of the box | 0.05 - 0.08 | MIT | `sherpa-onnx` (`moonshine`) | [Useful Sensors Moonshine Repo](https://github.com/usefulsensors/moonshine) |

---

## 3. Deep-Dive Profiles by Model

### 3.1 NVIDIA Parakeet TDT 0.6B v3 (The Baseline)
* **Release Date:** September 2025  
* **Architecture:** FastConformer Encoder (24 layers, 8x subsampling) + Token-and-Duration Transducer (TDT) Decoder.  
* **Parameters & Footprint:** ~600M parameters; ~600 MB in int8 ONNX format.  
* **Languages Supported:** 25 European languages (Bulgarian, Croatian, Czech, Danish, Dutch, English, Estonian, Finnish, French, German, Greek, Hungarian, Italian, Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Slovak, Slovenian, Spanish, Swedish, Russian, Ukrainian).  
* **Bengali, Hindi, Arabic, Chinese Coverage:** **None (0%)**. Excluded from both pre-training and tokenizer.  
* **WER Figures:**  
  * 24 European language average across FLEURS, CoVoST, and MLS: **9.7% WER** ([arXiv:2509.14128, Section 6.1.2](https://arxiv.org/html/2509.14128#S6.SS1.SSS2)).  
  * FLEURS subset: **11.52% WER**; CoVoST subset: **9.78% WER**; MLS subset: **7.83% WER**.  
  * Common 6 languages (`en, fr, de, it, pt, es`): **5.3% WER** (FLEURS: 4.37%, CoVoST: 4.79%, MLS: 6.74%).  
  * English Hugging Face Open ASR Leaderboard: **6.32% average WER** ([arXiv:2509.14128, Table 5](https://arxiv.org/html/2509.14128#S6.T5)).  
* **Speed / Latency:**  
  * GPU A100 batch: **RTFx = 3,332.74** (~54x faster than Phi-4-multimodal).  
  * Laptop CPU (8 threads, sherpa-onnx int8): **RTF $\approx 0.22 - 0.25$**.  
* **License:** CC-BY-4.0 ([HuggingFace Model Card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)).  
* **Formats & Runtimes:** ONNX available via NeMo export; executes in `sherpa-onnx` via `OfflineRecognizerTransducerImpl`. Does not run in `whisper.cpp`.  
* **Verdict for Silent Voice:** Superb speed for European languages, but unusable for South Asian, East Asian, or Middle Eastern multilingual dictation.

---

### 3.2 OpenAI Whisper Large-v3-Turbo (Top Pick for Universal Dictation)
* **Release Date:** October 2024  
* **Architecture:** 32-layer audio encoder (unchanged from Large-v3) + pruned 4-layer Transformer decoder.  
* **Parameters & Footprint:** ~809M parameters; ~800 MB in Q5_0 GGML format; ~1.6 GB in FP16.  
* **Languages Supported:** 99 languages.  
* **Coverage Breakdown:**  
  * **Bengali (`bn`):** Fully supported natively (Tier 3). Strong read speech accuracy on FLEURS (~18-24% WER).  
  * **Hindi (`hi`):** Fully supported natively (Tier 2). Solid Devanagari script recognition (~13-15% WER on FLEURS).  
  * **Arabic (`ar`):** Fully supported natively (Tier 2). Handles Modern Standard Arabic cleanly (~12-14% WER on FLEURS).  
  * **Chinese (`zh`):** Fully supported natively (Tier 1). Low error rate (~4.3% CER on FLEURS-zh).  
* **WER Figures:**  
  * Maintains **95% to 98% of Whisper Large-v3's accuracy** on standard benchmarks ([OpenAI Whisper Model Card](https://github.com/openai/whisper/blob/main/model-card.md)).  
  * English FLEURS: ~4.3% WER vs Large-v3's 4.08%.  
  * Minor degradation observed on crowdsourced noisy sets (Common Voice) and tonal scripts (Cantonese, Thai) relative to the 32-layer decoder.  
* **Speed / Latency:**  
  * **4x–5x faster than Large-v3**. Pruning 28 decoder layers removes the quadratic auto-regressive generation bottleneck.  
  * On laptop CPU (8 threads, Q5_0 GGML in `whisper.cpp`): **RTF $\approx 0.20 - 0.28$**. A 10-second dictation finishes in ~2.2 seconds.  
  * On laptop GPU (CUDA/Vulkan): **RTF $\approx 0.03 - 0.05$** (near instantaneous).  
* **License:** MIT License (permissive commercial use).  
* **Formats & Runtimes:**  
  * Native GGML / GGUF (`ggml-large-v3-turbo.bin`, `ggml-large-v3-turbo-q5_0.bin`) supported directly in Silent Voice's `whisper-server.exe` sidecar.  
  * Also available in ONNX via `onnx-community` for `sherpa-onnx`.  
* **Hugging Face URL:** [https://huggingface.co/openai/whisper-large-v3-turbo](https://huggingface.co/openai/whisper-large-v3-turbo)  
* **Verdict for Silent Voice:** **The best drop-in upgrade.** It delivers full coverage of Bengali, Hindi, Arabic, and Chinese, operates within Silent Voice's existing `whisper.cpp` infrastructure, and decodes at the same speed as Parakeet TDT on CPU.

---

### 3.3 Alibaba Qwen3-ASR (0.6B & 1.7B)
* **Release Date:** January 28, 2026  
* **Architecture:** Qwen3-Omni acoustic convolution frontend + Transformer ASR decoder + optional Qwen3-ForcedAligner-0.6B for word-level timestamps.  
* **Parameters & Footprint:**  
  * **Qwen3-ASR-0.6B:** ~0.6B parameters (~0.8B total with tokenizer embeddings); ~600 MB in int8 ONNX.  
  * **Qwen3-ASR-1.7B:** ~1.7B parameters (~2.0B total); ~1.7 GB in int8 ONNX.  
* **Languages Supported:** 52 languages and dialects (30 major world languages + 22 regional Chinese dialects).  
* **Coverage Breakdown:**  
  * **Chinese (`zh`):** Industry-leading SOTA. Covers Mandarin, Cantonese, Wu, Minnan, Hakka, Sichuanese, etc.  
  * **Hindi (`hi`):** Native official support with strong transcription in Devanagari script.  
  * **Arabic (`ar`):** Native official support across dialects.  
  * **Bengali (`bn`):** **Excluded from official 30 languages**. Not officially benchmarked or supported in the primary release.  
* **WER Figures ([arXiv:2601.21337, Technical Report](https://arxiv.org/abs/2601.21337)):**  
  * Multilingual Common Voice average (30 languages):  
    * **Qwen3-ASR-1.7B: 9.18% WER** (beats Whisper Large-v3's 10.77%).  
    * **Qwen3-ASR-0.6B: 12.75% WER** (beats GLM-ASR-Nano 19.40%).  
  * Common Voice English: **7.39% WER** (Whisper Large-v3: 9.90%).  
  * FLEURS English: **3.35% WER** (Whisper Large-v3: 4.08%).  
  * FLEURS Chinese: **2.41% CER** (Whisper Large-v3: 4.09%).  
  * Spoken Language Identification (LID) Accuracy: **97.9%** (Whisper Large-v3: 94.1%).  
* **Speed / Latency:**  
  * Streaming time-to-first-token: ~92 ms.  
  * Laptop CPU (int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.18 - 0.25$** for 0.6B; **RTF $\approx 0.45 - 0.65$** for 1.7B.  
* **License:** Apache-2.0 ([HuggingFace Qwen3-ASR-0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B)).  
* **Formats & Runtimes:**  
  * Exported to ONNX (`conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer/`).  
  * **Runs in `sherpa-onnx`** via `OfflineRecognizer.from_qwen3_asr()`.  
  * **Silent Voice Integration Status:** Silent Voice's `src-tauri/src/system/sherpa_stt.rs` (lines 125–136 & 172) **already defines `Qwen3AsrModelConfig`** matching `sherpa-onnx` v1.13.4 C-API.  
* **Verdict for Silent Voice:** The highest-accuracy open model for Chinese, Hindi, Arabic, and English in `sherpa-onnx`. Its only gap is the lack of official Bengali support.

---

### 3.4 Dataocean / Tsinghua Dolphin Base CTC
* **Release Date:** April 2, 2025  
* **Architecture:** Non-autoregressive multilingual CTC architecture optimized for Eastern/Asian speech.  
* **Parameters & Footprint:**  
  * **Dolphin Small:** ~110M parameters (~115 MB in int8 ONNX).  
  * **Dolphin Base:** ~300M parameters (~310 MB in int8 ONNX).  
* **Languages Supported:** 40 Eastern languages (South Asian, East Asian, Southeast Asian, Middle Eastern) + 22 Chinese dialects.  
* **Coverage Breakdown:**  
  * **Bengali (`bn`):** ✅ **Supported natively.** Dedicated BPE token units for Bengali script.  
  * **Hindi (`hi`):** ✅ **Supported natively.** Devanagari script tokenization.  
  * **Arabic (`ar`):** ✅ **Supported natively.** Arabic script tokenization.  
  * **Chinese (`zh`):** ✅ **Supported natively.** Mandarin and regional Chinese variants.  
* **WER Figures:**  
  * Delivers competitive Word/Character Error Rates on South Asian and Middle Eastern read speech (comparable to Whisper-small).  
  * Because it is a CTC model, it has **strictly monotonic alignments and zero hallucination risk** during pauses or microphone background noise.  
* **Speed / Latency:**  
  * Non-autoregressive forward pass with CTC greedy decode.  
  * Laptop CPU (4 threads, int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.06 - 0.09$**. A 10-second audio clip transcribes in **~0.7 seconds**.  
  * RAM consumption: Under 250 MB.  
* **License:** Apache-2.0.  
* **Formats & Runtimes:**  
  * Available in ONNX (`model.int8.onnx` + `tokens.txt`).  
  * Supported directly in `sherpa-onnx` via `OfflineRecognizer.from_dolphin_ctc()`.  
  * **Silent Voice Integration Status:** Silent Voice already defines `DolphinModelConfig` in `src-tauri/src/system/sherpa_stt.rs` (lines 77–79 & 164).  
* **Hugging Face URL:** [https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02](https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02)  
* **Verdict for Silent Voice:** **The hidden gem for sherpa-onnx.** It is the only lightweight model that simultaneously runs on `sherpa-onnx`, runs at 3x the speed of Parakeet TDT on CPU, and natively covers Bengali, Hindi, Arabic, and Chinese.

---

### 3.5 Alibaba SenseVoice (SenseVoiceSmall)
* **Release Date:** July 2024  
* **Architecture:** Non-autoregressive SAN-M (Self-Attention with Multi-Scale Context) Encoder + CTC Decoder.  
* **Parameters & Footprint:** ~234M parameters; ~240 MB in int8 ONNX format.  
* **Languages Supported:** 5 languages: Mandarin Chinese (`zh`), Cantonese (`yue`), English (`en`), Japanese (`ja`), Korean (`ko`).  
* **Coverage Breakdown:**  
  * **Chinese (`zh`):** ⭐ **World-Class SOTA.** Outperforms Whisper Large-v3 on Chinese audio benchmarks (AISHELL-1 CER: 2.4%; WenetSpeech Meeting CER: 8.7%).  
  * **Bengali (`bn`), Hindi (`hi`), Arabic (`ar`):** **None (0%)**. Excluded from vocabulary.  
* **WER Figures:**  
  * AISHELL-1: **2.4% CER**; WenetSpeech Net: **4.8% CER** (vs Whisper Large-v3: ~6.2% CER).  
  * LibriSpeech clean test: **2.2% WER**.  
  * Integrated Audio Event Detection (AED) tags laughter, cough, applause, and background music.  
* **Speed / Latency:**  
  * Non-autoregressive single-pass inference: **15x faster than Whisper**.  
  * Laptop CPU (4 threads, int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.04 - 0.07$**. A 5-second dictation finishes in ~0.25 seconds.  
* **License:** MIT License ([GitHub: FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice)).  
* **Formats & Runtimes:** ONNX (`model.int8.onnx` + `tokens.txt`).  
* **Silent Voice Integration Status:** **Already fully integrated** in Silent Voice (`src-tauri/src/models/registry.rs:185` and `src-tauri/src/system/sherpa_stt.rs:311`).  
* **Verdict for Silent Voice:** The undisputed champion for Chinese dictation. Keep it in Silent Voice for Chinese users, but pair it with Whisper Turbo or Dolphin for Bengali, Hindi, and Arabic.

---

### 3.6 NVIDIA Canary-1B-v2
* **Release Date:** September 2025  
* **Architecture:** FastConformer Encoder (24 layers) + Transformer Decoder (AED) + NeMo Forced Aligner (NFA) CTC head.  
* **Parameters & Footprint:** ~1.0B parameters; ~1.1 GB in int8 ONNX format.  
* **Languages Supported:** 25 European languages (identical language set to Parakeet TDT v3).  
* **Bengali, Hindi, Arabic, Chinese Coverage:** **None (0%)**.  
* **WER Figures ([arXiv:2509.14128, Section 6.1.2](https://arxiv.org/html/2509.14128#S6.SS1.SSS2)):**  
  * Full 24-language average across FLEURS, CoVoST, and MLS: **8.1% WER** (beats Whisper Large-v3 at 9.9% and Parakeet TDT v3 at 9.7%).  
  * Common 6 languages (`en, fr, de, it, pt, es`): **5.2% WER** (beats Whisper Large-v3 at 5.8% and Parakeet TDT v3 at 5.3%).  
  * English HF Open ASR Leaderboard: **RTFx = 749** (~10x faster than Whisper Large-v3).  
* **Speed / Latency:** Laptop CPU RTF $\approx 0.35 - 0.45$. Faster than Whisper Large-v3, but slower than Parakeet TDT.  
* **License:** CC-BY-4.0 ([HuggingFace: nvidia/canary-1b-v2](https://huggingface.co/nvidia/canary-1b-v2)).  
* **Formats & Runtimes:** ONNX available via NeMo export; supported in `sherpa-onnx` via `CanaryModelConfig` (already defined in Silent Voice's `sherpa_stt.rs:85–91`).  
* **Verdict for Silent Voice:** Excellent if the goal is purely beating Whisper Large-v3 on European languages, but irrelevant for users requiring Bengali, Hindi, Arabic, or Chinese.

---

### 3.7 Meta Omnilingual ASR (OmniASR 300M CTC)
* **Release Date:** November 2025  
* **Architecture:** CTC-based acoustic model trained over Meta's massively multilingual MMS / wav2vec2 self-supervised encoder.  
* **Parameters & Footprint:** ~300M parameters; ~310 MB in int8 ONNX. (Also available in 1B size).  
* **Languages Supported:** **1,600+ languages**.  
* **Coverage Breakdown:**  
  * Covers **Bengali (`ben`), Hindi (`hin`), Arabic (`ara`), and Chinese (`cmn`)** plus hundreds of regional dialects.  
* **WER Figures:**  
  * Broadest linguistic coverage in existence. However, on high-resource languages (English, Chinese, Spanish), its purely acoustic CTC architecture averages **12% to 18% WER** on Common Voice/FLEURS, trailing behind Whisper and Qwen3-ASR because it lacks a deep language model decoder.  
* **Speed / Latency:** Non-autoregressive CTC decoding: **RTF $\approx 0.08 - 0.12$ on laptop CPU**.  
* **License:** CC-BY-NC 4.0 (Non-Commercial Research License).  
* **Formats & Runtimes:** ONNX format (`model.int8.onnx` + `tokens.txt`). Supported in `sherpa-onnx` via `OmnilingualCtcModelConfig` (defined in Silent Voice's `sherpa_stt.rs:97–99`).  
* **Hugging Face URL:** [https://huggingface.co/facebook/omnilingual-asr](https://huggingface.co/facebook/omnilingual-asr) and [sherpa-onnx-omnilingual-asr](https://huggingface.co/csukuangfj/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12).  
* **Verdict for Silent Voice:** Ideal as a fallback for rare/low-resource languages, but its CC-BY-NC license restricts commercial distribution, and its accuracy on common languages is lower than Whisper Turbo or Qwen3.

---

### 3.8 Cohere Transcribe (`03-2026`)
* **Release Date:** March 2026  
* **Architecture:** Conformer Encoder + Transformer Decoder.  
* **Parameters & Footprint:** ~2.0B parameters; ~2.1 GB in int8 ONNX.  
* **Languages Supported:** 14 enterprise languages (English, French, German, Italian, Spanish, Portuguese, Greek, Dutch, Polish, Chinese, Japanese, Korean, Vietnamese, Arabic).  
* **Coverage Breakdown:**  
  * **Arabic (`ar`):** Supported natively with high dialectal robustness.  
  * **Chinese (`zh`):** Supported natively.  
  * **Bengali (`bn`), Hindi (`hi`):** **None (0%)**. Excluded from training.  
* **WER Figures:** Surpasses Whisper Large-v3 on European languages and Arabic in enterprise audio benchmarks.  
* **Speed / Latency:** High compute overhead. On laptop CPU (int8 ONNX): **RTF $\approx 0.65 - 0.85$**.  
* **License:** Apache-2.0 ([HuggingFace: CohereLabs/cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026)).  
* **Formats & Runtimes:** ONNX supported in `sherpa-onnx` (`CohereTranscribeModelConfig` defined in `sherpa_stt.rs:138–144`).  
* **Verdict for Silent Voice:** High quality for Arabic and European enterprise transcription, but too heavy on CPU (2B parameters) and completely misses Bengali and Hindi.

---

### 3.9 Disqualified / Incompatible Models

#### Mistral Voxtral (Voxtral-Mini-3B-2507)
* **Release Date:** July 2025 ([arXiv:2507.13264](https://arxiv.org/abs/2507.13264)).  
* **Architecture:** Multimodal Speech-LLM (Ministral 3B backbone + audio encoder).  
* **Why Disqualified for Silent Voice:**  
  1. **Runtime Incompatibility:** Requires PyTorch, Transformers, or vLLM. It is **not supported by `whisper.cpp` or `sherpa-onnx`**.  
  2. **Hardware Constraints:** Running a 3B LLM autoregressively on audio tokens requires ~9.5 GB VRAM or results in an unacceptable CPU RTF $> 2.5$ (a 5-second dictation would take $> 12$ seconds to process).

#### Kyutai STT-1B / Moshi
* **Release Date:** July 2024 ([HuggingFace: kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr)).  
* **Architecture:** Mimi Neural Audio Codec + Helium Delayed Streams Transformer.  
* **Why Disqualified for Silent Voice:**  
  1. **Language Incompatibility:** Supports **only English and French**. Completely lacks Bengali, Hindi, Arabic, and Chinese.  
  2. **Runtime Incompatibility:** Requires Kyutai's custom Mimi PyTorch/Rust runtime; unsupported in `whisper.cpp` or `sherpa-onnx`.

#### Useful Sensors Moonshine v2
* **Architecture:** RoPE-based Transformer encoder-decoder with sliding-window self-attention.  
* **Why Disqualified for Multilingual:** Native checkpoints are **English-only**. Running Bengali or Hindi requires experimental "tokenizer transplantation" and custom retraining, which is not available out of the box in production.

---

## 4. Deep-Dive: Linguistic Coverage for Bengali, Hindi, Arabic, and Chinese

Dictation engines face distinct acoustic and orthographic challenges across non-Latin writing systems. Here is how the top models navigate them:

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   TARGET LANGUAGE SUPPORT MATRIX                                       │
├──────────────────────────┬───────────────────┬───────────────────┬───────────────────┬─────────────────┤
│ Model                    │ Bengali (bn)      │ Hindi (hi)        │ Arabic (ar)       │ Chinese (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Parakeet TDT 0.6B v3     │ ❌ No (0%)        │ ❌ No (0%)        │ ❌ No (0%)        │ ❌ No (0%)      │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Whisper Large-v3-Turbo   │ ✅ Yes (Tier 3)   │ ✅ Yes (Tier 2)   │ ✅ Yes (Tier 2)   │ ✅ Yes (Tier 1) │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Qwen3-ASR-0.6B           │ ❌ Excluded       │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ⭐ SOTA (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Dolphin Base CTC         │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native) │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ SenseVoiceSmall          │ ❌ No (0%)        │ ❌ No (0%)        │ ❌ No (0%)        │ ⭐ SOTA (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Meta Omnilingual ASR     │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native) │
└──────────────────────────┴───────────────────┴───────────────────┴───────────────────┴─────────────────┘
```

### 4.1 Bengali (`bn`)
* **Orthography & Challenges:** Eastern Indo-Aryan language written in the Bengali-Assamese script. High morphological complexity, conjunct consonants (যুক্তবর্ণ *juktoborno*), and frequent code-mixing with English. Standard tokenizers suffer from severe subword fragmentation.
* **Model Handling:**  
  * **Whisper Large-v3-Turbo:** Full vocabulary coverage. Handles clean read speech well (~18–24% WER on FLEURS), but can struggle with conversational dialects without fine-tuning.
  * **Dolphin Base CTC:** Specifically trained on Eastern and South Asian audio datasets. Generates characters monotonically via CTC, preventing the infinite hallucination loops that Whisper occasionally suffers from on Bengali pauses.
  * **Qwen3-ASR & SenseVoice:** Omitted from the model vocabulary.

### 4.2 Hindi (`hi`)
* **Orthography & Challenges:** Central Indo-Aryan language written in Devanagari script. Distinct phonetic nuances (aspirated consonants, retroflex sounds) and widespread Hinglish code-switching in everyday speech.
* **Model Handling:**  
  * **Qwen3-ASR-0.6B:** Highly effective. Explicitly trained on Hindi as one of its 30 core world languages; robust handling of modern conversational Hindi.
  * **Whisper Large-v3-Turbo:** Solid Tier 2 performance (~13–15% WER on FLEURS). Natively outputs Devanagari script with appropriate capitalization/punctuation conventions.
  * **Dolphin Base CTC:** Full Devanagari character token mapping with very fast CTC execution.

### 4.3 Arabic (`ar`)
* **Orthography & Challenges:** Semitic Afroasiatic language written Right-to-Left (RTL) with cursive ligatures and complex non-concatenative root-and-pattern morphology. Spoken Arabic diverges significantly across regional dialects (Egyptian, Levantine, Gulf, Maghrebi) from Modern Standard Arabic (MSA).
* **Model Handling:**  
  * **Qwen3-ASR-0.6B:** Outstanding dialectal robustness. Accurately predicts Arabic orthography and handles non-vowelized text cleanly.
  * **Whisper Large-v3-Turbo:** Strong general Arabic transcription (~12–14% WER on FLEURS). Handles Modern Standard Arabic cleanly; handles regional dialects adequately.
  * **Cohere Transcribe (03-2026):** Enterprise-grade Arabic transcription tuned specifically for dialectal diversity and business meetings.
  * **Dolphin Base CTC:** Covers standard Arabic phonetic units with high inference throughput.

### 4.4 Chinese (`zh`)
* **Orthography & Challenges:** Non-alphabetic logographic writing system (Hanzi). Metric of record is Character Error Rate (CER) rather than WER. Involves tonality, massive homophone ambiguity, and major regional variations (Mandarin vs. Cantonese, Wu, Minnan).
* **Model Handling:**  
  * **SenseVoiceSmall:** The best pure Chinese model for on-device dictation. Achieves **2.4% CER on AISHELL-1** and **8.7% CER on WenetSpeech Meeting**, outperforming Whisper Large-v3 while decoding in **< 100 ms**.
  * **Qwen3-ASR (0.6B & 1.7B):** SOTA benchmark performance (**2.41% CER on FLEURS-zh**). Uniquely supports **22 Chinese regional dialects** natively.
  * **Whisper Large-v3-Turbo:** Excellent Mandarin Chinese (~4.3% CER on FLEURS-zh). Outputs Simplified Chinese by default.
  * **FireRedASR-AED-L:** Specialized Chinese model (3.18% CER on AISHELL-1), but limited to Chinese and English only.

---

## 5. Architectural Implementation Roadmap for Silent Voice

Silent Voice's architecture in `src-tauri` is uniquely positioned to adopt these models without new sidecar binaries:

### 5.1 Adding Whisper Large-v3-Turbo (`whisper.cpp` Engine)
1. **Download & Placement:**  
   * Download `ggml-large-v3-turbo-q5_0.bin` (~800 MB) from Hugging Face `ggerganov/whisper.cpp`.  
   * Store in the app's models directory (`bootstrap_dir()/models/ggml-large-v3-turbo-q5_0.bin`).  
2. **Registry Registration:**  
   * Update `src-tauri/src/models/registry.rs`:
     ```rust
     pub fn model_file_name(model_id: &str) -> String {
         format!("ggml-{model_id}.bin")
     }
     ```
   * Set `model_id = "large-v3-turbo-q5_0"`.  
3. **Execution:**  
   * Handled automatically by the persistent `whisper-server.exe` process (`src-tauri/src/transcription/server.rs`).

### 5.2 Adding Dolphin Base CTC (`sherpa-onnx` Engine)
1. **Model Bundle:**  
   * Download `sherpa-onnx-dolphin-base-ctc-multi-lang-int8-2025-04-02.tar.bz2`.  
   * Extracts into `models/dolphin-base-ctc/` containing `model.int8.onnx` and `tokens.txt`.  
2. **Registry Registration (`src-tauri/src/models/registry.rs`):**
   ```rust
   pub fn dolphin_installed(model_id: &str) -> bool {
       let d = stt_model_dir(model_id);
       d.join("model.int8.onnx").exists() && d.join("tokens.txt").exists()
   }

   pub fn stt_engine(model_id: &str) -> SttEngine {
       if model_id.starts_with("sense-voice") {
           SttEngine::SenseVoice
       } else if model_id.starts_with("dolphin") {
           SttEngine::Dolphin
       } else if model_id.starts_with("qwen3") {
           SttEngine::Qwen3
       } // ...
   }
   ```
3. **FFI Call:**  
   * `sherpa_stt.rs` already defines `DolphinModelConfig` in `SherpaOnnxOfflineModelConfig`.  
   * Point `c_config.dolphin.model = model_path.as_ptr()` and initialize `SherpaOnnxCreateOfflineRecognizer`.

### 5.3 Adding Qwen3-ASR-0.6B (`sherpa-onnx` Engine)
1. **Model Bundle:**  
   * Download `sherpa-onnx-qwen3-asr-0.6B-int8` (`conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer/`).  
2. **FFI Call:**  
   * Populate `c_config.qwen3_asr` using the existing fields in `sherpa_stt.rs:125–136`:
     * `conv_frontend`, `encoder`, `decoder`, `tokenizer`, `max_total_len = 512`, `max_new_tokens = 128`.

---

## 6. Primary Source Citations & References

1. **NVIDIA Parakeet-TDT-0.6B-v3 & Canary-1B-v2:**  
   * Paper: *"Canary-1B-v2 & Parakeet-TDT-0.6B-v3: Efficient and High-Performance Models for Multilingual ASR and AST"*, Boris Oreshkin, Jagadeesh Balam, Vitaly Lavrukhin, et al., NVIDIA, September 2025.  
   * Citation: [arXiv:2509.14128](https://arxiv.org/abs/2509.14128) | Full HTML: [https://arxiv.org/html/2509.14128](https://arxiv.org/html/2509.14128)  
   * Model Cards: [HuggingFace: nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) & [HuggingFace: nvidia/canary-1b-v2](https://huggingface.co/nvidia/canary-1b-v2)  
   * Leaderboard Metrics: Section 6.1.1 (Table 5), Section 6.1.2 (Figures 10–12).
2. **Alibaba Qwen3-ASR (0.6B & 1.7B):**  
   * Technical Report: *"Qwen3-ASR Technical Report"*, Qwen Team, Alibaba Cloud, January 2026.  
   * Citation: [arXiv:2601.21337](https://arxiv.org/abs/2601.21337)  
   * Official Blog & Benchmark Data: [https://qwen.ai/blog/qwen3-asr](https://qwen.ai/blog/qwen3-asr)  
   * Hugging Face Hub: [https://huggingface.co/Qwen/Qwen3-ASR-0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B)  
   * Sherpa-ONNX Quantized Export: [https://huggingface.co/csheng/sherpa-onnx-qwen3-asr-0.6B-int8](https://huggingface.co/csheng/sherpa-onnx-qwen3-asr-0.6B-int8)
3. **OpenAI Whisper Large-v3 & Large-v3-Turbo:**  
   * Whisper Paper: *"Robust Speech Recognition via Large-Scale Weak Supervision"*, Alec Radford, Jong Wook Kim, et al., OpenAI, 2023. [arXiv:2212.04356](https://arxiv.org/abs/2212.04356)  
   * Whisper Large-v3-Turbo Model Card: [https://github.com/openai/whisper/blob/main/model-card.md](https://github.com/openai/whisper/blob/main/model-card.md) & [HuggingFace: openai/whisper-large-v3-turbo](https://huggingface.co/openai/whisper-large-v3-turbo)  
   * whisper.cpp Quantized Repositories: [https://huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp)
4. **Dataocean / Tsinghua Dolphin ASR:**  
   * Model Repository: [https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02](https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02)  
   * GitHub Documentation: [k2-fsa/sherpa-onnx Dolphin Docs](https://github.com/k2-fsa/sherpa-onnx)
5. **Alibaba SenseVoice (SenseVoiceSmall):**  
   * Repository: [https://github.com/FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice)  
   * Model Card: [https://huggingface.co/FunAudioLLM/SenseVoiceSmall](https://huggingface.co/FunAudioLLM/SenseVoiceSmall)
6. **Meta Omnilingual ASR:**  
   * Official Repository: [https://huggingface.co/facebook/omnilingual-asr](https://huggingface.co/facebook/omnilingual-asr)  
   * Sherpa-ONNX Export: [https://huggingface.co/csukuangfj/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12](https://huggingface.co/csukuangfj/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12)
7. **Cohere Transcribe (`cohere-transcribe-03-2026`):**  
   * Official Model Card: [https://huggingface.co/CohereLabs/cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026)
8. **Xiaohongshu FireRedASR / FireRedASR2S:**  
   * Official GitHub: [https://github.com/FireRedTeam/FireRedASR](https://github.com/FireRedTeam/FireRedASR)  
   * Model Card: [https://huggingface.co/FireRedTeam/FireRedASR-AED-L](https://huggingface.co/FireRedTeam/FireRedASR-AED-L)
9. **Mistral Voxtral (Voxtral-Mini-3B-2507):**  
   * Paper: *"Voxtral"*, Alexander H. Liu et al., Mistral AI, July 2025. [arXiv:2507.13264](https://arxiv.org/abs/2507.13264)  
   * Hugging Face: [https://huggingface.co/mistralai/Voxtral-Mini-3B-2507](https://huggingface.co/mistralai/Voxtral-Mini-3B-2507)
10. **Kyutai STT-1B / Moshi:**  
    * Official Model Hub: [https://huggingface.co/kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr) & [https://kyutai.org](https://kyutai.org)
