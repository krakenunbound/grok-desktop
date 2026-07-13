import { get, writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface AgentDefinition {
  name: string;
  description: string;
  source: string;
}

export interface AgentRun {
  id: string;
  agent: string;
  prompt: string;
  cwd: string;
  started_at: string;
  status: "running" | "completed" | "failed" | "cancelled";
  output: string;
  exit_code?: number;
}

interface AgentRunEvent {
  run_id: string;
  kind: "status" | "stdout" | "stderr" | "done";
  status: AgentRun["status"];
  chunk: string;
  exit_code: number | null;
}

export const agentDefinitions = writable<AgentDefinition[]>([]);
export const agentRuns = writable<AgentRun[]>([]);
export const activeAgentRunId = writable<string | null>(null);
export const agentsError = writable("");

let eventBinding: Promise<UnlistenFn> | null = null;

export async function bindAgentEvents(): Promise<void> {
  if (eventBinding) return void (await eventBinding);
  eventBinding = listen<AgentRunEvent>("agent-run-event", ({ payload }) => {
    agentRuns.update((runs) =>
      runs.map((run) => {
        if (run.id !== payload.run_id) return run;
        const appended = payload.chunk
          ? `${run.output}${payload.chunk}`.slice(-2_000_000)
          : run.output;
        return {
          ...run,
          output: appended,
          status: payload.status,
          exit_code: payload.exit_code ?? run.exit_code,
        };
      }),
    );
  });
  await eventBinding;
}

export async function loadAgentDefinitions(cwd: string): Promise<void> {
  agentsError.set("");
  try {
    agentDefinitions.set(await invoke<AgentDefinition[]>("list_agent_definitions", { cwd }));
  } catch (error) {
    agentsError.set(String(error));
    agentDefinitions.set([]);
  }
}

export async function dispatchAgent(args: {
  cwd: string;
  agent: string;
  prompt: string;
  model: string;
  yolo: boolean;
}): Promise<AgentRun> {
  await bindAgentEvents();
  agentsError.set("");
  const started = await invoke<Omit<AgentRun, "status" | "output">>("start_agent_run", args);
  const run: AgentRun = { ...started, status: "running", output: "" };
  agentRuns.update((runs) => [...runs, run]);
  activeAgentRunId.set(run.id);
  return run;
}

export async function stopAgent(runId: string): Promise<void> {
  try {
    await invoke("stop_agent_run", { runId });
  } catch (error) {
    const run = get(agentRuns).find((item) => item.id === runId);
    if (run?.status === "running") agentsError.set(String(error));
  }
}

export function closeAgentTab(runId: string): void {
  const remaining = get(agentRuns).filter((run) => run.id !== runId);
  agentRuns.set(remaining);
  if (get(activeAgentRunId) === runId) {
    activeAgentRunId.set(remaining.at(-1)?.id ?? null);
  }
}

export async function createAgent(args: {
  cwd: string;
  scope: "project" | "user";
  name: string;
  description: string;
  instructions: string;
}): Promise<string> {
  const path = await invoke<string>("create_agent_definition", args);
  await loadAgentDefinitions(args.cwd);
  return path;
}
