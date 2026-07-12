<script lang="ts">
  import { settings, persistSettings, type AppSettings } from "$lib/stores/settings";
  import { verboseMode } from "$lib/stores/chat";
  import {
    inventory,
    loadInventory,
    setMcpEnabled,
    setPluginEnabled,
  } from "$lib/stores/capabilities";
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();

  let draft = $state<AppSettings>({ ...$settings });
  let grokPath = $state<string>("");
  let appData = $state<string>("");
  let saving = $state(false);
  let msg = $state("");
  let advancedOpen = $state(false);

  $effect(() => {
    if (open) {
      draft = { ...$settings };
      void loadMeta();
      void loadInventory();
    }
  });

  async function loadMeta() {
    try {
      grokPath = await invoke<string>("resolve_grok_binary");
    } catch (e) {
      grokPath = String(e);
    }
    try {
      appData = await invoke<string>("get_app_data_dir");
    } catch {
      appData = "";
    }
  }

  async function save() {
    saving = true;
    msg = "";
    try {
      await persistSettings(draft);
      verboseMode.set(!!draft.verbose_mode);
      msg = "Saved";
      setTimeout(onclose, 400);
    } catch (e) {
      msg = String(e);
    } finally {
      saving = false;
    }
  }

  function onBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={onBackdrop}
    onkeydown={(e) => {
      if (e.key === "Escape") onclose();
    }}
  >
    <div class="modal" role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1">
      <header>
        <div>
          <h2>Settings</h2>
          <p>Session defaults and Grok Build controls</p>
        </div>
        <button type="button" class="x" onclick={onclose} aria-label="Close settings">x</button>
      </header>

      <div class="grid">
        <label>
          Default model
          <input bind:value={draft.default_model} />
        </label>

        <label>
          Permission mode
          <select bind:value={draft.permission_mode}>
            <option value="default">Default</option>
            <option value="acceptEdits">Accept edits</option>
            <option value="auto">Auto</option>
            <option value="dontAsk">Don't ask</option>
            <option value="bypassPermissions">Bypass permissions</option>
            <option value="plan">Plan</option>
          </select>
        </label>
      </div>

      <div class="toggles">
        <label class="check">
          <input type="checkbox" bind:checked={draft.yolo_default} />
          YOLO by default
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.plan_mode} />
          Plan mode
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.subagents_enabled} />
          Subagents
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.memory_enabled} />
          Memory
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.disable_web_search} />
          Disable web search
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.verbose_mode} />
          Verbose output
        </label>
      </div>

      <button type="button" class="advanced-toggle" onclick={() => (advancedOpen = !advancedOpen)}>
        {advancedOpen ? "Hide advanced options" : "Show advanced options"}
      </button>

      {#if advancedOpen}
        <div class="advanced">
          <label>
            Max turns
            <input bind:value={draft.max_turns} placeholder="Optional, e.g. 20" />
          </label>
          <label>
            Allowed built-in tools
            <input bind:value={draft.tools} placeholder="Comma-separated --tools" />
          </label>
          <label>
            Disabled built-in tools
            <input
              bind:value={draft.disallowed_tools}
              placeholder="Comma-separated --disallowed-tools"
            />
          </label>
          <label>
            Allow rules
            <textarea bind:value={draft.allow_rules} placeholder="One --allow rule per line"
            ></textarea>
          </label>
          <label>
            Deny rules
            <textarea bind:value={draft.deny_rules} placeholder="One --deny rule per line"
            ></textarea>
          </label>
          <label>
            Extra rules
            <textarea bind:value={draft.extra_rules} placeholder="Passed to --rules"></textarea>
          </label>
        </div>
      {/if}

      <section>
        <div class="section-head">
          <h3>MCP Servers</h3>
          <button type="button" class="ghost small" onclick={() => loadInventory()}>Refresh</button>
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
            <span>
              <strong>{server.name}</strong>
              <small>{server.status} · {server.source}</small>
              <small>{server.command}</small>
            </span>
          </label>
        {:else}
          <div class="empty">No MCP servers detected.</div>
        {/each}
      </section>

      <section>
        <h3>Plugins</h3>
        {#each $inventory.plugins as plugin (plugin.name)}
          <label class="capability">
            <input
              type="checkbox"
              checked={plugin.enabled}
              onchange={(e) =>
                setPluginEnabled(plugin.name, (e.currentTarget as HTMLInputElement).checked)}
            />
            <span>
              <strong>{plugin.name}</strong>
              <small>{plugin.detail}</small>
            </span>
          </label>
        {:else}
          <div class="empty">No plugins installed.</div>
        {/each}
      </section>

      <div class="paths">
        <label>
          Grok binary override
          <input
            bind:value={draft.grok_binary}
            placeholder="Leave empty to use PATH / ~/.grok/bin"
          />
        </label>
        <label>
          Attachment storage directory
          <input bind:value={draft.temp_images_dir} placeholder="App data / temp_images" />
        </label>
      </div>

      <div class="meta">
        <div><strong>Resolved grok:</strong> {grokPath}</div>
        <div><strong>App data:</strong> {appData}</div>
        <div><strong>Grok config:</strong> {$inventory.config_path || "Not found"}</div>
      </div>

      {#if msg}
        <div class="msg">{msg}</div>
      {/if}

      <footer>
        <button type="button" class="ghost" onclick={onclose}>Cancel</button>
        <button type="button" class="primary" onclick={save} disabled={saving}>
          {saving ? "Saving..." : "Save"}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.58);
    display: grid;
    place-items: center;
    z-index: 50;
    padding: 1rem;
  }
  .modal {
    width: min(760px, 100%);
    max-height: min(860px, calc(100vh - 2rem));
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1rem;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  header,
  .section-head,
  footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
  }
  h2,
  h3,
  p {
    margin: 0;
  }
  h2 {
    font-size: 1.05rem;
  }
  h3 {
    font-size: 0.82rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  header p {
    color: var(--muted);
    font-size: 0.78rem;
  }
  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.1rem;
    cursor: pointer;
  }
  .grid,
  .paths {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.75rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 0.45rem;
    color: var(--text);
  }
  .toggles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.45rem 0.75rem;
  }
  input:not([type="checkbox"]),
  select,
  textarea {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.45rem 0.6rem;
    color: var(--text);
    font: inherit;
  }
  textarea {
    min-height: 64px;
    resize: vertical;
  }
  .advanced-toggle {
    align-self: flex-start;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    border-radius: 8px;
    padding: 0.4rem 0.65rem;
    cursor: pointer;
  }
  .advanced {
    display: grid;
    gap: 0.65rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
  }
  section {
    display: grid;
    gap: 0.4rem;
  }
  .capability {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 0.55rem;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    color: var(--text);
  }
  .capability span {
    display: grid;
    min-width: 0;
  }
  .capability small,
  .empty,
  .meta {
    color: var(--muted);
    font-size: 0.75rem;
  }
  .capability small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: grid;
    gap: 0.25rem;
    word-break: break-all;
  }
  .ghost,
  .primary {
    border-radius: 8px;
    padding: 0.45rem 0.9rem;
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
  }
  .small {
    padding: 0.3rem 0.55rem;
    font-size: 0.75rem;
  }
  .primary {
    border: none;
    background: var(--accent-gradient);
    color: var(--accent-contrast);
  }
  .msg {
    font-size: 0.85rem;
    color: var(--accent);
  }
  @media (max-width: 760px) {
    .grid,
    .paths,
    .toggles {
      grid-template-columns: 1fr;
    }
  }
</style>
