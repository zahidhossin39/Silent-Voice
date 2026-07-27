import { useState } from "react";
import { useHistoryStore } from "../../stores/historyStore";
import { useSettingsStore } from "../../stores/settingsStore";
import TranscriptCard from "./TranscriptCard";

export default function TranscriptList({ emptyState }: { emptyState?: React.ReactNode }) {
  const entries = useHistoryStore((s) => s.entries);
  const update = useHistoryStore((s) => s.update);
  const remove = useHistoryStore((s) => s.remove);
  const blurHistory = useSettingsStore((s) => s.settings.blur_history);

  const [query, setQuery] = useState("");
  const [learnedMsg, setLearnedMsg] = useState<string | null>(null);

  const filtered = entries.filter((e) =>
    (e.processed_text + e.raw_text)
      .toLowerCase()
      .includes(query.toLowerCase())
  );

  function handleLearn(msg: string) {
    setLearnedMsg(msg);
  }

  if (entries.length === 0) {
    return emptyState || (
      <div className="rounded-xl border border-dashed border-sv-border bg-sv-surface p-8 text-center text-sm text-sv-muted">
        No transcriptions yet.
      </div>
    );
  }

  return (
    <>
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search transcriptions…"
        className="mb-4 w-full rounded-lg border border-sv-border bg-sv-surface px-3 py-2 text-sm"
      />

      {learnedMsg && (
        <div className="mb-4 rounded-lg border border-sv-good/30 bg-sv-good/10 px-3 py-2 text-xs text-sv-good">
          {learnedMsg}
        </div>
      )}

      {filtered.length === 0 ? (
        <div className="rounded-xl border border-dashed border-sv-border bg-sv-surface p-8 text-center text-sm text-sv-muted">
          No matches for your search.
        </div>
      ) : (
        <ul className="space-y-3">
          {filtered.map((e) => (
            <TranscriptCard
              key={e.id}
              entry={e}
              blurred={blurHistory}
              onUpdate={update}
              onRemove={remove}
              onLearn={handleLearn}
            />
          ))}
        </ul>
      )}
    </>
  );
}
