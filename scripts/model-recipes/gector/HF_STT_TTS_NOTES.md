# Hugging Face STT & TTS Deep Research Notes

This document contains deep research findings on integrating Hugging Face model browsing for Speech-to-Text (whisper.cpp) and Text-to-Speech (Piper) tracks, strictly verified against the live HF API.

## TRACK 1 - Speech-to-Text (whisper.cpp)

### 1. Canonical Repo (`ggerganov/whisper.cpp`)
Verified via `https://huggingface.co/api/models/ggerganov/whisper.cpp?blobs=true`. The repo hosts official ggml converted bins across all major Whisper variants, including `.en` (English-only) and quantized (`q5_0`, `q5_1`, `q8_0`) files.

**Actual `ggml-*.bin` files and sizes:**
*   **tiny**: `ggml-tiny.bin` (77 MB), `ggml-tiny.en.bin` (77 MB)
    *   Quantized: `ggml-tiny-q5_1.bin` (32 MB), `ggml-tiny-q8_0.bin` (43 MB)
*   **small**: `ggml-small.bin` (487 MB), `ggml-small.en.bin` (487 MB)
    *   Quantized: `ggml-small-q5_1.bin` (190 MB), `ggml-small-q8_0.bin` (264 MB)
*   **base**: `ggml-base.bin` (147 MB), `ggml-base.en.bin` (147 MB)
    *   Quantized: `ggml-base-q5_1.bin` (59 MB), `ggml-base-q8_0.bin` (81 MB)
*   **medium**: `ggml-medium.bin` (1.53 GB), `ggml-medium.en.bin` (1.53 GB)
    *   Quantized: `ggml-medium-q5_0.bin` (539 MB), `ggml-medium-q8_0.bin` (823 MB)
*   **large-v1/v2/v3**: `ggml-large-v1.bin` (3.09 GB), `ggml-large-v2.bin` (3.09 GB), `ggml-large-v3.bin` (3.09 GB)
    *   Quantized (v2/v3): `ggml-large-v2-q5_0.bin` (1.08 GB), `ggml-large-v2-q8_0.bin` (1.65 GB), `ggml-large-v3-q5_0.bin` (1.08 GB)
*   **turbo**: `ggml-large-v3-turbo.bin` (1.62 GB)
    *   Quantized: `ggml-large-v3-turbo-q5_0.bin` (574 MB), `ggml-large-v3-turbo-q8_0.bin` (874 MB)

*Note: Distil-whisper is not in the canonical repo, but can be found via search (see below).*

### 2. Discovering Other Compatible Repos
Verified via `https://huggingface.co/api/models?search=whisper.cpp`.
*   **Does search work?** Yes, searching for `whisper.cpp` reliably surfaces community ggml ports. 
*   **Other well-known ggml repos discovered:**
    *   `bobqianic/whisper.cpp-distilled` (Distil-whisper ggml conversions)
    *   `Intel/whisper.cpp-openvino-models`
    *   `techiaith/whisper-base-ft-commonvoice-cy-cpp` (Welsh fine-tunes)
    *   `mobilint/whisper.cpp` (multilingual ggml variants)
*   **Reliable Discovery Strategy:** Query the search API (`search=whisper.cpp` or filter by tags like `whisper.cpp` / `ggml`). For each result, hit `api/models/{repo}?blobs=true` and client-side filter the `siblings` array for `rfilename` matching `ggml-*.bin`. This guarantees you are only parsing repos that actually have the compatible files.

### 3. Filename to Language/Quality Mapping
Based on standard naming conventions across these repos:
*   **Language Coverage:**
    *   Files containing `.en` (e.g., `ggml-base.en.bin`) are **English-only** models.
    *   Files without `.en` (e.g., `ggml-base.bin`) are **Multilingual**.
