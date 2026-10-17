import { useState } from "react";
import Page from "../shared/Page";
import ConfirmDialog from "../shared/ConfirmDialog";
import type { ApiProvider } from "../../types";
import { useProviders } from "./providers/shared";
import ProviderCards from "./providers/ProviderCards";

// Cloud providers only: local models are downloaded and run in the Model Store.

export default function ApiKeys() {
  const api = useProviders();
  const [pendingDelete, setPendingDelete] = useState<ApiProvider | null>(null);

  return (
    <Page
      title="Cloud providers"
      subtitle="Use a cloud model for AI writing, meeting summaries or speech-to-text. Keys stay on this PC."
    >
      <ProviderCards api={api} onRemove={setPendingDelete} />
      <ConfirmDialog
        open={pendingDelete !== null}
        title="Remove this provider?"
        message={
          <>
            This deletes <span className="text-sv-text">{pendingDelete?.name}</span> and its saved API key. You'll have to paste the key again to add it back.
          </>
        }
        confirmLabel="Remove"
        onConfirm={() => {
          if (pendingDelete) api.remove(pendingDelete.id);
          setPendingDelete(null);
        }}
        onCancel={() => setPendingDelete(null)}
      />
    </Page>
  );
}
