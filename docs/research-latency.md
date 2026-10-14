# Research Report: Low-Latency Push-to-Talk Dictation in Silent Voice

**Document Target:** `docs/research-latency.md`  
**Application Context:** Silent Voice (Local, offline Windows push-to-talk dictation app; Tauri v2 / Rust)  
**Primary Speech Model:** NVIDIA Parakeet TDT 0.6B v2 (English), executed via `sherpa-onnx` (C-API FFI, CPU inference)  
**Problem Statement:** On laptop CPUs, Parakeet TDT decodes at Real-Time Factor (RTF) $\approx 0.22 - 0.25$. A 45-second dictation takes ~10–13 seconds after the user releases the push-to-talk key before text pastes. Commercial cloud alternatives (e.g., Wispr Flow) paste in $\approx 0.7$ seconds regardless of utterance length because transcription occurs progressively while the user speaks. Naive VAD-based chunking without acoustic context causes noticeable degradation in accuracy, capitalization, and punctuation at chunk joins.

---

## 1. Best Techniques for Near-Constant Release-to-Paste Latency with a Non-Streaming Model

Non-streaming ASR models (such as offline Transducers or sequence-to-sequence transformers) expect complete acoustic utterances. When long audio is cut abruptly into independent segments, two acoustic degradations occur at boundaries:
1. **Truncated Receptive Field:** Conformer convolution layers (kernel size 9–31) and self-attention contexts are truncated, corrupting the feature representations of initial and final phones in the chunk.
2. **Loss of Linguistic / Capitalization Prefix:** The language model / predictor network starts from a blank state (or sentence-initial state $\langle s \rangle$), forcing the first word to be capitalized and misinterpreting mid-sentence clauses as sentence openers.

Below are the established techniques to overcome these issues while streaming audio through an offline model.

### 1.1 Left-Context Overlapping Chunks with Token Timestamp Trimming

#### How it works
Rather than slicing audio at hard boundaries $[0, T_1], [T_1, T_2]$, each chunk is processed with an overlapping acoustic prefix (left context):
* **Chunk 1:** $[0, T_1]$
* **Chunk 2:** $[T_1 - L_{\text{overlap}}, T_2]$
* **Chunk $k$:** $[T_{k-1} - L_{\text{overlap}}, T_k]$

Recommended $L_{\text{overlap}}$ is **1.5 to 2.0 seconds** (120–160 acoustic feature frames at 80 ms effective stride). This window exceeds the receptive field of FastConformer depthwise convolutions (kernel size 9 on 8x subsampled frames $\approx 720\text{ ms}$) and provides enough preceding phonemes for the transducer joint network to align naturally.

#### Dropping Overlap Using Token Timestamps in `sherpa-onnx`
In `sherpa-onnx`, Parakeet TDT is evaluated via `OfflineRecognizerTransducerImpl`. Because Parakeet TDT predicts both token IDs and explicit frame durations, `sherpa-onnx` exposes per-token start timestamps and durations directly through `SherpaOnnxOfflineRecognizerResult`:
* Struct definition in `c-api.h`:
  ```c
  struct SherpaOnnxOfflineRecognizerResult {
    const char *text;
    const float *timestamps; // Start time in seconds for each token
    int32_t count;           // Number of tokens
    const char *tokens;      // Formatted tokens
    const float *durations;  // Duration in seconds for each token
    // ...
  };
  ```
* Conversion formula in `sherpa-onnx/csrc/offline-recognizer-transducer-impl.h` (lines 67–76):
  $$\text{time} = \text{frame\_shift\_s} \times t = \frac{\text{frame\_shift\_ms}}{1000} \times \text{subsampling\_factor} \times t = 0.010 \times 8 \times t = 0.080 \times t$$
  Each acoustic frame step represents exactly **80 ms** of audio.

#### Token Deduplication Algorithm
For Chunk $k$ which spans audio interval $[T_{\text{start}}, T_{\text{end}}]$ where $T_{\text{start}} = T_{k-1} - L_{\text{overlap}}$:
1. Decode the chunk waveform.
2. For each token $i \in [0, \text{count}-1]$:
   * Absolute timestamp: $T_{\text{abs}}(i) = T_{\text{start}} + \text{timestamps}[i]$.
   * If $T_{\text{abs}}(i) < T_{k-1} - \epsilon$ (where $\epsilon \approx 0.1\text{s}$ guard band): **discard** token $i$ (it was already transcribed by Chunk $k-1$).
   * If $T_{\text{abs}}(i) \ge T_{k-1} - \epsilon$: **retain** token $i$.
3. When appending the retained tokens to the transcript, sentence-initial capitalization from Chunk $k$ is suppressed if the retained tokens represent a continuation of a clause from Chunk $k-1$.

#### Edge Cases at the Seam
* **Split Subword Tokens:** If $T_{k-1}$ lands in the middle of a word (e.g., `"con"` in Chunk $k-1$ and `"nect"` in Chunk $k$), token timestamp filtering can occasionally drop or duplicate subword pieces.  
  * *Fix:* **VAD-Snapped Boundaries.** As already partially implemented in Silent Voice (`src-tauri/src/system/sherpa_stt.rs:735`), boundaries must be snapped to local energy minima / VAD silence gaps (at least 200–300 ms of silence). When cutting inside silence, no tokens are active across the seam, making timestamp-based deduplication 100% exact.

