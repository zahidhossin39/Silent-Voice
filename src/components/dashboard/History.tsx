import { useState } from "react";
import Page from "../shared/Page";
import { useHistoryStore } from "../../stores/historyStore";
import TranscriptList from "../shared/TranscriptList";
import ConfirmDialog from "../shared/ConfirmDialog";

export default function History() {
  const entries = useHistoryStore((s) => s.entries);
  const clear = useHistoryStore((s) => s.clear);
  const [confirmClear, setConfirmClear] = useState(false);

  return (
    <Page
      title="History"
      subtitle="Past transcriptions. Edit one to fix mistakes — corrections teach the app new words."
      actions={
        entries.length > 0 && (
          <button
            onClick={() => setConfirmClear(true)}
            className="rounded-lg border border-sv-border px-3 py-1.5 text-sm text-sv-muted hover:text-sv-bad"
          >
            Clear all
          </button>
        )
      }
    >
      <TranscriptList />
      <ConfirmDialog
        open={confirmClear}
        title="Clear all history?"
        message={
          <>
            This permanently deletes all {entries.length} saved transcription
            {entries.length === 1 ? "" : "s"} and their audio. This can't be undone.
          </>
        }
        confirmLabel="Clear all"
        onConfirm={() => {
          clear();
          setConfirmClear(false);
        }}
        onCancel={() => setConfirmClear(false)}
      />
    </Page>
  );
}
