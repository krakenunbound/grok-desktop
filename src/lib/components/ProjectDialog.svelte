<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    open: boolean;
    onclose: () => void;
    onselect: (path: string) => void | Promise<void>;
  }

  let { open: visible, onclose, onselect }: Props = $props();
  let view = $state<"choose" | "create">("choose");
  let name = $state("");
  let parent = $state("");
  let busy = $state(false);
  let error = $state("");

  $effect(() => {
    if (visible) {
      view = "choose";
      name = "";
      parent = "";
      error = "";
    }
  });

  async function chooseExisting() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose the folder for this project",
    });
    if (selected && !Array.isArray(selected)) await finish(selected);
  }

  async function chooseParent() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose where to create the project folder",
    });
    if (selected && !Array.isArray(selected)) parent = selected;
  }

  async function createFolder() {
    if (!name.trim() || !parent) return;
    busy = true;
    error = "";
    try {
      const path = await invoke<string>("create_project_folder", { parent, name });
      await finish(path);
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function finish(path: string) {
    busy = true;
    error = "";
    try {
      await onselect(path);
      onclose();
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  function onBackdrop(event: MouseEvent) {
    if (event.target === event.currentTarget && !busy) onclose();
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (visible && event.key === "Escape" && !busy) onclose();
  }}
/>

{#if visible}
  <div class="backdrop" role="presentation" onclick={onBackdrop}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="project-title"
      tabindex="-1"
    >
      <header>
        <div>
          <h2 id="project-title">
            {view === "choose" ? "New Project" : "Create a new project"}
          </h2>
          <p>
            {view === "choose"
              ? "Choose an existing folder, or create a new folder for this project."
              : "Name the project and choose where its new folder should be created."}
          </p>
        </div>
        <button type="button" class="close" onclick={onclose} aria-label="Close">×</button>
      </header>

      {#if view === "choose"}
        <div class="choices">
          <button type="button" class="choice primary" onclick={() => (view = "create")}>
            <span class="choice-icon" aria-hidden="true">＋</span>
            <span
              ><strong>Create a new project</strong><small>Create a new folder and workspace</small
              ></span
            >
            <span aria-hidden="true">›</span>
          </button>
          <button type="button" class="choice" onclick={chooseExisting}>
            <span class="choice-icon folder" aria-hidden="true">◇</span>
            <span
              ><strong>Use an existing folder</strong><small
                >Make an existing folder the project workspace</small
              ></span
            >
            <span aria-hidden="true">›</span>
          </button>
        </div>
      {:else}
        <div class="fields">
          <label>
            Project name
            <input bind:value={name} maxlength="128" placeholder="My project" />
          </label>
          <label>
            Project location
            <div class="location">
              <input value={parent} readonly placeholder="Choose where to create it" />
              <button type="button" onclick={chooseParent}>Browse…</button>
            </div>
          </label>
          {#if parent && name.trim()}
            <div class="preview" title={`${parent}\\${name.trim()}`}>{parent}\{name.trim()}</div>
          {/if}
        </div>
        <footer>
          <button type="button" class="secondary" onclick={() => (view = "choose")}>Back</button>
          <button
            type="button"
            class="create"
            disabled={busy || !name.trim() || !parent}
            onclick={createFolder}
          >
            {busy ? "Creating…" : "Create project"}
          </button>
        </footer>
      {/if}

      {#if error}<div class="error" role="alert">{error}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: grid;
    place-items: center;
    background: rgba(3, 5, 9, 0.68);
    backdrop-filter: blur(5px);
    padding: 1.5rem;
  }
  .dialog {
    width: min(520px, 100%);
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.55);
    padding: 1.1rem;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: start;
    margin-bottom: 1rem;
  }
  h2 {
    margin: 0;
    font-size: 1.2rem;
    color: var(--text);
  }
  p {
    margin: 0.35rem 0 0;
    color: var(--muted);
    font-size: 0.84rem;
    line-height: 1.45;
  }
  .close {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 1.25rem;
    cursor: pointer;
  }
  .choices {
    display: grid;
    gap: 0.7rem;
  }
  .choice {
    width: 100%;
    display: grid;
    grid-template-columns: 42px 1fr auto;
    align-items: center;
    gap: 0.8rem;
    text-align: left;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    padding: 0.85rem;
    cursor: pointer;
  }
  .choice:hover,
  .choice:focus-visible {
    border-color: var(--accent-dim);
    transform: translateY(-1px);
  }
  .choice.primary {
    background: linear-gradient(135deg, rgba(74, 198, 255, 0.11), rgba(25, 224, 205, 0.05));
  }
  .choice-icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    font-size: 1.2rem;
  }
  .choice-icon.folder {
    background: var(--surface);
    color: var(--accent);
    border: 1px solid var(--border);
  }
  .choice span:nth-child(2) {
    display: grid;
    gap: 0.2rem;
  }
  .choice small {
    color: var(--muted);
    font-size: 0.76rem;
  }
  .fields {
    display: grid;
    gap: 0.9rem;
  }
  label {
    display: grid;
    gap: 0.38rem;
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 650;
  }
  input {
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--bg);
    color: var(--text);
    padding: 0.65rem 0.7rem;
    font: inherit;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .location {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.45rem;
  }
  .location button,
  footer button {
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 0.6rem 0.75rem;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .preview {
    border-radius: 8px;
    background: var(--bg);
    color: var(--muted);
    padding: 0.55rem 0.65rem;
    font-size: 0.72rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  footer {
    display: flex;
    justify-content: space-between;
    margin-top: 1rem;
  }
  footer .create {
    border: 0;
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    font-weight: 750;
  }
  footer button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .error {
    margin-top: 0.8rem;
    border: 1px solid rgba(255, 90, 90, 0.35);
    border-radius: 8px;
    background: rgba(255, 70, 70, 0.08);
    color: #ffb0b0;
    padding: 0.6rem 0.7rem;
    font-size: 0.78rem;
  }
</style>
