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

/** A graphics card worth running models on: not Intel (built into the CPU,
 * shares system memory) and with ≥ 3 GB of its own memory. Mirrors `has_dgpu`
 * in hardware.rs. Intel chips report ~128 MB, which once read as "dedicated". */
export function hasDedicatedGpu(hw: HardwareInfo | null): boolean {
  return !!hw && (hw.gpu_vram_gb ?? 0) >= 3 && hw.gpu_vendor !== "Intel";
}

// Mirror of the Rust-side recommendation logic — build plan §6.
// Green = runs smoothly, Yellow = compatible but may be slow, Red = insufficient.

export function llmCompatibility(
  model: LlmModel,
  hw: HardwareInfo | null
): Compatibility {
  if (!hw) return { level: "warn", reason: "Scanning device…" };

  const hasGpu = hasDedicatedGpu(hw) && (hw.gpu_vram_gb ?? 0) >= 4;

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


