<script lang="ts">
  import { onMount } from "svelte";
  import Message from "./Message.svelte";
  import ImageDropZone from "./ImageDropZone.svelte";
  import {
    currentChat,
    isRunning,
    runningSince,
    sendUserMessage,
    stopGeneration,
    status,
  } from "$lib/stores/chat";
  import { activeProject } from "$lib/stores/projects";

  let input = $state("");
  let scroller: HTMLDivElement | undefined = $state();
  let now = $state(Date.now());
  let elapsed = $derived($runningSince ? formatElapsed(now - $runningSince) : "");

  onMount(() => {
    const id = window.setInterval(() => {
      now = Date.now();
    }, 1000);
    return () => window.clearInterval(id);
  });

  function formatElapsed(ms: number): string {
    const total = Math.max(0, Math.floor(ms / 1000));
    const minutes = Math.floor(total / 60);
    const seconds = total % 60;
    if (minutes === 0) return `${seconds}s`;
    return `${minutes}m ${seconds.toString().padStart(2, "0")}s`;
  }

  $effect(() => {
    // Auto-scroll when messages or streaming content change.
    const _msgs = $currentChat?.messages;
    const _status = $status.detail;
    queueMicrotask(() => {
      if (scroller) scroller.scrollTop = scroller.scrollHeight;
    });
  });

  async function onSubmit(e: Event) {
    e.preventDefault();
    const text = input.trim();
    // Prefer active project path; otherwise backend start_session uses app data dir from boot.
    const cwd = $activeProject?.path ?? "";
    if (!cwd) {
      // Chat store ensureSession requires a real directory — resolve via settings path later.
      // Empty string is rejected by backend; use a last-resort of process-relative path only if needed.
    }
    input = "";
    await sendUserMessage(text, cwd || ".");
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void onSubmit(e);
    }
  }
</script>

<section class="chat">
  <div class="messages" bind:this={scroller}>
    {#if !$currentChat || $currentChat.messages.length === 0}
      <div class="empty">
        <img class="logo" src="/grok-gui-logo.webp" alt="" aria-hidden="true" />
        <h2>Grok Desktop</h2>
        <p>
          Chat with Grok Build without the terminal. Drop images, toggle YOLO, pick a project, and
          go.
        </p>
        <ul>
          <li><kbd>Enter</kbd> send · <kbd>Shift+Enter</kbd> newline</li>
          <li><kbd>Ctrl+N</kbd> new chat · <kbd>Ctrl+Shift+Y</kbd> YOLO</li>
          <li><kbd>Ctrl+Shift+V</kbd> Verbose / Hidden agent output</li>
          <li>Paste or drop screenshots · say “show raw output” to reveal</li>
        </ul>
      </div>
    {:else}
      {#each $currentChat.messages as msg (msg.id)}
        <Message message={msg} />
      {/each}
    {/if}
  </div>

  <footer class="composer">
    <div class="status-line" class:running={$isRunning} role="status" aria-live="polite">
      {#if $isRunning}
        <span class="pulse" aria-hidden="true"></span>
      {/if}
      <span>{$status.detail || ($isRunning ? "Thinking…" : "Ready")}</span>
      {#if $activeProject}
        <span class="sep">·</span>
        <span class="cwd" title={$activeProject.path}>{$activeProject.name}</span>
      {/if}
      {#if $isRunning && elapsed}
        <span class="sep">·</span>
        <span>{elapsed}</span>
      {/if}
    </div>
    <form class="box" onsubmit={onSubmit}>
      <ImageDropZone />
      <textarea
        rows="2"
        placeholder="Message Grok..."
        bind:value={input}
        onkeydown={onKeydown}
        disabled={$isRunning}
      ></textarea>
      <div class="actions">
        {#if $isRunning}
          <button type="button" class="stop" onclick={() => stopGeneration()}>Stop</button>
        {/if}
        <button type="submit" class="send" disabled={$isRunning}>Send</button>
      </div>
    </form>
  </footer>
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--bg);
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem 1.5rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .empty {
    margin: auto;
    text-align: center;
    max-width: 420px;
    color: var(--muted);
    padding: 2rem 1rem;
  }
  .logo {
    width: 92px;
    height: 92px;
    margin: 0 auto 0.85rem;
    object-fit: contain;
    filter: drop-shadow(0 12px 26px rgba(0, 220, 220, 0.18));
  }
  .empty h2 {
    margin: 0 0 0.4rem;
    color: var(--text);
    font-size: 1.35rem;
  }
  .empty p {
    margin: 0 0 1rem;
    line-height: 1.5;
  }
  .empty ul {
    list-style: none;
    padding: 0;
    margin: 0;
    font-size: 0.85rem;
    display: grid;
    gap: 0.35rem;
  }
  kbd {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.78em;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--surface);
  }
  .composer {
    padding: 0.5rem 1.25rem 1rem;
    border-top: 1px solid var(--border);
    background: linear-gradient(180deg, transparent, var(--bg) 30%);
  }
  .status-line {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: var(--muted);
    margin-bottom: 0.45rem;
    min-height: 1.1rem;
  }
  .status-line.running {
    color: var(--accent);
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
  }
  .status-line.running .pulse {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
    animation: pulse 1.2s ease infinite;
  }
  .sep {
    opacity: 0.5;
  }
  .cwd {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .box {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 0.65rem 0.75rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .box:focus-within {
    border-color: var(--accent-dim);
  }
  textarea {
    width: 100%;
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    line-height: 1.45;
    min-height: 3rem;
    max-height: 160px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  .send,
  .stop {
    border: none;
    border-radius: 8px;
    padding: 0.45rem 1rem;
    font-weight: 600;
    font-size: 0.88rem;
    cursor: pointer;
    font-family: inherit;
  }
  .send {
    background: var(--accent-gradient);
    color: var(--accent-contrast);
  }
  .send:hover {
    filter: brightness(1.06);
  }
  .send:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .stop {
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }
</style>
