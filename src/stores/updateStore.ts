import { create } from "zustand";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { checkForUpdates } from "../services/updater";

interface UpdateState {
  available: boolean;
  version: string | null;
  installing: boolean;
  error: string | null;
  checkSilently: () => Promise<void>;
  installNow: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set) => ({
  available: false,
  version: null,
  installing: false,
  error: null,
  checkSilently: async () => {
    const { available, version } = await checkForUpdates();
    set({ available, version });
  },
  installNow: async () => {
    set({ installing: true, error: null });
    try {
      const update = await check();
      if (!update) {
        set({ installing: false, available: false, version: null });
        return;
      }
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      set({ installing: false, error: String(error) });
    }
  },
}));
