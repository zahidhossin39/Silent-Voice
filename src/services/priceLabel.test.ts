import { describe, expect, it } from "vitest";
import { modelCost, priceLabel } from "../components/dashboard/providers/shared";

describe("model prices", () => {
  it("shows chat prices per million tokens", () => {
    expect(priceLabel({ id: "a", pricing: { prompt: "0.0000003", completion: "0.0000012" } }, false)).toBe("$0.30 in · $1.20 out per 1M tokens");
  });
  it("shows audio-only speech-to-text per hour of audio", () => {
    expect(priceLabel({ id: "w", pricing: { prompt: "0.00000333", completion: "0" } }, true)).toBe("$0.012 per hour of audio");
  });
  it("reads MAI-Transcribe's price as already per hour", () => {
    expect(priceLabel({ id: "microsoft/mai-transcribe-1.5", pricing: { prompt: "0.36", completion: "0" } }, true)).toBe("$0.36 per hour of audio");
    const whisper = { id: "w", pricing: { prompt: "0.0001", completion: "0" } }; // $0.36/hour per second
    expect(modelCost(whisper, true)).toBeCloseTo(modelCost({ id: "m", pricing: { prompt: "0.36", completion: "0" } }, true)!);
  });
  it("says Free, and gives no price when the provider has none or a per-request one", () => {
    expect(priceLabel({ id: "f", pricing: { prompt: "0", completion: "0" } }, false)).toBe("Free");
    expect(priceLabel({ id: "g" }, false)).toBeNull();
    expect(modelCost({ id: "auto", pricing: { prompt: "-1", completion: "-1" } })).toBeNull();
  });
});
