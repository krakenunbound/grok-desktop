<script lang="ts">
  import { onMount } from "svelte";
  import {
    checkForUpdates,
    installAvailableUpdate,
    updateError,
    updatePhase,
    updateProgress,
    updateVersion,
  } from "$lib/stores/updater";

  let transientLabel = $state("");

  onMount(() => {
    const initial = window.setTimeout(() => void checkForUpdates(true), 5_000);
    const periodic = window.setInterval(() => void checkForUpdates(true), 6 * 60 * 60 * 1_000);
    return () => {
      window.clearTimeout(initial);
      window.clearInterval(periodic);
    };
  });

  async function activate() {
    if ($updatePhase === "available") {
      const confirmed = window.confirm(`Install Grok Desktop v${$updateVersion} and restart now?`);
      if (confirmed) await installAvailableUpdate();
      return;
    }
    const found = await checkForUpdates(false);
    if (!found && $updatePhase === "current") {
      transientLabel = "Up to date";
      window.setTimeout(() => (transientLabel = ""), 2_500);
    }
  }

  let label = $derived(
    transientLabel ||
      ($updatePhase === "available"
        ? `Update v${$updateVersion}`
        : $updatePhase === "checking"
          ? "Checking…"
          : $updatePhase === "downloading"
            ? `Updating ${$updateProgress}%`
            : "Updates"),
  );
</script>

<button
  type="button"
  class:available={$updatePhase === "available"}
  class:error={$updatePhase === "error"}
  disabled={$updatePhase === "checking" || $updatePhase === "downloading"}
  title={$updateError || "Check GitHub Releases for a signed update"}
  onclick={activate}
>
  {#if $updatePhase === "available"}<span class="dot" aria-hidden="true"></span>{/if}
  {label}
</button>

<style>
  button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 8px;
    padding: 0.35rem 0.7rem;
    cursor: pointer;
    font: inherit;
  }
  button.available {
    border-color: var(--accent-dim);
    color: var(--accent);
  }
  button.error {
    color: #ffb0a8;
  }
  button:disabled {
    opacity: 0.65;
    cursor: default;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
  }
</style>
