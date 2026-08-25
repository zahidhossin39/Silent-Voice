import { describe, it, expect } from "vitest";
import { llmCompatibility } from "./recommend";
import type { HardwareInfo, LlmModel } from "../types";

function hw(over: Partial<HardwareInfo> = {}): HardwareInfo {
  return {
    cpu_brand: "Intel(R) Core(TM) i5 @ 2.50GHz",
    physical_cores: 4,
    logical_cores: 8,
    total_ram_gb: 16,
    available_ram_gb: 8,
    has_avx2: true,
    has_avx512: false,
    gpu_vendor: null,
    gpu_name: null,
    gpu_vram_gb: null,
    free_disk_gb: 100,
    os: "Windows",
    ...over,
  };
}

function llm(over: Partial<LlmModel> = {}): LlmModel {
  return {
    id: "test",
    name: "Test",
    provider: "Test",
    url: "https://example.com/m.gguf",
    params: "3B",
    size_mb: 2000,
    ram_gb: 4,
    tier: "small",
    speed_label: "fast",
    languages: "en",
    license: "MIT",
    best_for: "testing",
    ...over,
  };
}

describe("llmCompatibility", () => {
  it("returns 'warn' when hardware is null", () => {
    expect(llmCompatibility(llm(), null).level).toBe("warn");
  });

  it("returns 'bad' when model needs more total RAM than machine has", () => {
    expect(llmCompatibility(llm({ ram_gb: 64 }), hw()).level).toBe("bad");
  });

  it("returns 'bad' for a 'large' tier model with no GPU", () => {
    expect(llmCompatibility(llm({ tier: "large" }), hw()).level).toBe("bad");
  });

  it("returns 'warn' for a 'small' tier model with no GPU", () => {
    expect(llmCompatibility(llm({ tier: "small" }), hw()).level).toBe("warn");
  });

  it("returns 'good' for a 'large' tier model with capable GPU", () => {
    expect(
      llmCompatibility(llm({ tier: "large" }), hw({ gpu_vram_gb: 8 })).level
    ).toBe("good");
  });
});
