import type { HardwareInfo } from "../types";

// Accuracy/speed metrics for STT models, shown as bars in the Model Store.
//
// Accuracy is intrinsic to the model (derived from its word-error-rate).
// Speed is DEVICE-DEPENDENT and computed from real hardware specs — not a
// hardcoded bar. The catalog's "~Nx realtime" figures assume a reference
// machine (4 physical cores, AVX2, ~2.5 GHz); we scale that by how this
// device compares, then map to a 0..1 bar.

// "~3.4%" → 3.4
function parseWer(wer: string): number | null {
  const m = wer.match(/([\d.]+)\s*%/);
  return m ? parseFloat(m[1]) : null;
}

// "~3x realtime" / "~0.5x realtime" → 3 / 0.5
export function parseRealtime(speedLabel: string): number | null {
  const m = speedLabel.match(/([\d.]+)\s*x/i);
  return m ? parseFloat(m[1]) : null;
}

// Base clock in GHz from a CPU brand string ("… @ 1.90GHz"), when present.
function parseClockGhz(cpuBrand: string): number | null {
  const m = cpuBrand.match(/@\s*([\d.]+)\s*GHz/i);
  return m ? parseFloat(m[1]) : null;
}

// Accuracy as 0..1 from WER (lower error = higher). ~2% WER (large models)
// lands near the top, ~8% (tiny) near the middle-low.
export function accuracyScore(wer: string): number {
  const w = parseWer(wer);
  if (w === null) return 0.6;
  const score = 1 - (w - 2) / 14; // 2% → 1.0, 16% → 0
  return Math.max(0.3, Math.min(0.99, score));
}

// How much faster/slower this device runs whisper.cpp vs the reference machine
// the catalog's realtime figures assume (reference = 1.0). whisper.cpp is
// CPU-bound, thread-scales well up to ~8 threads, gains from wider SIMD
// (AVX-512), and scales with clock. Missing clock falls back to a nominal
// 2.5 GHz so the estimate stays reasonable rather than zero.
export function deviceSpeedFactor(hw: HardwareInfo | null): number {
  if (!hw) return 1;
  const cores = Math.min(Math.max(hw.physical_cores, 1), 8);
  const simd = hw.has_avx512 ? 1.6 : hw.has_avx2 ? 1.0 : 0.55;
  const clock = parseClockGhz(hw.cpu_brand) ?? 2.5;
  const score = cores * clock * simd;
  const reference = 4 * 2.5 * 1.0; // = 10
  return score / reference;
}

// The model's realtime factor adjusted for THIS device (× realtime).
export function deviceRealtime(
  speedLabel: string,
  hw: HardwareInfo | null
): number | null {
  const base = parseRealtime(speedLabel);
  if (base === null) return null;
  return base * deviceSpeedFactor(hw);
}

// Speed as 0..1 for the bar, from the device-adjusted realtime factor on a log
// scale: ~0.2x realtime ≈ empty, ~10x realtime ≈ full.
export function speedScore(
  speedLabel: string,
  hw: HardwareInfo | null
): number {
  const rt = deviceRealtime(speedLabel, hw);
  if (rt === null) return 0.5;
  const lo = Math.log2(0.2);
  const hi = Math.log2(10);
  const s = (Math.log2(Math.max(rt, 0.05)) - lo) / (hi - lo);
  return Math.max(0.05, Math.min(1, s));
}

// Human caption for the device-adjusted speed, e.g. "≈5.3x realtime".
export function deviceRealtimeLabel(
  speedLabel: string,
  hw: HardwareInfo | null
): string | null {
  const rt = deviceRealtime(speedLabel, hw);
  if (rt === null) return null;
  const rounded = rt >= 10 ? Math.round(rt) : Math.round(rt * 10) / 10;
  return `≈${rounded}x realtime on your device`;
}

// Quality tracks param count, but sub-linearly — a 4B is not four times as good
// as a 1B. A log scale over the range the catalog actually spans (~0.5B-14B)
// keeps the small models this app ships with visually distinguishable; a linear
// scale against a large ceiling squashes them all into an identical stub.
export function llmQualityScore(paramsStr: string): number {
  const params = parseFloat(paramsStr);
  if (isNaN(params) || params <= 0) return 0.5;
  const lo = Math.log2(0.5);
  const hi = Math.log2(14);
  const score = (Math.log2(params) - lo) / (hi - lo);
  return Math.max(0.1, Math.min(0.99, score));
}

// Coarse 3-step speed score for LLMs using compatibility level
export function llmSpeedScore(level: string): number {
  if (level === "good") return 0.8;
  if (level === "warn") return 0.4;
  return 0.1;
}

export function llmSpeedLabel(level: string): string {
  if (level === "good") return "fast on your device";
  if (level === "warn") return "may be slow";
  return "too heavy for device";
}

// TTS quality (naturalness) based on tier
export function ttsNaturalnessScore(quality: string): number {
  if (quality === "natural") return 0.9;
  if (quality === "balanced") return 0.6;
  if (quality === "fast") return 0.3;
  return 0.5;
}

export function ttsNaturalnessLabel(quality: string): string {
  if (quality === "natural") return "closest to a human voice";
  if (quality === "balanced") return "natural enough for most text";
  if (quality === "fast") return "clear, noticeably synthetic";
  return quality;
}

// TTS speed based on tier (fast = highest score)
export function ttsSpeedScore(quality: string): number {
  if (quality === "fast") return 0.9;
  if (quality === "balanced") return 0.6;
  if (quality === "natural") return 0.3;
  return 0.5;
}

export function ttsSpeedLabel(quality: string): string {
  if (quality === "fast") return "speaks almost instantly";
  if (quality === "balanced") return "starts speaking quickly";
  if (quality === "natural") return "takes a moment to start";
  return quality;
}
