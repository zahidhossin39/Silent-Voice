import { useEffect } from "react";
import { listenEvent } from "../services/tauriBridge";
import { useHistoryStore } from "../stores/historyStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useUiStore } from "../stores/uiStore";
import { applyDownloadProgress } from "../stores/modelStore";
import type { RecordingState, DownloadProgress } from "../types";

interface PipelineResult {
  raw_text: string;
  processed_text: string;
  model_id: string;
  duration_ms: number;
}

/**
 * Subscribes the dashboard to backend pipeline events:
 *  - pipeline://state  → live recording status
 *  - pipeline://result → append to history
 *  - pipeline://error  → surface errors
 *  - download://progress → model download bars
 */
export function usePipeline() {
  const addFull = useHistoryStore((s) => s.addFull);
  const activeModeId = useSettingsStore((s) => s.settings.active_mode_id);
  const setRecordingState = useUiStore((s) => s.setRecordingState);
  const setError = useUiStore((s) => s.setError);

  useEffect(() => {
    const unsubs = [
      listenEvent<{ state: RecordingState }>("pipeline://state", (p) =>
        setRecordingState(p.state)
      ),
      listenEvent<PipelineResult>("pipeline://result", (r) => {
        addFull({
          id: Date.now(),
          timestamp: Date.now(),
          raw_text: r.raw_text,
          processed_text: r.processed_text,
          mode_id: activeModeId,
          model_id: r.model_id,
          duration_ms: r.duration_ms,
        });
      }),
      listenEvent<string>("pipeline://error", (e) => setError(e)),
      listenEvent<DownloadProgress>("download://progress", (p) =>
        applyDownloadProgress(p)
      ),
    ];
    return () => {
      unsubs.forEach((u) => u.then((fn) => fn()));
    };
  }, [addFull, activeModeId, setRecordingState, setError]);
}
