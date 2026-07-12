<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    addPendingAttachment,
    pendingAttachments,
    removePendingAttachment,
    showError,
    type PendingAttachment,
  } from "$lib/stores/chat";

  interface Props {
    onchange?: () => void;
  }
  let { onchange }: Props = $props();

  let dragOver = $state(false);
  let busy = $state(false);
  const MAX_ATTACHMENTS = 16;
  const MAX_PASTED_IMAGE_BYTES = 20 * 1024 * 1024;

  interface SavedAttachment {
    id: string;
    path: string;
    filename: string;
    mime: string;
    size_bytes: number;
    kind: PendingAttachment["kind"];
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          dragOver = true;
        } else if (event.payload.type === "leave") {
          dragOver = false;
        } else if (event.payload.type === "drop") {
          dragOver = false;
          void importPaths(event.payload.paths);
        }
      })
      .then((stop) => (unlisten = stop))
      .catch((error) => showError(error));
    return () => unlisten?.();
  });

  function toPending(saved: SavedAttachment, previewUrl?: string): PendingAttachment {
    return {
      id: saved.id,
      path: saved.path,
      filename: saved.filename,
      mime: saved.mime,
      kind: saved.kind,
      sizeBytes: saved.size_bytes,
      previewUrl,
    };
  }

  async function importPaths(paths: string[]) {
    if (!paths.length) return;
    const available = MAX_ATTACHMENTS - $pendingAttachments.length;
    if (available <= 0) {
      showError(`You can attach up to ${MAX_ATTACHMENTS} files per message.`);
      return;
    }
    busy = true;
    try {
      for (const path of paths.slice(0, available)) {
        try {
          const saved = await invoke<SavedAttachment>("import_attachment_path", { path });
          addPendingAttachment(toPending(saved));
        } catch (error) {
          showError(`${path.split(/[/\\]/).pop() || "File"}: ${String(error)}`);
        }
      }
      if (paths.length > available) {
        showError(`Only the first ${available} files were attached (maximum ${MAX_ATTACHMENTS}).`);
      }
      onchange?.();
    } finally {
      busy = false;
    }
  }

  async function pickFiles() {
    try {
      const selected = await open({
        directory: false,
        multiple: true,
        title: "Attach files to Grok",
      });
      if (!selected) return;
      await importPaths(Array.isArray(selected) ? selected : [selected]);
    } catch (error) {
      showError(error);
    }
  }

  async function savePastedImage(file: File) {
    if (file.size > MAX_PASTED_IMAGE_BYTES) {
      showError("Pasted image is too large (maximum 20 MB). Save it and attach the file instead.");
      return;
    }
    if ($pendingAttachments.length >= MAX_ATTACHMENTS) {
      showError(`You can attach up to ${MAX_ATTACHMENTS} files per message.`);
      return;
    }
    busy = true;
    try {
      const dataUrl = await readAsDataUrl(file);
      const saved = await invoke<SavedAttachment>("save_image_base64", {
        dataBase64: dataUrl,
        mimeHint: file.type || null,
        filenameHint: file.name || null,
      });
      addPendingAttachment(toPending(saved, dataUrl));
      onchange?.();
    } catch (error) {
      showError(error);
    } finally {
      busy = false;
    }
  }

  function readAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(reader.error ?? new Error("Could not read pasted image"));
      reader.readAsDataURL(file);
    });
  }

  async function onPaste(event: ClipboardEvent) {
    for (const item of Array.from(event.clipboardData?.items ?? [])) {
      if (item.type.startsWith("image/")) {
        event.preventDefault();
        const file = item.getAsFile();
        if (file) await savePastedImage(file);
      }
    }
  }

  function mediaUrl(attachment: PendingAttachment): string {
    return attachment.previewUrl || convertFileSrc(attachment.path);
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  function fileIcon(attachment: PendingAttachment): string {
    if (attachment.kind === "video") return "▶";
    if (attachment.kind === "audio") return "♪";
    return attachment.filename.split(".").pop()?.toUpperCase().slice(0, 5) || "FILE";
  }
</script>

<svelte:window onpaste={onPaste} />

{#if dragOver}
  <div class="drop-overlay" role="status">
    <div>
      <strong>Drop files to attach</strong><span>Images, video, audio, documents, or code</span>
    </div>
  </div>
{/if}

<div class="zone" class:busy aria-label="Message attachments">
  {#if $pendingAttachments.length}
    <div class="previews">
      {#each $pendingAttachments as attachment (attachment.id)}
        <div class="card" title={attachment.path}>
          {#if attachment.kind === "image"}
            <img src={mediaUrl(attachment)} alt={attachment.filename} />
          {:else if attachment.kind === "video"}
            <video
              src={mediaUrl(attachment)}
              muted
              preload="metadata"
              aria-label={attachment.filename}
            ></video>
            <span class="media-badge">VIDEO</span>
          {:else}
            <div class="file-icon">{fileIcon(attachment)}</div>
          {/if}
          <div class="file-meta">
            <span>{attachment.filename}</span>
            <small>{formatSize(attachment.sizeBytes)}</small>
          </div>
          <button
            type="button"
            class="remove"
            onclick={() => removePendingAttachment(attachment.id)}
            title="Remove attachment"
            aria-label={`Remove ${attachment.filename}`}>×</button
          >
        </div>
      {/each}
    </div>
  {/if}

  <button
    type="button"
    class="add"
    disabled={busy || $pendingAttachments.length >= MAX_ATTACHMENTS}
    onclick={pickFiles}
  >
    {busy ? "Attaching…" : "+ Attach"}
  </button>
</div>

<style>
  .zone {
    display: grid;
    gap: 0.5rem;
    min-height: 0;
  }
  .zone.busy {
    opacity: 0.8;
  }
  .previews {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .card {
    position: relative;
    width: 156px;
    min-height: 62px;
    display: grid;
    grid-template-columns: 54px minmax(0, 1fr);
    align-items: center;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
  }
  .card img,
  .card video,
  .file-icon {
    width: 54px;
    height: 62px;
    object-fit: cover;
    background: #050507;
  }
  .file-icon {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 800;
  }
  .file-meta {
    min-width: 0;
    display: grid;
    gap: 0.18rem;
    padding: 0.4rem 1.25rem 0.4rem 0.5rem;
  }
  .file-meta span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.72rem;
  }
  .file-meta small {
    color: var(--muted);
    font-size: 0.65rem;
  }
  .media-badge {
    position: absolute;
    left: 4px;
    bottom: 4px;
    padding: 0.1rem 0.25rem;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.72);
    color: white;
    font-size: 0.5rem;
    font-weight: 800;
  }
  .remove {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.72);
    color: white;
    cursor: pointer;
    line-height: 1;
    padding: 0;
  }
  .add {
    width: max-content;
    border: 1px dashed var(--border);
    border-radius: 8px;
    padding: 0.35rem 0.65rem;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .add:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .add:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .drop-overlay {
    position: fixed;
    inset: 0.75rem;
    z-index: 90;
    display: grid;
    place-items: center;
    pointer-events: none;
    border: 2px dashed var(--accent);
    border-radius: 16px;
    background: rgba(8, 10, 14, 0.88);
    backdrop-filter: blur(6px);
  }
  .drop-overlay div {
    display: grid;
    gap: 0.4rem;
    text-align: center;
  }
  .drop-overlay strong {
    color: var(--text);
    font-size: 1.15rem;
  }
  .drop-overlay span {
    color: var(--muted);
    font-size: 0.85rem;
  }
</style>
