<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    addPendingImage,
    pendingImages,
    removePendingImage,
    showError,
    type PendingImage,
  } from "$lib/stores/chat";

  interface Props {
    /** Called after images change (optional). */
    onchange?: () => void;
  }
  let { onchange }: Props = $props();

  let dragOver = $state(false);
  let busy = $state(false);

  const MAX_BYTES = 20 * 1024 * 1024;
  const MAX_ATTACHMENTS = 8;
  const IMAGE_RE = /\.(png|jpe?g|gif|webp|bmp)$/i;

  function isImageFile(file: File): boolean {
    return file.type.startsWith("image/") || IMAGE_RE.test(file.name);
  }

  async function saveFromFile(file: File) {
    if (!isImageFile(file)) {
      showError("Unsupported file. Attach a PNG, JPEG, GIF, WebP, or BMP image.");
      return;
    }
    if (file.size > MAX_BYTES) {
      showError("Image is too large (max 20 MB). Try a smaller screenshot.");
      return;
    }
    if ($pendingImages.length >= MAX_ATTACHMENTS) {
      showError(`You can attach up to ${MAX_ATTACHMENTS} images per message.`);
      return;
    }

    busy = true;
    try {
      const dataUrl = await readAsDataUrl(file);
      const saved = await invoke<{
        id: string;
        path: string;
        filename: string;
        mime: string;
        size_bytes: number;
      }>("save_image_base64", {
        dataBase64: dataUrl,
        mimeHint: file.type || null,
        filenameHint: file.name || null,
      });
      const pending: PendingImage = {
        id: saved.id,
        path: saved.path,
        filename: saved.filename,
        previewUrl: dataUrl,
      };
      addPendingImage(pending);
      onchange?.();
    } catch (e) {
      showError(e);
    } finally {
      busy = false;
    }
  }

  function readAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(reader.error ?? new Error("Could not read image file"));
      reader.readAsDataURL(file);
    });
  }

  function onDragOver(e: DragEvent) {
    e.preventDefault();
    dragOver = true;
  }
  function onDragLeave() {
    dragOver = false;
  }
  async function onDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    const files = e.dataTransfer?.files;
    if (!files?.length) return;
    let accepted = 0;
    for (const file of Array.from(files)) {
      if (isImageFile(file)) {
        await saveFromFile(file);
        accepted += 1;
      }
    }
    if (accepted === 0) {
      showError("Drop image files only (PNG, JPEG, GIF, WebP, BMP).");
    }
  }

  async function onPaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        e.preventDefault();
        const file = item.getAsFile();
        if (file) await saveFromFile(file);
      }
    }
  }

  async function onFileInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const files = input.files;
    if (!files) return;
    for (const file of Array.from(files)) {
      await saveFromFile(file);
    }
    input.value = "";
  }
</script>

<svelte:window onpaste={onPaste} />

<div
  class="zone"
  class:dragOver
  class:busy
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
  role="region"
  aria-label="Image attachments"
>
  {#if $pendingImages.length}
    <div class="previews">
      {#each $pendingImages as img (img.id)}
        <div class="card">
          {#if img.previewUrl}
            <img src={img.previewUrl} alt={img.filename} />
          {:else}
            <div class="placeholder">{img.filename}</div>
          {/if}
          <button
            type="button"
            class="rm"
            onclick={() => removePendingImage(img.id)}
            title="Remove attachment"
            aria-label={`Remove ${img.filename}`}
          >
            ×
          </button>
        </div>
      {/each}
    </div>
  {/if}
  <label class="add" class:disabled={busy || $pendingImages.length >= MAX_ATTACHMENTS}>
    <input
      type="file"
      accept="image/png,image/jpeg,image/gif,image/webp,image/bmp"
      multiple
      onchange={onFileInput}
      hidden
      disabled={busy || $pendingImages.length >= MAX_ATTACHMENTS}
    />
    <span>{busy ? "Saving…" : "+ Attach"}</span>
  </label>
</div>

<style>
  .zone {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    min-height: 0;
  }
  .zone.dragOver {
    outline: 1px dashed var(--accent);
    outline-offset: 4px;
    border-radius: 8px;
  }
  .zone.busy {
    opacity: 0.85;
  }
  .previews {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }
  .card {
    position: relative;
    width: 56px;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--surface-2);
  }
  .card img {
    display: block;
    width: 56px;
    height: 56px;
    object-fit: cover;
  }
  .placeholder {
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
    font-size: 0.6rem;
    padding: 4px;
    text-align: center;
    color: var(--muted);
  }
  .rm {
    position: absolute;
    top: 2px;
    right: 2px;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    cursor: pointer;
    font-size: 12px;
    line-height: 1;
    padding: 0;
  }
  .add {
    cursor: pointer;
    font-size: 0.8rem;
    color: var(--muted);
    border: 1px dashed var(--border);
    border-radius: 8px;
    padding: 0.35rem 0.65rem;
    transition:
      color 0.15s,
      border-color 0.15s;
  }
  .add:hover:not(.disabled) {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .add.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
