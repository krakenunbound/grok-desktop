<script lang="ts">
  import {
    activeAgentRunId,
    agentDefinitions,
    agentRuns,
    agentsError,
    bindAgentEvents,
    closeAgentTab,
    createAgent,
    dispatchAgent,
    loadAgentDefinitions,
    stopAgent,
  } from "$lib/stores/agents";
  import { activeProject } from "$lib/stores/projects";
  import { selectedModel, yoloEnabled } from "$lib/stores/chat";

  interface Props {
    open: boolean;
    fallbackCwd: string;
    onclose: () => void;
  }
  let { open, fallbackCwd, onclose }: Props = $props();
  let prompt = $state("");
  let selectedAgent = $state("");
  let creating = $state(false);
  let dispatching = $state(false);
  let newName = $state("");
  let newDescription = $state("");
  let newInstructions = $state("");
  let newScope = $state<"project" | "user">("project");
  let cwd = $derived($activeProject?.path ?? fallbackCwd);
  let activeRun = $derived($agentRuns.find((run) => run.id === $activeAgentRunId) ?? null);
  let runningCount = $derived($agentRuns.filter((run) => run.status === "running").length);

  $effect(() => {
    if (open && cwd) {
      void bindAgentEvents();
      void loadAgentDefinitions(cwd);
    }
  });

  async function dispatch() {
    if (!prompt.trim() || !cwd || dispatching) return;
    dispatching = true;
    try {
      await dispatchAgent({
        cwd,
        agent: selectedAgent,
        prompt: prompt.trim(),
        model: $selectedModel,
        yolo: $yoloEnabled,
      });
      prompt = "";
    } catch (error) {
      agentsError.set(String(error));
    } finally {
      dispatching = false;
    }
  }

  async function saveDefinition() {
    try {
      await createAgent({
        cwd,
        scope: newScope,
        name: newName,
        description: newDescription,
        instructions: newInstructions,
      });
      selectedAgent = newName.trim().toLowerCase();
      newName = "";
      newDescription = "";
      newInstructions = "";
      creating = false;
    } catch (error) {
      agentsError.set(String(error));
    }
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && onclose()}
  >
    <div class="workspace" role="dialog" aria-modal="true" aria-labelledby="agents-title">
      <header>
        <div>
          <h2 id="agents-title">Agents <span>{$agentRuns.length}</span></h2>
          <p>{runningCount} working · {$agentRuns.length - runningCount} idle</p>
        </div>
        <div class="header-actions">
          <button type="button" onclick={() => (creating = !creating)}>+ Agent definition</button>
          <button type="button" class="close" onclick={onclose} aria-label="Close agents">×</button>
        </div>
      </header>

      {#if creating}
        <div class="definition-form">
          <div class="form-row">
            <label>Name <input bind:value={newName} placeholder="security-review" /></label>
            <label
              >Scope
              <select bind:value={newScope}>
                <option value="project">This project</option>
                <option value="user">All projects</option>
              </select>
            </label>
          </div>
          <label
            >Description <input
              bind:value={newDescription}
              placeholder="Reviews code for security risks"
            /></label
          >
          <label
            >Instructions <textarea
              bind:value={newInstructions}
              rows="4"
              placeholder="You are a careful security reviewer..."
            ></textarea></label
          >
          <div class="form-actions">
            <button type="button" onclick={() => (creating = false)}>Cancel</button>
            <button type="button" class="primary" onclick={saveDefinition}>Save definition</button>
          </div>
        </div>
      {/if}

      <nav class="tabs" aria-label="Agent runs">
        {#each $agentRuns as run (run.id)}
          <div class="tab" class:active={run.id === $activeAgentRunId}>
            <button type="button" class="tab-main" onclick={() => activeAgentRunId.set(run.id)}>
              <span class:working={run.status === "running"} class="dot"></span>
              <span>{run.agent}</span>
              <small>{run.status}</small>
            </button>
            <button
              type="button"
              class="tab-close"
              disabled={run.status === "running"}
              aria-label={`Close ${run.agent} tab`}
              onclick={() => closeAgentTab(run.id)}>×</button
            >
          </div>
        {:else}
          <span class="empty-tabs">Dispatch a task to create an agent tab.</span>
        {/each}
      </nav>

      <main>
        {#if activeRun}
          <div class="run-head">
            <div>
              <strong>{activeRun.prompt}</strong>
              <small>{activeRun.agent} · {activeRun.cwd}</small>
            </div>
            {#if activeRun.status === "running"}
              <button type="button" class="stop" onclick={() => stopAgent(activeRun.id)}
                >Stop</button
              >
            {/if}
          </div>
          <pre>{activeRun.output ||
              (activeRun.status === "running" ? "Agent is starting…" : "No output returned.")}</pre>
        {:else}
          <div class="empty-state">
            <strong>Parallel Grok agents</strong>
            <p>Start independent tasks, switch tabs, and let them continue in the background.</p>
          </div>
        {/if}
      </main>

      {#if $agentsError}<div class="error" role="alert">{$agentsError}</div>{/if}

      <footer>
        <select bind:value={selectedAgent} title="Agent definition">
          <option value="">Default Grok agent</option>
          {#each $agentDefinitions as agent (agent.name)}
            <option value={agent.name}>{agent.name} — {agent.description}</option>
          {/each}
        </select>
        <textarea bind:value={prompt} rows="2" placeholder="Dispatch a new agent task…"></textarea>
        <button
          type="button"
          class="primary"
          disabled={!prompt.trim() || dispatching}
          onclick={dispatch}
        >
          {dispatching ? "Starting…" : "Dispatch"}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 1.5rem;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(8px);
  }
  .workspace {
    width: min(1120px, 96vw);
    height: min(760px, 92vh);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--bg);
    box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55);
  }
  header,
  .header-actions,
  .form-row,
  .form-actions,
  .run-head,
  footer {
    display: flex;
    align-items: center;
    gap: 0.65rem;
  }
  header {
    justify-content: space-between;
    padding: 0.9rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--sidebar);
  }
  h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  h2 span {
    color: var(--muted);
    font-weight: 500;
    margin-left: 0.25rem;
  }
  header p {
    margin: 0.2rem 0 0;
    color: var(--muted);
    font-size: 0.75rem;
  }
  button,
  select,
  input,
  textarea {
    font: inherit;
  }
  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4rem 0.65rem;
    color: var(--text);
    background: var(--surface-2);
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .close {
    font-size: 1.1rem;
    padding: 0.25rem 0.55rem;
  }
  .primary {
    border: none;
    color: var(--accent-contrast);
    background: var(--accent-gradient);
    font-weight: 700;
  }
  .definition-form {
    display: grid;
    gap: 0.65rem;
    padding: 0.85rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .definition-form label {
    flex: 1;
    display: grid;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.75rem;
  }
  input,
  select,
  textarea {
    box-sizing: border-box;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.5rem 0.6rem;
    outline: none;
    color: var(--text);
    background: var(--surface-2);
  }
  input:focus,
  select:focus,
  textarea:focus {
    border-color: var(--accent-dim);
  }
  .form-actions {
    justify-content: flex-end;
  }
  .tabs {
    min-height: 44px;
    display: flex;
    gap: 0.35rem;
    align-items: center;
    overflow-x: auto;
    padding: 0.45rem 0.65rem;
    border-bottom: 1px solid var(--border);
    background: var(--sidebar);
  }
  .tab {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    overflow: hidden;
    max-width: 260px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
  }
  .tab.active {
    border-color: var(--accent-dim);
    background: var(--surface);
  }
  .tab-main {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
  }
  .tab-close {
    border: 0;
    border-left: 1px solid var(--border);
    border-radius: 0;
    padding: 0.4rem 0.5rem;
    background: transparent;
  }
  .tabs small {
    color: var(--muted);
    font-size: 0.65rem;
  }
  .tab-close {
    color: var(--muted);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
  }
  .dot.working {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
  }
  .empty-tabs {
    padding: 0 0.5rem;
    color: var(--muted);
    font-size: 0.78rem;
  }
  main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 1rem;
  }
  .run-head {
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }
  .run-head div {
    min-width: 0;
    display: grid;
    gap: 0.25rem;
  }
  .run-head strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .run-head small {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stop {
    color: #ffaea5;
    border-color: rgba(255, 100, 90, 0.45);
  }
  pre {
    flex: 1;
    overflow: auto;
    margin: 0;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text);
    background: #0a0b0e;
    font:
      0.78rem/1.55 ui-monospace,
      SFMono-Regular,
      Menlo,
      Consolas,
      monospace;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .empty-state {
    margin: auto;
    text-align: center;
    color: var(--muted);
  }
  .empty-state strong {
    color: var(--text);
    font-size: 1.1rem;
  }
  .empty-state p {
    margin: 0.45rem 0;
  }
  .error {
    margin: 0 1rem 0.65rem;
    padding: 0.55rem 0.7rem;
    border: 1px solid rgba(255, 100, 90, 0.4);
    border-radius: 8px;
    color: #ffb0a8;
    background: rgba(255, 80, 70, 0.08);
    font-size: 0.78rem;
  }
  footer {
    padding: 0.75rem 1rem 1rem;
    border-top: 1px solid var(--border);
    background: var(--sidebar);
  }
  footer select {
    width: 230px;
  }
  footer textarea {
    flex: 1;
    resize: none;
  }
  footer button {
    align-self: stretch;
  }
  @media (max-width: 760px) {
    .backdrop {
      padding: 0.5rem;
    }
    .workspace {
      width: 100%;
      height: 96vh;
    }
    footer {
      align-items: stretch;
      flex-direction: column;
    }
    footer select {
      width: 100%;
    }
  }
</style>
