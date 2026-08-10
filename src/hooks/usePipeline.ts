import { useEffect } from "react";
import { listenEvent } from "../services/tauriBridge";
import { useHistoryStore } from "../stores/historyStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useUiStore } from "../stores/uiStore";
import { useAnnounceStore } from "../stores/announceStore";
import { applyDownloadProgress } from "../stores/modelStore";
import type { RecordingState, DownloadProgress } from "../types";

interface PipelineResult {
  id: number;
  raw_text: string;
  processed_text: string;
  mode_id?: string;
  model_id: string;
  duration_ms: number;
  audio_ms?: number;
  audio_file?: string;
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
  const announce = useAnnounceStore((s) => s.announce);

  useEffect(() => {
    const unsubs = [
      listenEvent<{ state: RecordingState }>("pipeline://state", (p) =>
        setRecordingState(p.state)
      ),
      listenEvent<PipelineResult>("pipeline://result", (r) => {
        addFull({
          id: r.id,
          timestamp: r.id,
          raw_text: r.raw_text,
          processed_text: r.processed_text,
          mode_id: r.mode_id ?? activeModeId,
          model_id: r.model_id,
          duration_ms: r.duration_ms,
          audio_ms: r.audio_ms,
          audio_file: r.audio_file,
        });
        const words = r.processed_text.trim().split(/\s+/).filter(Boolean).length;
        announce(`Dictation done — ${words} ${words === 1 ? "word" : "words"} ready at the cursor.`);
      }),
      listenEvent<string>("pipeline://error", (e) => {
        setError(e);
        announce(e);
      }),
      listenEvent<DownloadProgress>("download://progress", (p) =>
        applyDownloadProgress(p)
      ),
    ];
    return () => {
      unsubs.forEach((u) => u.then((fn) => fn()));
    };
  }, [addFull, activeModeId, setRecordingState, setError, announce]);
}
