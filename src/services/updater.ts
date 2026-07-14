import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { isTauri } from "./tauriBridge";

export async function checkForUpdates(): Promise<{ available: boolean; version: string | null }> {
  if (!isTauri()) return { available: false, version: null };

  try {
    const update = await check();
    if (update) {
      console.log("Update available:", update.version);
      return { available: true, version: update.version };
    }
  } catch (error) {
    console.error("Updater error:", error);
  }
  return { available: false, version: null };
}

export type UpdateCheckResult =
  | { status: "none" }
  | { status: "available"; version: string }
  | { status: "error"; message: string }
  | { status: "unsupported" };

// Manual "Check for updates" button: same install flow, but reports a
// result so the UI can give feedback (checkForUpdates stays silent for the
// automatic startup check).
export async function checkForUpdatesManual(): Promise<UpdateCheckResult> {
  if (!isTauri()) return { status: "unsupported" };
  try {
    const update = await check();
    if (!update) return { status: "none" };
    await update.downloadAndInstall();
    await relaunch();
    return { status: "available", version: update.version };
  } catch (error) {
    return { status: "error", message: String(error) };
  }
}
