import { writable } from "svelte/store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdatePhase = "idle" | "checking" | "available" | "downloading" | "current" | "error";

export const updatePhase = writable<UpdatePhase>("idle");
export const updateVersion = writable("");
export const updateNotes = writable("");
export const updateProgress = writable(0);
export const updateError = writable("");

let availableUpdate: Update | null = null;

export async function checkForUpdates(silent = false): Promise<boolean> {
  if (!silent) updatePhase.set("checking");
  updateError.set("");
  try {
    const update = await check({ timeout: 15_000 });
    if (!update) {
      availableUpdate = null;
      updateVersion.set("");
      updatePhase.set("current");
      return false;
    }
    availableUpdate = update;
    updateVersion.set(update.version);
    updateNotes.set(update.body ?? "");
    updatePhase.set("available");
    return true;
  } catch (error) {
    if (!silent) {
      updateError.set(String(error));
      updatePhase.set("error");
    }
    return false;
  }
}

export async function installAvailableUpdate(): Promise<void> {
  if (!availableUpdate) {
    const found = await checkForUpdates(false);
    if (!found || !availableUpdate) return;
  }
  updatePhase.set("downloading");
  updateProgress.set(0);
  updateError.set("");
  let total = 0;
  let downloaded = 0;
  try {
    await availableUpdate.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      if (event.event === "Progress") downloaded += event.data.chunkLength;
      if (total > 0) updateProgress.set(Math.min(100, Math.round((downloaded / total) * 100)));
    });
    await relaunch();
  } catch (error) {
    updateError.set(String(error));
    updatePhase.set("error");
  }
}
