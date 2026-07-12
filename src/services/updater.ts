import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { isTauri } from "./tauriBridge";

export async function checkForUpdates(): Promise<void> {
  if (!isTauri()) return;

  try {
    const update = await check();
    if (update) {
      console.log("Update available:", update.version);
      await update.downloadAndInstall();
      await relaunch();
    }
  } catch (error) {
    console.error("Updater error:", error);
  }
}
