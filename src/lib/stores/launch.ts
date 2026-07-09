/**
 * Honest launch / WebView status.
 * Success is only claimed after Rust receives report_ui_ready from a mounted UI.
 */
import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface LaunchStatusSnapshot {
  phase: string;
  ui_ready: boolean;
  load_failed: boolean;
  last_error: string | null;
  success: boolean;
}

export const launchStatus = writable<LaunchStatusSnapshot>({
  phase: "Starting…",
  ui_ready: false,
  load_failed: false,
  last_error: null,
  success: false,
});

let unlisten: UnlistenFn | null = null;

export async function bindLaunchStatus(): Promise<void> {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  try {
    const snap = await invoke<LaunchStatusSnapshot>("get_launch_status");
    launchStatus.set(snap);
  } catch {
    /* not in tauri */
  }
  try {
    unlisten = await listen<LaunchStatusSnapshot>("launch-status", (ev) => {
      launchStatus.set(ev.payload);
    });
  } catch {
    /* ignore */
  }
}

/** Must be called only after the real app shell has mounted successfully. */
export async function reportUiReady(): Promise<void> {
  try {
    const snap = await invoke<LaunchStatusSnapshot>("report_ui_ready");
    launchStatus.set(snap);
  } catch (e) {
    // If invoke fails, we are not inside Tauri or IPC is broken — surface that.
    launchStatus.set({
      phase: "IPC unavailable",
      ui_ready: false,
      load_failed: true,
      last_error: String(e),
      success: false,
    });
  }
}

export function isLaunchSuccess(): boolean {
  return get(launchStatus).success === true;
}
