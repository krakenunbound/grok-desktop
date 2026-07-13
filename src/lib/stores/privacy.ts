import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writable } from "svelte/store";

export interface RepositoryUploadSummary {
  path: string;
  events: number;
  bytes: number;
}

export interface PrivacyAudit {
  guard_enabled: boolean;
  account_retention_opt_out: boolean | null;
  telemetry_disabled_in_config: boolean;
  trace_upload_disabled_in_config: boolean;
  log_exists: boolean;
  log_bytes: number;
  upload_start_events: number;
  upload_enqueued_events: number;
  upload_bytes: number;
  largest_upload_bytes: number;
  first_upload_at: string | null;
  last_upload_at: string | null;
  repositories: RepositoryUploadSummary[];
}

export interface PrivacyAlert {
  message: string;
  cwd?: string;
  run_id?: string;
}

export const privacyAudit = writable<PrivacyAudit | null>(null);
export const privacyAlert = writable<PrivacyAlert | null>(null);

let unlisten: UnlistenFn | null = null;

export async function loadPrivacyAudit(): Promise<PrivacyAudit> {
  const audit = await invoke<PrivacyAudit>("get_privacy_audit");
  privacyAudit.set(audit);
  return audit;
}

export async function bindPrivacyEvents(): Promise<void> {
  if (unlisten) return;
  unlisten = await listen<PrivacyAlert>("privacy-alert", (event) => {
    privacyAlert.set(event.payload);
    void loadPrivacyAudit();
  });
}
