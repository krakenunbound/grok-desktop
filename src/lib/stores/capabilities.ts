import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { debugWarn } from "$lib/log";

export interface McpServerInfo {
  name: string;
  command: string;
  enabled: boolean;
  source: string;
  can_toggle: boolean;
  status: string;
}

export interface PluginInfo {
  name: string;
  enabled: boolean;
  detail: string;
}

export interface GrokInventory {
  mcp_servers: McpServerInfo[];
  plugins: PluginInfo[];
  config_path: string;
}

export interface GrokCliSession {
  id: string;
  created: string;
  updated: string;
  status: string;
  summary: string;
}

export interface GrokCliWorktree {
  id: string;
  name: string;
  path: string;
  branch: string;
  status: string;
}

export interface GrokCliCapability {
  name: string;
  detail: string;
  available: boolean;
}

export interface GrokCliOverview {
  version: string;
  commit: string;
  channel: string;
  compatibility: string;
  sessions: GrokCliSession[];
  worktrees: GrokCliWorktree[];
  capabilities: GrokCliCapability[];
  errors: string[];
}

export const inventory = writable<GrokInventory>({
  mcp_servers: [],
  plugins: [],
  config_path: "",
});

export const cliOverview = writable<GrokCliOverview>({
  version: "",
  commit: "",
  channel: "",
  compatibility: "",
  sessions: [],
  worktrees: [],
  capabilities: [],
  errors: [],
});

export async function loadInventory(): Promise<void> {
  try {
    inventory.set(await invoke<GrokInventory>("get_grok_inventory"));
  } catch (e) {
    debugWarn("loadInventory", e);
  }
}

export async function loadCliOverview(cwd?: string | null): Promise<void> {
  try {
    cliOverview.set(await invoke<GrokCliOverview>("get_grok_cli_overview", { cwd: cwd || null }));
  } catch (e) {
    debugWarn("loadCliOverview", e);
    cliOverview.update((current) => ({ ...current, errors: [String(e)] }));
  }
}

export async function setMcpEnabled(name: string, enabled: boolean): Promise<void> {
  inventory.set(await invoke<GrokInventory>("set_mcp_server_enabled", { name, enabled }));
}

export async function setPluginEnabled(name: string, enabled: boolean): Promise<void> {
  inventory.set(await invoke<GrokInventory>("set_plugin_enabled", { name, enabled }));
}
