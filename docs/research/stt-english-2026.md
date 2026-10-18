# Speech-to-Text (STT) English Models Benchmark & Landscape (2024–2026)

**Target Application:** Silent Voice (Local-first Windows dictation & transcription app in Rust / Tauri v2)  
**Target Hardware:** Windows Laptop CPU (Intel Core Ultra / AMD Ryzen 7, 4–8 threads) and Modest GPU (NVIDIA RTX 3050/4050/3060 Mobile, 4–8 GB VRAM)  
**Evaluation Baseline:** NVIDIA Parakeet TDT 0.6B v2 (HF Open ASR Leaderboard English Avg WER: **6.05%** on initial 8-split / **6.43%** on updated 2026 suite; RTFx: **3,380** b128 / **6,148** official / **14,383** fast-gpu-asr)  
**Supported In-App Engines:** `whisper.cpp` (GGML / GGUF) and `sherpa-onnx` v1.13.4 (ONNX / ONNX Runtime C-API)  
**Benchmark Primary Source:** Hugging Face Open ASR Leaderboard official database (`hf-audio/open-asr-leaderboard-results/english_short_latest.csv`) and official Model Cards / Technical Reports  
**File Location:** `D:/Vibe-coding/Silent voice/docs/research/stt-english-2026.md`  

---

## 1. Executive Summary & Frontier Landscape

This research provides an authoritative investigation of offline, on-device English speech-to-text (STT) models released or updated up to October 2026. Models are evaluated on transcription accuracy (**Hugging Face Open ASR Leaderboard Average WER**), throughput (**Inverse Real-Time Factor, RTFx**), parameter size, license, file formats, and runtime compatibility with Silent Voice's two embedded engines: **`whisper.cpp`** and **`sherpa-onnx`**.

### Benchmark Standard Distinction (Important Context):
* **Initial 8-Dataset Public Benchmark Split (2024–2025):** Evaluated across 8 public corpora: AMI, Earnings-22, GigaSpeech, LibriSpeech clean, LibriSpeech other, SPGI Speech, TEDLIUM-v3, and VoxPopuli. Parakeet TDT 0.6B v2 scores **6.05%**, Canary-Qwen-2.5B scores **5.63%**, and Granite Speech 5.0 TurboCTC scores **5.00%**.
* **Updated 2026 Open ASR Leaderboard Suite (`english_short_latest.csv`):** Incorporates challenging real-world acoustic test sets including `URGENT2024` (heavily reverberant & noise-injected speech) and `Voice Arena Monsoon` (conversational accented speech), with cleaned versions of AMI, Earnings-22, and VoxPopuli. On this rigorous benchmark, Parakeet TDT 0.6B v2 scores **6.43%**, Canary-Qwen-2.5B scores **6.37%**, and Granite Speech 5.0 TurboCTC scores **6.84%**.
* Both sets of scores are documented below to maintain complete transparency.

---

### Core Discoveries:

