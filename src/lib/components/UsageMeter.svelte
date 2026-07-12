<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";

  interface UsageSnapshot {
    available: boolean;
    usagePercent: number;
    remainingPercent: number;
    periodStart: string | null;
    periodEnd: string | null;
    prepaidBalanceCents: number;
    onDemandUsedCents: number;
    onDemandCapCents: number;
    subscriptionTier: string | null;
    updatedAt: string | null;
    source: string;
    detail: string;
  }

  let usage = $state<UsageSnapshot | null>(null);
  let open = $state(false);
  let loading = $state(false);
  let container: HTMLDivElement;

  onMount(() => {
    let doneUnlisten: UnlistenFn | undefined;
    const timer = window.setInterval(() => void refresh(), 60_000);
    void listen("grok-done", () => void refresh()).then((stop) => (doneUnlisten = stop));
    void refresh();
    return () => {
      window.clearInterval(timer);
      doneUnlisten?.();
    };
  });

  async function refresh() {
    loading = true;
    try {
      usage = await invoke<UsageSnapshot>("get_usage");
    } catch {
      usage = {
        available: false,
        usagePercent: 0,
        remainingPercent: 0,
        periodStart: null,
        periodEnd: null,
        prepaidBalanceCents: 0,
        onDemandUsedCents: 0,
        onDemandCapCents: 0,
        subscriptionTier: null,
        updatedAt: null,
        source: "Grok CLI telemetry",
        detail: "The usage snapshot could not be read.",
      };
    } finally {
      loading = false;
    }
  }

  function toggle(event: MouseEvent) {
    open = !open;
    if (open) void refresh();
  }

  function closeOutside(event: MouseEvent) {
    if (!container?.contains(event.target as Node)) open = false;
  }

  function money(cents: number): string {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: "USD" }).format(
      cents / 100,
    );
  }

  function resetLabel(value: string | null): string {
    if (!value) return "Unknown";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return date.toLocaleString([], { weekday: "short", hour: "numeric", minute: "2-digit" });
  }

  function freshness(value: string | null): string {
    if (!value) return "No snapshot";
    const elapsed = Math.max(0, Date.now() - new Date(value).getTime());
    const minutes = Math.floor(elapsed / 60_000);
    if (minutes < 1) return "Updated now";
    if (minutes < 60) return `Updated ${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `Updated ${hours}h ago`;
    return `Updated ${Math.floor(hours / 24)}d ago`;
  }
</script>

<svelte:window
  onclick={closeOutside}
  onkeydown={(event) => {
    if (event.key === "Escape") open = false;
  }}
/>

<div class="usage-wrap" bind:this={container}>
  <button
    type="button"
    class="usage-pill"
    class:warning={(usage?.usagePercent ?? 0) >= 80}
    class:exhausted={(usage?.usagePercent ?? 0) >= 100}
    aria-expanded={open}
    aria-haspopup="dialog"
    title="Grok usage and credits"
    onclick={toggle}
  >
    <span class="ring" style={`--used: ${usage?.usagePercent ?? 0}%`}></span>
    <span>{usage?.available ? `${Math.round(usage.remainingPercent)}% left` : "Usage"}</span>
  </button>

  {#if open}
    <div class="usage-card" role="dialog" aria-label="Grok usage and credits" tabindex="-1">
      <header>
        <div><strong>Usage</strong><small>{usage?.subscriptionTier || "Grok account"}</small></div>
        <button type="button" class="refresh" disabled={loading} onclick={() => refresh()}>
          {loading ? "…" : "↻"}
        </button>
      </header>

      {#if usage?.available}
        <div class="allocation-head">
          <strong>{Math.round(usage.remainingPercent)}% remaining</strong>
          <span>{Math.round(usage.usagePercent)}% used</span>
        </div>
        <div class="bar" aria-label={`${usage.usagePercent}% used`}>
          <span style={`width: ${Math.min(100, usage.usagePercent)}%`}></span>
        </div>
        <div class="reset">Resets {resetLabel(usage.periodEnd)}</div>

        <div class="money-grid">
          <div>
            <span>Paid credits left</span><strong>{money(usage.prepaidBalanceCents)}</strong>
          </div>
          <div><span>On-demand used</span><strong>{money(usage.onDemandUsedCents)}</strong></div>
          <div><span>On-demand cap</span><strong>{money(usage.onDemandCapCents)}</strong></div>
        </div>

        {#if usage.usagePercent >= 100}
          <div class="notice">
            Normal allocation is exhausted. Paid credits or on-demand billing may now apply.
          </div>
        {/if}
        <footer title={usage.detail}>{freshness(usage.updatedAt)} · {usage.source}</footer>
      {:else}
        <div class="unavailable">
          <strong>Usage unavailable</strong>
          <span
            >Grok has not published local billing telemetry yet. Run an authenticated CLI session,
            then refresh.</span
          >
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .usage-wrap {
    position: relative;
  }
  .usage-pill {
    height: 30px;
    display: inline-flex;
    align-items: center;
    gap: 0.42rem;
    padding: 0 0.65rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.74rem;
    cursor: pointer;
    white-space: nowrap;
  }
  .usage-pill:hover,
  .usage-pill[aria-expanded="true"] {
    border-color: var(--accent-dim);
    background: var(--surface-2);
  }
  .ring {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: conic-gradient(var(--accent) var(--used), var(--border) 0);
    position: relative;
  }
  .ring::after {
    content: "";
    position: absolute;
    inset: 3px;
    border-radius: 50%;
    background: var(--surface);
  }
  .usage-pill.warning .ring {
    background: conic-gradient(#f4b942 var(--used), var(--border) 0);
  }
  .usage-pill.exhausted .ring {
    background: #ff6868;
  }
  .usage-card {
    position: absolute;
    top: calc(100% + 0.55rem);
    right: 0;
    z-index: 80;
    width: 310px;
    padding: 0.9rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.5);
    display: grid;
    gap: 0.7rem;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  header div {
    display: grid;
    gap: 0.1rem;
  }
  header small,
  .allocation-head span,
  .reset,
  footer {
    color: var(--muted);
    font-size: 0.7rem;
  }
  .refresh {
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface-2);
    color: var(--text);
    width: 28px;
    height: 28px;
    cursor: pointer;
  }
  .allocation-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .bar {
    height: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-2);
  }
  .bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent-gradient);
  }
  .money-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.35rem;
  }
  .money-grid div {
    min-width: 0;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
    display: grid;
    gap: 0.18rem;
  }
  .money-grid span {
    color: var(--muted);
    font-size: 0.62rem;
    line-height: 1.2;
  }
  .money-grid strong {
    font-size: 0.78rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .notice {
    padding: 0.55rem 0.65rem;
    border: 1px solid rgba(255, 104, 104, 0.35);
    border-radius: 8px;
    background: rgba(255, 104, 104, 0.08);
    color: #ffc0c0;
    font-size: 0.72rem;
    line-height: 1.35;
  }
  .unavailable {
    display: grid;
    gap: 0.25rem;
    padding: 0.65rem;
    border-radius: 8px;
    background: var(--surface-2);
  }
  .unavailable span {
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.4;
  }
  footer {
    border-top: 1px solid var(--border);
    padding-top: 0.55rem;
  }
</style>
