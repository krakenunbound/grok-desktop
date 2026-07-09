/**
 * App settings store - mirrors Rust AppSettings and persists via Tauri.
 */
import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { debugWarn } from "$lib/log";

export type PermissionMode =
  "default" | "acceptEdits" | "auto" | "dontAsk" | "bypassPermissions" | "plan";

export interface AppSettings {
  default_model: string;
  yolo_default: boolean;
  theme: string;
  sidebar_collapsed: boolean;
  right_panel_open: boolean;
  grok_binary: string;
  temp_images_dir: string;
  last_project_id: string | null;
  /** Agent Transparency: false = Hidden (default), true = Verbose raw stream. */
  verbose_mode: boolean;
  plan_mode: boolean;
  disable_web_search: boolean;
  subagents_enabled: boolean;
  memory_enabled: boolean;
  permission_mode: PermissionMode;
  tools: string;
  disallowed_tools: string;
  allow_rules: string;
  deny_rules: string;
  extra_rules: string;
  max_turns: string;
}

export const defaultSettings = (): AppSettings => ({
  default_model: "grok-4.5",
  yolo_default: false,
  theme: "dark",
  sidebar_collapsed: false,
  right_panel_open: false,
  grok_binary: "",
  temp_images_dir: "",
  last_project_id: null,
  verbose_mode: false,
  plan_mode: true,
  disable_web_search: false,
  subagents_enabled: true,
  memory_enabled: false,
  permission_mode: "default",
  tools: "",
  disallowed_tools: "",
  allow_rules: "",
  deny_rules: "",
  extra_rules: "",
  max_turns: "",
});

function normalizeSettings(s: Partial<AppSettings>): AppSettings {
  return {
    ...defaultSettings(),
    ...s,
    verbose_mode: !!s.verbose_mode,
    plan_mode: s.plan_mode !== false,
    disable_web_search: !!s.disable_web_search,
    subagents_enabled: s.subagents_enabled !== false,
    memory_enabled: !!s.memory_enabled,
    permission_mode: s.permission_mode || "default",
  };
}

export const settings = writable<AppSettings>(defaultSettings());
export const models = writable<string[]>(["grok-4.5", "grok-4", "grok-3", "grok-3-mini", "grok-2"]);

export async function loadSettings(): Promise<AppSettings> {
  try {
    const s = await invoke<AppSettings>("get_settings");
    const merged = normalizeSettings(s);
    settings.set(merged);
    return merged;
  } catch (e) {
    debugWarn("loadSettings", e);
    return get(settings);
  }
}

export async function persistSettings(next: AppSettings): Promise<void> {
  const payload = normalizeSettings(next);
  const saved = await invoke<AppSettings>("save_settings", { settings: payload });
  settings.set(normalizeSettings(saved));
}

export async function loadModels(): Promise<void> {
  try {
    const res = await invoke<{ models: string[] }>("get_models");
    if (res.models?.length) models.set(res.models);
  } catch (e) {
    debugWarn("loadModels", e);
  }
}
