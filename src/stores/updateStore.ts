import { create } from "zustand";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { checkForUpdates } from "../services/updater";

interface UpdateState {
  available: boolean;
  version: string | null;
  installing: boolean;
  // 0..1 while downloading, or -1 when the total size isn't known yet
  // (indeterminate). Lets the sidebar show a real progress bar.
  progress: number;
  // User hid the "Update available" prompt for this run (resets next launch).
  dismissed: boolean;
  error: string | null;
  checkSilently: () => Promise<void>;
  installNow: () => Promise<void>;
  cancel: () => void;
  dismiss: () => void;
}

// Module-level so cancel() can flip it without a re-render. The native download
// can't be aborted mid-chunk, so we cancel at the install boundary: finish the
// current download, then skip install + relaunch so nothing is applied.
// ponytail: true mid-download abort would need a Rust-side cancel token in the
// updater plugin; the install-boundary cancel is the honest ceiling here.
let cancelRequested = false;

export const useUpdateStore = create<UpdateState>((set) => ({
  available: false,
  version: null,
  installing: false,
  progress: -1,
  dismissed: false,
  error: null,
  checkSilently: async () => {
    const { available, version } = await checkForUpdates();
    // A freshly-found update clears any earlier dismissal.
    set({ available, version, dismissed: false });
  },
  installNow: async () => {
    cancelRequested = false;
    set({ installing: true, error: null, progress: -1 });
    try {
      const update = await check();
      if (!update) {
        set({ installing: false, available: false, version: null });
        return;
      }

      let total = 0;
      let got = 0;
      await update.download((e) => {
        if (e.event === "Started") {
          total = e.data.contentLength ?? 0;
          set({ progress: total > 0 ? 0 : -1 });
        } else if (e.event === "Progress") {
          got += e.data.chunkLength;
          if (total > 0) set({ progress: Math.min(got / total, 1) });
        } else if (e.event === "Finished") {
          set({ progress: 1 });
        }
      });

      if (cancelRequested) {
        // Downloaded bytes are discarded by never installing them.
        try {
          await update.close();
        } catch {
          /* close is best-effort */
        }
        set({ installing: false, progress: -1 });
        return;
      }

      await update.install();
      await relaunch();
    } catch (error) {
      set({ installing: false, progress: -1, error: String(error) });
    }
  },
  cancel: () => {
    cancelRequested = true;
  },
  dismiss: () => set({ dismissed: true }),
}));
