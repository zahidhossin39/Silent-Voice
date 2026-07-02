import type {
  HardwareInfo,
  SttModel,
  LlmModel,
  CompatibilityLevel,
} from "../types";
import { formatMB, formatGB } from "./format";

export interface Compatibility {
  level: CompatibilityLevel;
  reason: string;
}

// Mirror of the Rust-side recommendation logic — build plan §6.
// Green = runs smoothly, Yellow = compatible but may be slow, Red = insufficient.

export function sttCompatibility(
  model: SttModel,
  hw: HardwareInfo | null
): Compatibility {
  if (!hw) return { level: "warn", reason: "Scanning device…" };

  const ramNeededGb = model.ram_mb / 1024;
  const hasGpu = !!hw.gpu_vram_gb && hw.gpu_vram_gb >= 4;

  if (hw.available_ram_gb < ramNeededGb) {
    return {
      level: "bad",
      reason: `Needs ~${formatMB(model.ram_mb)} RAM, only ${formatGB(
        hw.available_ram_gb
      )} free`,
    };
  }

  // Large / medium models are slow on CPU without a GPU
  const heavy = model.size_mb >= 1400;
  if (heavy && !hasGpu) {
    return {
      level: "warn",
      reason: `Runs, but slow on CPU (${model.speed_label}). A GPU helps.`,
    };
  }

  return {
    level: "good",
    reason: `Runs smoothly — ${model.speed_label}, ~${formatMB(
      model.ram_mb
    )} RAM`,
  };
}

export function llmCompatibility(
  model: LlmModel,
  hw: HardwareInfo | null
): Compatibility {
  if (!hw) return { level: "warn", reason: "Scanning device…" };

  const hasGpu = !!hw.gpu_vram_gb && hw.gpu_vram_gb >= 4;

  if (hw.total_ram_gb < model.ram_gb) {
    return {
      level: "bad",
      reason: `Needs ~${formatGB(model.ram_gb)} RAM, you have ${formatGB(
        hw.total_ram_gb
      )}`,
    };
  }

  if (model.tier === "large" && !hasGpu) {
    return {
      level: "bad",
      reason: "Needs a powerful GPU to be usable",
    };
  }

  if ((model.tier === "medium" || model.tier === "small") && !hasGpu) {
    return {
      level: "warn",
      reason: `Runs on CPU but slow — a GPU is recommended`,
    };
  }

  return {
    level: "good",
    reason: `Runs smoothly — ${model.speed_label}`,
  };
}

export function badgeText(level: CompatibilityLevel): string {
  switch (level) {
    case "good":
      return "Recommended";
    case "warn":
      return "Compatible";
    case "bad":
      return "Not recommended";
  }
}
