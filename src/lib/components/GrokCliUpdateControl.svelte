<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  interface GrokUpdateStatus {
    currentVersion: string;
    latestVersion: string | null;
    updateAvailable: boolean;
    installer: string | null;
    channel: string;
    autoUpdate: boolean;
    error: string | null;
  }

  interface Props {
    open: boolean;
    onclose: () => void;
    onstatus?: (available: boolean) => void;
  }

  let { open, onclose, onstatus = () => {} }: Props = $props();
  let checking = $state(false);
  let installing = $state(false);
  let confirmInstall = $state(false);
  let status = $state<GrokUpdateStatus | null>(null);
  let error = $state("");
  let message = $state("");

  onMount(() => {
    const initial = window.setTimeout(() => void check(), 8_000);
    const periodic = window.setInterval(() => void check(), 6 * 60 * 60 * 1_000);
    return () => {
      window.clearTimeout(initial);
      window.clearInterval(periodic);
    };
  });

  $effect(() => {
    if (open && !status && !checking) void check();
  });

  async function check() {
    if (checking || installing) return;
    checking = true;
    error = "";
    try {
      status = await invoke<GrokUpdateStatus>("check_grok_cli_update");
      onstatus(status.updateAvailable);
    } catch (cause) {
      error = String(cause);
    } finally {
      checking = false;
    }
  }

  function close() {
    if (installing) return;
    confirmInstall = false;
    onclose();
  }

  async function install() {
    if (!status?.updateAvailable || installing) return;
    installing = true;
    error = "";
    message = "";
    try {
      const target = status.latestVersion ?? "update";
      status = await invoke<GrokUpdateStatus>("install_grok_cli_update");
      onstatus(status.updateAvailable);
      confirmInstall = false;
      message = `Grok CLI ${target} installed successfully.`;
    } catch (cause) {
      error = String(cause);
    } finally {
      installing = false;
    }
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && close()}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="cli-update-title"
      tabindex="-1"
    >
      <header>
        <div>
          <h2 id="cli-update-title">Grok CLI updates</h2>
          <p>Check and install releases through Grok Build's official self-updater.</p>
        </div>
        <button type="button" class="close" onclick={close} aria-label="Close Grok CLI updates"
          >×</button
        >
      </header>

      {#if status}
        <div class="versions">
          <div><span>Installed</span><strong>{status.currentVersion}</strong></div>
          <div><span>Latest</span><strong>{status.latestVersion ?? "Unavailable"}</strong></div>
          <div><span>Channel</span><strong>{status.channel}</strong></div>
        </div>

        <section class:available={status.updateAvailable}>
          <div>
            <strong>{status.updateAvailable ? "Update available" : "Grok CLI is up to date"}</strong
            >
            <p>
              {status.updateAvailable
                ? `Version ${status.latestVersion} can replace the installed ${status.currentVersion} CLI.`
                : `Version ${status.currentVersion} is the newest ${status.channel} release.`}
            </p>
          </div>
          {#if status.updateAvailable}
            <button
              type="button"
              class="primary"
              disabled={installing}
              onclick={() => (confirmInstall = true)}
            >
              Install update
            </button>
          {/if}
        </section>

        {#if confirmInstall}
          <section class="confirm">
            <strong>Install Grok CLI {status.latestVersion}?</strong>
            <p>
              Stop any Grok work first. The official updater will replace the CLI executable; your
              projects, chats, settings, and xAI login remain in place. Do not close Grok Desktop
              until installation finishes.
            </p>
            <div>
              <button type="button" disabled={installing} onclick={() => (confirmInstall = false)}
                >Cancel</button
              >
              <button type="button" class="primary" disabled={installing} onclick={install}>
                {installing ? "Installing…" : "Install now"}
              </button>
            </div>
          </section>
        {/if}
      {:else}
        <div class="loading">{checking ? "Checking Grok CLI…" : "Update status unavailable."}</div>
      {/if}

      {#if message}<div class="message" role="status">{message}</div>{/if}
      {#if error}<div class="error" role="alert">{error}</div>{/if}

      <footer>
        <button type="button" disabled={checking || installing} onclick={check}>
          {checking ? "Checking…" : "Check again"}
        </button>
        <button type="button" class="primary" disabled={installing} onclick={close}>Done</button>
      </footer>
    </div>
  </div>
{/if}

<svelte:window onkeydown={(event) => event.key === "Escape" && open && close()} />

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(7px);
  }
  .modal {
    width: min(620px, 100%);
    display: grid;
    gap: 0.85rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 15px;
    background: var(--surface);
    box-shadow: 0 25px 80px rgba(0, 0, 0, 0.55);
  }
  header,
  footer,
  section,
  .confirm > div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.8rem;
  }
  header div,
  section div {
    display: grid;
    gap: 0.2rem;
  }
  h2,
  p {
    margin: 0;
  }
  h2 {
    font-size: 1.1rem;
  }
  p,
  .loading {
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.45;
  }
  .close {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 1.2rem;
  }
  .versions {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.55rem;
  }
  .versions div {
    display: grid;
    gap: 0.2rem;
    padding: 0.7rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
  }
  .versions span {
    color: var(--muted);
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  section {
    padding: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  section.available {
    border-color: var(--accent-dim);
    background: rgba(74, 198, 255, 0.06);
  }
  section.confirm {
    display: grid;
    border-color: #9a6743;
    background: rgba(154, 103, 67, 0.1);
  }
  section.confirm > div {
    justify-content: flex-end;
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
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  button.primary {
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    font-weight: 700;
  }
  .message,
  .error,
  .loading {
    padding: 0.65rem;
    border-radius: 8px;
  }
  .message {
    background: rgba(74, 198, 255, 0.08);
    color: var(--accent);
  }
  .error {
    background: rgba(255, 70, 70, 0.1);
    color: #ffaaa2;
  }
  footer {
    justify-content: flex-end;
  }
  @media (max-width: 540px) {
    .versions {
      grid-template-columns: 1fr;
    }
    section {
      align-items: flex-start;
      flex-direction: column;
    }
  }
</style>
