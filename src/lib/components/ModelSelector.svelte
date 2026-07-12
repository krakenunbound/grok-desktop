<script lang="ts">
  import { reasoningEffort, selectedModel } from "$lib/stores/chat";
  import {
    models as modelList,
    persistSettings,
    settings,
    type ReasoningEffort,
  } from "$lib/stores/settings";
  import { invoke } from "@tauri-apps/api/core";

  let list = $derived($modelList.length ? $modelList : ["grok-4.5", "grok-composer-2.5-fast"]);

  function modelLabel(model: string): string {
    if (model === "grok-4.5") return "Grok 4.5 · Frontier";
    if (model === "grok-composer-2.5-fast") return "Composer 2.5 · Fast";
    return model;
  }

  async function changeModel(event: Event) {
    const value = (event.target as HTMLSelectElement).value;
    selectedModel.set(value);
    await persistSettings({ ...$settings, default_model: value });
    try {
      await invoke("set_session_model", { model: value });
    } catch {
      /* The next session start will use the selection. */
    }
  }

  async function changeEffort(event: Event) {
    const value = (event.target as HTMLSelectElement).value as ReasoningEffort;
    reasoningEffort.set(value);
    await persistSettings({ ...$settings, reasoning_effort: value });
  }
</script>

<div class="selectors">
  <label title="Model">
    <span class="sr">Model</span>
    <select value={$selectedModel} onchange={changeModel} aria-label="Model">
      {#each list as model}
        <option value={model}>{modelLabel(model)}</option>
      {/each}
    </select>
  </label>
  <label title="Reasoning effort">
    <span class="sr">Reasoning effort</span>
    <select
      class="effort"
      value={$reasoningEffort}
      onchange={changeEffort}
      aria-label="Reasoning effort"
    >
      <option value="low">Low · Quick</option>
      <option value="medium">Medium · Balanced</option>
      <option value="high">High · Deep</option>
    </select>
  </label>
</div>

<style>
  .selectors {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  select {
    appearance: none;
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4rem 1.7rem 0.4rem 0.65rem;
    font-size: 0.78rem;
    font-family: inherit;
    cursor: pointer;
    background-image:
      linear-gradient(45deg, transparent 50%, var(--muted) 50%),
      linear-gradient(135deg, var(--muted) 50%, transparent 50%);
    background-position:
      calc(100% - 13px) 55%,
      calc(100% - 8px) 55%;
    background-size: 5px 5px;
    background-repeat: no-repeat;
  }
  select:hover,
  select:focus {
    border-color: var(--accent-dim);
    outline: none;
  }
  .effort {
    max-width: 142px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  @media (max-width: 1050px) {
    .selectors {
      gap: 0.25rem;
    }
    select {
      max-width: 145px;
    }
    .effort {
      max-width: 122px;
    }
  }
</style>
