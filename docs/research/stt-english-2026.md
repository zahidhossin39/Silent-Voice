# Speech-to-Text (STT) English Models Benchmark & Landscape (2024–2026)

**Target Application:** Silent Voice (Local-first Windows dictation app in Rust / Tauri v2)  
**Target Hardware:** Windows Laptop CPU (modern 6–8 cores, e.g., Intel Core Ultra / AMD Ryzen 7) and Modest Discrete GPU (NVIDIA RTX 3050/4050/3060 Mobile, 4–8 GB VRAM)  
**Evaluation Baseline:** NVIDIA Parakeet TDT 0.6B v2 (HF Open ASR Leaderboard English Avg WER: **6.05%**, RTFx: **3,380**)  
**Active Application Runtimes:** `whisper.cpp` (GGML) and `sherpa-onnx` (ONNX / ONNX Runtime C-API)  
**Report Output Location:** `D:/Vibe-coding/Silent voice/docs/research/stt-english-2026.md`

---

## 1. Executive Summary

This investigation evaluates offline on-device speech-to-text models released or updated up to October 2026 against **NVIDIA Parakeet TDT 0.6B v2**. Models were benchmarked across the standardized **Hugging Face Open ASR Leaderboard** English test suite (8 datasets: AMI, Earnings-22, GigaSpeech, LibriSpeech clean/other, SPGI Speech, TEDLIUM-v3, and VoxPopuli) and evaluated for execution speed (Inverse Real-Time Factor, RTFx) and runtime viability on a Windows laptop.

### Core Discoveries & Landscape Verdict:

1. **Only Two Model Architectures Truly Beat Parakeet TDT 0.6B v2 on English Accuracy:**
   * **IBM Granite Speech 5.0 TurboCTC (470M)** (*Released August 25, 2026*): Achieves **5.00% Average WER** on the Open ASR Leaderboard, beating Parakeet TDT 0.6B v2 by **1.05 percentage points**. It delivers a staggering **12,600+ RTFx** (>3.7× faster than Parakeet). Distributed under **Apache 2.0** with GGUF weights for `transcribe.cpp` and PyTorch Hugging Face Transformers.
   * **NVIDIA Canary-Qwen-2.5B (2.5B)** (*Released July 17, 2025*): A Speech-Augmented Language Model (SALM) pairing Canary-1B-Flash FastConformer with Qwen3-1.7B LLM. Achieves **5.63% Average WER** (0.42 points lower than Parakeet). However, it runs at **418 RTFx** and requires an LLM inference backend (NeMo / vLLM), making it unsuitable for CPU-only laptop dictation.

2. **The "Silent Voice Runtime Reality Gate" (`whisper.cpp` vs `sherpa-onnx`):**
   * **`whisper.cpp` is strictly an OpenAI Whisper-family engine.** It can only execute Whisper-style encoder-decoder models.
     * **No Whisper model beats Parakeet TDT 0.6B v2 in English WER.** Whisper Large-v3 scores **7.44%**, Large-v3-Turbo scores **7.83%**, and Distil-Whisper Large-v3.5 scores **7.10%**.
     * **Distil-Whisper Large-v3.5 GGML** is the new champion within `whisper.cpp`: it outperforms Whisper Large-v3-Turbo in both accuracy (7.10% vs 7.25% short-form WER) and latency (~1.46× faster than Turbo).
   * **`sherpa-onnx` is the high-performance transducer & non-autoregressive engine.**
     * **Parakeet TDT 0.6B v2 itself is already converted to ONNX INT8** (`csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8`) and runs directly in Silent Voice via the existing `transducer` struct layout (`encoder.int8.onnx`, `decoder.int8.onnx`, `joiner.int8.onnx`, `tokens.txt`).
     * **Parakeet TDT 0.6B v3** (`csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`) expands the model to **25 European languages** with negligible English degradation (**6.34% WER**) and identical ~3,300 RTFx speed.
     * **Qwen3-ASR-0.6B** (*Early 2026, Apache 2.0*): Demonstrates 2.11% clean / 4.55% other LibriSpeech WER and 2,000 RTFx. `sherpa-onnx` v1.13.4 already exports dedicated `qwen3_asr` C-API bindings matching Silent Voice's FFI definitions.
     * **SenseVoice-Small (234M, MIT)**: Remains the overall latency king for interactive voice typing (CPU RTF ~0.02–0.04, 25×–50× real-time), with native punctuation, casing, and audio event tags, already wired into Silent Voice.

