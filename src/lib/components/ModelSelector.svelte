<script lang="ts">
  import { selectedModel } from "$lib/stores/chat";
  import { models as modelList } from "$lib/stores/settings";
  import { invoke } from "@tauri-apps/api/core";

  // Prefer settings-backed model list; fall back to defaults.
  let list = $derived($modelList.length ? $modelList : ["grok-4.5", "grok-4", "grok-3"]);

  async function onChange(e: Event) {
    const value = (e.target as HTMLSelectElement).value;
    selectedModel.set(value);
    try {
      await invoke("set_session_model", { model: value });
    } catch {
      /* no session yet — start_session will pick it up */
    }
  }
</script>

<label class="model">
  <span class="sr">Model</span>
  <select value={$selectedModel} onchange={onChange} title="Model">
    {#each list as m}
      <option value={m}>{m}</option>
    {/each}
  </select>
</label>

<style>
  .model select {
    appearance: none;
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4rem 1.8rem 0.4rem 0.7rem;
    font-size: 0.85rem;
    font-family: inherit;
    cursor: pointer;
    background-image:
      linear-gradient(45deg, transparent 50%, var(--muted) 50%),
      linear-gradient(135deg, var(--muted) 50%, transparent 50%);
    background-position:
      calc(100% - 14px) 55%,
      calc(100% - 9px) 55%;
    background-size:
      5px 5px,
      5px 5px;
    background-repeat: no-repeat;
  }
  .model select:hover {
    border-color: var(--accent-dim);
  }
  .model select:focus {
    outline: none;
    border-color: var(--accent);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
