# Research Report: Desert Ant Labs Evaluation for Silent Voice

**Target Document:** `docs/research-desertant.md`  
**Primary URLs Inspected:**  
- Main & Architecture: `https://desertant.com/`, `https://desertant.com/about/`, `https://desertant.com/press/`, `https://desertant.com/docs/`, `https://desertant.com/catalog.json`, `https://desertant.com/llms.txt`  
- Model Profiles: `https://desertant.com/models/{voz,clear,align,uhm,redact,ear,clips}/`  
- Licensing & Legal: `https://license.desertant.com/` (v1.0, 3 July 2026), `https://license.desertant.com/attribution`  
- Source & Repos: `https://github.com/Desert-Ant-Labs/desert-ant-core`, `https://huggingface.co/desert-ant-labs`

---

## 1. What is Desert Ant?

- **Company & Founders:** Desert Ant Labs B.V. is an on-device AI startup incorporated in Amsterdam, Netherlands (KvK 42136051, August 2026). Founded by Paul Veugen (CEO), Laurier Rochon (CTO), and Fredrik Wallin (Engineering) — creators of Human (acquired by Mapbox 2016), Mapbox mobile SDKs, Detail (Apple iPad App of the Year 2025), and Subwave.
- **Product Offerings:** 12 available on-device models + 6 closed-beta models:
  - *Audio:* **Voz** (STT), **Clear** (speech enhancement/dereverb), **Align** (word timestamp refiner), **Uhm** (acoustic filler detection), **Ear** (spoken language ID), **Clips** (transcript highlight extraction), **Who** (speaker diarization, beta).
  - *Text:* **Redact** (PII masking), **Tongue** (text language ID), **Emo** (emoji suggestions), **Gist** (topic tagging), **Title** (summarization), **Schemer** (structured JSON, beta), **Toxic** (hate speech triage, beta).
  - *Vision:* **Shapes** (single-stroke shape recognition), **Moderator** (NSFW filter, beta), **Eye** (frame scoring, beta), **Face** (matching, beta).
  - *SDKs:* `desert-ant-core` supporting Swift (Core ML for iOS/macOS), Kotlin (LiteRT for Android), and JavaScript/TypeScript (LiteRT.js / ONNX Runtime for Web/Node). CLI tool `desert-ant-cli` for macOS/Linux.
- **Is it Open Source?** **No.** It uses a proprietary *Source-Available License v1.0* (`LicenseRef-DAL-Source-Available-1.0`). Key restrictions:
  1. *Mandatory Telemetry (Sec 6f & 7):* Hardcoded daily/monthly HTTP beacons (`UsageClient.swift`) reporting `deviceId` to meter Monthly Active Devices (MAD). Disabling telemetry is explicitly forbidden with no cure period.
  2. *Attribution (Sec 9):* Mandatory user-visible credit ("Powered by Desert Ant Labs" linked to `desertant.com`).
  3. *Non-Compete (Sec 6):* Prohibits using models, outputs, or logs to train, fine-tune, or distill competing models.
  4. *Redistribution:* Prohibits standalone sublicensing or distribution.
- **Pricing:** Free tier up to **100,000 MAD** per model per platform. Above 100k MAD, commercial licensing is required via sales negotiation (`licensing@desertant.com`); no public pricing schedule exists (404 on `/pricing/`).

---

## 2. Usability Analysis for Silent Voice

Silent Voice stack: Free, local-first Tauri v2 (Rust backend, React/TS frontend) push-to-talk dictation on laptop CPUs (e.g., Intel i7-8650U, no discrete GPU). Already includes Parakeet TDT 0.6B, Whisper (whisper.cpp), Moonshine, SenseVoice (via `sherpa-onnx`), local LLM (`llama.cpp`), and GECToR.

| Candidate | Underlying Technology | Fit for Silent Voice Stack | License / Cost | Offline? | Effort | Benefit for Silent Voice |
|---|---|---|---|---|---|---|
| **Voz** (STT) | NVIDIA Parakeet TDT 0.6B v3 (palettized) | **Zero / Redundant.** No Rust crate. Windows path requires DirectML GPU; CPU path is Node-only. Enforces 15s window padding. | Source-Available (100k MAD cap, telemetry) vs Upstream CC BY 4.0 | Weights offline; license mandates periodic telemetry | High | **None.** Silent Voice already runs Parakeet TDT in native Rust via `sherpa-onnx` without padding overhead. |
| **Clear** (Enhance) | Fine-tuned DeepFilterNet 3 (DFN3-half) | Poor via DAL SDK. Upstream DeepFilterNet has an official **native Rust crate (`deep_filter`)**. | DAL: Proprietary. Upstream: **MIT / Apache-2.0**. | 100% offline | High via DAL; Low via `deep_filter` | **Low.** Pre-filtering raw audio on a dual/quad-core 15W CPU adds dictation latency; Parakeet already resists noise. |
| **Align** (Timestamps) | 0.7MB LiteRT/CoreML model (Qwen3-ForcedAligner base) | Incompatible (requires LiteRT/CoreML, designed for Apple `SpeechTranscriber`). | Source-Available | Yes | High | **None.** Silent Voice pastes text directly to cursor; Parakeet & whisper.cpp already produce token timestamps. |
| **Uhm** (Filler Words) | 45MB acoustic DistilHuBERT classifier | Swift/CoreML only (Apple). No Windows CPU runtime. | Source-Available | Yes (Apple only) | Very High | **None.** Silent Voice removes fillers in text faster and with zero audio latency via `llama.cpp` or GECToR. |
| **Redact** (PII Masking) | 23M-param MiniLM-L12 (6 layers) + regex checksums | Feasible via ONNX or JS in Tauri webview. | Source-Available vs Upstream MiniLM (MIT) | Yes | Medium | **Low.** Dictation is local push-to-cursor; no external API transmission occurs where redaction is necessary. |
| **Ear** (Spoken Lang ID) | Subgraph of `openai/whisper-tiny` | Redundant (SenseVoice and Whisper already perform language identification). | Source-Available vs Upstream MIT | Yes | High | **None.** |

