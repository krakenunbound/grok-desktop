<script lang="ts">
  /**
   * Agent Transparency Mode toggle.
   * Off (default) = Hidden black-box status; On = Verbose raw agent stream.
   */
  import { verboseMode, setVerboseMode } from "$lib/stores/chat";

  async function toggle() {
    await setVerboseMode(!$verboseMode);
  }
</script>

<button
  type="button"
  class="verbose"
  class:on={$verboseMode}
  onclick={toggle}
  title={$verboseMode
    ? "Verbose Mode ON — raw agent/CLI output"
    : "Hidden mode (default) — high-level status only. Click for Verbose."}
  aria-pressed={$verboseMode}
>
  <span class="dot"></span>
  {$verboseMode ? "Verbose" : "Hidden"}
</button>

<style>
  .verbose {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--muted);
    border-radius: 999px;
    padding: 0.35rem 0.75rem;
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    cursor: pointer;
    transition:
      background 0.15s,
      border-color 0.15s,
      color 0.15s;
  }
  .verbose:hover {
    border-color: var(--accent-dim);
  }
  .verbose.on {
    color: #e8f0ff;
    background: linear-gradient(135deg, #2a3a55, #1a2840);
    border-color: #4a6a9a;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.45;
  }
  .verbose.on .dot {
    opacity: 1;
    background: #7eb6ff;
    box-shadow: 0 0 6px rgba(126, 182, 255, 0.5);
  }
</style>
