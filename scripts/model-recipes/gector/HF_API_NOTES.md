# Hugging Face API Research for GGUF Local Model Browser

This document outlines the findings for integrating a Hugging Face model browser (LM Studio style) into a Tauri application. All API shapes have been verified against real endpoints.

## 1. Search Endpoint
**Endpoint:** `GET https://huggingface.co/api/models`

**Query Parameters:**
- `search=<query>` (e.g., `search=llama`) - Text search query.
- `filter=gguf` - Filters results to only include repos tagged with `gguf`. This is the most reliable way to find GGUF repos (over `library=gguf`, which HF sometimes aliases, but `filter=gguf` maps directly to tags).
- `sort=<field>` - Options include `downloads`, `likes`, `lastModified`, `trending`.
- `direction=-1` - For descending order (highest downloads first).
- `limit=20` - Number of results to return.

**Example Request:** `https://huggingface.co/api/models?search=llama&filter=gguf&sort=downloads&direction=-1&limit=2`

**Fields Returned:**
The standard response includes `_id`, `id` (repo name), `likes`, `private`, `downloads`, `tags`, `pipeline_tag`, `createdAt`, and `modelId`.
*Note: Fields like `gated` and `lastModified` might not appear in the compact search response by default and may require `&full=true` or fetching the model details.*

**Real Sample Response Snippet:**
```json
[
  {
    "_id": "66f42ee6c27b6621b245d4e8",
    "id": "hugging-quants/Llama-3.2-1B-Instruct-Q8_0-GGUF",
    "likes": 50,
    "private": false,
    "downloads": 551968,
    "tags": [
      "gguf",
      "llama-3",
      "text-generation",
      "en",
      "base_model:meta-llama/Llama-3.2-1B-Instruct"
    ],
    "pipeline_tag": "text-generation",
    "createdAt": "2024-09-25T15:40:22.000Z",
    "modelId": "hugging-quants/Llama-3.2-1B-Instruct-Q8_0-GGUF"
  }
]
```

## 2. Model Details
**Endpoint:** `GET https://huggingface.co/api/models/{repo_id}`

For GGUF repos, HF provides a special `gguf` metadata object extracted directly from the files, as well as a `siblings` array for the file list.

**Without sizes (Default):**
Calling `https://huggingface.co/api/models/bartowski/Qwen2.5-7B-Instruct-GGUF` returns the siblings array with just the filenames:
```json
"siblings": [
  {"rfilename": ".gitattributes"},
  {"rfilename": "Qwen2.5-7B-Instruct-Q4_K_M.gguf"},
  {"rfilename": "README.md"}
]
```

**With sizes (`?blobs=true`):**
Calling `https://huggingface.co/api/models/bartowski/Qwen2.5-7B-Instruct-GGUF?blobs=true` adds size and sha256 data, critical for UI display:
```json
"siblings": [
  {
    "rfilename": "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
    "blobId": "2fcc22e848744755ba4f9d41f8273cf45813dca3",
    "size": 4683074240,
    "lfs": {
      "sha256": "65b8fcd92af6b4fefa935c625d1ac27ea29dcb6ee14589c55a8f115ceaaa1423",
      "size": 4683074240,
      "pointerSize": 135
    }
  }
]
```

**GGUF Metadata Object (`gguf`):**
HF parses the primary GGUF file and exposes its metadata under the `gguf` key.
```json
"gguf": {
  "total": 7615616512,
  "architecture": "qwen2",
  "context_length": 32768,
  "chat_template": "{%- if tools %}\n    {{- '<|im_start|>system\\n'... (full Jinja template)",
  "bos_token": "<|endoftext|>",
  "eos_token": "<|im_end|>",
  "totalFileSize": 15237853600
}
```

*Note: For GGUF repos, `safetensors` params field is usually absent; instead, the `gguf` object is present.*

## 3. README and Model Card
**Endpoint:** `GET https://huggingface.co/{repo_id}/raw/main/README.md`

You can fetch the raw Markdown file directly. The file starts with YAML frontmatter containing metadata about the repo.

**Real YAML Frontmatter Snippet:**
```yaml
---
base_model: Qwen/Qwen2.5-7B-Instruct
language:
- en
license: apache-2.0
license_link: https://huggingface.co/Qwen/Qwen2.5-7B-Instruct/blob/main/LICENSE
pipeline_tag: text-generation
tags:
- chat
quantized_by: bartowski
---
```
This is useful for extracting the exact `base_model` to link back to the original non-quantized repo, and for extracting the `license`.