### Ideas & Competitor Techniques Worth Borrowing:
1. **Algorithmic Verification Layer:** Redact's hybrid pattern of combining text extraction with deterministic checksum algorithms (Luhn for credit cards, ISO-13616 Mod 97-10 for IBANs, EU VAT checksums) is an excellent pattern to copy if Silent Voice ever adds regex/PII masking rules. These are standard open algorithms easily written in pure Rust.
2. **Upstream DeepFilterNet 3 (`deep_filter`):** If users ever request an optional background noise filter, integrate Hendrik Schröter's official `deep_filter` Rust crate directly from crates.io under MIT/Apache-2.0, completely bypassing Desert Ant Labs.

---

## 3. Marketing Claims Without Substantiated Evidence

1. ~~**Fictitious Future Hardware Benchmarks:**~~ *(Claude correction: wrong. iPhone 17/18 Pro, M5 and iOS/macOS 26 are all released as of 2026-10; Gemini used stale knowledge.)* Benchmark tables on `desertant.com` and Hugging Face claim empirical measurements on unreleased hardware: *"iPhone 17 Pro"*, *"iPhone 18 Pro"*, *"MacBook Pro M5"*, running on *"iOS 26"* and *"macOS 26"*. These are simulated or extrapolated figures masquerading as measured physical hardware data.
2. **"14x Faster than Whisper" (Voz):** Misleading cherry-picked comparison. Compares a non-autoregressive 0.6B transducer (Parakeet TDT) against a 1.5B autoregressive transformer (Whisper large-v3-turbo) on long 10-minute files on Apple Silicon. For short push-to-talk dictation clips (1–5s), speed plummets to 50–62x because Voz forces a 15-second mel window padding penalty.
3. **"470x Less Energy than Claude Sonnet" (Clips):** Compares a tiny 100M parameter on-device highlight classifier running for 2.4s at an assumed 3W against a theoretical datacenter inference of Claude 3.5 Sonnet (a 70B+ LLM) based on extrapolated MLPerf numbers.
4. **"Proprietary Frontier On-Device Intelligence":** Obscures that almost the entire model library consists of repackaged or fine-tuned community open-source models: Voz (NVIDIA Parakeet TDT), Clear (DeepFilterNet 3), Ear (OpenAI Whisper-tiny), Redact (Microsoft MiniLM), Align (Qwen3-ForcedAligner / wav2vec 2.0), Title (IBM Granite 4.0), Clips (XLM-RoBERTa).
5. **"Private by Default / Nothing Leaves Device" vs Mandatory Telemetry:** Despite privacy-heavy branding, the license (Section 6f & 7) explicitly forbids disabling telemetry turnstiles, and `UsageClient.swift` transmits periodic HTTP beacons with persistent `deviceId` identifiers. Disabling telemetry triggers immediate license termination without a cure period.
6. **Unsubstantiated Cross-Lingual Transfer:** Advertises Uhm as transferring from English to Spanish, French, German, and Dutch, but the documentation admits transfer *"has not been separately measured against per-language ground truth"*.

---

## 4. Verdict: IGNORE IT

**Verdict:** **Ignore Desert Ant Labs entirely.**

**Detailed Rationale:**
1. **Zero Technical Gain & Architectural Mismatch:** Desert Ant provides no native Rust crate, C-FFI, or Windows CPU engine. Voz on Windows requires DirectML GPU (incompatible with low-power CPU-only Intel UHD 620). Silent Voice already runs the exact same model (Parakeet TDT 0.6B) in native Rust via `sherpa-onnx` without 15s window padding.
2. **License Poison Pill & Telemetry Obligation:** The Source-Available license imposes a 100k MAD cap, non-compete terms, mandatory user-facing branding, and non-tamperable telemetry turnstiles. Shipping DAL violates Silent Voice's core promise as an open, private, air-gapped, offline utility.
3. **Upstream Open Source is Strictly Superior:** Every useful underlying model is already available upstream under permissive licenses (MIT, Apache 2.0, CC BY 4.0). If Silent Voice ever requires audio denoising, Hendrik Schröter's `deep_filter` Rust crate provides native, zero-dependency, MIT-licensed integration without Desert Ant's legal overhead.
