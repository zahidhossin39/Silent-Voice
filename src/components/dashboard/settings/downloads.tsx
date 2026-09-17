// Download controls for the optional grammar models, with real progress,
// pause/resume and cancel (ported from the previous Settings page).
import { useState, type ReactNode } from "react";
import Select from "../../shared/Select";
import { useModelStore } from "../../../stores/modelStore";
import {
  cancelDownload,
  coeditInstalled,
  deleteCoeditModel,
  deleteGectorModel,
  downloadCoeditModel,
  downloadGectorModel,
  gectorInstalled,
  pauseDownload,
} from "../../../services/tauriBridge";
import type { Ctx } from "./catalog";
import { GhostButton, IconButton, Toggle } from "./parts";
import { PauseIcon, PlayIcon, XIcon } from "./icons";

function DownloadButton({
  modelId,
  sizeLabel,
  start,
  before,
}: {
  modelId: "coedit" | "gector";
  sizeLabel: string;
  start: () => Promise<void>;
  before?: ReactNode;
}) {
  const progress = useModelStore((s) => s.progress[modelId]);
  // GECToR is several files and "downloaded" fires after each one, so the
  // progress event alone would briefly re-show the Download button between
  // files. The local flag covers the whole call.
  const [fetching, setFetching] = useState(false);
  const run = async () => {
    setFetching(true);
    try {
      await start();
    } finally {
      setFetching(false);
    }
  };

  const paused = progress?.status === "paused";
  const downloading = fetching || progress?.status === "downloading";

  if (downloading || paused) {
    const known = !!progress && progress.total_bytes > 0;
    const pct = known ? Math.round((progress!.downloaded_bytes / progress!.total_bytes) * 100) : 0;
    return (
      <div className="flex items-center gap-1">
        <div className="flex w-32 items-center gap-2">
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-sv-surface-2">
            {known ? (
              <div
                className={`h-full rounded-full transition-[width] duration-150 ${paused ? "bg-sv-muted" : "bg-sv-accent"}`}
                style={{ width: `${pct}%` }}
              />
            ) : (
              <div className="h-full w-1/3 rounded-full bg-sv-accent animate-[sv-indeterminate_1.1s_ease-in-out_infinite]" />
            )}
          </div>
          <span className="w-11 shrink-0 text-right text-[11.5px] tabular-nums text-sv-muted">
            {paused ? "Paused" : known ? `${pct}%` : "…"}
          </span>
        </div>
        {paused ? (
          <IconButton label="Resume download" onClick={run}>
            <PlayIcon />
          </IconButton>
        ) : (
          <IconButton label="Pause download" onClick={() => pauseDownload(modelId)}>
            <PauseIcon />
          </IconButton>
        )}
        <IconButton label="Cancel download" danger onClick={() => cancelDownload(modelId)}>
          <XIcon />
        </IconButton>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      {before}
      <GhostButton onClick={run}>Download · {sizeLabel}</GhostButton>
    </div>
  );
}

function RemoveButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-md px-1.5 py-1 text-[12.5px] text-sv-muted transition-colors duration-150 hover:text-sv-bad focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent"
    >
      Remove
    </button>
  );
}

export function CoeditControl({ c, label }: { c: Ctx; label: string }) {
  if (c.coeditReady)
    return (
      <div className="flex items-center gap-3">
        <RemoveButton
          onClick={async () => {
            await deleteCoeditModel();
            c.setCoeditReady(false);
          }}
        />
        <Toggle label={label} checked={c.s.coedit_enabled} onChange={(v) => c.set({ coedit_enabled: v })} />
      </div>
    );
  return (
    <DownloadButton
      modelId="coedit"
      sizeLabel="818 MB"
      start={async () => {
        const ok = await downloadCoeditModel();
        if (ok) c.setCoeditReady(await coeditInstalled());
      }}
    />
  );
}

export function GectorControl({ c, label }: { c: Ctx; label: string }) {
  const [variant, setVariant] = useState("int8");
  if (c.gectorReady) {
    const on = !c.s.proofread_disabled_rules.includes("Gector");
    return (
      <div className="flex items-center gap-3">
        <RemoveButton
          onClick={async () => {
            await deleteGectorModel();
            c.setGectorReady(false);
          }}
        />
        <Toggle label={label} checked={on} onChange={(v) => c.toggleRule("Gector", v)} />
      </div>
    );
  }
  return (
    <DownloadButton
      modelId="gector"
      sizeLabel={variant === "fp32" ? "512 MB" : "122 MB"}
      before={
        <Select value={variant} onChange={setVariant} className="w-40">
          <option value="int8">Balanced</option>
          <option value="fp32">Best quality</option>
        </Select>
      }
      start={async () => {
        const ok = await downloadGectorModel(variant);
        if (ok) c.setGectorReady(await gectorInstalled());
      }}
    />
  );
}
