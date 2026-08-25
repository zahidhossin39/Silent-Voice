import { describe, it, expect } from "vitest";
import {
  parseRealtime,
  accuracyScore,
  deviceSpeedFactor,
  speedScore,
} from "./modelMetrics";
import type { HardwareInfo } from "../types";

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

describe("modelMetrics", () => {
  it("parseRealtime parses speed labels", () => {
    expect(parseRealtime("~10x realtime")).toBe(10);
    expect(parseRealtime("~0.5x realtime")).toBe(0.5);
    expect(parseRealtime("fast")).toBeNull();
  });

  it("accuracyScore computes accuracy bounded score", () => {
    expect(accuracyScore("~2%")).toBe(0.99);
    expect(accuracyScore("~16%")).toBe(0.3);
    const score = accuracyScore("~8%");
    expect(score).toBeGreaterThan(0.5);
    expect(score).toBeLessThan(0.6);
    expect(accuracyScore("unknown")).toBe(0.6);
  });

  it("deviceSpeedFactor computes speed factor and core limits", () => {
    expect(deviceSpeedFactor(null)).toBe(1);
    expect(deviceSpeedFactor(hw())).toBe(1);
    expect(deviceSpeedFactor(hw({ has_avx512: true }))).toBeGreaterThan(1);
    expect(deviceSpeedFactor(hw({ has_avx2: false }))).toBeLessThan(1);
    expect(deviceSpeedFactor(hw({ physical_cores: 64 }))).toBe(
      deviceSpeedFactor(hw({ physical_cores: 8 }))
    );
  });

  it("deviceSpeedFactor falls back to nominal clock when brand has no GHz", () => {
    expect(deviceSpeedFactor(hw({ cpu_brand: "Apple M1" }))).toBe(1);
  });

  it("speedScore stays within 0.05 and 1 inclusive", () => {
    const slow = speedScore("~0.05x realtime", hw());
    expect(slow).toBeGreaterThanOrEqual(0.05);
    expect(slow).toBeLessThanOrEqual(1);

    const fast = speedScore("~100x realtime", hw());
    expect(fast).toBeGreaterThanOrEqual(0.05);
    expect(fast).toBeLessThanOrEqual(1);

    // Clamping alone would still hold if the score were a constant — the bar
    // is only meaningful if a faster model actually scores higher.
    expect(fast).toBeGreaterThan(slow);
    expect(speedScore("~8x realtime", hw())).toBeGreaterThan(
      speedScore("~1x realtime", hw())
    );

    expect(speedScore("unparseable", hw())).toBe(0.5);
  });


});
