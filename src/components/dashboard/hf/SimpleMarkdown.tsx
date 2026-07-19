import { useMemo, useState } from "react";

export default function SimpleMarkdown({ content }: { content: string }) {
  const [expanded, setExpanded] = useState(false);

  const html = useMemo(() => {
    let md = content || "";
    // Remove HTML tags entirely
    md = md.replace(/<[^>]+>/g, "");
    
    // Remove images: ![alt](url)
    md = md.replace(/!\[([^\]]*)\]\([^)]+\)/g, "");

    // Tables: roughly strip lines starting with | and containing |
    md = md.replace(/^\|.*\|$/gm, "");

    // Blockquotes: > text -> strip >
    md = md.replace(/^>\s?(.*)$/gm, "$1");

    // Process blocks: split by lines
    const lines = md.split('\n');
    const parsed = lines.map(line => {
      let l = line.trim();
      if (!l) return "<br />";

      // Headings
      if (l.startsWith("### ")) return `<h4 class="mt-4 mb-2 font-semibold text-sv-text">${l.slice(4)}</h4>`;
      if (l.startsWith("## ")) return `<h3 class="mt-5 mb-2 text-lg font-bold text-sv-text">${l.slice(3)}</h3>`;
      if (l.startsWith("# ")) return `<h2 class="mt-6 mb-3 text-xl font-bold text-sv-text">${l.slice(2)}</h2>`;

      // Bullet lists
      if (l.startsWith("- ") || l.startsWith("* ")) {
        l = `<li class="ml-4 list-disc mb-1">${l.slice(2)}</li>`;
      } else {
        l = `<p class="mb-2">${l}</p>`;
      }

      // Inline styles
      l = l.replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>"); // bold
      l = l.replace(/__(.*?)__/g, "<strong>$1</strong>"); // bold
      l = l.replace(/\*(.*?)\*/g, "<em>$1</em>"); // italic
      l = l.replace(/_(.*?)_/g, "<em>$1</em>"); // italic
      l = l.replace(/`(.*?)`/g, '<code class="rounded bg-sv-surface-2 px-1 py-0.5 text-xs text-sv-accent font-mono">$1</code>'); // inline code

      // Links: [text](url) -> plain text
      l = l.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");

      return l;
    });

    return parsed.join("");
  }, [content]);

  return (
    <div className="relative">
      <div 
        className={`max-w-none text-sv-text break-words ${expanded ? "" : "max-h-[300px] overflow-hidden"}`}
        dangerouslySetInnerHTML={{ __html: html }} 
      />
      {!expanded && (
        <div className="absolute bottom-0 left-0 right-0 h-32 bg-gradient-to-t from-sv-surface to-transparent flex items-end justify-center pb-2">
          <button 
            onClick={() => setExpanded(true)}
            className="rounded-full bg-sv-surface-2 px-4 py-1.5 text-xs font-medium text-sv-text shadow hover:bg-sv-border transition"
          >
            Show more
          </button>
        </div>
      )}
      {expanded && (
        <div className="mt-4 text-center">
          <button 
            onClick={() => setExpanded(false)}
            className="rounded-full bg-sv-surface-2 px-4 py-1.5 text-xs font-medium text-sv-text shadow hover:bg-sv-border transition"
          >
            Show less
          </button>
        </div>
      )}
    </div>
  );
}