*   **Quality Tier / Performance:**
    *   Quality scales with size: `tiny` < `small` < `base` < `medium` < `large`. 
    *   `turbo` (`large-v3-turbo`) offers near-large quality with much faster inference.
    *   Files containing `-q5_*` or `-q8_0` are **quantized**. They trade a slight drop in accuracy for a significant reduction in file size (e.g., base goes from 147MB to 59MB) and lower RAM usage.

---

## TRACK 2 - Text-to-Speech (Piper)

### 1. The `rhasspy/piper-voices` Repo & Index
*   **Machine-Readable Index:** CRITICALLY, a `voices.json` file **does exist** at the repo root.
    *   URL: `https://huggingface.co/rhasspy/piper-voices/raw/main/voices.json`
*   **Directory Layout:** Files are structured strictly as `{language_family}/{language_code}/{voice_name}/{quality}/...`
*   **Exact JSON Schema:**
    The root is an object where keys are the unique voice identifiers (e.g., `ar_JO-kareem-low`).
    ```json
    {
        "ar_JO-kareem-low": {
            "key": "ar_JO-kareem-low",
            "name": "kareem",
            "language": {
                "code": "ar_JO",
                "family": "ar",
                "region": "JO",
                "name_native": "العربية",
                "name_english": "Arabic",
                "country_english": "Jordan"
            },
            "quality": "low",
            "num_speakers": 1,
            "speaker_id_map": {}, 
            "files": {
                "ar/ar_JO/kareem/low/ar_JO-kareem-low.onnx": {
                    "size_bytes": 63201294,
                    "md5_digest": "d335cd06fe4045a7ee9d8fb0712afaa9"
                },
                "ar/ar_JO/kareem/low/ar_JO-kareem-low.onnx.json": {
                    "size_bytes": 5022,
                    "md5_digest": "465724f7d2d5f2ff061b53acb8e7f7cc"
                }
            },
            "aliases": []
        },
        "bn_BD-google-medium": {
             // Example of multi-speaker voice
             "num_speakers": 16,
             "speaker_id_map": {
                 "00737": 0,
                 "01232": 1
             }
             // ...
        }
    }
    ```

### 2. Download URL Pattern & Typical Sizes
*   **URL Pattern:** `https://huggingface.co/rhasspy/piper-voices/resolve/main/{file_path_from_json}`
    *   *Example:* `https://huggingface.co/rhasspy/piper-voices/resolve/main/ar/ar_JO/kareem/low/ar_JO-kareem-low.onnx`
*   **Typical Sizes (ONNX binary):**
    *   `x_low`: ~20 MB - 28 MB (e.g., `ca_ES-upc_ona-x_low.onnx` is 20.6 MB)
    *   `low`: ~63 MB (e.g., `ar_JO-kareem-low.onnx` is 63.2 MB)
    *   `medium`: ~63 MB - 77 MB (e.g., `de_DE-mls-medium.onnx` is 76.9 MB)
    *   `high`: *[ASSUMPTION]* Likely larger (>80MB) and computationally heavier, though less frequent in the index compared to low/medium.

### 3. Alternative Piper Voice Repos
Verified via `https://huggingface.co/api/models?search=piper`.
There is a vibrant ecosystem of alternative repos hosting custom Piper voices. By querying models with tags `piper` and `onnx`, you can discover:
*   `csukuangfj/*`: A massive collection of individual repos hosting specific VITS Piper conversions (e.g., `csukuangfj/vits-piper-en_US-glados`, `csukuangfj/vits-piper-en_US-amy-medium`).
*   `campwill/HAL-9000-Piper-TTS`: Novelty pop-culture voices.
*   `MahtaFetrat/Mana-Persian-Piper`: Regional/Language specific datasets not in the main repo.
*   `eduardem/piper-tts-romanian`: Romanian fine-tunes.

**Integration Note:** These alternative repos do *not* share a single centralized `voices.json` index. Your app would need to either build a custom scraper to parse their file trees (`?blobs=true` looking for `.onnx` and `.onnx.json` pairs) or stick to the `rhasspy/piper-voices` `voices.json` for guaranteed structured integration.