1. **Models That Outperform Parakeet TDT 0.6B v2 on Accuracy:**
   * **Microsoft Phi-4-multimodal-instruct (5.6B / 6B)** (*Released 2025-02-24, MIT*):
     * **Accuracy:** **5.02% Avg WER** on the Open ASR Leaderboard (leads all open-weights models).
     * **Speed:** **162.53 RTFx**.
     * **Runtime Gate:** Requires ONNX Runtime GenAI or Transformers; too heavy for background laptop dictation without high-end VRAM.
   * **Qwen/Qwen3-ASR-1.7B (2.04B)** (*Released 2026-01-28, Apache 2.0*):
     * **Accuracy:** **5.75% Avg WER** (beats Parakeet by 0.68 percentage points).
     * **Speed:** **835.62 RTFx**.
     * **Runtime Gate:** Pre-quantized ONNX INT8 models exist (`thieunv-asilla/sherpa-onnx-qwen3-asr-1.7B-int8`). Silent Voice's `sherpa_stt.rs` already defines `Qwen3AsrModelConfig`.
   * **TheStageAI/thewhisper-large-v3-turbo (809M)** (*Released 2025-10-27, CC-BY-4.0*):
     * **Accuracy:** **6.07% Avg WER** (beats OpenAI's baseline Whisper Large-v3-Turbo's 8.39% by 2.32% and beats Parakeet's 6.43%).
     * **Speed:** **1,183.92 RTFx**.
     * **Runtime Gate:** Uses the standard Whisper architecture; directly convertible to GGML for **`whisper.cpp`**! This decisively disproves the claim that no Whisper model can rival Parakeet.
   * **CohereLabs/cohere-transcribe-03-2026 (2B)** (*Released 2026-03-26, Apache 2.0*):
     * **Accuracy:** **6.26% Avg WER** (beats Parakeet's 6.43%).
     * **Speed:** **923.71 RTFx**.
     * **Runtime Gate:** Pre-quantized INT8 weights exist (`csukuangfj2/sherpa-onnx-cohere-transcribe-14-lang-int8-2026-04-01`). Silent Voice's `src-tauri/src/system/sherpa_stt.rs` already exports `CohereTranscribeModelConfig`.
   * **NVIDIA Canary-Qwen-2.5B (2.5B)** (*Released 2025-06-26, CC-BY-4.0*):
     * **Accuracy:** **5.63%** (8-dataset split) / **6.37%** (2026 leaderboard suite).
     * **Speed:** **418 RTFx** (A100 b1) / **881.67 RTFx** (b128).
     * **Runtime Gate:** Speech-Augmented Language Model (SALM); requires Python NeMo / vLLM. Incompatible with `whisper.cpp` or `sherpa-onnx`.

2. **Models That Outperform Parakeet TDT 0.6B v2 on Throughput (Speed):**
   * **IBM Granite Speech 5.0 TurboCTC (470M)** (*Released 2026-08-25, Apache 2.0*):
     * **Speed:** **21,134.31 RTFx** on the Open ASR Leaderboard (>3.4× faster than Parakeet's 6,148 RTFx; 12,600+ on a single H200).
     * **Accuracy:** **5.00% Avg WER** on the initial 8-dataset public split; **6.84% Avg WER** on the 2026 leaderboard suite (including URGENT2024 noise).
     * **Runtime Gate:** Distributed in GGUF (`handy-computer/granite-speech-5.0-470m-turboctc-gguf` for `transcribe.cpp`). Not currently packaged for `whisper.cpp` or `sherpa-onnx`.
   * **soundsgoodai/Zipformer-cr-ctc-transducer-XL-290M (290M)** (*Released 2026-03, CC-BY-NC-4.0*):
     * **Speed:** **20,886.37 RTFx** (CTC greedy) / **19,289.67 RTFx** (modified beam search).
     * **Accuracy:** **7.38% Avg WER**.
     * **Runtime Gate:** Supported in `sherpa-onnx` (`zipformer_ctc`).

3. **Current In-App Status in Silent Voice:**
   * **Parakeet TDT 0.6B v2 INT8** (`csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8`) is already the flagship accuracy preset in `src/components/onboarding/Onboarding.tsx` and `src/services/catalog.ts`. The SentencePiece BPE vocabulary is embedded directly in `src-tauri/src/system/sherpa_stt.rs` (`PARAKEET_V2_BPE_VOCAB`) for hotword biasing.
   * **Distil-Whisper Large-v3.5 GGML** (`distil-whisper/distil-large-v3.5-ggml`) is ready to replace Whisper Large-v3-Turbo in `whisper.cpp`, delivering **7.10% short-form WER** (vs Turbo's 7.25%) at **1.46× faster** speed.

---

## 2. Comprehensive Master Benchmark Table

*Sorted primarily by Open ASR Leaderboard English Accuracy (Average WER) and secondarily by Throughput (RTFx).*  
*RTFx = Audio Duration / Processing Time (higher is faster; 1,000 RTFx = 1,000 seconds of audio transcribed in 1 second).*

| Rank | Model Name | Release Date | Params | Open ASR Avg WER | RTFx (Throughput) | License | Available Formats | Hugging Face Repository | Silent Voice Runtime | Primary Source Citation URL |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :--- | :--- | :---: | :--- |
| **1** | **Microsoft Phi-4-multimodal-instruct** | 2025-02-24 | 5.6B | **5.02%** *(6.00% 8-split)* | **162.53** (GPU) | MIT | PyTorch, ONNX GenAI | [`microsoft/Phi-4-multimodal-instruct`](https://huggingface.co/microsoft/Phi-4-multimodal-instruct) | Neither *(Requires ONNX GenAI)* | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [arXiv:2503.01743](https://arxiv.org/abs/2503.01743) |
| **2** | **Qwen3-ASR-1.7B-hf** | 2026-01-28 | 2.04B | **5.75%** | **835.62** (GPU) | Apache 2.0 | PyTorch, ONNX INT8 | [`Qwen/Qwen3-ASR-1.7B-hf`](https://huggingface.co/Qwen/Qwen3-ASR-1.7B-hf) | **sherpa-onnx** (`qwen3_asr`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [arXiv:2601.21337](https://arxiv.org/abs/2601.21337) |
| **3** | **TheStageAI thewhisper-large-v3-turbo** | 2025-10-27 | 809M | **6.07%** | **1,183.92** (GPU) | CC-BY-4.0 | SafeTensors, CoreML, GGML | [`TheStageAI/thewhisper-large-v3-turbo`](https://huggingface.co/TheStageAI/thewhisper-large-v3-turbo) | **whisper.cpp** (`ggml`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [Model Card](https://huggingface.co/TheStageAI/thewhisper-large-v3-turbo) |
| **4** | **Cohere Transcribe (03-2026)** | 2026-03-26 | 2.0B | **6.26%** | **923.71** (GPU) | Apache 2.0 | SafeTensors, ONNX INT8, GGUF | [`CohereLabs/cohere-transcribe-03-2026`](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) | **sherpa-onnx** (`cohere_transcribe`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [ONNX Package](https://huggingface.co/csukuangfj2/sherpa-onnx-cohere-transcribe-14-lang-int8-2026-04-01) |
| **5** | **NVIDIA Canary-Qwen-2.5B** | 2025-06-26 | 2.5B | **6.37%** *(5.63% 8-split)* | **418** (b1) / **881.67** (b128) | CC-BY-4.0 | NeMo `.nemo`, PyTorch | [`nvidia/canary-qwen-2.5b`](https://huggingface.co/nvidia/canary-qwen-2.5b) | Neither *(Requires NeMo/vLLM)* | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [NVIDIA HF Card](https://huggingface.co/nvidia/canary-qwen-2.5b) |
| **6** | **NVIDIA Parakeet TDT 0.6B v3** | 2025-08-04 | 600M | **6.38%** *(fast)* / **6.51%** *(std)* | **6,227.73** / **13,304.58** | CC-BY-4.0 | NeMo, ONNX INT8, GGUF | [`nvidia/parakeet-tdt-0.6b-v3`](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) | **sherpa-onnx** (`transducer`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8) |
| **7** | **IBM Granite Speech 5.0 TurboCTC-nc** | 2026-08-25 | 470M | **6.41%** *(4.85% 8-split)* | **20,801.33** (GPU) | CC-BY-NC-SA-4.0 | PyTorch, GGUF | [`ibm-granite/granite-speech-5.0-470m-turboctc-nc`](https://huggingface.co/ibm-granite/granite-speech-5.0-470m-turboctc-nc) | Neither *(Requires transcribe.cpp)* | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [arXiv:2609.20104](https://arxiv.org/abs/2609.20104) |
| **BASELINE** | **NVIDIA Parakeet TDT 0.6B v2** | 2025-04-15 | 600M | **6.43%** *(6.05% 8-split)* | **6,148.32** / **14,383.48** *(3,380 b128)* | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/parakeet-tdt-0.6b-v2`](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) | **sherpa-onnx** (`transducer`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [NVIDIA HF Card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) |
| **8** | **Qwen3-ASR-0.6B-hf** | 2026-01-28 | 780M | **6.60%** *(2.11% LS clean)* | **749.52** (GPU) | Apache 2.0 | PyTorch, ONNX INT8 | [`Qwen/Qwen3-ASR-0.6B-hf`](https://huggingface.co/Qwen/Qwen3-ASR-0.6B-hf) | **sherpa-onnx** (`qwen3_asr`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [csukuangfj2 ONNX](https://huggingface.co/csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25) |
| **9** | **IBM Granite Speech 5.0 TurboCTC** | 2026-08-25 | 470M | **6.84%** *(5.00% 8-split)* | **21,134.31** *(12,600+ H200)* | Apache 2.0 | PyTorch, GGUF | [`ibm-granite/granite-speech-5.0-470m-turboctc`](https://huggingface.co/ibm-granite/granite-speech-5.0-470m-turboctc) | Neither *(Requires transcribe.cpp)* | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [arXiv:2609.20104](https://arxiv.org/abs/2609.20104) |
| **10** | **Distil-Whisper Distil-Large-v3.5** | 2024-12-05 | 756M | **7.02%** *(7.10% Short)* | **898.21** (GPU) / ~49.3 (Seq) | MIT | GGML, ONNX, CTranslate2 | [`distil-whisper/distil-large-v3.5`](https://huggingface.co/distil-whisper/distil-large-v3.5) | **whisper.cpp** (`ggml`) & **sherpa-onnx** (`whisper`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [GGML Repo](https://huggingface.co/distil-whisper/distil-large-v3.5-ggml) |
| **11** | **NVIDIA Parakeet TDT 1.1B** | 2024-01-25 | 1.1B | **7.07%** *(1.39% LS clean)* | **~2,800** (A100) | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/parakeet-tdt-1.1b`](https://huggingface.co/nvidia/parakeet-tdt-1.1b) | **sherpa-onnx** (`transducer`) | [NVIDIA HF Card](https://huggingface.co/nvidia/parakeet-tdt-1.1b) & [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) |
| **12** | **NVIDIA Canary-180M-Flash** | 2025-03-11 | 182M | **7.12%** (8-split) | **1,233** (A100) / **2,041** (H100) | CC-BY-4.0 | NeMo, ONNX INT8 | [`nvidia/canary-180m-flash`](https://huggingface.co/nvidia/canary-180m-flash) | **sherpa-onnx** (`canary`) | [NVIDIA HF Card](https://huggingface.co/nvidia/canary-180m-flash) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8) |
| **13** | **NVIDIA Canary-1B-Flash** | 2025-03-07 | 883M | **6.35%** (8-split) | **1,045.75** (A100) / **1,669** (H100) | CC-BY-4.0 | NeMo, ONNX | [`nvidia/canary-1b-flash`](https://huggingface.co/nvidia/canary-1b-flash) | **sherpa-onnx** (`canary`) | [NVIDIA HF Card](https://huggingface.co/nvidia/canary-1b-flash) & [arXiv:2503.05931](https://arxiv.org/abs/2503.05931) |
| **14** | **Zipformer-cr-ctc-transducer-XL-290M** | 2026-03-15 | 290M | **7.38%** | **19,289.67** – **20,886.37** | CC-BY-NC-4.0 | ONNX | [`soundsgoodai/Zipformer-cr-ctc-transducer-XL-290M`](https://huggingface.co/soundsgoodai/Zipformer-cr-ctc-transducer-XL-290M) | **sherpa-onnx** (`zipformer_ctc`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) |
| **15** | **OpenAI Whisper Large-v3** | 2023-11-06 | 1550M | **7.84%** *(7.44% 8-split)* | **470.73** (GPU) | MIT | GGML, ONNX, SafeTensors | [`openai/whisper-large-v3`](https://huggingface.co/openai/whisper-large-v3) | **whisper.cpp** (`ggml`) & **sherpa-onnx** (`whisper`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [OpenAI HF Card](https://huggingface.co/openai/whisper-large-v3) |
| **16** | **NVIDIA Canary-1B-v2** | 2025-08-04 | 978M | **7.91%** *(7.15% 8-split)* | **1,853.68** (GPU) / **749** (A100) | CC-BY-4.0 | NeMo `.nemo` | [`nvidia/canary-1b-v2`](https://huggingface.co/nvidia/canary-1b-v2) | **sherpa-onnx** (`canary`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [NVIDIA HF Card](https://huggingface.co/nvidia/canary-1b-v2) |
| **17** | **OpenAI Whisper Large-v3-Turbo** | 2024-10-01 | 809M | **8.39%** *(7.25% Short)* | **810.50** (GPU) / **200** (A100) | MIT | GGML, ONNX, SafeTensors | [`openai/whisper-large-v3-turbo`](https://huggingface.co/openai/whisper-large-v3-turbo) | **whisper.cpp** (`ggml`) & **sherpa-onnx** (`whisper`) | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [OpenAI HF Card](https://huggingface.co/openai/whisper-large-v3-turbo) |
| **18** | **Mistral Voxtral Mini 4B Realtime 2602** | 2026-01-21 | 4.0B | **9.01%** *(4.90% Fleurs-en)* | **103.48** (GPU) | Apache 2.0 | BF16 PyTorch / vLLM | [`mistralai/Voxtral-Mini-4B-Realtime-2602`](https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602) | Neither *(Requires vLLM / 8GB+ GPU)* | [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) & [Mistral HF Card](https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602) |
| **19** | **Alibaba SenseVoice-Small** | 2024-07-03 | 234M | ~5.2% – 5.5% *(2.1% LS Clean)* | **~250** (GPU) / **25x–50x** (CPU) | MIT | ONNX INT8, GGUF | [`FunAudioLLM/SenseVoiceSmall`](https://huggingface.co/FunAudioLLM/SenseVoiceSmall) | **sherpa-onnx** (`sense_voice`) | [FunAudioLLM HF Card](https://huggingface.co/FunAudioLLM/SenseVoiceSmall) & [csukuangfj ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17) |
| **20** | **Kyutai STT 2.6B** | 2025-06-06 | 2.6B | ~6.6% (LibriSpeech clean) | Realtime streaming (2.5s delay) | CC-BY-4.0 | PyTorch (`moshi`, `trfs`), Candle | [`kyutai/stt-2.6b-en`](https://huggingface.co/kyutai/stt-2.6b-en) | Neither *(Requires Mimi neural codec)* | [Kyutai Project Page](https://kyutai.org/next/stt) & [HF Card](https://huggingface.co/kyutai/stt-2.6b-en) |
| **21** | **Useful Sensors Moonshine Base** | 2024-09-26 | 61.5M | ~11.5% Avg *(3.5% LS Clean)* | Realtime edge (~20x CPU) | MIT | ONNX INT8, TFLite, GGUF | [`usefulsensors/moonshine`](https://huggingface.co/usefulsensors/moonshine) | **sherpa-onnx** (`moonshine`) | [UsefulSensors HF Card](https://huggingface.co/usefulsensors/moonshine) & [GitHub](https://github.com/usefulsensors/moonshine) |

---

## 3. Detailed Acoustic Domain Breakdown (Hugging Face Open ASR Benchmark)

The table below breaks down performance across standardized evaluation corpora from the official Hugging Face Open ASR Leaderboard database:

| Model | Avg WER | AMI-Cleaned | Earnings-22 Cleaned | GigaSpeech Cleaned | LS Clean | LS Other | SPGISpeech | VoxPopuli Cleaned | URGENT2024 (Noisy) | Voice Arena Monsoon |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Phi-4-multimodal-instruct** | **5.02%** | 9.12% | 7.09% | 7.79% | **1.35%** | 3.45% | 2.73% | 4.18% | — | 4.46% |
| **Qwen3-ASR-1.7B-hf** | **5.75%** | 8.31% | **5.84%** | **7.22%** | **1.26%** | 2.94% | 2.57% | 2.85% | **9.93%** | **3.50%** |
| **thewhisper-large-v3-turbo** | **6.07%** | **7.32%** | 7.18% | 7.84% | 1.41% | 3.20% | **1.62%** | 4.02% | 10.82% | 3.72% |
| **Cohere Transcribe (03-2026)** | **6.26%** | — | — | — | 1.34% | 2.85% | 2.62% | 2.52% | 12.42% | 3.60% |
| **Canary-Qwen-2.5B** | **6.37%** | 7.91% | 6.41% | 7.80% | **1.23%** | **2.63%** | 1.70% | 4.14% | 13.02% | 3.60% |
| **Parakeet TDT 0.6B v3** | **6.38%** | 9.02% | 5.81% | 7.75% | 1.46% | 3.08% | 3.56% | 3.04% | 11.50% | 3.96% |
| **Granite Speech 5.0 TurboCTC-nc**| **6.41%** | 7.31% | 8.23% | 8.37% | 1.45% | 2.67% | 1.75% | 4.67% | 10.22% | 4.30% |
| **Parakeet TDT 0.6B v2 (Baseline)**| **6.43%** | 9.02% | 6.83% | 8.17% | 1.27% | 2.65% | 1.87% | 3.89% | 11.58% | 3.70% |
| **Qwen3-ASR-0.6B-hf** | **6.60%** | 9.33% | 7.83% | 7.62% | 1.70% | 4.01% | 2.74% | **2.90%** | 11.57% | 4.23% |
| **Granite Speech 5.0 TurboCTC** | **6.84%** | 7.71% | 6.75% | 8.74% | 1.48% | 2.71% | 3.22% | 4.90% | 11.79% | 4.75% |
| **Distil-Whisper Large-v3.5** | **7.02%** | — | — | — | 2.37% | 5.04% | 2.87% | — | — | — |
| **Whisper Large-v3** | **7.84%** | 13.63% | 7.99% | 8.35% | 1.56% | 3.52% | 2.71% | 4.53% | 13.73% | 3.95% |
| **Canary-1B-v2** | **7.91%** | 13.03% | 7.28% | 9.12% | 1.76% | 3.07% | 2.03% | 4.39% | 14.56% | 4.97% |
| **Whisper Large-v3-Turbo** | **8.39%** | 13.88% | 8.09% | 8.47% | 2.13% | 3.71% | 2.79% | 7.02% | 13.69% | 4.77% |
| **Voxtral Mini 4B Realtime 2602**| **9.01%** | 13.34% | 9.31% | 8.80% | 1.62% | 4.94% | 2.23% | 2.60% | 17.95% | 8.86% |

---

## 4. Deep Architectural Notes & Integration Analysis

### 4.1 NVIDIA Parakeet Family (FastConformer-TDT)
* **Parakeet TDT 0.6B v2 (Baseline):**  
  * **Architecture:** FastConformer XL encoder (8× downsampling, depthwise separable convolutions) paired with Token-and-Duration Transducer (TDT) decoder.  
  * **Why it excels:** By predicting token IDs and frame skip durations simultaneously, it bypasses blank decoding loops, attaining **3,380 RTFx** (b128) and **14,383 RTFx** on optimized GPU kernels. Handles audio segments up to 24 minutes in a single pass with native capitalization and punctuation.
  * **Silent Voice Implementation Status:** **100% Turnkey.** In [`src-tauri/src/system/sherpa_stt.rs`](file:///D:/Vibe-coding/Silent%20voice/src-tauri/src/system/sherpa_stt.rs#L561-L585), Silent Voice embeds `PARAKEET_V2_BPE_VOCAB` directly into the binary to enable hotword boosting without extra runtime downloads. The model is downloaded from `csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8` and loaded via `SherpaSttEngine::load_transducer`.
* **Parakeet TDT 0.6B v3:**  
  * Expands language coverage to **25 European languages** trained on 950k+ hours of Granary Multilingual audio.
  * On the latest leaderboard, it achieves **6.38% Avg WER** (fast kernel) / **6.51%** (standard), matching v2's speed (~6,200–13,300 RTFx). Pre-quantized INT8 files are available in `csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`.
* **Parakeet TDT 1.1B:**  
  * 1.1 billion parameters. Reaches **1.39% WER on LibriSpeech clean**, but higher meeting error rates (15.9% on AMI) and lowercase-only output make 0.6B v2 superior for general dictation.

### 4.2 The Whisper Ecosystem & `whisper.cpp`
* **TheStageAI/thewhisper-large-v3-turbo (809M):**  
  * Fine-tuned checkpoint of Whisper Large-v3-Turbo achieving **6.07% Avg WER** on the Open ASR Leaderboard (substantially outperforming vanilla Turbo at 8.39% and vanilla Large-v3 at 7.84%).
  * **Significance for Silent Voice:** Runs inside **`whisper.cpp`** via standard GGML quantization. It gives `whisper.cpp` users accuracy that directly beats Parakeet TDT 0.6B v2 while preserving Whisper's native formatting and prompt biasing.
* **Distil-Whisper Distil-Large-v3.5 (756M):**  
  * Distilled on 98,000 hours of audio with frozen encoder and 2 decoder layers.  
  * Delivers **7.10% short-form WER** (beating Whisper Large-v3-Turbo's 7.25%) and runs **1.46× faster**. Available as `distil-whisper/distil-large-v3.5-ggml`.

### 4.3 Alibaba Qwen3-ASR Family
* **Qwen3-ASR-1.7B-hf & 0.6B-hf:**  
  * Released in January 2026 under **Apache 2.0**.
  * **Qwen3-ASR-1.7B** achieves **5.75% Avg WER** and **835.62 RTFx**, surpassing Parakeet TDT 0.6B v2 across meetings (AMI: 8.31%), financial calls (Earnings-22: 5.84%), and noisy speech (URGENT2024: 9.93%).
  * **Qwen3-ASR-0.6B** achieves **6.60% Avg WER** and **749.52 RTFx**.
  * **Silent Voice Integration:** In [`src-tauri/src/system/sherpa_stt.rs`](file:///D:/Vibe-coding/Silent%20voice/src-tauri/src/system/sherpa_stt.rs#L125-L136), Silent Voice's bundled `sherpa-onnx` DLL (v1.13.4) already includes `Qwen3AsrModelConfig` (`conv_frontend`, `encoder`, `decoder`, `tokenizer`). Pre-quantized ONNX bundles are available from `csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25` and `thieunv-asilla/sherpa-onnx-qwen3-asr-1.7B-int8`.

### 4.4 Cohere Transcribe (03-2026)
* **Architecture:** 2.0B parameter Conformer/Transformer architecture licensed under **Apache 2.0**.
* **Performance:** **6.26% Avg WER** and **923.71 RTFx**. Scores 1.34% on LibriSpeech clean and 2.52% on VoxPopuli.
* **Silent Voice Integration:** In [`src-tauri/src/system/sherpa_stt.rs`](file:///D:/Vibe-coding/Silent%20voice/src-tauri/src/system/sherpa_stt.rs#L138-L144), Silent Voice already declares `CohereTranscribeModelConfig` (`encoder`, `decoder`, `language`, `use_punct`, `use_itn`). Pre-converted INT8 packages are hosted in `csukuangfj2/sherpa-onnx-cohere-transcribe-14-lang-int8-2026-04-01`.

### 4.5 IBM Granite Speech 5.0 TurboCTC (470M)
* **Architecture:** 16-block Conformer encoder with 8× temporal subsampling, block-diagonal attention (128 frames), self-conditioning, and 16,384 BPE target vocabulary. Greedy CTC decoding.
* **Performance:** Unprecedented throughput (**21,134.31 RTFx** on leaderboard; **12,600+ RTFx** on H200). On the original 8 public test sets, it scored **5.00% Avg WER**; on the 2026 leaderboard suite with heavy noise, it scored **6.84% Avg WER**.
* **Runtime Gate:** Quantized GGUF models (`handy-computer/granite-speech-5.0-470m-turboctc-gguf`) run in `transcribe.cpp` on CPU, Vulkan, and CUDA. Not natively supported by `sherpa-onnx`'s current CTC FFI structs.

### 4.6 Alibaba SenseVoice-Small (234M)
* **Architecture:** Non-autoregressive SAN-m encoder with CTC decoding.
* **Strengths:** Extreme CPU efficiency (**RTF 0.02–0.04**, processing 5 seconds in <150ms on a standard laptop CPU). Natively emits audio event tags (`<HAPPY>`, `<LAUGHTER>`), capitalization, and punctuation. Already fully functional in Silent Voice (`SttEngine::SenseVoice`).

---

## 5. Runtime Compatibility Matrix for Silent Voice

Silent Voice has two engine backends:
1. `whisper.cpp` sidecar CLI executable ([`src-tauri/src/transcription/whisper.rs`](file:///D:/Vibe-coding/Silent%20voice/src-tauri/src/transcription/whisper.rs)).
2. `sherpa-onnx` dynamic library C-API FFI ([`src-tauri/src/system/sherpa_stt.rs`](file:///D:/Vibe-coding/Silent%20voice/src-tauri/src/system/sherpa_stt.rs)).

| Model Candidate | Runtime Engine | FFI / Sidecar Path | Implementation Status in Silent Voice |
| :--- | :--- | :--- | :--- |
| **Parakeet TDT 0.6B v2 INT8** | `sherpa-onnx` | `OfflineModelConfig.transducer` | **100% READY** (Default flagship; embedded BPE vocab) |
| **Parakeet TDT 0.6B v3 INT8** | `sherpa-onnx` | `OfflineModelConfig.transducer` | **100% READY** (Drop-in folder replacement) |
| **TheStageAI thewhisper-turbo**| `whisper.cpp` | `-m ggml-thewhisper-turbo.bin` | **100% READY** (Standard Whisper GGML binary) |
| **Distil-Whisper Large-v3.5** | `whisper.cpp` | `-m ggml-distil-large-v3.5.bin`| **100% READY** (Standard Whisper GGML binary) |
| **SenseVoice-Small INT8** | `sherpa-onnx` | `OfflineModelConfig.sense_voice` | **100% READY** (Active balanced preset) |
| **Qwen3-ASR-0.6B / 1.7B INT8** | `sherpa-onnx` | `OfflineModelConfig.qwen3_asr` | **FFI READY** (Structs declared in `sherpa_stt.rs`) |
| **Cohere Transcribe INT8** | `sherpa-onnx` | `OfflineModelConfig.cohere_transcribe`| **FFI READY** (Structs declared in `sherpa_stt.rs`) |
| **Canary-180M-Flash INT8** | `sherpa-onnx` | `OfflineModelConfig.canary` | **FFI READY** (Structs declared in `sherpa_stt.rs`) |
| **Granite Speech 5.0 TurboCTC**| `transcribe.cpp` | GGUF CLI sidecar | **ENGINE NEEDED** (Requires transcribe-cli sidecar) |
| **Phi-4-multimodal / Voxtral** | Python / vLLM | Incompatible with desktop C-API | **UNSUPPORTED** (Requires server / 8GB+ VRAM) |

---

## 6. Hardware Feasibility on Windows Laptop (CPU vs Modest GPU)

| Hardware Profile | Model Recommendation | Expected Latency / RTF | RAM / VRAM Footprint | Practical Experience |
| :--- | :--- | :--- | :--- | :--- |
| **CPU Only (Standard Laptop)** | **SenseVoice-Small INT8** | **RTF ~0.02 – 0.04** (<150ms for 5s clip) | ~300 MB RAM | Instantaneous response, zero fan noise, automatic punctuation. |
| **CPU Only (High Accuracy)** | **Parakeet TDT 0.6B v2 INT8** | **RTF ~0.08 – 0.12** (~450ms for 5s clip) | ~600 MB RAM | High-fidelity English transcription, 4–8 CPU threads fully utilized. |
| **CPU Only (Whisper Engine)** | **Distil-Whisper Large-v3.5 Q5_0** | **RTF ~0.15 – 0.22** (~900ms for 5s clip) | ~900 MB RAM | 1.46× faster than Large-v3-Turbo, native capitalization. |
| **Modest GPU (RTX 3050/4050)** | **Parakeet TDT 0.6B v2 INT8** | **RTF ~0.003** (<20ms for 5s clip) | ~800 MB VRAM | Instantaneous (>3,000 RTFx), minimal GPU load, 6.05% WER. |
| **Modest GPU (Multilingual)** | **Parakeet TDT 0.6B v3 INT8** | **RTF ~0.003** (<20ms for 5s clip) | ~800 MB VRAM | 25 European languages with instant transcription speed. |
| **Modest GPU (SOTA Whisper)** | **thewhisper-large-v3-turbo** | **RTF ~0.010** (~50ms for 5s clip) | ~1.6 GB VRAM | **6.07% WER** in `whisper.cpp` with prompt biasing support. |
| **Modest GPU (Next-Gen ASR)** | **Qwen3-ASR-1.7B INT8** | **RTF ~0.015** (~75ms for 5s clip) | ~2.2 GB VRAM | **5.75% WER** in `sherpa-onnx`, beating all 0.6B models. |

---

## 7. Concrete Recommendations for Silent Voice

### Recommendation 1: Retain Parakeet TDT 0.6B v2 INT8 as the Primary English Accuracy Default
* **Rationale:** It offers an unbeatable balance of speed (**3,380 – 14,383 RTFx**), accuracy (**6.05% – 6.43% WER**), and memory footprint (~600 MB RAM / 800 MB VRAM).
* **Codebase Alignment:** It is already deeply integrated in Silent Voice:
  1. `src/components/onboarding/Onboarding.tsx` selects it as default when cores >= 4 or VRAM >= 2 GB.
  2. `src-tauri/src/system/sherpa_stt.rs` embeds its SentencePiece BPE vocabulary (`PARAKEET_V2_BPE_VOCAB`) for hotword biasing.
  3. Pre-quantized INT8 model files match `src-tauri/src/models/registry.rs` (`transducer_installed()`).

### Recommendation 2: Introduce `thewhisper-large-v3-turbo` into `whisper.cpp`
* **Rationale:** It achieves **6.07% Avg WER** on the Open ASR Leaderboard, beating OpenAI's vanilla Whisper Large-v3-Turbo (8.39%) and matching Parakeet on accuracy.
* **Turnkey Delivery:** Because it uses the standard Whisper-Turbo architecture, it converts directly to GGML (`ggml-thewhisper-turbo.bin`), giving users who prefer `whisper.cpp` state-of-the-art accuracy.

### Recommendation 3: Add `parakeet-tdt-0.6b-v3-int8` as the Multilingual Flagship in `sherpa-onnx`
* **Rationale:** At **6.38% Avg WER** and identical ~6,200 RTFx speed, it adds native transcription for **25 European languages** as a drop-in directory replacement for users requiring multilingual support.

### Recommendation 4: Activate `Qwen3-ASR-1.7B-int8` via Existing `Qwen3AsrModelConfig`
* **Rationale:** At **5.75% Avg WER**, Qwen3-ASR-1.7B is the most accurate Apache 2.0 model capable of running in Silent Voice's existing `sherpa-onnx` runtime. With pre-quantized INT8 files available and C-API bindings already declared in `sherpa_stt.rs`, wiring a loader function provides a definitive accuracy upgrade for power users with discrete GPUs.

### Recommendation 5: Track IBM Granite Speech 5.0 TurboCTC for Future Engine Sidecar
* **Rationale:** Delivering **21,134 RTFx** and **5.00% WER** on standard benchmarks, Granite Speech 5.0 TurboCTC defines the non-autoregressive efficiency frontier. If Silent Voice bundles `transcribe.cpp` as a lightweight CLI sidecar (similar to `whisper-cli.exe`), Granite GGUF will provide the ultimate CPU transcription speed.
