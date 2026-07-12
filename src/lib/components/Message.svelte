<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { type ChatMessage, toggleMessageRaw, verboseMode } from "$lib/stores/chat";
  import { cleanTerminalText } from "$lib/text";

  interface Props {
    message: ChatMessage;
    onretry?: () => void;
  }
  let { message, onretry }: Props = $props();
  let copied = $state(false);

  let showRaw = $derived(!!message.rawVisible || ($verboseMode && !!message.rawContent));
  let hasRawExtras = $derived(
    !!message.rawContent &&
      message.rawContent.trim() !== (message.content || "").trim() &&
      message.rawContent.trim().length > 0,
  );

  const STDERR_TOOL_ERROR_RE =
    /^\[stderr\].*(?:tool error:\s*tool_output_error|error_kind="?tool_output_error"?|tool_output_error)/i;
  const STDERR_THOUGHT_RE = /^\[stderr\]\s*\[thought\]/i;

  function displayContent(text: string): string {
    const clean = cleanTerminalText(text);
    if (message.role !== "assistant") return clean;

    return clean
      .split(/\r?\n/)
      .filter(
        (line) => !STDERR_TOOL_ERROR_RE.test(line.trim()) && !STDERR_THOUGHT_RE.test(line.trim()),
      )
      .join("\n")
      .trim();
  }

  /**
   * Escape HTML then apply a tiny markdown subset.
   * Always escape first — never inject untrusted agent/CLI text as raw HTML (XSS).
   */
  function renderContent(text: string): string {
    if (!text) return "";
    const escaped = displayContent(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");

    let html = escaped.replace(
      /```(\w*)\n([\s\S]*?)```/g,
      (_m, lang, code) =>
        `<pre class="code-block" data-lang="${String(lang || "text").replace(/[^\w-]/g, "")}"><button type="button" class="copy-code" data-copy-code>Copy</button><code>${code}</code></pre>`,
    );
    html = html.replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>');
    html = html.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    const parts = html.split(/(<pre[\s\S]*?<\/pre>)/g);
    html = parts.map((p) => (p.startsWith("<pre") ? p : p.replace(/\n/g, "<br/>"))).join("");
    return html;
  }

  function assetUrl(src: string): string {
    if (/^(data:|https?:|asset:)/i.test(src)) return src;
    return convertFileSrc(src);
  }

  function filename(src: string): string {
    return (src.split(/[/\\]/).pop() || "Attachment").replace(
      /^file_\d{8}_\d{6}_[0-9a-f]{8}_/i,
      "",
    );
  }

  function attachmentKind(src: string): "image" | "video" | "audio" | "file" {
    const ext = src.split(".").pop()?.toLowerCase() || "";
    if (["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(ext)) return "image";
    if (["mp4", "webm", "mov", "m4v", "avi", "mkv"].includes(ext)) return "video";
    if (["mp3", "wav", "m4a", "aac", "ogg", "flac"].includes(ext)) return "audio";
    return "file";
  }

  function extension(src: string): string {
    return filename(src).split(".").pop()?.toUpperCase().slice(0, 6) || "FILE";
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      window.setTimeout(() => (copied = false), 1400);
    } catch {
      copied = false;
    }
  }

  function onBodyClick(event: MouseEvent) {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-copy-code]");
    if (!button) return;
    const code = button.parentElement?.querySelector("code")?.textContent ?? "";
    void copyText(code);
    button.textContent = "Copied";
    window.setTimeout(() => (button.textContent = "Copy"), 1400);
  }

  function onBodyKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    const target = event.target;
    if (!(target instanceof HTMLElement) || !target.matches("[data-copy-code]")) return;
    event.preventDefault();
    target.click();
  }
</script>