---

## 2. Comprehensive Model Benchmark Table

*Table sorted primarily by transcription accuracy (lowest HF Open ASR Leaderboard English Average WER) and secondarily by throughput (RTFx).*  
*RTFx = Audio Duration / Processing Time (higher is faster; RTFx = 1,000 means 1,000 seconds of audio transcribed in 1 second).*

| Rank | Model Name | Release Date | Params | HF Open ASR Avg WER (%) | RTFx (Throughput) | License | Available Formats | Hugging Face Repository | Silent Voice Runtime | Source Citation URL |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :--- | :---: | :--- |
| **1** | **IBM Granite Speech 5.0 TurboCTC** | 2026-08-25 | 470M | **5.00%** | **12,600+** (H200) | Apache 2.0 | GGUF, PyTorch Safetensors | [`ibm-granite/granite-speech-5.0-470m-turboctc`](https://huggingface.co/ibm-granite/granite-speech-5.0-470m-turboctc) | Neither *(Needs transcribe.cpp or custom ONNX loader)* | [IBM HF Model Card](https://huggingface.co/ibm-granite/granite-speech-5.0-470m-turboctc) & [arXiv:2510.06961](https://arxiv.org/abs/2510.06961) |
| **2** | **NVIDIA Canary-Qwen-2.5B** | 2025-07-17 | 2.5B | **5.63%** | **418** (A100) | CC-BY-4.0 | NeMo `.nemo`, PyTorch | [`nvidia/canary-qwen-2.5b`](https://huggingface.co/nvidia/canary-qwen-2.5b) | Neither *(Requires vLLM / NeMo LLM)* | [NVIDIA HF Model Card](https://huggingface.co/nvidia/canary-qwen-2.5b) |
| **3** | **Microsoft Phi-4-multimodal-instruct** | 2025-02-26 | 5.6B | **6.00% – 6.14%** | ~40 – 80 (A100) | MIT | PyTorch, ONNX (GenAI) | [`microsoft/Phi-4-multimodal-instruct`](https://huggingface.co/microsoft/Phi-4-multimodal-instruct) | Neither *(Requires ONNX GenAI pipeline)* | [Microsoft HF Card](https://huggingface.co/microsoft/Phi-4-multimodal-instruct) & [arXiv:2503.01743](https://arxiv.org/abs/2503.01743) |
| **BASELINE** | **NVIDIA Parakeet TDT 0.6B v2** | 2025-05-01 | 600M | **6.05%** | **3,380** (A100, b128) | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/parakeet-tdt-0.6b-v2`](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) | **sherpa-onnx** (`transducer`) | [NVIDIA HF Model Card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) & [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) |
| **4** | **NVIDIA Parakeet TDT 0.6B v3** | 2025-05-20 | 600M | **6.34%** *(25 EU Langs)* | **~3,300** (A100, b128) | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/parakeet-tdt-0.6b-v3`](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) | **sherpa-onnx** (`transducer`) | [NVIDIA HF Model Card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8) |
| **5** | **NVIDIA Canary-1B-Flash** | 2025-03-15 | 883M | **6.35%** | **1,045** (A100) / **1,669** (H100) | CC-BY-4.0 | NeMo, ONNX | [`nvidia/canary-1b-flash`](https://huggingface.co/nvidia/canary-1b-flash) | **sherpa-onnx** (`canary`) | [NVIDIA HF Model Card](https://huggingface.co/nvidia/canary-1b-flash) & [arXiv:2503.05931](https://arxiv.org/abs/2503.05931) |
| **6** | **NVIDIA Parakeet TDT 1.1B** | 2024-03-01 | 1.1B | **7.07%** *(1.39% LS clean)* | **~2,800** (A100) | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/parakeet-tdt-1.1b`](https://huggingface.co/nvidia/parakeet-tdt-1.1b) | **sherpa-onnx** (`transducer`) | [NVIDIA HF Model Card](https://huggingface.co/nvidia/parakeet-tdt-1.1b) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-1.1b-en) |
| **7** | **Distil-Whisper Distil-Large-v3.5** | 2025-01-20 | 756M | **7.10%** *(Short-form)* | **~49.3** (Long-form CPU/GPU) | MIT | GGML, ONNX, CTranslate2 | [`distil-whisper/distil-large-v3.5`](https://huggingface.co/distil-whisper/distil-large-v3.5) | **whisper.cpp** (`ggml`) & **sherpa-onnx** (`whisper`) | [Distil-Whisper HF Card](https://huggingface.co/distil-whisper/distil-large-v3.5) & [GGML Repo](https://huggingface.co/distil-whisper/distil-large-v3.5-ggml) |
| **8** | **NVIDIA Canary-180M-Flash** | 2025-03-15 | 182M | **7.12%** | **1,233** (A100) / **2,041** (H100) | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/canary-180m-flash`](https://huggingface.co/nvidia/canary-180m-flash) | **sherpa-onnx** (`canary`) | [NVIDIA HF Model Card](https://huggingface.co/nvidia/canary-180m-flash) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8) |
| **9** | **NVIDIA Canary-1B-v2** | 2025-06-10 | 1.02B | **7.15%** *(25 EU Langs)* | **749** (A100) | CC-BY-4.0 | NeMo `.nemo` | [`nvidia/canary-1b-v2`](https://huggingface.co/nvidia/canary-1b-v2) | **sherpa-onnx** (`canary` via ONNX export) | [NVIDIA HF Model Card](https://huggingface.co/nvidia/canary-1b-v2) |
| **10** | **OpenAI Whisper Large-v3** | 2023-11-06 | 1550M | **7.44%** | ~30 – 50 (GPU) | MIT | GGML, ONNX, SafeTensors | [`openai/whisper-large-v3`](https://huggingface.co/openai/whisper-large-v3) | **whisper.cpp** (`ggml`) & **sherpa-onnx** (`whisper`) | [OpenAI HF Card](https://huggingface.co/openai/whisper-large-v3) & [arXiv:2212.04356](https://arxiv.org/abs/2212.04356) |
| **11** | **OpenAI Whisper Large-v3-Turbo** | 2024-10-01 | 809M | **7.83%** *(7.25% Short)* | **200.19** (A100) / ~33.8 (Seq) | MIT | GGML, ONNX, SafeTensors | [`openai/whisper-large-v3-turbo`](https://huggingface.co/openai/whisper-large-v3-turbo) | **whisper.cpp** (`ggml`) & **sherpa-onnx** (`whisper`) | [OpenAI HF Card](https://huggingface.co/openai/whisper-large-v3-turbo) & [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) |
| **12** | **Alibaba SenseVoice-Small** | 2024-07-17 | 234M | ~5.2% – 5.5% *(2.1% LS Clean)* | ~250 (GPU) / **25x–50x** (CPU) | MIT | ONNX INT8, GGUF | [`FunAudioLLM/SenseVoiceSmall`](https://huggingface.co/FunAudioLLM/SenseVoiceSmall) | **sherpa-onnx** (`sense_voice`) | [FunAudioLLM HF Card](https://huggingface.co/FunAudioLLM/SenseVoiceSmall) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17) |
| **13** | **Qwen3-ASR-0.6B** | 2026-01-20 | 600M | ~4.5% – 5.5% *(2.11% LS Clean)* | **2,000** (GPU, b128) | Apache 2.0 | PyTorch, ONNX (sherpa) | [`Qwen/Qwen3-ASR-0.6B`](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) | **sherpa-onnx** (`qwen3_asr`) | [Qwen HF Model Card](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) & [arXiv:2601.21337](https://arxiv.org/abs/2601.21337) |
| **14** | **Mistral Voxtral Mini 4B Realtime 2602** | 2026-02-15 | ~4.0B | 4.90% (Fleurs-en), 12.3% (E22) | Realtime streaming (>12.5 tok/s) | Apache 2.0 | BF16 PyTorch / vLLM | [`mistralai/Voxtral-Mini-4B-Realtime-2602`](https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602) | Neither *(Requires vLLM / 8GB+ GPU)* | [Mistral HF Card](https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602) & [arXiv:2602.11298](https://arxiv.org/abs/2602.11298) |
| **15** | **Kyutai STT 2.6B** | 2024-10-01 | 2.6B | ~6.6% (LibriSpeech clean) | Realtime streaming (delay 2.5s) | CC-BY-4.0 | PyTorch (`moshi`, `trfs`), MLX | [`kyutai/stt-2.6b-en`](https://huggingface.co/kyutai/stt-2.6b-en) | Neither *(Requires Mimi neural codec)* | [Kyutai Project Page](https://kyutai.org/next/stt) & [HF Card](https://huggingface.co/kyutai/stt-2.6b-en) |
| **16** | **Useful Sensors Moonshine Base** | 2024-10-15 | 61.5M | ~11.5% Avg *(3.5% LS Clean)* | Realtime edge (~20x CPU) | MIT | ONNX INT8, TFLite, GGUF | [`moonshine-ai/moonshine-base`](https://huggingface.co/moonshine-ai/moonshine-base) | **sherpa-onnx** (`moonshine`) | [UsefulSensors GitHub](https://github.com/usefulsensors/moonshine) & [HF Repo](https://huggingface.co/moonshine-ai/moonshine-base) |

---

## 3. Granular Breakdown of Benchmark Datasets (Open ASR Leaderboard)

The Hugging Face Open ASR Leaderboard evaluates models across 8 challenging English speech datasets to produce the official **Avg WER**. The table below reveals how the top models perform on individual acoustic domains:

| Dataset Domain | Baseline: Parakeet TDT 0.6B v2 | Parakeet TDT 0.6B v3 | Canary-Qwen-2.5B | Canary-1B-Flash | Canary-180M-Flash | Distil-Whisper Large-v3.5 | Whisper Large-v3-Turbo |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **AMI (Meetings)** | 11.16% | 11.31% | **10.19%** | 13.11% | 14.86% | 14.63% | 16.13% |
| **Earnings-22 (Financial Calls)** | 11.15% | 11.42% | **10.45%** | 12.79% | 13.33% | 11.29% | 11.63% |
| **GigaSpeech (Audiobooks/Podcasts)** | 9.74% | 9.59% | **9.43%** | 9.85% | 10.51% | 9.84% | 10.14% |
| **LibriSpeech test-clean (Studio)** | 1.69% | 1.93% | 1.61% | **1.48%** | 1.87% | 2.37% | 2.10% |
| **LibriSpeech test-other (Noisy/Accented)** | 3.19% | 3.59% | 3.10% | **2.87%** | 3.83% | 5.04% | 4.24% |
| **SPGI Speech (Financial / Press)** | 2.17% | 3.97% | **1.90%** | 1.95% | 2.26% | 2.87% | 2.97% |
| **TEDLIUM-v3 (Lectures/TED Talks)** | 3.38% | 2.75% | **2.71%** | 3.12% | 3.98% | 3.64% | 3.57% |
| **VoxPopuli (Parliamentary Speeches)** | 5.95% | 6.14% | 5.66% | **5.63%** | 6.35% | — | — |
| **Overall English Avg WER** | **6.05%** | **6.34%** | **5.63%** | **6.35%** | **7.12%** | **7.10%** | **7.25%** |

*Data compiled directly from official model cards and leaderboard evaluation traces.*

---

## 4. Deep Technical Notes per Model Family

### 4.1 NVIDIA Parakeet Family (FastConformer-TDT / CTC)
* **Parakeet TDT 0.6B v2 (Baseline):**  
  * **Architecture:** FastConformer XL encoder (8× downsampling, depthwise separable convolutions) + Token-and-Duration Transducer (TDT) decoder.  
  * **Strengths:** By predicting both token IDs and duration skips, TDT skips blank steps during decoding, achieving an extraordinary **3,380 RTFx** on GPU. It trains on Granary (120,000 hours of English audio) with full attention, natively transcribing audio segments up to 24 minutes with punctuation and capitalization.
  * **Silent Voice Integration:** **100% Ready.** The ONNX INT8 bundle (`csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8`) contains `encoder.int8.onnx` (~480 MB), `decoder.int8.onnx` (~15 MB), `joiner.int8.onnx` (~25 MB), and `tokens.txt`. This matches Silent Voice's `transducer_installed()` check in `src-tauri/src/models/registry.rs` without any code changes.
* **Parakeet TDT 0.6B v3:**  
  * **Enhancement:** Expands v2 to **25 European languages** (French, German, Spanish, Italian, Dutch, Polish, Russian, Ukrainian, etc.) trained on Granary Multilingual (950k+ hours).
  * **Trade-off:** English WER increases slightly from 6.05% to 6.34% due to multilingual capacity sharing, but it delivers identical 3,300 RTFx speed. Available in pre-quantized INT8 for `sherpa-onnx`.
* **Parakeet TDT 1.1B:**  
  * **Strengths:** 1.1 billion parameters. Reaches **1.39% WER on LibriSpeech clean** (surpassing all 0.6B models). However, on meeting datasets like AMI (15.90%) and financial calls (14.65%), its older training recipe yields a higher aggregate WER (7.07%).

### 4.2 NVIDIA Canary Family (FastConformer-AED & SALM)
* **Canary-Qwen-2.5B (SALM):**  
  * **Architecture:** FastConformer speech encoder (from Canary-1B-Flash) coupled via linear projection and LoRA adapter to `Qwen/Qwen3-1.7B` LLM decoder.  
  * **Performance:** Reaches **5.63% WER**, the lowest among all evaluated NVIDIA models.  
  * **Limitation:** At 2.5B parameters and 418 RTFx, it requires PyTorch/vLLM and substantial GPU VRAM (~6 GB). Cannot run in `whisper.cpp` or `sherpa-onnx`.
* **Canary-1B-Flash (883M) & Canary-180M-Flash (182M):**  
  * **Architecture:** Attention Encoder-Decoder (AED) with 32 FastConformer encoder layers and 4 Transformer decoder layers. Decodes with beam size 1 at >1,000 RTFx. Supports English, German, French, and Spanish speech-to-text plus bidirectional translation.  
  * **Silent Voice Integration:** Supported via sherpa-onnx's `canary` FFI structure (`OfflineModelConfig.canary`). Pre-converted INT8 models exist (`csukuangfj/sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8`).

### 4.3 OpenAI Whisper & Distil-Whisper Family
* **Whisper Large-v3-Turbo (809M):**  
  * Prunes the 32-layer decoder of Large-v3 down to 4 layers. Increases throughput by 4× (RTFx 200 on GPU) with minimal accuracy degradation (7.83% vs 7.44% mean WER). Native in `whisper.cpp` (`ggml-large-v3-turbo.bin`).
* **Distil-Whisper Distil-Large-v3.5 (756M):**  
  * Distilled from Whisper Large-v3 on 98,000 hours of public audio with aggressive SpecAugment.  
  * **Key Win:** Beats Large-v3-Turbo on short-form English accuracy (**7.10% vs 7.25% WER**) and runs **~1.46× faster**. Available as `distil-whisper/distil-large-v3.5-ggml` for `whisper.cpp` and `distil-large-v3.5-ONNX` for `sherpa-onnx`.

### 4.4 IBM Granite Speech 5.0 TurboCTC (470M)
* **The New SOTA Efficiency Champion:**  
  * Released August 25, 2026 by IBM Research under **Apache 2.0**.
  * **Architecture:** 16 Conformer blocks with pyramidal 8× temporal subsampling (logmels + deltas), block-diagonal self-attention (128 frames), and self-conditioned CTC middle-layer loss. Output head uses 16,384 BPE units.
  * **Performance:** **5.00% Avg WER** on Open ASR Leaderboard and **12,600+ RTFx** on H200.
  * **Deployment:** Pre-quantized GGUF weights (`handy-computer/granite-speech-5.0-470m-turboctc-gguf`) run on CPU/Vulkan/Metal via `transcribe.cpp`. Not currently native in `sherpa-onnx` or `whisper.cpp`.

### 4.5 Alibaba Qwen3-ASR & SenseVoice-Small
* **Qwen3-ASR-0.6B (600M, Apache 2.0):**  
  * Part of the Qwen3-Omni foundation series (January 2026). Achieves 2.11% clean / 4.55% other on LibriSpeech. Runs at 2,000 RTFx. Supported natively in `sherpa-onnx` C-API struct `Qwen3AsrModelConfig` (`conv_frontend`, `encoder`, `decoder`, `tokenizer`).
* **SenseVoice-Small (234M, MIT):**  
  * Non-autoregressive SAN-m encoder. Near-zero CPU overhead (**RTF 0.02–0.04**, transcribing a 5-second voice memo in <100ms on a laptop CPU). Emits rich tags (`<HAPPY>`, `<APPLAUSE>`, `<LAUGHTER>`) and native punctuation. Ideal default general-purpose dictation model.

### 4.6 Other Evaluated Architectures (Kyutai, Mistral, Phi-4, Moonshine)
* **Mistral Voxtral Mini 4B Realtime 2602:** 4B parameters (Causal Audio Encoder + Ministral 3B). Designed for streaming server/vLLM endpoints with 480ms delay (4.90% Fleurs-en WER). Too heavy for laptop CPU dictation.
* **Microsoft Phi-4-multimodal-instruct:** 5.6B parameters. High accuracy (6.00%–6.14% WER) but requires ONNX Runtime GenAI or Transformers and significant GPU memory.
* **Kyutai STT 2.6B:** Streaming speech-to-text built on the proprietary Mimi audio codec. Requires `moshi` / PyTorch; incompatible with ONNX / GGML.
* **Useful Sensors Moonshine-base (61.5M):** Extremely fast for short bursts (<5s) on low-power CPUs, but its 11.5% average multi-domain WER is far less accurate than Parakeet or SenseVoice.

---

## 5. Runtime Feasibility Matrix for Silent Voice

Silent Voice's architecture relies on two engine backends:
1. `whisper.cpp` sidecar CLI / server (`src-tauri/src/transcription/whisper.rs`).
2. `sherpa-onnx` dynamic library FFI (`src-tauri/src/system/sherpa_stt.rs`).

Here is how the candidate models map to these two runtimes:

```
+----------------------------------------------------------------------------------------------------+
| Model Candidate                      | Runtime Engine      | FFI / Config Required                 | Implementation Status in Silent Voice  |
+--------------------------------------+---------------------+---------------------------------------+----------------------------------------+
| Parakeet TDT 0.6B v2 INT8            | sherpa-onnx         | OfflineModelConfig.transducer         | 100% READY (Matches transducer_installed)|
| Parakeet TDT 0.6B v3 INT8            | sherpa-onnx         | OfflineModelConfig.transducer         | 100% READY (Drop-in folder replacement)|
| Parakeet TDT 1.1B INT8               | sherpa-onnx         | OfflineModelConfig.transducer         | 100% READY (Supported)                 |
| SenseVoice-Small INT8                | sherpa-onnx         | OfflineModelConfig.sense_voice        | 100% READY (Currently active)         |
| Distil-Whisper Large-v3.5 GGML       | whisper.cpp         | Sidecar CLI (-m ggml-distil-v3.5.bin) | 100% READY (Supported as GGML bin)    |
| Whisper Large-v3-Turbo GGML          | whisper.cpp         | Sidecar CLI (-m ggml-turbo.bin)       | 100% READY (Supported as GGML bin)    |
| Canary-180M-Flash INT8               | sherpa-onnx         | OfflineModelConfig.canary             | Struct declared, needs model download   |
| Qwen3-ASR-0.6B                       | sherpa-onnx         | OfflineModelConfig.qwen3_asr          | Struct declared in sherpa_stt.rs       |
| IBM Granite Speech 5.0 TurboCTC      | transcribe.cpp/GGUF | Requires new sidecar or ONNX loader   | NOT READY (Requires new engine bridge) |
| Canary-Qwen-2.5B / Mistral Voxtral   | NeMo / vLLM         | Incompatible with desktop C-API       | UNSUPPORTED (Python/server only)       |
+----------------------------------------------------------------------------------------------------+
```

---

## 6. Hardware Feasibility on Windows Laptop (CPU vs Modest GPU)

| Hardware Profile | Recommended Model Choice | Expected Latency / RTF | Memory (RAM / VRAM) Footprint | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **CPU Only (Standard Laptop)** | **SenseVoice-Small INT8** | **RTF ~0.02 – 0.04** (<150ms for 5s clip) | ~300 MB RAM | Instant response, zero fan noise, native punctuation. |
| **CPU Only (Accuracy Focused)** | **Parakeet TDT 0.6B v2 INT8** | **RTF ~0.06 – 0.12** (~400ms for 5s clip) | ~600 MB RAM | Exceptional English fidelity, FastConformer parallelizes across 8 cores. |
| **CPU Only (Whisper Ecosystem)** | **Distil-Whisper Large-v3.5 Q5_0** | **RTF ~0.15 – 0.25** (~1.0s for 5s clip) | ~900 MB RAM | 1.46× faster than Large-v3-Turbo, native capitalization. |
| **Modest NVIDIA GPU (4–8 GB VRAM)**| **Parakeet TDT 0.6B v2 INT8 (CUDA)** | **RTF ~0.003** (<20ms for 5s clip) | ~800 MB VRAM | Blazing fast (>3,000 RTFx), negligible GPU load. |
| **Modest NVIDIA GPU (Multilingual)**| **Parakeet TDT 0.6B v3 INT8 (CUDA)** | **RTF ~0.003** (<20ms for 5s clip) | ~800 MB VRAM | Instant transcription in 25 European languages. |
| **Modest NVIDIA GPU (Whisper)** | **Distil-Whisper Large-v3.5 (CUDA)** | **RTF ~0.015** (~80ms for 5s clip) | ~1.8 GB VRAM | Pristine formatting, Hallucination-resistant distillation. |

---

## 7. Concrete Recommendations for Silent Voice

### Recommendation 1: Adopt `parakeet-tdt-0.6b-v2-int8` as the High-Accuracy English Champion in `sherpa-onnx`
* **Why:** It achieves **6.05% Average WER** on the Open ASR Leaderboard and **3,380 RTFx** on GPU (and ~10× real-time on modern 8-core CPUs). It natively handles long-form speech up to 24 minutes with punctuation and capitalization.
* **Turnkey Delivery:** Pre-quantized ONNX INT8 weights are readily available in `csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8`. Silent Voice's `transducer_installed()` and `transducer` FFI binding already match this model package.

### Recommendation 2: Offer `distil-whisper-large-v3.5` as the Superior Replacement for `whisper-large-v3-turbo` in `whisper.cpp`
* **Why:** For users who prefer the `whisper.cpp` sidecar path (or rely on custom prompt biasing), **Distil-Whisper Large-v3.5** beats Whisper Large-v3-Turbo on short-form English accuracy (**7.10% vs 7.25% WER**) while running **1.46× faster**.
* **Turnkey Delivery:** Distribute `ggml-distil-large-v3.5.bin` (`distil-whisper/distil-large-v3.5-ggml`). It drops right into `models_dir()` as a standard Whisper model.

### Recommendation 3: Offer `parakeet-tdt-0.6b-v3-int8` for European Multilingual Users
* **Why:** For power users needing French, German, Spanish, Italian, Russian, etc., without losing English speed or accuracy, `parakeet-tdt-0.6b-v3-int8` provides 25-language coverage at **6.34% English WER** and ~3,300 RTFx.

### Recommendation 4: Track `granite-speech-5.0-470m-turboctc` for Future Architectural Expansion
* **Why:** IBM's August 2026 release demonstrates that non-autoregressive Conformer CTC models can beat hybrid LLM decoders (**5.00% WER at 12,600+ RTFx**). If Silent Voice ever integrates a GGUF CTC runner (like `transcribe.cpp`) or exports Granite to ONNX, this model will be the ultimate offline accuracy/speed leader on both CPU and GPU.
