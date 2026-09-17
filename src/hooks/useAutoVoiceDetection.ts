import { useEffect } from "react";
import { useModelStore } from "../stores/modelStore";
import { downloadVadModel, isTauri, vadInstalled } from "../services/tauriBridge";

// Smart voice detection is a 2 MB model that tells speech apart from fans,
// keyboards and music. Nobody should have to find a button for it, so fetch it
// once there's a voice model to use it with. It applies automatically once on
// disk. One attempt per app run; a failure (offline) retries next launch.
let attempted = false;

export function useAutoVoiceDetection() {
  const hasVoiceModel = useModelStore((s) => s.downloaded.size > 0);
  useEffect(() => {
    if (!isTauri() || !hasVoiceModel || attempted) return;
    attempted = true;
    vadInstalled()
      .then((installed) => (installed ? undefined : downloadVadModel()))
      .catch(() => {
        // Offline or a download already running: try again on next launch.
      });
  }, [hasVoiceModel]);
}
