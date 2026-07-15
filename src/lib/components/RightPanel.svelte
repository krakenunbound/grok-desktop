<script lang="ts">
  import { onMount } from "svelte";
  import {
    status,
    isRunning,
    yoloEnabled,
    selectedModel,
    reasoningEffort,
    verboseMode,
  } from "$lib/stores/chat";
  import { activeProject } from "$lib/stores/projects";
  import { settings } from "$lib/stores/settings";
  import {
    cliOverview,
    inventory,
    loadCliOverview,
    loadInventory,
    setMcpEnabled,
    setPluginEnabled,
  } from "$lib/stores/capabilities";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();

  onMount(() => {
    void refreshContext();
  });

  async function refreshContext() {
    await Promise.all([loadInventory(), loadCliOverview($activeProject?.path ?? null)]);
  }
</script>

{#if open}
  <aside class="panel">
    <header>
      <h3>Context</h3>
      <button type="button" onclick={onclose} title="Close">x</button>
    </header>

    <section>
      <div class="label">Status</div>
      <div class="value" class:hot={$isRunning}>{$status.detail}</div>
    </section>

    <section class="matrix">
      <div>
        <div class="label">Model</div>
        <div class="value">{$selectedModel}</div>
      </div>
      <div>
        <div class="label">YOLO</div>
        <div class="value">{$yoloEnabled ? "On" : "Off"}</div>
      </div>
      <div>
        <div class="label">Reasoning</div>
        <div class="value">{$reasoningEffort}</div>
      </div>
      <div>
        <div class="label">Plan</div>
        <div class="value">{$settings.plan_mode ? "On" : "Off"}</div>
      </div>
      <div>
        <div class="label">Output</div>
        <div class="value">{$verboseMode ? "Verbose" : "Hidden"}</div>
      </div>
    </section>

    <section>
      <div class="label">Project</div>
      <div class="value">
        {#if $activeProject}
          <div>{$activeProject.name}</div>
          <div class="path">{$activeProject.path}</div>
          <div class="chips">
            <span>{$activeProject.project_type}</span>
            {#if $activeProject.archived}<span>Archived</span>{/if}
          </div>
        {:else}
          Workspace default
        {/if}
      </div>
    </section>

    <section>
      <div class="section-head">
        <div class="label">MCP Servers</div>
        <button type="button" onclick={refreshContext}>Refresh</button>
      </div>
      {#each $inventory.mcp_servers as server (server.name)}
        <label class="capability">
          <input
            type="checkbox"
            checked={server.enabled}
            disabled={!server.can_toggle}
            title={server.can_toggle
              ? "Enable or disable this config server"
              : "Controlled by plugin or Grok connector settings"}
            onchange={(e) =>
              setMcpEnabled(server.name, (e.currentTarget as HTMLInputElement).checked)}
          />
          <span title={`${server.status} · ${server.source} · ${server.command}`}>
            <strong>{server.name}</strong>
            <small>{server.status}</small>
          </span>
        </label>
      {:else}
        <div class="muted">No MCP servers detected.</div>
      {/each}
    </section>

    <section>
      <div class="label">Grok Build runtime</div>
      <div class="value">
        <div>
          v{$cliOverview.version || "unknown"} · {$cliOverview.channel || "unknown channel"}
        </div>
        {#if $cliOverview.commit}<div class="path">Commit {$cliOverview.commit}</div>{/if}
        <div class="chips">
          <span>{$cliOverview.compatibility || "Compatibility unknown"}</span>
        </div>
      </div>
    </section>

    <section>
      <div class="label">Grok CLI Capabilities</div>
      <div class="cap-list">
        {#each $cliOverview.capabilities as item (item.name)}
          <div class="cap-row" class:enabled={item.available}>
            <span class="cap-dot" aria-hidden="true"></span>
            <span>
              <strong>{item.name}</strong>
              <small>{item.detail}</small>
            </span>
          </div>
        {:else}
          <div class="muted">No CLI capability data loaded.</div>
        {/each}
      </div>
    </section>

    <section>
      <div class="label">Recent Grok Sessions</div>
      {#each $cliOverview.sessions as session (session.id)}
        <div class="data-row" title={session.id}>
          <strong>{session.summary || session.id}</strong>
          <small>{session.status} · updated {session.updated}</small>
        </div>
      {:else}
        <div class="muted">No recent Grok CLI sessions found.</div>
      {/each}
    </section>

    <section>
      <div class="label">Tracked Worktrees</div>
      {#each $cliOverview.worktrees as worktree (worktree.id || worktree.path)}
        <div class="data-row" title={worktree.path}>
          <strong>{worktree.name || worktree.branch || worktree.path}</strong>
          <small>{worktree.status || "tracked"} · {worktree.branch || "no branch"}</small>
        </div>
      {:else}
        <div class="muted">No Grok worktrees tracked for this context.</div>
      {/each}
    </section>

    {#if $cliOverview.errors.length}
      <section>
        <div class="label">CLI Warnings</div>
        {#each $cliOverview.errors as error}
          <div class="warning">{error}</div>
        {/each}
      </section>
    {/if}

    <section>
      <div class="label">Plugins</div>
      {#each $inventory.plugins as plugin (plugin.name)}
        <label class="capability">
          <input
            type="checkbox"
            checked={plugin.enabled}
            onchange={(e) =>
              setPluginEnabled(plugin.name, (e.currentTarget as HTMLInputElement).checked)}
          />
          <span title={plugin.detail}>{plugin.name}</span>
        </label>
      {:else}
        <div class="muted">No plugins installed.</div>
      {/each}
    </section>

    <section>
      <div class="label">Active Flags</div>
      <div class="flags">
        {#if !$settings.subagents_enabled}<span>--no-subagents</span>{/if}
        {#if $settings.disable_web_search}<span>--disable-web-search</span>{/if}
        {#if $settings.memory_enabled}<span>--experimental-memory</span>{:else}<span
            >--no-memory</span
          >{/if}
        {#if $settings.permission_mode !== "default"}<span
            >--permission-mode {$settings.permission_mode}</span
          >{/if}
      </div>
    </section>
  </aside>
{/if}

<style>
  .panel {
    width: 310px;
    min-width: 310px;
    border-left: 1px solid var(--border);
    background: var(--sidebar);
    padding: 0.85rem;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    overflow-y: auto;
  }
  header,
  .section-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  h3 {
    margin: 0;
    font-size: 0.95rem;
  }
  header button,
  .section-head button {
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 7px;
    padding: 0.25rem 0.45rem;
    cursor: pointer;
  }
  section {
    display: grid;
    gap: 0.35rem;
  }
  .matrix {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.65rem;
  }
  .label {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
  }
  .value {
    font-size: 0.88rem;
    color: var(--text);
  }
  .value.hot {
    color: var(--accent);
  }
  .path {
    font-size: 0.75rem;
    color: var(--muted);
    word-break: break-all;
    margin-top: 0.2rem;
  }
  .chips,
  .flags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-top: 0.45rem;
  }
  .chips span,
  .flags span {
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.12rem 0.45rem;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.72rem;
  }
  .capability {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 0.45rem;
    min-width: 0;
    font-size: 0.82rem;
    color: var(--text);
  }
  .capability span {
    display: grid;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .capability small {
    color: var(--muted);
    font-size: 0.68rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .muted {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .cap-list {
    display: grid;
    gap: 0.35rem;
  }
  .cap-row,
  .data-row {
    display: grid;
    gap: 0.1rem;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.42rem 0.5rem;
    background: rgba(255, 255, 255, 0.02);
    color: var(--text);
    font-size: 0.8rem;
  }
  .cap-row {
    grid-template-columns: auto 1fr;
    align-items: start;
    gap: 0.45rem;
  }
  .cap-row span:last-child,
  .data-row {
    overflow: hidden;
  }
  .cap-row strong,
  .data-row strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cap-row small,
  .data-row small {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cap-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    margin-top: 0.35rem;
    background: var(--muted);
  }
  .cap-row.enabled .cap-dot {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
  }
  .warning {
    border: 1px solid rgba(255, 180, 120, 0.3);
    border-radius: 8px;
    padding: 0.42rem 0.5rem;
    color: #ffd0a0;
    background: rgba(255, 180, 120, 0.06);
    font-size: 0.75rem;
    word-break: break-word;
  }
</style>
