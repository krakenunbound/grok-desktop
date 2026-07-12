<script lang="ts">
  import { yoloEnabled } from "$lib/stores/chat";
  import { invoke } from "@tauri-apps/api/core";

  async function toggle() {
    const prev = $yoloEnabled;
    const next = !prev;
    yoloEnabled.set(next);
    try {
      await invoke("set_session_yolo", { yolo: next });
    } catch {
      // No session yet is fine — flag applies on next start.
      // Unexpected failures keep the optimistic UI; send path re-reads store.
    }
  }
</script>

<button
  type="button"
  class="yolo"
  class:on={$yoloEnabled}
  onclick={toggle}
  title={$yoloEnabled
    ? "Full access (YOLO) — tools are always approved"
    : "Confirm access — Grok asks before protected tool actions"}
  aria-pressed={$yoloEnabled}
>
  <span class="shield" aria-hidden="true">◇</span>
  {$yoloEnabled ? "Full access" : "Confirm access"}
</button>

<style>
  .yolo {
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
    letter-spacing: 0.04em;
    cursor: pointer;
    transition:
      background 0.15s,
      border-color 0.15s,
      color 0.15s;
  }
  .yolo:hover {
    border-color: var(--accent-dim);
  }
  .yolo.on {
    color: var(--accent-contrast);
    background: var(--accent-gradient);
    border-color: transparent;
    box-shadow: 0 0 0 1px var(--accent-glow);
  }
  .shield {
    font-size: 0.9rem;
    line-height: 1;
    opacity: 0.75;
  }
  .yolo.on .shield {
    opacity: 1;
  }
</style>