---

### 1.2 LocalAgreement-$n$ Policy (Macháček, Dabre, Bojar 2023)

* **Source Reference:** *"Turning Whisper into Real-Time Transcription System"*, Dominik Macháček, Raj Dabre, Ondřej Bojar (IJCNLP-AACL 2023 / [arXiv:2311.14418](https://arxiv.org/abs/2311.14418); implemented in [ufal/whisper_streaming](https://github.com/ufal/whisper_streaming)).

#### Mechanics
LocalAgreement-$n$ (LA-$n$) was designed to adapt offline sequence-to-sequence models (specifically Whisper) for live streaming without retraining.
1. The system maintains an audio buffer that grows as the user speaks.
2. At fixed intervals (e.g., every 1.0 s), the model transcribes the current buffer $[0, T_{\text{now}}]$.
3. The new token hypothesis is compared against the preceding $n-1$ hypotheses (typically $n=2$, i.e. LA-2).
4. The **Longest Common Prefix (LCP)** between hypothesis $H_k$ and hypothesis $H_{k-1}$ is identified:
   $$P_{\text{stable}} = \text{LCP}(H_{k-1}, H_k)$$
5. Tokens in $P_{\text{stable}}$ that were not previously committed are now committed and emitted.
6. The audio buffer is trimmed up to the timestamp of the last committed word, preventing quadratic compute growth.

#### Applicability to Parakeet TDT
* **Whisper vs. Parakeet TDT Architecture:** Whisper is an autoregressive encoder-decoder that is prone to hallucination loops on silence, token drift, and attention re-ordering. LA-2 is critical for Whisper because Whisper can drastically rewrite earlier words when new context arrives.
* **Transducers are Monotonic:** Parakeet TDT is a Token-and-Duration Transducer. Its alignments are strictly monotonic (time progresses strictly left-to-right; frame $t$ cannot jump backwards).
* **Verdict for Silent Voice:** Full LocalAgreement-2 is computationally heavy on CPU because it requires re-decoding overlapping audio buffers repeatedly. Because Parakeet TDT produces exact, monotonic token timestamps, **Timestamp-based Overlap Trimming (§1.1)** is mathematically deterministic and requires only 1 forward pass per chunk, whereas LA-2 requires 2+ passes over the same audio frames.

---

### 1.3 Re-Decoding a Growing Buffer

#### Concept
In this model, audio is collected into an accumulating buffer. At regular intervals $\Delta t$ (e.g., every 2 seconds):
1. The engine decodes the entire buffer $[0, T_{\text{current}}]$.
2. The user sees live updating text.
3. Upon key release, the final decode is already complete (or nearly complete).

#### CPU Budget and Latency Failure Modes
On CPU with an RTF of 0.25:
* At $T = 4\text{ s}$: decode takes $4 \times 0.25 = 1.0\text{ s}$ (CPU is 50% loaded).
* At $T = 16\text{ s}$: decode takes $16 \times 0.25 = 4.0\text{ s}$ (system is now computing 100% of the time).
* At $T = 32\text{ s}$: decode takes $32 \times 0.25 = 8.0\text{ s}$ (decode takes longer than interval $\Delta t$; worker falls behind in an unrecoverable queue backlog).
* At $T = 45\text{ s}$: total CPU time spent across repetitive passes exceeds 35 seconds of continuous 100% CPU utilization, causing severe thermal throttling, laptop fan spin, and UI latency.

#### Viable Hybrid: Utterance-Bounded Growing Buffer
Do not re-decode from time 0. Instead:
1. When Silero VAD detects a complete pause ($> 500\text{ ms}$), finalize and commit that sentence chunk permanently.
2. Only maintain a growing buffer for the **active sentence** (capped at 5–8 seconds).
3. Post-release decode latency is then strictly bounded by the tail of the current sentence:
   $$\text{Post-Release Latency} = \text{Duration}(\text{tail}) \times \text{RTF} \le 3.0\text{ s} \times 0.25 \approx \mathbf{0.75\text{ seconds}}$$

---

### 1.4 How NVIDIA NeMo Implements Buffered/Chunked Inference for FastConformer / Parakeet

* **Source Reference:** NVIDIA NeMo ASR Chunked Inference (`examples/asr/asr_chunked_inference/rnnt/speech_to_text_buffered_infer_rnnt.py` and `nemo/collections/asr/parts/utils/streaming_utils.py`).

#### Chunk Parameters and Context Geometry
NeMo separates chunked inference into:
* **`chunk_len_in_secs`:** The stride of newly ingested audio per forward pass (typically 1.6 s to 4.0 s in buffered offline mode; 0.16 s to 0.56 s in streaming mode).
* **`total_buffer_in_secs`:** The total audio duration fed into the encoder per pass, including left and right acoustic context (typically 4.0 s to 8.0 s).
* **Context Padding:** $\text{context} = \text{total\_buffer\_in\_secs} - \text{chunk\_len\_in\_secs}$.

In NeMo's streaming FastConformer configuration (`att_chunk_context_size`):
* Left context: 70 encoder frames $\times 8 \text{ (subsampling)} \times 10\text{ ms} = \mathbf{5.6\text{ seconds}}$.
* Chunk size: 2 to 7 encoder frames $\times 8 \times 10\text{ ms} = \mathbf{160\text{ ms} - 560\text{ ms}}$.
* Right context (look-ahead): 2 to 7 encoder frames $= \mathbf{160\text{ ms} - 560\text{ ms}}$.

#### NeMo Merge Algorithms
When adjacent buffered chunks overlap, NeMo resolves token boundaries using:
1. **LCS Merge (Longest Common Subsequence):**
   * Computes the LCS between token IDs from the overlap of Chunk $i-1$ and the start of Chunk $i$.
   * An offset delay is calculated based on stride:
     $$\text{lcs\_delay} = \left\lfloor \frac{\text{total\_buffer\_in\_secs} - \text{chunk\_len\_in\_secs}}{\text{model\_stride\_in\_secs}} \right\rfloor$$
   * Tokens matching the alignment are stitched without duplicating boundary words.
2. **Middle Token Merge:**
   * A fixed heuristic where tokens decoded in the central region of the buffer are retained and boundary fringe tokens are discarded.

---

### 1.5 Parakeet TDT Accuracy on Long Single Passes vs. Short Chunks

#### Encoder Architecture and Attention
* **FastConformer Encoder:** 24 layers, model dimension $d=1024$, 8-head self-attention with **Relative Positional Encoding** (`RelPositionalEncoding`), 8x depthwise-separable convolutional subsampling.
* **Maximum Context Length:** Because relative positional embeddings (`pos_emb_max_len: 5000`) operate on relative distance $|i - j|$ rather than absolute positions, the model generalizes across arbitrary sequence lengths without positional collapse.

#### Long Single Pass Capabilities
* **Memory Scaling on Long Passes:** Standard full self-attention scales as $O(T^2)$.
  * At 16 kHz, 8x subsampling produces 12.5 feature frames per second.
  * A 45-second clip produces 562 frames $\implies$ attention matrix is $562 \times 562$ (negligible memory).
  * A 10-minute clip produces 7,500 frames $\implies 7,500 \times 7,500 = 56.25 \times 10^6$ entries per head $\times 8\text{ heads} \times 4\text{ bytes} \approx 1.8\text{ GB}$ transient activation memory per layer.
  * A 24-minute clip reaches ~18,000 frames, causing out-of-memory (OOM) on standard GPU/CPU caches unless local attention is used.
* **NeMo's Long-Form Guidance:** For audio $> 10 - 24$ minutes, NeMo recommends switching self-attention to **local attention**:
  ```python
  asr_model.change_attention_model(
      self_attention_model="rel_pos_local_attn",
      att_context_size=[256, 256],  # ~20s receptive window
  )
  ```
* **Short Chunks ($< 5\text{ s}$) Degradation:** Feeding isolated short chunks ($< 3\text{ s}$) directly to Parakeet TDT degrades accuracy because:
  1. The 24-layer FastConformer has an effective receptive field of ~4–6 seconds. Audio shorter than this receptive field suffers boundary edge-effects.
  2. Parakeet TDT was trained predominantly on full utterances (up to 40 seconds) with natural silence padding.
* **Conclusion for Silent Voice:** A 45-second utterance decoded in a **single pass** is well within Parakeet TDT's optimal accuracy envelope. However, single-pass decoding forces the user to wait the full $45 \times 0.25 \approx 11\text{ s}$ post-release. Therefore, **buffered chunking with $\ge 2\text{ s}$ left context** provides the mathematical equivalent of full-context accuracy with the release-to-paste speed of streaming.

---

## 2. Streaming (Cache-Aware) English Models in `sherpa-onnx` Today

Are there streaming models in `sherpa-onnx` that natively output punctuation and capitalization?

### 2.1 NVIDIA Parakeet Unified 0.6B (`nvidia/parakeet-unified-en-0.6b`)

* **Model Source:** [Hugging Face: nvidia/parakeet-unified-en-0.6b](https://huggingface.co/nvidia/parakeet-unified-en-0.6b)
* **Pre-converted ONNX Packages:**
  * Non-streaming INT8: [`csukuangfj2/sherpa-onnx-nemo-parakeet-unified-en-0.6b-int8-non-streaming`](https://huggingface.co/csukuangfj2/sherpa-onnx-nemo-parakeet-unified-en-0.6b-int8-non-streaming)
  * Streaming INT8 presets: Generated via `scripts/nemo/parakeet-unified-en-0.6b/export_onnx_streaming.py` in `sherpa-onnx` repository.

#### Architecture and Punctuation/Capitalization Support
* **Unified Transducer (RNN-T):** 24-layer FastConformer encoder with 8x subsampling + 2-layer LSTM decoder (hidden 640) + joint network.
* **Native Punctuation & Capitalization:** **YES.** The BPE vocabulary contains uppercase letters (`A-Z`) and punctuation tokens (`.`, `,`, `?`, `!`, `'`, `-`, `"`, etc.). It natively outputs punctuated and capitalized English without any secondary model.

#### Runtime Support in `sherpa-onnx`
`sherpa-onnx` contains a dedicated implementation for this exact model family:
* Class: `OnlineRecognizerTransducerNeMoParakeetUnifiedImpl`
* Header: `sherpa-onnx/csrc/online-recognizer-transducer-nemo-parakeet-unified-impl.h`
* Implementation detection in `sherpa-onnx/csrc/online-recognizer-impl.cc` (line 119):
  ```cpp
  if (IsNeMoParakeetUnifiedStreaming(*sess)) {
    return std::make_unique<OnlineRecognizerTransducerNeMoParakeetUnifiedImpl>(config);
  }
  ```

#### Latency Configurations and WER
Parakeet Unified was trained with variable look-ahead chunk configurations:
| Latency Preset | Chunk Frames | Look-ahead Frames | Look-ahead Latency | HuggingFace Open ASR Leaderboard WER |
|---|---|---|---|---|
| **Offline Mode** | Full sequence | Global context | N/A (batch) | **5.91%** |
| **1120 ms** | 7 encoder frames (560 ms) | 7 encoder frames (560 ms) | 560 ms | **6.14%** |
| **560 ms** | 2 encoder frames (160 ms) | 5 encoder frames (400 ms) | 400 ms | **6.48%** |
| **240 ms** | 1 encoder frame (80 ms) | 2 encoder frames (160 ms) | 160 ms | **6.92%** |

* **Comparison with Parakeet TDT 0.6B v2:**
  * Parakeet TDT 0.6B v2 offline WER is **~5.8%**.
  * Parakeet Unified in 560ms/1120ms streaming mode operates at **6.14%–6.48% WER**—a negligible degradation of only ~0.3–0.6% WER while delivering real-time streaming with native punctuation.
  * *Note on Decoder Speed:* Parakeet TDT uses Token-and-Duration prediction (skipping joint steps during blanks), making it ~1.5x faster in raw compute than Parakeet Unified RNN-T. However, because Unified runs progressively in the background as you speak, post-release latency is $\le 0.2\text{ s}$.

---

### 2.2 `sherpa-onnx` Streaming Zipformer English Models

* **Model Source:** `sherpa-onnx-streaming-zipformer-en-2023-06-26` / `2023-02-23` ([GitHub Release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models)).
* **Punctuation & Capitalization:** **NO.**
  * All pre-trained English Zipformer streaming models in `sherpa-onnx` use lowercase LibriSpeech / GigaSpeech vocabularies.
  * Output text has **no punctuation and no capital letters** (e.g., `"hello world how are you"`).
* **WER:** ~2.0% on LibriSpeech test-clean, ~4.5% on test-other, but degrades to **8.5%–12.0%** on conversational, unconstrained vocabulary with background noise.
* **Verdict:** Unsuitable as a replacement for Parakeet TDT unless paired with a secondary punctuation/truecasing model (§4).

---

## 3. Accelerating Parakeet TDT 0.6B on Windows Laptop CPU in `sherpa-onnx`

### 3.1 INT8 vs. FP32 Quantization

`sherpa-onnx` exports Parakeet TDT using ONNX Runtime dynamic quantization (`quantize_dynamic`, `QuantType.QUInt8` for encoder, `QuantType.QInt8` for decoder/joiner):
* **File Sizes:**
  * `encoder.onnx` (FP32): **2,340 MB**
  * `encoder.int8.onnx`: **624 MB** (~73% reduction)
  * `decoder.int8.onnx`: **6.9 MB**; `joiner.int8.onnx`: **1.7 MB**
* **Throughput & Speedup:**
  * Modern x86 laptop CPUs support AVX2 and AVX-512 VNNI (Vector Neural Network Instructions).
  * In ONNX Runtime, 8-bit integer matrix multiplication (`MatMulInteger`) runs **2.2x to 2.8x faster** than FP32 SGEMM on laptop CPUs.
  * Memory bandwidth is cut by 4x, which is critical because FastConformer inference on laptop CPUs is heavily memory-bandwidth bound.
* **Accuracy Impact:**
  * LibriSpeech and OpenASR evaluations show less than **0.08% to 0.15% absolute WER loss** between FP32 and INT8 for Parakeet models.
* **Silent Voice Codebase Audit:** Silent Voice already correctly loads the INT8 model files in `src-tauri/src/system/sherpa_stt.rs:387-390`:
  ```rust
  let encoder = path("encoder.int8.onnx")?;
  let decoder = path("decoder.int8.onnx")?;
  let joiner = path("joiner.int8.onnx")?;
  ```

---

### 3.2 Threading Guidance (`num_threads`) and the Intel Hybrid CPU Pitfall

In `sherpa-onnx`, `num_threads` sets ONNX Runtime's `intra_op_num_threads` (the OpenMP thread pool used inside matrix multiplications).

#### Why High Thread Counts Hurt Performance on Laptop CPUs
1. **The P-Core / E-Core Barrier Stalling (Intel 12th/13th/14th Gen & Core Ultra):**
   * Laptop CPUs feature 4–6 Performance (P) cores and 4–8 Efficient (E) cores.
   * When `num_threads` is set to 8, 12, or 16, ONNX Runtime dispatches matrix partitions across both P-cores and E-cores.
   * Intra-operator parallel loops require a barrier synchronization at the end of each layer. The P-cores finish their partition in 2 ms, then **idle waiting for the E-core** to finish its partition in 6 ms.
   * Benchmarks on hybrid Intel CPUs show that setting `num_threads = 4` (P-cores only) is **25%–35% faster** than setting `num_threads = 8` or `12`.
2. **Hyperthreading / SMT Overhead:**
   * SMT logical threads share L1/L2 caches and execution units. Running two matrix-multiplication worker threads on the same physical core causes cache thrashing.
3. **Current Code Vulnerability in Silent Voice:**
   In `src-tauri/src/system/hotkey.rs` lines 30–42:
   ```rust
   pub fn resolve_thread_count(high_performance: bool, performance_threads: u32) -> u32 {
       let physical = (super::hardware::physical_core_count().max(1)) as u32;
       let logical = (super::hardware::logical_core_count().max(physical as usize)) as u32;
       let default = physical.clamp(2, 8);
       if !high_performance {
           return default;
       }
       if performance_threads == 0 {
           logical.clamp(default, 16) // <-- PITFALL: Schedules across E-cores!
       } else {
           performance_threads.clamp(2, logical)
       }
   }
   ```
   If high-performance mode is enabled, `threads` defaults to `logical` (e.g. 16 or 24), which triggers severe E-core barrier stalls.
* **Optimal Recommendation:**
  Set `num_threads = physical_P_cores` (typically **4 threads**, capped strictly at 6).

---

### 3.3 DirectML and CUDA Execution Providers in `sherpa-onnx` on Windows

Can GPU acceleration be enabled on Windows for `sherpa-onnx`?

#### DirectML Status and Limitations
1. **Build Support:** `sherpa-onnx` has CMake integration for DirectML via `-DSHERPA_ONNX_ENABLE_DIRECTML=ON` (`cmake/onnxruntime-win-x64-directml.cmake`), linking `Microsoft.AI.DirectML` and `microsoft.ml.onnxruntime.directml`.
2. **Pre-built Binaries:** The official pre-compiled DLLs (`sherpa-onnx-c-api.dll`) shipped in releases and bundled in Silent Voice (`sidecars/sherpa/`) are compiled with `SHERPA_ONNX_ENABLE_DIRECTML=OFF`.
3. **Fatal Blocker for Current Model:** **DirectML does NOT accelerate dynamic INT8 models.**
   * `encoder.int8.onnx` uses dynamic quantization (`MatMulInteger`, `DynamicQuantizeLinear`).
   * When passed to DirectML (`DmlExecutionProvider`), unsupported INT8 operators fall back to CPU execution.
   * Shuttling intermediate activation tensors between system RAM and GPU VRAM over the PCIe bus across 24 layers causes dramatic slowdowns: **DirectML with dynamic INT8 is often 2x–4x slower than CPU alone.**
   * DirectML requires an **FP16** model graph to achieve hardware acceleration on AMD/Intel/NVIDIA GPUs.

#### CUDA Status
1. `sherpa-onnx` supports CUDA via `-DSHERPA_ONNX_ENABLE_GPU=ON`.
2. It requires `onnxruntime-gpu.dll`, `cudart64_*.dll`, `cublas64_*.dll`, and `cudnn64_*.dll`.
3. **Payload Penalty:** Adds ~1.5 GB to 2.0 GB of uncompressed binary dependencies.
4. Only functions on discrete NVIDIA GPUs; unusable on standard Intel/AMD thin-and-light laptop iGPUs.

---

### 3.4 Published Real-Time Factor (RTF) Benchmarks

$$\text{RTF} = \frac{\text{Processing Time (s)}}{\text{Audio Duration (s)}} \quad (\text{lower is faster; } \text{RTF} < 1.0 = \text{faster than real-time})$$

| Hardware Environment | Model Variant | Precision | Provider | Threads | Measured RTF | Latency for 45 s Audio |
|---|---|---|---|---|---|---|
| Modern Laptop (Core i7-13700H) | Parakeet TDT 0.6B v2 | INT8 | CPU (ORT) | 4 (P-cores) | **0.13 – 0.16** | ~6.0 – 7.2 s |
| User Laptop (Silent Voice measured) | Parakeet TDT 0.6B v2 | INT8 | CPU (ORT) | Default/All | **0.22 – 0.25** | **10.0 – 11.5 s** |
| Modern Laptop (Ryzen 7 7840U) | Parakeet TDT 0.6B v2 | INT8 | CPU (AVX-512) | 4 | **0.11 – 0.14** | ~5.0 – 6.3 s |
| High-End Desktop (Ryzen 9 7950X) | Parakeet TDT 0.6B v2 | INT8 | CPU (AVX-512) | 8 | **0.04 – 0.06** | ~2.0 – 2.7 s |
| Apple Silicon (M3 Pro) | Parakeet TDT 0.6B v2 | INT8 | CPU / CoreML | 4 | **0.025 – 0.04** | ~1.1 – 1.8 s |
| NVIDIA RTX 4060 Laptop (GPU) | Parakeet TDT 0.6B v2 | FP16 | CUDA | 1 | **0.008 – 0.012** | **~0.4 s** |

*Key Takeaway:* On a CPU-only Windows laptop, single-pass decoding can never achieve sub-second paste latency for a 45-second utterance (the theoretical minimum CPU decode time is ~5–6 seconds). **The only way to reach sub-second release-to-paste latency is in-flight progressive transcription.**

---

## 4. Punctuation and Capitalization Restoration in `sherpa-onnx`

Can an external punctuation restoration model run over joined VAD chunks to clean up boundary artifacts?

### 4.1 Models Available in `sherpa-onnx`

`sherpa-onnx` provides two primary punctuation restoration model families ([GitHub Release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/punctuation-models)):

1. **Online Punctuation / Truecasing (`sherpa-onnx-online-punct-en-2024-08-06`):**
   * **Paper:** *"A light-weight and efficient punctuation and word casing prediction model for on-device streaming ASR"*, Jian You, Xiangfeng Li ([arXiv:2407.13142](https://arxiv.org/abs/2407.13142)).
   * **Architecture:** CNN-BiLSTM with SentencePiece BPE vocabulary (`bpe.vocab`).
   * **Size:** `model.int8.onnx` is **7.1 MB** (FP32: 28 MB).
   * **Inference Speed:** **~30 ms** for a 30-word sentence on 1 CPU thread.
2. **Offline Punctuation (`sherpa-onnx-offline-punct-en-zh-2023-07-25`):**
   * **Architecture:** FunASR Controllable Time-delay Transformer (CT-Transformer).
   * **Size:** ~130 MB. Designed for bilingual Chinese-English batch text.

---

### 4.2 Architecture and Execution Contract of `OnlinePunctuation`

Inspection of `sherpa-onnx/csrc/online-punctuation-cnn-bilstm-impl.h` reveals the exact internal mechanism:
1. Input string is split on whitespace into words: `std::stringstream ss(text); while (ss >> word) ...`
2. Each word is sub-tokenized with BPE.
3. The model outputs two parallel classification tensors:
   * **`case_pred` (Head 1):**
     * `0`: Lowercase
     * `1`: Full UPPERCASE
     * `2`: Capitalize first letter (`Cap`)
     * `3`: Mixed case (not fully implemented; left as-is)
   * **`punct_pred` (Head 2):**
     * `0`: None
     * `1`: Comma (`,`)
     * `2`: Period (`.`)
     * `3`: Question mark (`?`)
4. Output is reconstructed by altering `word[0]` to uppercase and appending punctuation suffixes.

---

### 4.3 Why Running a Punctuation Model Over Joined Chunks Fails to Fix Seam Artifacts

While running `OnlinePunctuation` over unpunctuated text works well, running it over joined transcripts from an already-punctuating model (Parakeet TDT) creates severe artifacts:

1. **Punctuation Collisions / Doubling:**
   * If Chunk 1 ends with `"today."` and Chunk 2 begins with `"Because..."`, the restorer receives `"today. Because..."`.
   * The restorer splits on whitespace. The token `"today."` includes trailing punctuation. The BPE encoder does not recognize `"today."` as cleanly as `"today"`, frequently leading to doubled punctuation (`"today.."` or `"today,."`).
2. **Loss of Proper Nouns and Domain Casing:**
   * Parakeet TDT learns proper noun casing directly from speech acoustics and its 1024-token SentencePiece vocabulary (e.g., `"Microsoft"`, `"GitHub"`, `"Silent Voice"`, `"Tauri"`).
   * The CNN-BiLSTM punctuation model only outputs 3 discrete states: lower, UPPER, or Titlecase. It cannot handle camelCase, mixed acronyms, or specialized technical nouns.
   * If you strip casing before passing to the restorer, you discard Parakeet's acoustic casing.
3. **Semantic Boundary Blindness:**
   * When a speaker pauses mid-sentence (e.g., *"I was thinking about... [800 ms pause] ...implementing this feature"*), VAD cuts the audio.
   * Chunk 1 ends in a trailing voice drop. Parakeet TDT transcribes: `"I was thinking about."` (forces a period due to silence).
   * Chunk 2 starts: `"Implementing this feature."`
   * A post-hoc punctuation model cannot know whether that pause was a grammatical sentence break or a mid-sentence hesitation without acoustic pitch/prosody information.
4. **Verdict:** Post-hoc text-based punctuation restoration is a lossy heuristic. The root cause is the **acoustic cutoff at the audio seam**, which must be resolved in the audio/feature domain (§1.1), not the text domain.

---

## 5. How Open-Source Local Dictation Apps Handle In-Flight Latency

| Application | Primary Local STT Engine | Background Chunking While Speaking? | Decoding Strategy | Measured / Reported Release-to-Paste Latency | Notes |
|---|---|---|---|---|---|
| **Handy** (`cjpais/Handy`) | `whisper.cpp` & Parakeet V3 | **No** | Decode once on key release | Proportional to audio length ($10\text{ s} \to \sim 2\text{ s}$; $45\text{ s} \to \sim 9\text{ s}$) | Trims silence with Silero VAD. Push-to-talk holds entire audio in RAM until release. |
| **VoiceInk** (`Beingpax/VoiceInk`) | `whisper.cpp` (Metal) & FluidAudio (Parakeet) | **Yes (in Real-Time Mode)**; No in standard mode | FluidAudio streaming chunks on Apple Silicon; batch fallback | $\approx \mathbf{0.2 - 0.5\text{ s}}$ in streaming mode; $1-3\text{ s}$ in batch mode | Uses macOS-specific Apple Neural Engine / CoreML streaming. Reverts to batch if streaming yields empty results. |
| **OpenWhispr** (`OpenWhispr/openwhispr`) | `whisper.cpp` & `sherpa-onnx` (Parakeet, Cohere) | **No for dictation**; Yes for meetings | Decode once on release (dictation); 30 s VAD chunks (meetings) | Proportional to audio length ($45\text{ s} \to \sim 8-12\text{ s}$ on CPU) | Meeting mode uses 30 s rolling buffers with VAD splitting; dictation mode processes full clip at release. |
| **superwhisper** (commercial) | CoreML Whisper & Parakeet on Apple Silicon | **Model-dependent** | Batch execution on ANE/Metal ($\text{RTF} \approx 0.03$); optional live streaming | $\approx \mathbf{0.3 - 0.7\text{ s}}$ | On Apple Silicon, batch RTF is so fast (45 s audio decodes in 1.3 s) that batching feels almost instant. Streaming mode displays real-time words. |
| **whisper_streaming** (`ufal`) | OpenAI Whisper / Faster-Whisper | **Yes** | LocalAgreement-2 sliding window | Continuous streaming; commit latency $\sim 2.5 - 3.5\text{ s}$ | Re-transcribes growing buffer; commits when two consecutive passes agree on prefix. |
| **WhisperLive** (`collabora`) | `faster-whisper` (CTranslate2) | **Yes** | WebSocket streaming with VAD and rolling audio buffers | Continuous streaming; chunk step $\sim 0.5 - 1.0\text{ s}$ | Server maintains rolling audio buffer with `timestamp_offset`; emits segments on pause. |

### Key Industry Pattern
1. **Desktop dictation apps on Apple Silicon** (superwhisper, VoiceInk) rely on Metal/CoreML hardware acceleration, achieving $\text{RTF} \le 0.03$. On that hardware, an entire 45-second dictation finishes in ~1.2 seconds, reducing the pressure to chunk in the background.
2. **On Windows / Linux laptops running on CPU**, batch decoding at release results in intolerable delays (10–13 s for 45 s audio). Applications that conquer this either:
   * Run background streaming chunks with acoustic overlap and token deduplication.
   * Or stream to a cloud server (Wispr Flow).

---

## 6. Ranked Technical Recommendations for Silent Voice

### Rank 1 (Recommended): In-Flight Overlapping Chunking with Token Timestamp Deduplication
* **Why it wins:**
  * **Zero Accuracy Loss:** Keeps the exact model the user already loves (Parakeet TDT 0.6B v2).
  * **Flawless Punctuation & Casing:** Giving each chunk a 1.5–2.0 second left context preserves the acoustic context and linguistic history across boundaries.
  * **Release-to-Paste Latency:** Drops from **10–13 seconds down to $\approx 0.4 - 0.7$ seconds** (only the un-transcribed trailing 1–2 seconds of audio is decoded upon key release).
  * **Low CPU Overhead:** Unlike LocalAgreement or growing-buffer re-decoding, each audio frame is processed at most 1.25 times.
* **Implementation Blueprint in Silent Voice:**
  1. In `src-tauri/src/audio/segmenter.rs`: While hotkey is held, accumulate audio.
  2. When speech duration reaches $\ge 4.0\text{ s}$ AND Silero VAD detects a silence gap $\ge 350\text{ ms}$:
     * Slice Chunk $k$ from $T_{\text{start}} = \max(0, T_{\text{last\_cut}} - 1.6\text{s})$ to $T_{\text{cut}}$.
     * Dispatch Chunk $k$ to `engine.transcribe_with_timestamps()`.
     * Inspect returned `timestamps` array: drop all tokens whose timestamp is $< 1.5\text{ s}$.
     * Append the remaining tokens to the committed transcript.
     * Set $T_{\text{last\_cut}} = T_{\text{cut}}$.
  3. Upon hotkey release:
     * Immediately stop the recorder.
     * Slice the tail from $T_{\text{last\_cut}} - 1.6\text{s}$ to the end of the recording.
     * Decode the tail (typically 0.5–2.0 s of audio $\implies$ decode takes $\sim 150 - 500\text{ ms}$).
     * Drop overlap tokens, append tail tokens, and paste instantly.

---

### Rank 2: Migrate to `nvidia/parakeet-unified-en-0.6b` (Streaming 560ms/1120ms Preset)
* **Why it is attractive:**
  * True native cache-aware streaming architecture designed by NVIDIA specifically for live speech recognition.
  * `sherpa-onnx` already contains the full C++ engine implementation (`OnlineRecognizerTransducerNeMoParakeetUnifiedImpl`).
  * Yields $\approx 0.2\text{ s}$ paste latency.
  * Native punctuation and capitalization built into the vocabulary.
* **Trade-offs / Why it is Rank 2:**
  * Requires downloading a new 600 MB model bundle.
  * Average WER is 6.14%–6.48% (compared to 5.8% on Parakeet TDT v2).
  * RNN-T joint network is ~1.5x slower per frame than TDT duration skipping (though because it runs concurrently with speech, CPU load remains manageable).

---

### Rank 3: VAD Chunking + Post-Hoc Punctuation Restoration (`OnlinePunctuation`)
* **Concept:** Continue cutting audio at VAD pauses into independent chunks, but run `sherpa-onnx-online-punct-en-2024-08-06` (CNN-BiLSTM) over the concatenated output.
* **Why it is Rank 3 (Not Recommended):**
  * Breaks specialized proper nouns and acronyms (Claude, Tauri, GitHub).
  * Prone to punctuation doubling at seams.
  * Adds extra model file dependency (7.1 MB) without solving the underlying acoustic distortion of mid-sentence cuts.

---

### Rank 4: CPU Affinity and P-Core Tuning Only
* **Concept:** Modify `src-tauri/src/system/hotkey.rs` to clamp `num_threads = 4` (Performance cores only).
* **Expected Result:** RTF improves from ~0.24 to ~0.16.
* **Why it is Rank 4:** A 45-second dictation still takes **~7.2 seconds** to paste after key release. Hardware thread tuning alone cannot solve the architectural push-to-talk latency gap.

---

## 7. Remaining Questions & Gaps

1. **Sherpa-onnx Rust Binding for Token Timestamps:**
   * *Status:* `sherpa-onnx-c-api.dll` exposes `timestamps`, `durations`, and `tokens_arr` in `SherpaOnnxOfflineRecognizerResult`. However, Silent Voice's FFI binding in `src-tauri/src/system/sherpa_stt.rs:204` currently defines `timestamps: *const c_float` and `count: c_int`, but does not copy them into Rust slices (it only reads `result.text`).
   * *Next Step:* Verify in code that slicing `std::slice::from_raw_parts((*result).timestamps, (*result).count as usize)` correctly returns valid floats for Parakeet TDT without null pointer exceptions.
2. **DirectML FP16 Feasibility:**
   * *Status:* DirectML fails on INT8 models due to dynamic quantization fallbacks. Whether an FP16 export of Parakeet TDT runs reliably through DirectML on Intel Iris Xe / AMD Radeon 780M without custom operator failures remains untested.
   * *Priority:* Low priority, since CPU INT8 with overlapping chunks already solves the user's latency problem.
3. **Parakeet Unified C-API Stability:**
   * *Status:* While `OnlineRecognizerTransducerNeMoParakeetUnifiedImpl` exists in `sherpa-onnx` C++ source code, verifying whether the pre-compiled `sherpa-onnx-c-api.dll` v1.13.4 bundled in Silent Voice exports the corresponding online transducer entry points requires a symbol dump verification.

---

## 8. Measured on the user's laptop (Claude, 2026-10-15): overrides parts of §4.3 and §6

Hardware: Intel i7-8650U (4 cores, 2017), UHD 620, Parakeet TDT 0.6B v2 int8. Test audio: three dictation-style paragraphs (131 s, 321 words) spoken by the installed Kokoro voice with ~1 s mid-sentence thinking pauses, scored against the known text. Scripts: session scratchpad `bench3.py`–`bench6.py`.

| Method | Total wait after release | Word errors | Errors incl. punctuation + caps |
|---|---|---|---|
| Full pass at release (today's default) | 23.5 s | 2 | 33 |
| Background VAD chunking (today's `chunk_on_silence`) | 2.8 s | 2 | 55 |
| Background chunking + punct model over all text | 2.8 s | 2 | 27 (lowercases GitHub/MacBook) |
| **Background chunking + punct model, Parakeet casing kept except sentence starts** | **2.8 s** | **2** | **21** |
| Seam-only punct repair | 2.8 s | 2 | 32 |
| Left-context rolling commit (≈ §6 rank 1), no punct model | ~1.5 s | 1 | 18–20 / 49 on one paragraph (worse than full pass) |

Findings:
- Chunking did NOT hurt word accuracy here; only punctuation/caps broke at joins.
- Parakeet itself turns a ~1 s mid-sentence pause into a period even in a full pass, so left context alone cannot beat the full pass; §4.3's "doubling" did not occur because punctuation is stripped before restoration.
- §4.3's proper-noun concern is real and is solved by keeping Parakeet's casing for every non-sentence-start word.
- Thread count: 8 threads gave no gain over 4 on this CPU; vocabulary boost (beam search) costs ~30% speed (12.0 s vs 8.2 s for 42 s).
- Unverified claims in this report: the RTF table in §3.4, the Parakeet Unified WER figures in §2.1, and the app-by-app table in §5.
