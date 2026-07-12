<script lang="ts">
  import { yoloEnabled } from "$lib/stores/chat";
  import { persistSettings, settings, type PermissionMode } from "$lib/stores/settings";
  import { invoke } from "@tauri-apps/api/core";

  type AccessChoice = "ask" | "edits" | "plan" | "full";
  let open = $state(false);
  let choice = $derived<AccessChoice>(
    $yoloEnabled
      ? "full"
      : $settings.permission_mode === "acceptEdits"
        ? "edits"
        : $settings.permission_mode === "plan"
          ? "plan"
          : "ask",
  );

  const labels: Record<AccessChoice, string> = {
    ask: "Ask before actions",
    edits: "Auto-approve edits",
    plan: "Plan only",
    full: "Full access",
  };

  async function select(next: AccessChoice) {
    open = false;
    const yolo = next === "full";
    const permissionMode: PermissionMode =
      next === "edits" ? "acceptEdits" : next === "plan" ? "plan" : "default";
    yoloEnabled.set(yolo);
    await persistSettings({
      ...$settings,
      yolo_default: yolo,
      permission_mode: permissionMode,
    });
    try {
      await invoke("set_session_yolo", { yolo });
    } catch {
      /* The next session start will use the selection. */
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions: mouseleave only dismisses a menu that remains fully keyboard/click operable -->
<div class="access" onmouseleave={() => (open = false)}>
  <button
    type="button"
    class="trigger"
    class:full={choice === "full"}
    class:plan={choice === "plan"}
    onclick={() => (open = !open)}
    onmouseenter={() => (open = true)}
    aria-haspopup="menu"
    aria-expanded={open}
    title="Choose what Grok may do without asking"
  >
    <span class="shield" aria-hidden="true">◇</span>
    {labels[choice]}
    <span class="chevron" aria-hidden="true">⌃</span>
  </button>

  {#if open}
    <div class="menu" role="menu" aria-label="Approval mode">
      <button type="button" class:active={choice === "ask"} onclick={() => select("ask")}>
        <strong>Ask before actions</strong><small>Confirm protected tool calls</small>
      </button>
      <button type="button" class:active={choice === "edits"} onclick={() => select("edits")}>
        <strong>Auto-approve edits</strong><small>Write files; ask for riskier actions</small>
      </button>
      <button type="button" class:active={choice === "plan"} onclick={() => select("plan")}>
        <strong>Plan only</strong><small>Explore and propose before changing files</small>
      </button>
      <button type="button" class:active={choice === "full"} onclick={() => select("full")}>
        <strong>Full access</strong><small>Always approve all tool executions</small>
      </button>
    </div>
  {/if}
</div>

<style>
  .access {
    position: relative;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 0.38rem;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 999px;
    padding: 0.38rem 0.68rem;
    font-size: 0.74rem;
    font-weight: 700;
    cursor: pointer;
  }
  .trigger:hover {
    border-color: var(--accent-dim);
  }
  .trigger.full {
    color: var(--accent-contrast);
    background: var(--accent-gradient);
    border-color: transparent;
  }
  .trigger.plan {
    color: #f4c979;
    border-color: rgba(244, 201, 121, 0.35);
  }
  .shield,
  .chevron {
    opacity: 0.75;
  }
  .menu {
    position: absolute;
    left: 0;
    bottom: calc(100% + 0.4rem);
    z-index: 40;
    width: 245px;
    padding: 0.35rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface);
    box-shadow: 0 16px 45px rgba(0, 0, 0, 0.48);
  }
  .menu button {
    width: 100%;
    display: grid;
    gap: 0.12rem;
    text-align: left;
    border: 0;
    border-radius: 8px;
    padding: 0.55rem 0.6rem;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  .menu button:hover,
  .menu button.active {
    background: var(--surface-2);
  }
  .menu button.active {
    box-shadow: inset 2px 0 var(--accent);
  }
  .menu small {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 400;
  }
</style>
