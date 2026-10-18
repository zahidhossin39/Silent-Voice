import { describe, expect, it } from "vitest";
import { transcriptParagraphs } from "./tauriBridge";

describe("transcriptParagraphs", () => {
  it("waits for the end of a sentence, like transcripts.rs", () => {
    const lines = Array.from({ length: 30 }, (_, i) => ({ start_ms: i * 3000, end_ms: i * 3000 + 2900, text: "and then it", speaker: "others" as const }));
    lines[7].text = "so that is how it works.";
    lines[8].text = 'Is that "clear?"';
    expect(transcriptParagraphs(lines).map((p) => p.start / 3000).slice(0, 3)).toEqual([0, 8, 20]);
  });
});