{#if !(message.role === "assistant" && message.streaming && !message.content && !$verboseMode)}
  <article
    class="msg"
    class:user={message.role === "user"}
    class:assistant={message.role === "assistant"}
    class:system={message.role === "system"}
    class:streaming={message.streaming}
  >
    <div class="message-head">
      <div class="role">
        {message.role === "user" ? "You" : message.role === "assistant" ? "Grok" : "System"}
      </div>
      {#if !message.streaming}
        <div class="message-actions">
          {#if message.role === "user" && onretry}
            <button type="button" onclick={onretry} title="Send this message again">Retry</button>
          {/if}
          <button
            type="button"
            onclick={() => copyText(displayContent(message.content))}
            title="Copy message"
          >
            {copied ? "Copied" : "Copy"}
          </button>
        </div>
      {/if}
    </div>

    {#if message.images?.length}
      <div class="attachments">
        {#each message.images as src}
          <div class="attachment" class:wide={attachmentKind(src) !== "image"} title={src}>
            {#if attachmentKind(src) === "image"}
              <img src={assetUrl(src)} alt={filename(src)} />
            {:else if attachmentKind(src) === "video"}
              <!-- svelte-ignore a11y_media_has_caption: user-supplied videos may not include a caption track -->
              <video src={assetUrl(src)} controls preload="metadata" aria-label={filename(src)}
              ></video>
            {:else if attachmentKind(src) === "audio"}
              <div class="file-mark">AUDIO</div>
              <audio src={assetUrl(src)} controls preload="metadata" aria-label={filename(src)}
              ></audio>
            {:else}
              <div class="file-mark">{extension(src)}</div>
            {/if}
            <span class="path">{filename(src)}</span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions: delegated handlers serve the interactive code-copy buttons rendered from sanitized Markdown -->
    <div
      class="body"
      role="group"
      aria-label="Message content"
      onclick={onBodyClick}
      onkeydown={onBodyKeydown}
    >
      {@html renderContent(message.content)}
      {#if message.streaming && $verboseMode}
        <span class="cursor">▍</span>
      {/if}
    </div>

    {#if message.role === "assistant" && !message.streaming && (hasRawExtras || message.rawContent)}
      <div class="details">
        <button
          type="button"
          class="reveal"
          onclick={() => toggleMessageRaw(message.id)}
          title="Reveal raw agent output, tool traces, and diffs for this turn"
        >
          {showRaw ? "Hide agent details" : "Show agent details"}
        </button>
        {#if showRaw && message.rawContent}
          <pre class="raw">{cleanTerminalText(message.rawContent)}</pre>
        {/if}
      </div>
    {/if}
  </article>
{/if}

<style>
  .msg {
    padding: 0.9rem 1.1rem;
    border-radius: 12px;
    max-width: min(860px, 100%);
    border: 1px solid var(--border);
    background: var(--surface);
    animation: fadeIn 0.18s ease;
  }
  .msg.user {
    align-self: flex-end;
    width: min(860px, 100%);
    box-sizing: border-box;
    background: var(--user-bg);
    border-color: var(--user-border);
  }
  .msg.assistant {
    align-self: flex-start;
    background: var(--assistant-bg);
  }
  .msg.system {
    align-self: center;
    opacity: 0.85;
    font-size: 0.9rem;
  }
  .role {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    font-weight: 600;
  }
  .message-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    min-height: 1.35rem;
    margin-bottom: 0.35rem;
  }
  .message-actions {
    display: flex;
    gap: 0.2rem;
    opacity: 0;
    transition: opacity 0.15s ease;
  }
  .msg:hover .message-actions,
  .message-actions:focus-within {
    opacity: 1;
  }
  .message-actions button {
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--muted);
    padding: 0.18rem 0.35rem;
    font: inherit;
    font-size: 0.68rem;
    cursor: pointer;
  }
  .message-actions button:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .msg.user .role {
    color: var(--accent);
  }
  .body {
    font-size: 0.95rem;
    line-height: 1.55;
    color: var(--text);
    word-break: break-word;
  }
  .attachments {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 0.5rem;
  }
  .attachment {
    width: 92px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-size: 0.75rem;
    color: var(--muted);
  }
  .attachment.wide {
    width: min(320px, 100%);
  }
  .attachment img,
  .attachment video {
    display: block;
    width: 92px;
    height: 72px;
    object-fit: cover;
    background: #050507;
  }
  .attachment.wide video {
    width: 100%;
    height: 180px;
  }
  .attachment audio {
    width: calc(100% - 0.7rem);
    height: 34px;
    margin: 0 0.35rem 0.35rem;
  }
  .file-mark {
    height: 54px;
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 800;
    letter-spacing: 0.04em;
  }
  .path {
    display: block;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    padding: 0.25rem 0.35rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .details {
    margin-top: 0.65rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }
  .reveal {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 0.78rem;
    font-family: inherit;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .reveal:hover {
    color: var(--accent);
  }
  .raw {
    margin: 0.5rem 0 0;
    padding: 0.65rem 0.75rem;
    border-radius: 8px;
    background: #0a0a0c;
    border: 1px solid var(--border);
    font-size: 0.78rem;
    line-height: 1.4;
    overflow-x: auto;
    max-height: 280px;
    overflow-y: auto;
    color: #b8b8c8;
    white-space: pre-wrap;
    word-break: break-word;
  }
  :global(.code-block) {
    position: relative;
    margin: 0.6rem 0;
    padding: 0.75rem 0.9rem;
    border-radius: 8px;
    background: #0a0a0c;
    border: 1px solid var(--border);
    overflow-x: auto;
    font-size: 0.85rem;
    line-height: 1.45;
  }
  :global(.copy-code) {
    position: absolute;
    top: 0.4rem;
    right: 0.45rem;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--surface-2);
    color: var(--muted);
    padding: 0.2rem 0.4rem;
    font: inherit;
    font-size: 0.67rem;
    cursor: pointer;
  }
  :global(.copy-code:hover) {
    color: var(--text);
  }
  :global(.inline-code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    background: var(--surface-2);
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    font-size: 0.88em;
  }
  .cursor {
    display: inline-block;
    width: 0.5ch;
    margin-left: 1px;
    background: var(--accent);
    animation: blink 1s step-end infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
