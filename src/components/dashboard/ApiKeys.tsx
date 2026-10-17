import { useState } from "react";
import Page from "../shared/Page";
import ConfirmDialog from "../shared/ConfirmDialog";
import type { ApiProvider } from "../../types";
import { useProviders } from "./providers/shared";
import VariantA from "./providers/VariantA";
import VariantB from "./providers/VariantB";
import VariantC from "./providers/VariantC";

// Cloud providers only: local models are downloaded and run in the Model Store.
// PROTOTYPE: three designs behind a switch until one is picked.

const PROTO_KEY = "sv-providers-prototype";
const VARIANTS = { A: VariantA, B: VariantB, C: VariantC };
type V = keyof typeof VARIANTS;

function readV(): V {
  try {
    const v = localStorage.getItem(PROTO_KEY);
    return v === "B" || v === "C" ? v : "A";
  } catch {
    return "A";
  }
}

export default function ApiKeys() {
  const api = useProviders();
  const [pendingDelete, setPendingDelete] = useState<ApiProvider | null>(null);
  const [v, setV] = useState<V>(readV);
  const Variant = VARIANTS[v];

  const pick = (next: V) => {
    setV(next);
    try {
      localStorage.setItem(PROTO_KEY, next);
    } catch {
      /* this visit only */
    }
  };

  return (
    <Page
      title="Cloud providers"
      subtitle="Use a cloud model for AI writing, meeting summaries or speech-to-text. Keys stay on this PC."
      actions={
        <div role="group" aria-label="Design prototype" className="flex items-center gap-1 rounded-lg border border-dashed border-sv-border p-1">
          <span className="px-1.5 text-[11px] text-sv-muted">Prototype</span>
          {(Object.keys(VARIANTS) as V[]).map((k) => (
            <button
              key={k}
              onClick={() => pick(k)}
              aria-pressed={v === k}
              className={`h-7 w-7 rounded-md text-xs font-semibold transition ${v === k ? "bg-sv-accent text-sv-on-accent" : "text-sv-muted hover:bg-sv-surface-2 hover:text-sv-text"}`}
            >
              {k}
            </button>
          ))}
        </div>
      }
    >
      <Variant api={api} onRemove={setPendingDelete} />
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
