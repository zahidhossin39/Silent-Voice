import type {
  HardwareInfo,
  LlmModel,
  CompatibilityLevel,
} from "../types";
import { formatGB } from "./format";

export interface Compatibility {
  level: CompatibilityLevel;
  reason: string;
}

// Mirror of the Rust-side recommendation logic — build plan §6.
// Green = runs smoothly, Yellow = compatible but may be slow, Red = insufficient.

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


