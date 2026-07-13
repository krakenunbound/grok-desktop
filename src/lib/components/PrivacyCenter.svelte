<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { privacyAudit, loadPrivacyAudit } from "$lib/stores/privacy";
  import { persistSettings, settings } from "$lib/stores/settings";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();
  let busy = $state(false);
  let message = $state("");
  let error = $state("");

  $effect(() => {
    if (open) {
      message = "";
      error = "";
      void refresh();
    }
  });

  async function refresh() {
    busy = true;
    error = "";
    try {
      await loadPrivacyAudit();
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function toggleGuard(enabled: boolean) {
    busy = true;
    error = "";
    try {
      await persistSettings({ ...$settings, privacy_guard_enabled: enabled });
      await refresh();
    } catch (cause) {
      error = String(cause);
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function protectCliConfig() {
    busy = true;
    error = "";
    try {
      await invoke("apply_grok_privacy_config");
      message = "Grok telemetry, Mixpanel, and trace uploads are disabled in config.toml.";
      await refresh();
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function exportReport() {
    const destination = await save({
      title: "Export Grok privacy audit",
      defaultPath: "grok-privacy-audit.md",
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!destination) return;
    busy = true;
    error = "";
    try {
      await invoke("export_privacy_report", { destination });
      message = `Privacy audit exported to ${destination}`;
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function archiveAndClear() {
    const confirmed = window.confirm(
      "Archive the current Grok log inside Grok Desktop app data, then clear the original? This does not delete anything already uploaded to xAI.",
    );
    if (!confirmed) return;
    busy = true;
    error = "";
    try {
      const archive = await invoke<string>("archive_and_clear_grok_logs", {
        confirmation: "ARCHIVE AND CLEAR",
      });
      message = `Local log archived before clearing: ${archive}`;
      await refresh();
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function copyUrl(url: string, label: string) {
    try {
      await navigator.clipboard.writeText(url);
      message = `${label} copied to the clipboard.`;
    } catch (cause) {
      error = String(cause);
    }
  }

  function bytes(value: number): string {
    if (value < 1024) return `${value} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let size = value / 1024;
    let unit = units[0];
    for (let index = 1; index < units.length && size >= 1024; index += 1) {
      size /= 1024;
      unit = units[index];
    }
    return `${size.toFixed(size >= 10 ? 1 : 2)} ${unit}`;
  }

  function date(value: string | null): string {
    return value ? new Date(value).toLocaleString() : "None detected";
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (open && event.key === "Escape") onclose();
  }}
/>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && onclose()}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="privacy-title"
      tabindex="-1"
    >
      <header>
        <div>
          <h2 id="privacy-title">Privacy Center</h2>
          <p>Repository-upload protection, local evidence, and account controls</p>
        </div>
        <button type="button" class="close" onclick={onclose} aria-label="Close privacy center"
          >×</button
        >
      </header>

      {#if $privacyAudit}
        <section class="hero" class:danger={$privacyAudit.account_retention_opt_out === false}>
          <div class="shield" aria-hidden="true">{$privacyAudit.guard_enabled ? "◆" : "◇"}</div>
          <div>
            <strong>Privacy Guard {$privacyAudit.guard_enabled ? "is active" : "is off"}</strong>
            <p>
              When active, Grok Desktop disables optional telemetry sinks and immediately stops a
              task if Grok logs a repository-state upload attempt.
            </p>
          </div>
          <label class="switch">
            <input
              type="checkbox"
              checked={$privacyAudit.guard_enabled}
              disabled={busy}
              onchange={(event) => toggleGuard((event.currentTarget as HTMLInputElement).checked)}
            />
            <span>{$privacyAudit.guard_enabled ? "On" : "Off"}</span>
          </label>
        </section>

        <div class="cards">
          <article class:bad={$privacyAudit.account_retention_opt_out === false}>
            <span>Account retention</span>
            <strong>
              {$privacyAudit.account_retention_opt_out === true
                ? "Opted out"
                : $privacyAudit.account_retention_opt_out === false
                  ? "Not opted out"
                  : "Unknown"}
            </strong>
            <small
              >Use <code>/privacy</code> in Grok Build to review or change this server-side setting.</small
            >
          </article>
          <article>
            <span>Logged upload volume</span>
            <strong>{bytes($privacyAudit.upload_bytes)}</strong>
            <small>{$privacyAudit.upload_enqueued_events} enqueued upload events</small>
          </article>
          <article>
            <span>Largest upload</span>
            <strong>{bytes($privacyAudit.largest_upload_bytes)}</strong>
            <small>{$privacyAudit.upload_start_events} upload-start events</small>
          </article>
          <article>
            <span>Last detected</span>
            <strong>{date($privacyAudit.last_upload_at)}</strong>
            <small>Local log is {bytes($privacyAudit.log_bytes)}</small>
          </article>
        </div>

        <section>
          <div class="section-head">
            <div>
              <h3>Defense in depth</h3>
              <p>These local settings are separate from the account retention choice above.</p>
            </div>
            <button type="button" class="primary" onclick={protectCliConfig} disabled={busy}>
              Protect Grok CLI config
            </button>
          </div>
          <div class="checks">
            <span class:ok={$privacyAudit.telemetry_disabled_in_config}>
              {$privacyAudit.telemetry_disabled_in_config
                ? "✓ Anonymous telemetry disabled"
                : "! CLI telemetry config is not protected"}
            </span>
            <span class:ok={$privacyAudit.trace_upload_disabled_in_config}>
              {$privacyAudit.trace_upload_disabled_in_config
                ? "✓ Trace uploads disabled"
                : "! CLI trace-upload config is not protected"}
            </span>
          </div>
        </section>

        <section>
          <h3>Repositories in the local upload log</h3>
          <div class="repos">
            {#each $privacyAudit.repositories as repository (repository.path)}
              <div>
                <span title={repository.path}>{repository.path}</span>
                <small>{repository.events} events · {bytes(repository.bytes)}</small>
              </div>
            {:else}
              <p>No repository upload events were found.</p>
            {/each}
          </div>
        </section>

        <section class="actions">
          <button type="button" onclick={exportReport} disabled={busy}>Export audit report</button>
          <button
            type="button"
            onclick={archiveAndClear}
            disabled={busy || !$privacyAudit.log_exists}
          >
            Archive & clear local log
          </button>
          <button type="button" onclick={() => copyUrl("https://grok.com", "Grok privacy URL")}>
            Copy Grok privacy URL
          </button>
          <button
            type="button"
            onclick={() => copyUrl("https://accounts.x.ai/privacy", "xAI privacy-request URL")}
            >Copy deletion-request URL</button
          >
        </section>

        <p class="warning">
          Clearing this computer's log does not remove remote data. Use the xAI privacy-request
          portal for access or deletion requests.
        </p>
      {:else}
        <div class="loading">
          {busy ? "Auditing Grok privacy state…" : "Privacy audit unavailable."}
        </div>
      {/if}

      {#if message}<div class="message" role="status">{message}</div>{/if}
      {#if error}<div class="error" role="alert">{error}</div>{/if}

      <footer>
        <button type="button" onclick={refresh} disabled={busy}
          >{busy ? "Working…" : "Refresh"}</button
        >
        <button type="button" class="primary" onclick={onclose}>Done</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(7px);
  }
  .modal {
    width: min(920px, 100%);
    max-height: calc(100vh - 2rem);
    overflow: auto;
    display: grid;
    gap: 0.9rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: 0 25px 80px rgba(0, 0, 0, 0.55);
  }
  header,
  .section-head,
  footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
  h2,
  h3,
  p {
    margin: 0;
  }
  h2 {
    font-size: 1.15rem;
  }
  h3 {
    font-size: 0.84rem;
  }
  header p,
  section p,
  small {
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.45;
  }
  .close {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 1.25rem;
    cursor: pointer;
  }
  .hero {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0.85rem;
    padding: 0.9rem;
    border: 1px solid var(--accent-dim);
    border-radius: 12px;
    background: rgba(74, 198, 255, 0.07);
  }
  .hero.danger {
    border-color: #8b463f;
  }
  .shield {
    color: var(--accent);
    font-size: 1.5rem;
    text-shadow: 0 0 14px var(--accent-glow);
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8rem;
    font-weight: 700;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.55rem;
  }
  article {
    display: grid;
    gap: 0.25rem;
    padding: 0.7rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    min-width: 0;
  }
  article.bad {
    border-color: #8b463f;
  }
  article span {
    color: var(--muted);
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  article strong {
    overflow-wrap: anywhere;
  }
  section {
    display: grid;
    gap: 0.55rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.45rem 0.7rem;
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
    font: inherit;
  }
  button:hover {
    border-color: var(--accent-dim);
  }
  button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  button.primary {
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    font-weight: 700;
  }
  .checks {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .checks span {
    padding: 0.3rem 0.5rem;
    border-radius: 999px;
    background: rgba(255, 165, 90, 0.08);
    color: #ffc38f;
    font-size: 0.75rem;
  }
  .checks span.ok {
    background: rgba(74, 198, 255, 0.08);
    color: var(--accent);
  }
  .repos {
    display: grid;
    gap: 0.35rem;
    max-height: 150px;
    overflow: auto;
  }
  .repos div {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.4rem 0.5rem;
    background: var(--bg);
    border-radius: 7px;
  }
  .repos span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.78rem;
  }
  .actions {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .warning {
    color: #ffb38a;
    padding: 0.2rem 0.35rem;
  }
  .message,
  .error {
    padding: 0.55rem;
    border-radius: 8px;
    font-size: 0.78rem;
  }
  .message {
    background: rgba(74, 198, 255, 0.08);
    color: var(--accent);
  }
  .error {
    background: rgba(255, 70, 70, 0.1);
    color: #ffaaa2;
  }
  .loading {
    padding: 2rem;
    text-align: center;
    color: var(--muted);
  }
  footer {
    justify-content: flex-end;
  }
  code {
    font-size: 0.72rem;
  }
  @media (max-width: 760px) {
    .cards {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .hero {
      grid-template-columns: auto 1fr;
    }
    .switch {
      grid-column: 2;
    }
  }
</style>
