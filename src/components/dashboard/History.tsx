import Page from "../shared/Page";
import { useHistoryStore } from "../../stores/historyStore";
import TranscriptList from "../shared/TranscriptList";

export default function History() {
  const entries = useHistoryStore((s) => s.entries);
  const clear = useHistoryStore((s) => s.clear);

  return (
    <Page
      title="History"
      subtitle="Past transcriptions. Edit one to fix mistakes — corrections teach the app new words."
      actions={
        entries.length > 0 && (
          <button
            onClick={clear}
            className="rounded-lg border border-sv-border px-3 py-1.5 text-sm text-sv-muted hover:text-sv-bad"
          >
            Clear all
          </button>
        )
      }
    >
      <TranscriptList />
    </Page>
  );
}
