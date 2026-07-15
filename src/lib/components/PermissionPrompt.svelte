<script lang="ts">
  import { pendingPermission, resolvePermission } from "$lib/stores/chat";

  let deciding = $state(false);

  const details = $derived.by(() => {
    const input = $pendingPermission?.tool_call?.rawInput;
    if (input == null) return "";
    if (typeof input === "string") return input;
    try {
      return JSON.stringify(input, null, 2);
    } catch {
      return String(input);
    }
  });

  async function choose(optionId: string | null) {
    deciding = true;
    try {
      await resolvePermission(optionId);
    } finally {
      deciding = false;
    }
  }
</script>

{#if $pendingPermission}
  <div class="permission" role="alertdialog" aria-labelledby="permission-title">
    <div class="icon" aria-hidden="true">!</div>
    <div class="copy">
      <div class="eyebrow">Approval required</div>
      <strong id="permission-title">{$pendingPermission.title}</strong>
      {#if details}
        <pre>{details}</pre>
      {/if}
      <p>Grok is paused. Nothing happens until you choose.</p>
    </div>
    <div class="choices">
      {#each $pendingPermission.options as option (option.id)}
        <button
          type="button"
          class:allow={option.kind.startsWith("allow")}
          class:reject={option.kind.startsWith("reject")}
          disabled={deciding}
          onclick={() => choose(option.id)}>{option.name}</button
        >
      {/each}
      {#if !$pendingPermission.options.some((option) => option.kind.startsWith("reject"))}
        <button type="button" class="reject" disabled={deciding} onclick={() => choose(null)}
          >Deny</button
        >
      {/if}
    </div>
  </div>
{/if}

<style>
  .permission {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: start;
    gap: 0.75rem;
    margin-bottom: 0.65rem;
    padding: 0.8rem;
    border: 1px solid color-mix(in srgb, var(--accent) 65%, var(--border));
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface) 92%, var(--accent) 8%);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 1.65rem;
    height: 1.65rem;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-contrast);
    font-weight: 900;
  }
  .copy {
    min-width: 0;
  }
  .eyebrow {
    margin-bottom: 0.15rem;
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  strong {
    display: block;
    font-size: 0.9rem;
  }
  p {
    margin: 0.3rem 0 0;
    color: var(--muted);
    font-size: 0.75rem;
  }
  pre {
    max-height: 90px;
    overflow: auto;
    margin: 0.45rem 0 0;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg);
    color: var(--text);
    font:
      0.72rem/1.35 ui-monospace,
      SFMono-Regular,
      Menlo,
      Consolas,
      monospace;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.4rem;
    max-width: 320px;
  }
  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.45rem 0.7rem;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
    font-weight: 700;
    cursor: pointer;
  }
  button.allow {
    border-color: var(--accent-dim);
    background: var(--accent-gradient);
    color: var(--accent-contrast);
  }
  button.reject {
    color: #ff9b91;
  }
  button:disabled {
    cursor: wait;
    opacity: 0.55;
  }
  @media (max-width: 760px) {
    .permission {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .choices {
      grid-column: 1 / -1;
      justify-content: stretch;
      max-width: none;
    }
    .choices button {
      flex: 1;
    }
  }
</style>
