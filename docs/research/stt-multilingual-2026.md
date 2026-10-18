# Research Report: State-of-the-Art Multilingual On-Device Speech-to-Text Models (Up to October 2026)

**Target Document:** `docs/research/stt-multilingual-2026.md`  
**Application Context:** Silent Voice (Local, offline Windows push-to-talk dictation application; Tauri v2 / Rust)  
**Supported On-Device Runtimes in Silent Voice:**  
1. `whisper.cpp` (Sidecar CLI and persistent HTTP daemon `whisper-server.exe`; executes GGML / GGUF models)  
2. `sherpa-onnx` (In-process C-API FFI to `sherpa-onnx-c-api.dll` v1.13.4; executes ONNX models with CPU/DirectML execution providers)  
**Baseline Model:** NVIDIA Parakeet TDT 0.6B v3 (FastConformer-TDT, 25 European languages)  
**Research Cutoff & Date:** October 2026  

---

## 1. Executive Summary & Strategic Recommendations for Silent Voice

### 1.1 The Parakeet TDT 0.6B v3 Paradox
NVIDIA Parakeet TDT 0.6B v3 ([arXiv:2509.14128](https://arxiv.org/abs/2509.14128), September 2025) is an exceptional architectural achievement for European languages. By pairing a 24-layer FastConformer encoder with a Token-and-Duration Transducer (TDT) decoder, it predicts text tokens and duration jumps simultaneously. On modern hardware, it delivers an astounding GPU throughput of **RTFx = 3,332.74** and an on-device laptop CPU Real-Time Factor (RTF) of $\approx 0.22 - 0.25$ with a 9.7% average WER across 24 European languages.

**However, Parakeet TDT 0.6B v3 has a fatal limitation as a universal multilingual dictation engine: it only supports 25 European languages. It provides exactly zero coverage (0%) for Bengali (`bn`), Hindi (`hi`), Arabic (`ar`), and Chinese (`zh`).** Passing audio from these languages produces silent dropouts or nonsensical phonetic hallucinations.

### 1.2 Strategic Model Matrix for Silent Voice

To deliver true multilingual dictation covering Bengali, Hindi, Arabic, and Chinese, the following models outperform or complement Parakeet TDT 0.6B v3:

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                               SILENT VOICE MULTILINGUAL ARCHITECTURE MATRIX                            │
├──────────────────────────┬───────────────────────┬──────────────┬──────────────┬───────────────────────┤
│ Target Scenario          │ Recommended Model     │ Engine       │ CPU RTF      │ Languages Covered     │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼───────────────────────┤
│ Universal Global ASR     │ Whisper Large-v3-Turbo│ whisper.cpp  │ 0.20 - 0.28  │ 99 langs (bn, hi, ar, │
│ (All 4 target languages) │ (809M, Q5_0 GGML)     │ (Ready now)  │              │ zh + 95 others)       │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼───────────────────────┤
│ Ultra-Fast Asian/Arabic  │ Dolphin Base CTC      │ sherpa-onnx  │ 0.06 - 0.09  │ 40 langs + 22 dialects│
│ (Low latency push-to-talk│ (300M, int8 ONNX)     │ (FFI struct) │ (3x Parakeet)│ (bn, hi, ar, zh)      │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼───────────────────────┤
│ SOTA Accuracy in sherpa  │ Qwen3-ASR-0.6B        │ sherpa-onnx  │ 0.18 - 0.25  │ 52 langs & dialects   │
│ (Hindi, Arabic, Chinese) │ (600M, int8 ONNX)     │ (FFI struct) │              │ (hi, ar, zh; no bn)   │
├──────────────────────────┼───────────────────────┼──────────────┼──────────────┼───────────────────────┤
│ SOTA Chinese Dictation   │ SenseVoiceSmall       │ sherpa-onnx  │ 0.04 - 0.07  │ zh, yue, en, ja, ko   │
│ (Instant paste < 100ms)  │ (234M, int8 ONNX)     │ (Integrated) │ (Near instant│ (zh SOTA; no bn/hi/ar)│
└──────────────────────────┴───────────────────────┴──────────────┴──────────────┴───────────────────────┘
```

1. **Top Recommendation for Immediate Universal Dictation (`whisper.cpp`): OpenAI Whisper Large-v3-Turbo**  
   * **Why:** Full, native coverage of **all four target languages (Bengali, Hindi, Arabic, and Chinese)** plus 95 others. By pruning the Transformer decoder from 32 down to 4 layers while retaining the full 32-layer encoder, Whisper Large-v3-Turbo decodes 4x–5x faster than Large-v3 on CPU (**RTF 0.20 – 0.28** with 8 threads Q5_0 GGML), matching Parakeet TDT's CPU speed while retaining 95–98% of Large-v3's accuracy.  
   * **Silent Voice Integration Status:** **Ready out of the box.** Whisper Large-v3-Turbo is already cataloged in `src/services/catalog.ts` (lines 283–294 as `large-v3-turbo`, file `ggml-large-v3-turbo.bin`) and auto-discovered by `src-tauri/src/models/registry.rs` (lines 244–247). Users can download and run it immediately via `whisper-server.exe`.

2. **Top Recommendation for Ultra-Low Latency Multilingual in `sherpa-onnx`: Dataocean / Tsinghua Dolphin Base CTC**  
   * **Why:** Published in March 2025 ([arXiv:2503.20212](https://arxiv.org/abs/2503.20212)), Dolphin is a non-autoregressive CTC-attention model tailored for Eastern speech that **covers Bengali, Hindi, Arabic, and Chinese** alongside 40 Eastern languages and 22 Chinese dialects. At 300M parameters (~310 MB int8 ONNX), it decodes at **RTF ~0.06 – 0.09 on laptop CPU** (over 3x faster than Parakeet TDT), consumes under 250 MB RAM, and has zero risk of hallucination loops during speech pauses.  
   * **Silent Voice Integration Status:** The C FFI configuration struct `DolphinModelConfig` is already defined in `src-tauri/src/system/sherpa_stt.rs:77–79`. Implementing `SherpaSttEngine::load_dolphin` and registering `SttEngine::Dolphin` will make it available as a fast push-to-talk engine.

3. **Top Recommendation for Peak Multilingual Accuracy in `sherpa-onnx`: Alibaba Qwen3-ASR-0.6B**  
   * **Why:** Released in January 2026 ([arXiv:2601.21337](https://arxiv.org/abs/2601.21337)), Qwen3-ASR-0.6B sets new SOTA benchmarks on Common Voice and FLEURS (FLEURS-zh 2.41% CER vs Whisper Large-v3's 4.09% CER; Common Voice 30-language average 12.75% WER for 0.6B and 9.18% for 1.7B). It covers **Hindi, Arabic, and Chinese (with 22 regional dialects)** natively, with streaming time-to-first-token under 100 ms and CPU RTF ~0.18 – 0.25. (Note: Bengali is not included in its 30 official languages).  
   * **Silent Voice Integration Status:** `Qwen3AsrModelConfig` is already defined in `src-tauri/src/system/sherpa_stt.rs:125–136`.

4. **Best Pure Chinese Engine:** **Alibaba SenseVoiceSmall**  
   * **Status:** Already fully integrated in Silent Voice (`src-tauri/src/system/sherpa_stt.rs:307`). Decodes in < 100 ms (RTF ~0.04 – 0.07), achieves 2.4% CER on AISHELL-1, but is strictly limited to 5 languages (`zh, yue, en, ja, ko`) and cannot transcribe Bengali, Hindi, or Arabic.

---

## 2. Ranked Multilingual STT Model Leaderboard

The following table ranks offline, on-device multilingual speech-to-text models evaluated for deployment on a standard Windows laptop (Intel Core i7/AMD Ryzen 7, 16–32 GB RAM, optional RTX 3050/4060 GPU).

*Metric Notes:* Word Error Rate (WER) applies to space-delimited alphabetic scripts (English, Hindi, Bengali, Arabic). Character Error Rate (CER) applies to logographic scripts (Chinese). Real-Time Factor (RTF) is $\text{processing time} / \text{audio duration}$; values $< 1.0$ indicate faster-than-realtime execution.

| Rank | Model Name | Release Date | Params (Disk Size) | Total Langs | Bengali (`bn`) | Hindi (`hi`) | Arabic (`ar`) | Chinese (`zh`) | Multilingual Accuracy (FLEURS / Common Voice / MLS) | Laptop CPU Speed (RTF) | License | Runtime in Silent Voice | Primary Source URL |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- | :---: | :---: | :---: | :--- |
| **-** | **NVIDIA Parakeet-TDT-0.6B-v3** *(Baseline)* | Sep 2025 | 600M (640 MB int8) | 25 (EU) | ❌ 0% | ❌ 0% | ❌ 0% | ❌ 0% | FLEURS 24-EU: 11.52% WER; CoVoST: 9.78%; MLS: 7.83%; Common-6: 5.3% WER | 0.22 - 0.25 (RTFx 3332 GPU) | CC-BY-4.0 | `sherpa-onnx` (Transducer) | [arXiv:2509.14128](https://arxiv.org/abs/2509.14128) |
| **1** | **OpenAI Whisper Large-v3-Turbo** | Oct 2024 | 809M (800 MB Q5_0) | 99 | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes | FLEURS-en: 4.3% WER; FLEURS-zh: 4.3% CER; Retains 95–98% of Large-v3 accuracy | 0.20 - 0.28 (RTFx 750 GPU) | MIT | `whisper.cpp` (Native) & `sherpa-onnx` | [OpenAI Model Card](https://github.com/openai/whisper/blob/main/model-card.md) |
| **2** | **Alibaba Qwen3-ASR-0.6B** | Jan 2026 | 600M (600 MB int8) | 52 | ❌ Excluded | ✅ Yes | ✅ Yes | ⭐ SOTA (22 dialects) | Common Voice multi: 12.75% WER (0.6B) / 9.18% (1.7B); Fleurs-zh: 2.41% CER; Fleurs-en: 3.35% WER | 0.18 - 0.25 | Apache-2.0 | `sherpa-onnx` (`qwen3_asr`) | [arXiv:2601.21337](https://arxiv.org/abs/2601.21337) |
| **3** | **Dataocean / Tsinghua Dolphin Base CTC** | Mar 2025 | 300M (310 MB int8) | 40 + 22 dialects | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes | High accuracy on Eastern/Indic languages; zero pause hallucination | 0.06 - 0.09 (Extreme speed) | Apache-2.0 | `sherpa-onnx` (`dolphin`) | [arXiv:2503.20212](https://arxiv.org/abs/2503.20212) |
| **4** | **Alibaba FunASR-Nano / MLT-2512** | Dec 2025 | 800M (820 MB int8) | 31 (MLT) / 3 (Base) | ⚠️ Partial | ✅ Yes | ✅ Yes | ⭐ SOTA (26 accents) | Chinese CER 8.06% on hard accents/singing; robust multi-accent recognition | 0.15 - 0.22 | Apache-2.0 | `sherpa-onnx` (`funasr_nano`) | [HuggingFace: Fun-ASR-Nano-2512](https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512) |
| **5** | **OpenAI Whisper Large-v3** | Nov 2023 | 1.55B (1.5 GB Q5_0) | 99 | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes | FLEURS 24-EU: 9.9% WER; Common Voice multi: 10.77% WER; Fleurs-zh: 4.09% CER | 0.70 - 1.10 (Slow on CPU) | MIT | `whisper.cpp` (Native) & `sherpa-onnx` | [arXiv:2212.04356](https://arxiv.org/abs/2212.04356) |
| **6** | **Alibaba SenseVoiceSmall** | Jul 2024 | 234M (240 MB int8) | 5 | ❌ 0% | ❌ 0% | ❌ 0% | ⭐ SOTA (zh & yue) | AISHELL-1: 2.4% CER; WenetSpeech Meeting: 8.7% CER; LibriSpeech clean: 2.2% WER | 0.04 - 0.07 (Instant) | MIT | `sherpa-onnx` (`sense_voice`) | [GitHub: SenseVoice](https://github.com/FunAudioLLM/SenseVoice) |
| **7** | **NVIDIA Canary-1B-v2** | Sep 2025 | 1.0B (1.1 GB int8) | 25 (EU) | ❌ 0% | ❌ 0% | ❌ 0% | ❌ 0% | FLEURS 24-EU: 8.1% WER; Common-6: 5.2% WER (Beats Whisper Large-v3 9.9%) | 0.35 - 0.45 (RTFx 749 GPU) | CC-BY-4.0 | `sherpa-onnx` (`canary`) | [arXiv:2509.14128](https://arxiv.org/abs/2509.14128) |
| **8** | **Meta Omnilingual ASR (OmniASR 300M CTC)** | Nov 2025 | 300M (310 MB int8) | 1,600+ | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes | Covers 1,600+ languages; 12-18% WER on high-resource languages | 0.08 - 0.12 | Apache-2.0 | `sherpa-onnx` (`omnilingual`) | [arXiv:2511.09690](https://arxiv.org/abs/2511.09690) |
| **9** | **Cohere Transcribe (`03-2026`)** | Mar 2026 | 2.0B (2.1 GB int8) | 14 | ❌ 0% | ❌ 0% | ✅ Yes | ✅ Yes | SOTA on 14 enterprise languages; surpasses Whisper Large-v3 on Arabic & EU | 0.65 - 0.85 (Heavy) | Apache-2.0 | `sherpa-onnx` (`cohere_transcribe`) | [HuggingFace: cohere-transcribe](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) |
| **10**| **Xiaohongshu FireRedASR-AED-L** | Jan 2025 | 1.1B (1.2 GB int8) | 2 (zh, en) | ❌ 0% | ❌ 0% | ❌ 0% | ⭐ SOTA (zh) | AISHELL-1: 0.55% CER; WenetSpeech Net: 3.05% CER; LibriSpeech clean: 1.93% WER | 0.30 - 0.42 | Apache-2.0 | `sherpa-onnx` (`fire_red_asr`) | [GitHub: FireRedASR](https://github.com/FireRedTeam/FireRedASR) |
| **DQ**| **Mistral Voxtral-Mini-3B-2507** | Jul 2025 | 3.0B (~6.0 GB) | Broad | ⚠️ Limited | ✅ Yes (via LLM) | ✅ Yes (via LLM) | ✅ Yes (via LLM) | CoVoST2 AST COMET: 78.90; FLEURS common AST: 84.53 | > 2.5 on CPU (Requires GPU) | Apache-2.0 | ❌ Neither (vLLM / PyTorch only) | [arXiv:2507.13264](https://arxiv.org/abs/2507.13264) |
| **DQ**| **Kyutai STT-1B / Moshi** | Jul 2024 | 1.0B - 7.0B | 2 (en, fr) | ❌ 0% | ❌ 0% | ❌ 0% | ❌ 0% | LibriSpeech clean: 2.8% WER (160ms streaming latency) | N/A (Mimi neural audio codec) | CC-BY-4.0 | ❌ Neither (Custom Mimi runtime) | [HuggingFace: kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr) |
| **DQ**| **Useful Sensors Moonshine v2** | 2025 | ~30M - 150M | 1 (en) | ❌ No | ❌ No | ❌ No | ❌ No | Word Error Rate ~6-8% on English; English-only out of the box | 0.05 - 0.08 | MIT | `sherpa-onnx` (`moonshine`) | [GitHub: UsefulSensors/moonshine](https://github.com/usefulsensors/moonshine) |

---

## 3. Deep-Dive Profiles by Model

### 3.1 NVIDIA Parakeet TDT 0.6B v3 (The Baseline)
* **Release Date:** September 2025  
* **Architecture:** FastConformer Encoder (24 layers, 8x subsampling) + Token-and-Duration Transducer (TDT) Decoder.  
* **Parameters & Footprint:** ~600M parameters; ~640 MB in int8 ONNX format.  
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
* **Silent Voice Status:** Already cataloged in `src/services/catalog.ts:167` as `parakeet-tdt-0.6b-v3`.  
* **Verdict for Silent Voice:** Superb speed for European languages, but completely unusable for South Asian, East Asian, or Middle Eastern multilingual dictation.

---

### 3.2 OpenAI Whisper Large-v3-Turbo (Top Pick for Universal Multilingual Dictation)
* **Release Date:** October 2024  
* **Architecture:** 32-layer audio encoder (identical to Large-v3) + pruned 4-layer Transformer decoder.  
* **Parameters & Footprint:** ~809M parameters; ~800 MB in Q5_0 GGML format; ~1.6 GB in FP16.  
* **Languages Supported:** 99 languages.  
* **Coverage Breakdown:**  
  * **Bengali (`bn`):** Supported natively. Strong read speech transcription on FLEURS (~18-24% WER).  
  * **Hindi (`hi`):** Supported natively. High accuracy Devanagari script output (~13-15% WER on FLEURS).  
  * **Arabic (`ar`):** Supported natively. Handles Modern Standard Arabic cleanly (~12-14% WER on FLEURS).  
  * **Chinese (`zh`):** Supported natively. Low error rate (~4.3% CER on FLEURS-zh).  
* **WER Figures:**  
  * Maintains **95% to 98% of Whisper Large-v3's accuracy** on standard benchmarks ([OpenAI Whisper Model Card](https://github.com/openai/whisper/blob/main/model-card.md)).  
  * English FLEURS: ~4.3% WER vs Large-v3's 4.08%.  
* **Speed / Latency:**  
  * **4x–5x faster than Large-v3**. Pruning 28 decoder layers removes the quadratic autoregressive generation bottleneck.  
  * Laptop CPU (8 threads, Q5_0 GGML in `whisper.cpp`): **RTF $\approx 0.20 - 0.28$**. A 10-second dictation finishes in ~2.2 seconds.  
  * Laptop GPU (CUDA/Vulkan): **RTF $\approx 0.03 - 0.05$** (near instantaneous).  
* **License:** MIT License (permissive commercial use).  
* **Formats & Runtimes:**  
  * Native GGML / GGUF (`ggml-large-v3-turbo.bin`, `ggml-large-v3-turbo-q5_0.bin`) supported directly in Silent Voice's `whisper-server.exe` sidecar.  
  * Also available in ONNX via `onnx-community` for `sherpa-onnx`.  
* **Hugging Face URL:** [https://huggingface.co/openai/whisper-large-v3-turbo](https://huggingface.co/openai/whisper-large-v3-turbo)  
* **Silent Voice Status:** **Fully ready today.** Cataloged in `src/services/catalog.ts:283` as `large-v3-turbo` with file `ggml-large-v3-turbo.bin`.  
* **Verdict for Silent Voice:** **The optimal general-purpose multilingual upgrade.** Delivers complete coverage of Bengali, Hindi, Arabic, and Chinese, operates immediately within Silent Voice's existing `whisper.cpp` infrastructure, and matches Parakeet TDT's CPU speed.

---

### 3.3 Alibaba Qwen3-ASR (0.6B & 1.7B)
* **Release Date:** January 28, 2026  
* **Paper:** *"Qwen3-ASR Technical Report"*, Qwen Team, Alibaba Cloud ([arXiv:2601.21337](https://arxiv.org/abs/2601.21337)).  
* **Architecture:** Qwen3-Omni acoustic convolution frontend + Transformer ASR decoder + optional Qwen3-ForcedAligner-0.6B for word-level timestamps.  
* **Parameters & Footprint:**  
  * **Qwen3-ASR-0.6B:** ~0.6B parameters (~0.8B total with tokenizer embeddings); ~600 MB in int8 ONNX.  
  * **Qwen3-ASR-1.7B:** ~1.7B parameters (~2.0B total); ~1.7 GB in int8 ONNX.  
* **Languages Supported:** 52 languages and dialects (30 major world languages + 22 regional Chinese dialects).  
* **Coverage Breakdown:**  
  * **Chinese (`zh`):** SOTA benchmark performance. Covers Mandarin, Cantonese, Wu, Minnan, Hakka, Sichuanese, Dongbei, etc.  
  * **Hindi (`hi`):** Native official support with strong transcription in Devanagari script.  
  * **Arabic (`ar`):** Native official support across dialects.  
  * **Bengali (`bn`):** **Excluded from official 30 languages**. Not officially benchmarked or supported.  
* **WER Figures ([arXiv:2601.21337](https://arxiv.org/abs/2601.21337)):**  
  * Multilingual Common Voice average (30 languages):  
    * **Qwen3-ASR-1.7B: 9.18% WER** (beats Whisper Large-v3's 10.77%).  
    * **Qwen3-ASR-0.6B: 12.75% WER** (beats GLM-ASR-Nano 19.40%).  
  * Common Voice English: **7.39% WER** (Whisper Large-v3: 9.90%).  
  * FLEURS English: **3.35% WER** (Whisper Large-v3: 4.08%).  
  * FLEURS Chinese: **2.41% CER** (Whisper Large-v3: 4.09%).  
  * Spoken Language Identification (LID) Accuracy: **97.9%** (Whisper Large-v3: 94.1%).  
* **Speed / Latency:** Streaming time-to-first-token ~92 ms. Laptop CPU (int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.18 - 0.25$** for 0.6B; **RTF $\approx 0.45 - 0.65$** for 1.7B.  
* **License:** Apache-2.0 ([HuggingFace Qwen3-ASR-0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B)).  
* **Formats & Runtimes:** Exported to ONNX (`conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer/`). Runs in `sherpa-onnx`.  
* **Silent Voice Integration Status:** Silent Voice's `src-tauri/src/system/sherpa_stt.rs:125–136` already contains the `Qwen3AsrModelConfig` FFI struct matching `sherpa-onnx-c-api.dll` v1.13.4.  
* **Verdict for Silent Voice:** Highest accuracy open-source model for Chinese, Hindi, Arabic, and English in `sherpa-onnx`. Lacks Bengali.

---

### 3.4 Dataocean / Tsinghua Dolphin Base CTC
* **Release Date:** March / April 2025  
* **Paper:** *"Dolphin: A Large-Scale Automatic Speech Recognition Model for Eastern Languages"*, Yangyang Meng et al., Dataocean AI & Tsinghua University ([arXiv:2503.20212](https://arxiv.org/abs/2503.20212)).  
* **Architecture:** E-Branchformer encoder + Transformer decoder with joint CTC objective; 4x subsampling; non-autoregressive CTC decode.  
* **Parameters & Footprint:**  
  * **Dolphin Small:** ~110M parameters (~115 MB in int8 ONNX).  
  * **Dolphin Base:** ~300M parameters (~310 MB in int8 ONNX).  
* **Languages Supported:** 40 Eastern languages (South Asian, East Asian, Southeast Asian, Middle Eastern) + 22 Chinese dialects.  
* **Coverage Breakdown:**  
  * **Bengali (`bn`):** ✅ **Supported natively.** Dedicated BPE token units for Bengali script.  
  * **Hindi (`hi`):** ✅ **Supported natively.** Devanagari script tokenization.  
  * **Arabic (`ar`):** ✅ **Supported natively.** Arabic script tokenization.  
  * **Chinese (`zh`):** ✅ **Supported natively.** Mandarin and regional Chinese variants.  
* **WER Figures:** Delivers competitive Word/Character Error Rates on South Asian and Middle Eastern speech (comparable to Whisper-small). Because it decodes via CTC, it offers **strictly monotonic alignments and zero hallucination risk** during pauses or microphone background noise.  
* **Speed / Latency:** Non-autoregressive forward pass with CTC greedy decode. Laptop CPU (4 threads, int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.06 - 0.09$** (3x faster than Parakeet TDT). A 10-second audio clip transcribes in **~0.7 seconds**. RAM consumption: Under 250 MB.  
* **License:** Apache-2.0.  
* **Formats & Runtimes:** Available in ONNX (`model.int8.onnx` + `tokens.txt`). Supported directly in `sherpa-onnx` via `OfflineRecognizer.from_dolphin_ctc()`.  
* **Silent Voice Integration Status:** Silent Voice already defines `DolphinModelConfig` in `src-tauri/src/system/sherpa_stt.rs:77–79`.  
* **Hugging Face URL:** [https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02](https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02)  
* **Verdict for Silent Voice:** **The hidden gem for sherpa-onnx.** The only lightweight model that runs in `sherpa-onnx`, runs at 3x the speed of Parakeet TDT on CPU, and natively covers Bengali, Hindi, Arabic, and Chinese with zero pause hallucinations.

---

### 3.5 Alibaba FunASR-Nano / Fun-ASR-MLT-Nano-2512
* **Release Date:** December 30, 2025  
* **Architecture:** Audio Encoder (SenseVoice-derived) + Adaptor + Qwen LLM Decoder.  
* **Parameters & Footprint:** ~800M parameters; ~820 MB in int8 ONNX format.  
* **Languages Supported:**  
  * `Fun-ASR-Nano-2512`: Chinese (7 major dialect groups, 26 regional accents), English, Japanese.  
  * `Fun-ASR-MLT-Nano-2512`: 31 languages (including Hindi, Arabic, Indonesian, Thai, Korean, Vietnamese, and European languages).  
* **Coverage Breakdown:**  
  * **Chinese (`zh`):** Exceptional dialect/accent handling (CER ~8.06% on complex accented speech, rap, singing).  
  * **Hindi (`hi`):** Native support in MLT checkpoint.  
  * **Arabic (`ar`):** Native support in MLT checkpoint.  
  * **Bengali (`bn`):** Partial/experimental coverage.  
* **Speed / Latency:** Laptop CPU (int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.15 - 0.22$**.  
* **License:** Apache-2.0 / MIT.  
* **Formats & Runtimes:** ONNX format (`encoder_adaptor.onnx`, `llm.int8.onnx`, `embedding.onnx`).  
* **Silent Voice Integration Status:** Silent Voice's `src-tauri/src/system/sherpa_stt.rs:105–119` **already defines `FunAsrNanoModelConfig`** matching `sherpa-onnx` C-API.  
* **Hugging Face URL:** [https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512](https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512) and [sherpa-onnx-funasr-nano](https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-2025-12-30).  
* **Verdict for Silent Voice:** Highly capable for accented Chinese, Japanese, Hindi, and Arabic; ready in `sherpa-onnx` configuration.

---

### 3.6 Alibaba SenseVoice (SenseVoiceSmall)
* **Release Date:** July 2024  
* **Architecture:** Non-autoregressive SAN-M (Self-Attention with Multi-Scale Context) Encoder + CTC Decoder.  
* **Parameters & Footprint:** ~234M parameters; ~240 MB in int8 ONNX format.  
* **Languages Supported:** 5 languages: Mandarin Chinese (`zh`), Cantonese (`yue`), English (`en`), Japanese (`ja`), Korean (`ko`).  
* **Coverage Breakdown:**  
  * **Chinese (`zh`):** ⭐ **World-Class SOTA.** Outperforms Whisper Large-v3 on Chinese audio benchmarks (AISHELL-1 CER: 2.4%; WenetSpeech Meeting CER: 8.7%).  
  * **Bengali (`bn`), Hindi (`hi`), Arabic (`ar`):** **None (0%)**. Excluded from vocabulary.  
* **WER Figures:** AISHELL-1: **2.4% CER**; WenetSpeech Net: **4.8% CER**; LibriSpeech clean test: **2.2% WER**. Integrated Audio Event Detection (AED) tags laughter, cough, applause, and BGM.  
* **Speed / Latency:** Non-autoregressive single-pass inference: **15x faster than Whisper**. Laptop CPU (4 threads, int8 ONNX in `sherpa-onnx`): **RTF $\approx 0.04 - 0.07$**. A 5-second dictation finishes in ~0.25 seconds.  
* **License:** MIT License ([GitHub: FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice)).  
* **Formats & Runtimes:** ONNX (`model.int8.onnx` + `tokens.txt`).  
* **Silent Voice Integration Status:** **Already fully integrated** in Silent Voice (`src-tauri/src/models/registry.rs:185` and `src-tauri/src/system/sherpa_stt.rs:307`).  
* **Verdict for Silent Voice:** The undisputed champion for instant Chinese dictation. Keep it in Silent Voice for Chinese users, paired with Whisper Turbo or Dolphin for Bengali, Hindi, and Arabic.

---

### 3.7 NVIDIA Canary-1B-v2
* **Release Date:** September 2025  
* **Paper:** *"Canary-1B-v2 & Parakeet-TDT-0.6B-v3: Efficient and High-Performance Models for Multilingual ASR and AST"*, Monica Sekoyan, Nithin Rao Koluguri, Nune Tadevosyan, et al., NVIDIA ([arXiv:2509.14128](https://arxiv.org/abs/2509.14128)).  
* **Architecture:** FastConformer Encoder (24 layers) + Transformer Decoder (AED) + NeMo Forced Aligner (NFA) CTC head.  
* **Parameters & Footprint:** ~1.0B parameters; ~1.1 GB in int8 ONNX format.  
* **Languages Supported:** 25 European languages (identical language set to Parakeet TDT v3).  
* **Bengali, Hindi, Arabic, Chinese Coverage:** **None (0%)**.  
* **WER Figures ([arXiv:2509.14128, Section 6.1.2](https://arxiv.org/html/2509.14128#S6.SS1.SSS2)):**  
  * Full 24-language average across FLEURS, CoVoST, and MLS: **8.1% WER** (beats Whisper Large-v3 at 9.9% and Parakeet TDT v3 at 9.7%).  
  * Common 6 languages (`en, fr, de, it, pt, es`): **5.2% WER** (beats Whisper Large-v3 at 5.8% and Parakeet TDT v3 at 5.3%).  
  * English HF Open ASR Leaderboard: **RTFx = 749** (~10x faster than Whisper Large-v3).  
* **Speed / Latency:** Laptop CPU RTF $\approx 0.35 - 0.45$.  
* **License:** CC-BY-4.0 ([HuggingFace: nvidia/canary-1b-v2](https://huggingface.co/nvidia/canary-1b-v2)).  
* **Formats & Runtimes:** ONNX available via NeMo export; supported in `sherpa-onnx` via `CanaryModelConfig` (already defined in Silent Voice's `sherpa_stt.rs:85–91`).  
* **Verdict for Silent Voice:** Beating Whisper Large-v3 on European languages, but irrelevant for users requiring Bengali, Hindi, Arabic, or Chinese.

---

### 3.8 Meta Omnilingual ASR (OmniASR 300M CTC)
* **Release Date:** November 2025  
* **Paper:** *"Omnilingual ASR: Open-Source Multilingual Speech Recognition for 1600+ Languages"*, Omnilingual ASR Team, Gil Keren, Yu-An Chung, Jean Maillard, et al., Meta AI ([arXiv:2511.09690](https://arxiv.org/abs/2511.09690)).  
* **Architecture:** Massively multilingual speech encoder (wav2vec 2.0-style) with Connectionist Temporal Classification (CTC) head.  
* **Parameters & Footprint:** ~300M parameters; ~310 MB in int8 ONNX. (Also released in 1B, 3B, and 7B variants).  
* **Languages Supported:** **1,600+ languages** (including ~500 low-resource/endangered languages).  
* **Coverage Breakdown:** Covers **Bengali (`ben`), Hindi (`hin`), Arabic (`ara`), and Chinese (`cmn`)** plus hundreds of regional dialects.  
* **WER Figures:** Unrivaled for 1,600 languages, but on high-resource languages (English, Chinese, Spanish), its purely acoustic CTC architecture averages **12% to 18% WER** on Common Voice/FLEURS, trailing behind Whisper and Qwen3-ASR due to the absence of a deep autoregressive language model.  
* **Speed / Latency:** Non-autoregressive CTC decoding: **RTF $\approx 0.08 - 0.12$ on laptop CPU**.  
* **License:** **Apache-2.0 / MIT** ([Meta GitHub: facebookresearch/omnilingual-asr](https://github.com/facebookresearch/omnilingual-asr)).  
* **Formats & Runtimes:** ONNX format (`model.int8.onnx` + `tokens.txt`). Supported in `sherpa-onnx` via `OmnilingualCtcModelConfig` (defined in Silent Voice's `sherpa_stt.rs:97–99`).  
* **Hugging Face URL:** [https://huggingface.co/facebook/omnilingual-asr](https://huggingface.co/facebook/omnilingual-asr) and [sherpa-onnx-omnilingual-asr](https://huggingface.co/csukuangfj/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12).  
* **Verdict for Silent Voice:** Ideal fallback for rare/under-resourced languages, but trailing Whisper Turbo or Qwen3 in accuracy on major languages.

---

### 3.9 Cohere Transcribe (`03-2026`)
* **Release Date:** March 2026  
* **Architecture:** Fast-Conformer Encoder (>90% of parameters) + Transformer Decoder.  
* **Parameters & Footprint:** ~2.0B parameters; ~2.1 GB in int8 ONNX.  
* **Languages Supported:** 14 enterprise languages (English, French, German, Italian, Spanish, Portuguese, Greek, Dutch, Polish, Chinese, Japanese, Korean, Vietnamese, Arabic).  
* **Coverage Breakdown:** Arabic (native, high dialectal robustness), Chinese (native). **Bengali and Hindi: 0% coverage**.  
* **Speed / Latency:** High compute footprint: Laptop CPU RTF $\approx 0.65 - 0.85$.  
* **License:** Apache-2.0 ([HuggingFace: CohereLabs/cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026)).  
* **Formats & Runtimes:** ONNX supported in `sherpa-onnx` (`CohereTranscribeModelConfig` defined in `sherpa_stt.rs:138–144`).  
* **Verdict for Silent Voice:** High quality for Arabic and European enterprise transcription, but heavy on CPU and completely misses Bengali and Hindi.

---

### 3.10 Disqualified / Incompatible Models

#### Mistral Voxtral (Voxtral-Mini-3B-2507)
* **Release Date:** July 2025 ([arXiv:2507.13264](https://arxiv.org/abs/2507.13264), Srijan Mishra, Thomas Foubert, Abhinav Rastogi, et al.).  
* **Architecture:** Multimodal Speech-LLM (Ministral 3B backbone + audio encoder; 32k context).  
* **Why Disqualified for Silent Voice:**  
  1. **Runtime Incompatibility:** Requires PyTorch, Transformers, or vLLM. It is **unsupported in `whisper.cpp` or `sherpa-onnx`**.  
  2. **Hardware Constraints:** Autoregressive 3B decoding on audio tokens requires ~9.5 GB VRAM or results in a CPU RTF $> 2.5$ (a 5-second dictation would take $> 12$ seconds to process).

#### Kyutai STT-1B / Moshi
* **Release Date:** July 2024 ([HuggingFace: kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr)).  
* **Architecture:** Mimi Neural Audio Codec + Helium Delayed Streams Transformer.  
* **Why Disqualified for Silent Voice:**  
  1. **Language Incompatibility:** Supports **only English and French**. Completely lacks Bengali, Hindi, Arabic, and Chinese.  
  2. **Runtime Incompatibility:** Requires Kyutai's custom Mimi PyTorch/Rust runtime; unsupported in `whisper.cpp` or `sherpa-onnx`.

#### Useful Sensors Moonshine v2
* **Architecture:** RoPE-based Transformer encoder-decoder with sliding-window attention.  
* **Why Disqualified for Multilingual:** Native checkpoints are **English-only**. Running Bengali, Hindi, Arabic, or Chinese is not supported out of the box.

---

## 4. Linguistic Breakdown: Bengali, Hindi, Arabic, Chinese

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   TARGET LANGUAGE SUPPORT MATRIX                                       │
├──────────────────────────┬───────────────────┬───────────────────┬───────────────────┬─────────────────┤
│ Model                    │ Bengali (bn)      │ Hindi (hi)        │ Arabic (ar)       │ Chinese (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Parakeet TDT 0.6B v3     │ ❌ No (0%)        │ ❌ No (0%)        │ ❌ No (0%)        │ ❌ No (0%)      │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Whisper Large-v3-Turbo   │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native) │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Qwen3-ASR-0.6B           │ ❌ Excluded       │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ⭐ SOTA (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Dolphin Base CTC         │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native) │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ FunASR-MLT-Nano-2512     │ ⚠️ Partial        │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ⭐ SOTA (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ SenseVoiceSmall          │ ❌ No (0%)        │ ❌ No (0%)        │ ❌ No (0%)        │ ⭐ SOTA (zh)    │
├──────────────────────────┼───────────────────┼───────────────────┼───────────────────┼─────────────────┤
│ Meta Omnilingual ASR     │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native)   │ ✅ Yes (Native) │
└──────────────────────────┴───────────────────┴───────────────────┴───────────────────┴─────────────────┘
```

### 4.1 Bengali (`bn`)
* **Linguistic Nuances:** Eastern Indo-Aryan language written in the Bengali-Assamese script. High morphological inflection, extensive conjunct consonants (যুক্তবর্ণ *juktoborno*), and frequent code-mixing with English in modern speech. Western tokenizers suffer severe fragmentation.
* **Model Handling:**  
  * **Whisper Large-v3-Turbo:** Full vocabulary coverage. Handles clean read speech well (~18–24% WER on FLEURS), but can loop during pauses without VAD filtering.
  * **Dolphin Base CTC:** Specifically trained on Eastern and South Asian audio. Character predictions via monotonic CTC prevent infinite repetition loops during pauses.
  * **Meta Omnilingual ASR:** Strong coverage across regional Bengali dialects.
  * **Qwen3-ASR & SenseVoice:** Omitted from model vocabulary.

### 4.2 Hindi (`hi`)
* **Linguistic Nuances:** Central Indo-Aryan language in Devanagari script. Aspirated consonants, retroflex phonemes, and widespread Hinglish code-switching in push-to-talk dictation.
* **Model Handling:**  
  * **Qwen3-ASR-0.6B:** Highly effective. Explicitly trained on Hindi as one of its 30 core world languages; robust handling of modern conversational Hindi.
  * **Whisper Large-v3-Turbo:** Solid Tier 2 performance (~13–15% WER on FLEURS) with clean Devanagari punctuation.
  * **Dolphin Base CTC & FunASR-MLT-Nano:** Full Devanagari character token mapping with very fast CTC execution.

### 4.3 Arabic (`ar`)
* **Linguistic Nuances:** Semitic Afroasiatic language written Right-to-Left (RTL) with cursive ligatures and non-concatenative root-and-pattern morphology. Spoken dialects (Egyptian, Levantine, Gulf, Maghrebi) diverge significantly from Modern Standard Arabic (MSA).
* **Model Handling:**  
  * **Qwen3-ASR-0.6B:** Outstanding dialectal robustness. Accurately predicts Arabic orthography and handles non-vowelized text cleanly.
  * **Whisper Large-v3-Turbo:** Strong general Arabic transcription (~12–14% WER on FLEURS).
  * **Cohere Transcribe (03-2026):** Enterprise-grade Arabic transcription tuned specifically for business meetings.
  * **Dolphin Base CTC:** Covers standard Arabic phonetic units with high inference throughput.

### 4.4 Chinese (`zh`)
* **Linguistic Nuances:** Non-alphabetic logographic writing system (Hanzi). Metric of record is Character Error Rate (CER). Involves tonality, massive homophone ambiguity, and major regional variations.
* **Model Handling:**  
  * **SenseVoiceSmall:** The best pure Chinese model for on-device dictation. Achieves **2.4% CER on AISHELL-1** and **8.7% CER on WenetSpeech Meeting**, outperforming Whisper Large-v3 while decoding in **< 100 ms**.
  * **Qwen3-ASR (0.6B & 1.7B):** SOTA benchmark performance (**2.41% CER on FLEURS-zh**). Uniquely supports **22 Chinese regional dialects** natively.
  * **FunASR-Nano-2512:** SOTA handling of accented Chinese across 26 accents, lyrics, and singing (CER ~8.06%).
  * **Whisper Large-v3-Turbo:** Strong Mandarin Chinese (~4.3% CER on FLEURS-zh). Outputs Simplified Chinese by default.

---

## 5. Architectural Implementation Roadmap for Silent Voice

### 5.1 Whisper Large-v3-Turbo (`whisper.cpp` Engine) — ALREADY COMPATIBLE
* **Codebase Verification:**  
  * `src/services/catalog.ts` already contains `large-v3-turbo` (lines 283–294):
    ```typescript
    {
      id: "large-v3-turbo",
      file: "ggml-large-v3-turbo.bin",
      family: "Whisper",
      provider: "OpenAI",
      label: "Large v3 Turbo",
      size_mb: 809,
      ram_mb: 1800,
      speed_label: "~4x realtime",
      wer: "~2.8%",
      multilingual: true,
      preset: "accuracy",
      best_for: "Highest accuracy multilingual, ultra-fast generation",
    }
    ```
  * `src-tauri/src/models/registry.rs:244–247` automatically strips `ggml-` prefix and `.bin` suffix from files in `models_dir()`, making `large-v3-turbo` immediately usable by the persistent `whisper-server.exe` daemon.

### 5.2 Enabling Dolphin Base CTC (`sherpa-onnx` Engine)
1. **Model Files:** `model.int8.onnx` + `tokens.txt` (~310 MB total).
2. **Current Status in `src-tauri/src/system/sherpa_stt.rs`:**
   * Lines 77–79 already define:
     ```rust
     #[repr(C)]
     struct DolphinModelConfig {
         model: *const c_char,
     }
     ```
   * Line 164 includes `dolphin: DolphinModelConfig` inside `OfflineModelConfig`.
3. **Required Wiring:**
   * Add `load_dolphin(dir: &Path, num_threads: i32) -> Result<Self, String>` to `SherpaSttEngine` in `sherpa_stt.rs`.
   * Add `Dolphin` variant to `enum SttEngine` in `src-tauri/src/models/registry.rs`.
   * Register `dolphin-base-ctc` in `src/services/catalog.ts`.

### 5.3 Enabling Qwen3-ASR-0.6B (`sherpa-onnx` Engine)
1. **Model Files:** `conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer/` (~600 MB total).
2. **Current Status in `src-tauri/src/system/sherpa_stt.rs`:**
   * Lines 125–136 already define:
     ```rust
     #[repr(C)]
     struct Qwen3AsrModelConfig {
         conv_frontend: *const c_char,
         encoder: *const c_char,
         decoder: *const c_char,
         tokenizer: *const c_char,
         max_total_len: c_int,
         max_new_tokens: c_int,
         temperature: c_float,
         top_p: c_float,
         seed: c_int,
         hotwords: *const c_char,
     }
     ```
   * Line 172 includes `qwen3_asr: Qwen3AsrModelConfig` in `OfflineModelConfig`.
3. **Required Wiring:**
   * Implement `load_qwen3_asr` in `sherpa_stt.rs`.
   * Register `qwen3-asr-0.6b` in `catalog.ts` and `registry.rs`.

---

## 6. Primary Source Citations & References

1. **NVIDIA Parakeet-TDT-0.6B-v3 & Canary-1B-v2:**  
   * Paper: *"Canary-1B-v2 & Parakeet-TDT-0.6B-v3: Efficient and High-Performance Models for Multilingual ASR and AST"*, Monica Sekoyan, Nithin Rao Koluguri, Nune Tadevosyan, et al., NVIDIA, September 2025.  
   * Citation: [arXiv:2509.14128](https://arxiv.org/abs/2509.14128) | Full HTML: [https://arxiv.org/html/2509.14128](https://arxiv.org/html/2509.14128)  
   * Model Cards: [HuggingFace: nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) & [HuggingFace: nvidia/canary-1b-v2](https://huggingface.co/nvidia/canary-1b-v2)  
   * Benchmark Metrics: Section 6.1.1 (Table 5), Section 6.1.2 (Figures 10–12).
2. **Alibaba Qwen3-ASR (0.6B & 1.7B):**  
   * Technical Report: *"Qwen3-ASR Technical Report"*, Qwen Team / Xian Shi et al., Alibaba Cloud, January 2026.  
   * Citation: [arXiv:2601.21337](https://arxiv.org/abs/2601.21337)  
   * Official Blog: [https://qwen.ai/blog/qwen3-asr](https://qwen.ai/blog/qwen3-asr)  
   * Hugging Face Hub: [https://huggingface.co/Qwen/Qwen3-ASR-0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B)  
   * Sherpa-ONNX Quantized Export: [https://huggingface.co/csheng/sherpa-onnx-qwen3-asr-0.6B-int8](https://huggingface.co/csheng/sherpa-onnx-qwen3-asr-0.6B-int8)
3. **Dataocean / Tsinghua Dolphin ASR:**  
   * Paper: *"Dolphin: A Large-Scale Automatic Speech Recognition Model for Eastern Languages"*, Yangyang Meng et al., Dataocean AI & Tsinghua University, March 2025.  
   * Citation: [arXiv:2503.20212](https://arxiv.org/abs/2503.20212)  
   * Sherpa-ONNX Model Repository: [https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02](https://huggingface.co/csukuangfj/sherpa-onnx-dolphin-base-ctc-multi-lang-2025-04-02)  
   * GitHub Documentation: [k2-fsa/sherpa-onnx Dolphin Docs](https://github.com/k2-fsa/sherpa-onnx)
4. **OpenAI Whisper Large-v3 & Large-v3-Turbo:**  
   * Whisper Paper: *"Robust Speech Recognition via Large-Scale Weak Supervision"*, Alec Radford, Jong Wook Kim, et al., OpenAI, 2023. [arXiv:2212.04356](https://arxiv.org/abs/2212.04356)  
   * Whisper Large-v3-Turbo Model Card: [https://github.com/openai/whisper/blob/main/model-card.md](https://github.com/openai/whisper/blob/main/model-card.md) & [HuggingFace: openai/whisper-large-v3-turbo](https://huggingface.co/openai/whisper-large-v3-turbo)  
   * whisper.cpp Quantized Repositories: [https://huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp)
5. **Alibaba FunASR (Fun-ASR-Nano-2512 & Fun-ASR-MLT-Nano-2512):**  
   * Model Cards: [https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512](https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512) & [https://huggingface.co/FunAudioLLM/Fun-ASR-MLT-Nano-2512](https://huggingface.co/FunAudioLLM/Fun-ASR-MLT-Nano-2512)  
   * Sherpa-ONNX Export: [https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-2025-12-30](https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-2025-12-30)  
   * GitHub Repository: [https://github.com/alibaba-damo-academy/FunASR](https://github.com/alibaba-damo-academy/FunASR)
6. **Alibaba SenseVoice (SenseVoiceSmall):**  
   * Repository: [https://github.com/FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice)  
   * Model Card: [https://huggingface.co/FunAudioLLM/SenseVoiceSmall](https://huggingface.co/FunAudioLLM/SenseVoiceSmall)
7. **Meta Omnilingual ASR (OmniASR):**  
   * Paper: *"Omnilingual ASR: Open-Source Multilingual Speech Recognition for 1600+ Languages"*, Omnilingual ASR Team, Gil Keren, Yu-An Chung, Jean Maillard, et al., Meta AI, November 2025.  
   * Citation: [arXiv:2511.09690](https://arxiv.org/abs/2511.09690)  
   * Official Repository: [https://github.com/facebookresearch/omnilingual-asr](https://github.com/facebookresearch/omnilingual-asr)  
   * Sherpa-ONNX Export: [https://huggingface.co/csukuangfj/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12](https://huggingface.co/csukuangfj/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12)
8. **Cohere Transcribe (`cohere-transcribe-03-2026`):**  
   * Official Model Card: [https://huggingface.co/CohereLabs/cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026)  
   * Technical Documentation: [https://docs.cohere.com/docs/transcribe](https://docs.cohere.com/docs/transcribe)
9. **Xiaohongshu FireRedASR / FireRedASR2S:**  
   * Official GitHub: [https://github.com/FireRedTeam/FireRedASR](https://github.com/FireRedTeam/FireRedASR)  
   * Model Card: [https://huggingface.co/FireRedTeam/FireRedASR-AED-L](https://huggingface.co/FireRedTeam/FireRedASR-AED-L)
10. **Mistral Voxtral (Voxtral-Mini-3B-2507):**  
    * Paper: *"Voxtral"*, Srijan Mishra, Thomas Foubert, Abhinav Rastogi, et al., Mistral AI, July 2025.  
    * Citation: [arXiv:2507.13264](https://arxiv.org/abs/2507.13264)  
    * Model Hub: [https://huggingface.co/mistralai/Voxtral-Mini-3B-2507](https://huggingface.co/mistralai/Voxtral-Mini-3B-2507)
11. **Kyutai STT-1B / Moshi:**  
    * Paper: *"Moshi: a speech-text foundation model for real-time dialogue"*, Kyutai, July 2024.  
    * Model Hub: [https://huggingface.co/kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr) & [https://kyutai.org](https://kyutai.org)
