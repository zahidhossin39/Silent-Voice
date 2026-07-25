import { describe, it, expect } from "vitest";
import { parseFragment, type DefaultTreeAdapterTypes } from "parse5";
import { markdownToHtml } from "./SimpleMarkdown";

// parse5 is used rather than a string match because the old sanitizer produced strings that LOOK inert but that a real HTML parser still turns into a live element, so only real parsing proves it is safe.
function dangerousNodes(html: string): string[] {
  const fragment = parseFragment(html);
  const dangerousTags = new Set(["script", "iframe", "object", "embed", "img", "svg"]);
  const results: string[] = [];

  function walk(node: DefaultTreeAdapterTypes.Node) {
    if ("tagName" in node && typeof node.tagName === "string") {
      const tag = node.tagName.toLowerCase();
      const hasOnAttr = Array.isArray(node.attrs) && node.attrs.some(attr => attr.name.toLowerCase().startsWith("on"));
      if (hasOnAttr || dangerousTags.has(tag)) {
        results.push(`<${node.tagName}>`);
      }
    }
    if ("childNodes" in node && Array.isArray(node.childNodes)) {
      for (const child of node.childNodes) {
        walk(child);
      }
    }
    if ("content" in node && node.content && "childNodes" in node.content) {
      for (const child of node.content.childNodes) {
        walk(child as DefaultTreeAdapterTypes.Node);
      }
    }
  }

  for (const child of fragment.childNodes) {
    walk(child);
  }

  return results;
}

describe("markdownToHtml", () => {
  it("blocks injected event handlers and elements", () => {
    // Payloads 1 and 2 are the two that actually worked before the fix.
    const payloads: [string, string][] = [
      ["unterminated img at end of input", '# Cool Model\n\nDownload me!\n\n<img src=x onerror="fetch(1)"'],
      ["unterminated svg", 'nice model\n\n<svg onload="alert(1)"'],
      ["double-encoded entity", "x\n\n&lt;img src=x onerror=alert(1)&gt;"],
      ["pre-escaped ampersand", "x\n\n&amp;lt;img src=x onerror=alert(1)"],
      ["smuggled inside bold", "**<img src=x onerror=alert(1)**"],
      ["smuggled inside inline code", "`<img src=x onerror=alert(1)`"],
      ["inside a heading", "# <img src=x onerror=alert(1)"],
      ["inside a bullet", "- <img src=x onerror=alert(1)"],
      ["inside a link label", "[<img src=x onerror=alert(1)](http://a)"],
      ["blockquote entity re-injection", "&gt;<img src=x onerror=alert(1)"],
      ["attribute break attempt", '# " onmouseover=alert(1) x="'],
      ["script tag without closing bracket", "x\n\n<script>alert(1)"],
    ];

    for (const [name, payload] of payloads) {
      const result = dangerousNodes(markdownToHtml(payload));
      expect(result, `Failed payload: ${name}`).toHaveLength(0);
    }
  });

  it("still renders ordinary markdown", () => {
    const input = "# Title\n\n## Sub\n\nThis is **bold** and `code`.\n\n- one\n- two\n\n[link text](http://x.com)";
    const html = markdownToHtml(input);
    expect(html).toContain("<h2");
    expect(html).toContain("Title");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<code");
    expect(html).toContain("<li");
    expect(html).toContain("link text");
    expect(html).not.toContain("http://x.com");
  });

  it("renders blockquotes", () => {
    const html = markdownToHtml("> quoted line");
    expect(html).toContain("quoted line");
    expect(html).not.toContain("&gt;");
  });

  it("escapes literal angle brackets and ampersands as text", () => {
    const html = markdownToHtml("AT&T and 5 < 10");
    expect(html).toContain("&amp;");
    expect(html).toContain("&lt;");
    expect(dangerousNodes(html)).toHaveLength(0);
  });
});
