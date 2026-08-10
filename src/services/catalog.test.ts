import { describe, it, expect } from "vitest";
import { STT_MODELS, LLM_MODELS, isSherpaEngine } from "./catalog";

describe("STT_MODELS", () => {
  // Every other test here loops over the catalogue, so they would all pass
  // trivially if it were ever exported empty. Guard that first.
  it("is not empty", () => {
    expect(STT_MODELS.length).toBeGreaterThan(0);
    expect(LLM_MODELS.length).toBeGreaterThan(0);
  });

  // The Rust downloader derives the saved filename from this convention, so a mismatch means the app downloads a file it then cannot find.
  // Only applies to Whisper models — sherpa models (Moonshine, SenseVoice)
  // ship as a .tar.bz2 archive extracted into a folder, not a ggml .bin file.
  it("follows the ggml-<id>.bin filename convention", () => {
    for (const model of STT_MODELS) {
      if (isSherpaEngine(model.engine)) continue;
      expect(model.file, `Model ${model.id} filename convention`).toBe(`ggml-${model.id}.bin`);
    }
  });

  // Sherpa models must carry an archive URL (there's no base-URL fallback).
  it("sherpa models have an https archive url", () => {
    for (const model of STT_MODELS) {
      if (!isSherpaEngine(model.engine)) continue;
      expect(model.url, `Model ${model.id} archive url`).toBeTruthy();
      expect(model.url!.startsWith("https://")).toBe(true);
    }
  });

  it("has unique ids", () => {
    const ids = new Set(STT_MODELS.map((m) => m.id));
    expect(ids.size).toBe(STT_MODELS.length);
  });

  it("only uses https download urls", () => {
    for (const model of STT_MODELS) {
      if (model.url) {
        expect(model.url.startsWith("https://"), `Model ${model.id} URL`).toBe(true);
      }
    }
  });

  it("has sane numbers and non-empty labels", () => {
    for (const model of STT_MODELS) {
      expect(model.size_mb).toBeGreaterThan(0);
      expect(model.ram_mb).toBeGreaterThan(0);
      expect(model.id).not.toBe("");
      expect(model.file).not.toBe("");
      expect(model.label).not.toBe("");
      expect(model.speed_label).not.toBe("");
      expect(model.wer).not.toBe("");
    }
  });
});

describe("LLM_MODELS", () => {
  it("has unique ids", () => {
    const ids = new Set(LLM_MODELS.map((m) => m.id));
    expect(ids.size).toBe(LLM_MODELS.length);
  });

  it("only uses https download urls", () => {
    for (const model of LLM_MODELS) {
      expect(model.url.startsWith("https://"), `Model ${model.id} URL`).toBe(true);
    }
  });
});