## 4. Capability Detection (Vision, Tool Use, Reasoning)
Because HF tags are user-submitted, capability detection relies on a mix of tags and metadata heuristics. Be honest: this is mostly **educated guesswork**.

- **Vision (Multimodal):**
  - **Reliable:** Check `siblings` for files containing `mmproj` (the multimodal projector weights, e.g., `*mmproj*.gguf`). If present, it's a vision model.
  - **Tags:** Look for `pipeline_tag: image-text-to-text` or tags like `vision`, `multimodal`.
- **Tool Use (Function Calling):**
  - **Metadata:** Inspect `gguf.chat_template`. If it contains strings like `{{- if tools }}`, `<tools>`, or `tool_call`, the model supports tool use.
  - **Tags:** Look for `tool-use` or `function-calling`.
- **Reasoning (Thinking Models):**
  - **Tags:** Look for `reasoning`, `thinking`, or if the `base_model` string contains `DeepSeek-R1` or similar known reasoning families.
  - **Assumption:** No standard GGUF internal key guarantees reasoning capabilities yet; it's entirely convention-based.

## 5. Quantization File Naming Conventions
GGUF files use standard suffixes to denote the quantization method.

**Common Quants (Ranked roughly by Quality/Size tradeoff):**
1. **F16 / F32:** Unquantized (huge, max quality).
2. **Q8_0:** 8-bit (virtually indistinguishable from F16, very large).
3. **Q6_K:** 6-bit (excellent quality, recommended if RAM allows).
4. **Q5_K_M:** 5-bit (great balance, recommended).
5. **Q4_K_M:** 4-bit (**The standard recommended default**). It hits the sweet spot—reducing file size by ~60% while maintaining near-perfect perplexity. If unsure, download this.
6. **IQ4_XS / IQ3_M:** "I-quants" (Imatrix). Newer formats optimized for better quality at lower bitrates (especially good for 3-4 bit).
7. **Q3_K_M / Q2_K:** 3-bit and 2-bit (noticeable degradation, use only if severely RAM-constrained).

**Multi-part GGUFs:**
Models exceeding ~50GB are split into chunks. They follow the naming convention:
`model-name-Q8_0-00001-of-00002.gguf`
`model-name-Q8_0-00002-of-00002.gguf`
*Rule:* To run a multi-part model, all chunks of that specific quant must be downloaded to the same directory. The inference engine will load the `-00001-of-*` file and automatically find the rest.

## 6. Fit Heuristics (RAM & Performance)
To show "Fit on device" badges, use these heuristics based on the community/LM Studio standards:

**RAM Estimation:**
- `Estimated RAM Required = File Size * 1.2` (Assumption: +20% overhead for context window KV cache and inference engine overhead).

**3-Tier Fit Rule:**
Given `Total System RAM`, `Available System RAM`, and `GPU VRAM`:
1. **Fits Comfortably (Green):** `Estimated RAM < (Available RAM * 0.8)` (Leaves breathing room for OS).
2. **Tight Fit (Yellow):** `Estimated RAM < Available RAM` (Will fit, but might swap or slow down system).
3. **Won't Fit (Red):** `Estimated RAM > Available RAM` (Will crash or aggressively page to disk).
*Note:* If offloading to GPU, check if `Estimated RAM < GPU VRAM` for maximum speed.

**Tokens/Sec Expectation (Rough Heuristic):**
- **Consumer CPU (e.g., M1/M2 Mac, modern Ryzen/Intel):** 7B model -> ~10-20 t/s. 70B model -> ~1-3 t/s.
- **Consumer GPU (e.g., RTX 3060/4060 8GB):** 7B model (fully offloaded) -> ~40-70 t/s.

## 7. Rate Limits & Etiquette
- **Authentication:** The HF `api/models` and raw file endpoints are public and do not require an API token for public repos.
- **Rate Limits:** Unauthenticated API calls are generally subject to a rate limit of around 10-20 requests per second. Heavy scraping will result in HTTP 429 Too Many Requests.
- **Etiquette (CRITICAL):** HF asks developers building apps on their API to include a descriptive `User-Agent` header.
  - **Example:** `User-Agent: MyTauriApp/1.0 (contact@example.com)`
  - Do not use generic agents (like standard `fetch` or `axios` defaults) as they are more likely to be throttled if a spam wave hits.
