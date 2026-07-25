import { describe, it, expect } from "vitest";
import { sttCompatibility, llmCompatibility } from "./recommend";
import type { HardwareInfo, SttModel, LlmModel } from "../types";

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

function stt(over: Partial<SttModel> = {}): SttModel {
  return {
    id: "base.en",
    file: "ggml-base.en.bin",
    family: "Whisper",
    provider: "OpenAI",
    label: "Base",
    size_mb: 142,
    ram_mb: 500,
    speed_label: "~7x realtime",
    wer: "~5%",
    multilingual: false,
    preset: "balanced",
    best_for: "testing",
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

describe("sttCompatibility", () => {
  it("returns level 'warn' when hardware is null", () => {
    expect(sttCompatibility(stt(), null).level).toBe("warn");
  });

  it("returns 'bad' when model needs more RAM than available", () => {
    expect(
      sttCompatibility(stt({ ram_mb: 8000 }), hw({ available_ram_gb: 2 })).level
    ).toBe("bad");
  });

  it("returns 'warn' for a heavy model with no GPU", () => {
    expect(sttCompatibility(stt({ size_mb: 1500 }), hw()).level).toBe("warn");
  });

  it("returns 'good' for a heavy model when a capable GPU is present", () => {
    expect(
      sttCompatibility(stt({ size_mb: 1500 }), hw({ gpu_vram_gb: 8 })).level
    ).toBe("good");
  });

  it("returns 'good' for a small model on the default machine", () => {
    expect(sttCompatibility(stt(), hw()).level).toBe("good");
  });

  it("treats a weak GPU as no GPU", () => {
    expect(
      sttCompatibility(stt({ size_mb: 1500 }), hw({ gpu_vram_gb: 2 })).level
    ).toBe("warn");
  });
});

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
