import { describe, it, expect } from "vitest";
import { SETTINGS, type Ctx } from "./catalog";
import { resolveJumpTarget, searchSettings } from "./parts";
import { DEFAULT_SETTINGS } from "../../../stores/settingsStore";

function ctx(patch: Partial<Ctx["s"]> = {}, extra: Partial<Ctx> = {}): Ctx {
  return {
    s: { ...DEFAULT_SETTINGS, ...patch },
    set: () => {},
    onHotkey: () => {},
    hotkeyError: null,
    devices: [],
    gpuName: null,
    coeditReady: false,
    setCoeditReady: () => {},
    gectorReady: false,
    setGectorReady: () => {},
    toggleRule: () => {},
    ...extra,
  };
}

const byId = (id: string) => SETTINGS.find((x) => x.id === id)!;

describe("settings search", () => {
  it("finds settings by their old names via keywords", () => {
    const ids = searchSettings(SETTINGS, ctx(), "input sensitivity").map((x) => x.id);
    expect(ids).toEqual(["noise"]);
  });

  it("requires every word to match", () => {
    expect(searchSettings(SETTINGS, ctx(), "graphics").map((x) => x.id)).toContain("gpu");
    expect(searchSettings(SETTINGS, ctx(), "graphics banana")).toHaveLength(0);
  });

  it("still lists a hidden setting that has a parent to turn on", () => {
    // Underlines are off by default, so Oxford comma is hidden but findable.
    const ids = searchSettings(SETTINGS, ctx(), "oxford").map((x) => x.id);
    expect(ids).toEqual(["oxford"]);
  });

  it("every requires points at a real setting", () => {
    for (const x of SETTINGS.filter((s) => s.requires)) {
      expect(SETTINGS.some((s) => s.id === x.requires), `${x.id} → ${x.requires}`).toBe(true);
    }
  });
});

describe("jump target", () => {
  it("lands on the setting itself when it's visible", () => {
    expect(resolveJumpTarget(byId("oxford"), SETTINGS, ctx({ inline_proofread: true })).id).toBe("oxford");
  });

  it("lands on the parent while the setting is hidden", () => {
    expect(resolveJumpTarget(byId("oxford"), SETTINGS, ctx({ inline_proofread: false })).id).toBe("proofread");
  });

  it("walks up more than one level", () => {
    // Sensitivity needs the grammar model, which needs underlines on.
    const target = resolveJumpTarget(byId("gector-sens"), SETTINGS, ctx({ inline_proofread: false }));
    expect(target.id).toBe("proofread");
  });
});
