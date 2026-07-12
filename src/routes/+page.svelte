<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import ChatArea from "$lib/components/ChatArea.svelte";
  import VerboseToggle from "$lib/components/VerboseToggle.svelte";
  import SettingsModal from "$lib/components/SettingsModal.svelte";
  import DocumentationModal from "$lib/components/DocumentationModal.svelte";
  import UsageMeter from "$lib/components/UsageMeter.svelte";
  import RightPanel from "$lib/components/RightPanel.svelte";
  import { get } from "svelte/store";
  import {
    bindGrokEvents,
    createNewChat,
    openChat,
    chatList,
    errorToast,
    selectedModel,
    yoloEnabled,
    verboseMode,
    setVerboseMode,
    refreshChatList,
    currentChat,
  } from "$lib/stores/chat";
  import { loadSettings, loadModels, settings, persistSettings } from "$lib/stores/settings";
  import { loadProjects, activeProjectId, projects } from "$lib/stores/projects";
  import { bindLaunchStatus, reportUiReady, launchStatus } from "$lib/stores/launch";

  let sidebarCollapsed = $state(false);
  let rightOpen = $state(false);
  let settingsOpen = $state(false);
  let docsOpen = $state(false);
  let ready = $state(false);
  let bootPhase = $state("Starting Grok Desktop…");
  let bootError = $state<string | null>(null);
  let workspaceCwd = $state(".");

  onMount(() => {
    // Mark JS alive ASAP so a slow boot is not mis-reported as "dead port 1420".
    void reportUiReady().catch(() => {
      /* boot() will retry / surface errors */
    });
    void boot();
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n" && !e.shiftKey) {
        e.preventDefault();
        void createNewChat($activeProjectId);
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "y") {
        e.preventDefault();
        yoloEnabled.update((v) => {
          const next = !v;
          void invoke("set_session_yolo", { yolo: next }).catch(() => {});
          return next;
        });
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "v") {
        e.preventDefault();
        void setVerboseMode(!$verboseMode);
      }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        settingsOpen = true;
      }
      if (e.key === "F1" || ((e.ctrlKey || e.metaKey) && e.key === "/")) {
        e.preventDefault();
        docsOpen = true;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function boot() {
    bootError = null;
    try {
      bootPhase = "Connecting to app backend…";
      await bindLaunchStatus();

      bootPhase = "Binding chat events…";
      await bindGrokEvents();

      bootPhase = "Loading settings…";
      const s = await loadSettings();
      await loadModels();
      await loadProjects();

      sidebarCollapsed = s.sidebar_collapsed;
      rightOpen = s.right_panel_open;
      selectedModel.set(s.default_model || "grok-4.5");
      yoloEnabled.set(!!s.yolo_default);
      verboseMode.set(!!s.verbose_mode);

      if (s.last_project_id) {
        const found = $projects.find((p) => p.id === s.last_project_id);
        if (found) {
          activeProjectId.set(found.id);
          workspaceCwd = found.path;
        }
      }

      try {
        const appData = await invoke<string>("get_app_data_dir");
        if (!$activeProjectId) {
          workspaceCwd = appData;
        }
      } catch {
        /* ignore */
      }

      bootPhase = "Restoring chats…";
      await refreshChatList($activeProjectId);

      const project = $activeProjectId
        ? get(projects).find((p) => p.id === $activeProjectId)
        : null;
      const lastChatId = project?.last_chat_id ?? null;
      try {
        if (lastChatId) {
          await openChat(lastChatId);
        } else {
          const list = get(chatList);
          if (list[0]?.id) {
            await openChat(list[0].id);
          } else {
            await createNewChat($activeProjectId);
          }
        }
      } catch {
        if (!get(currentChat)) {
          await createNewChat($activeProjectId);
        }
      }

      bootPhase = "Starting Grok session (optional)…";
      try {
        await invoke("start_grok_session", {
          model: $selectedModel,
          yolo: $yoloEnabled,
          cwd: $activeProjectId
            ? ($projects.find((p) => p.id === $activeProjectId)?.path ?? workspaceCwd)
            : workspaceCwd,
          chatId: $currentChat?.id ?? null,
        });
      } catch {
        // Grok CLI may be missing — not a WebView failure.
      }

      // ONLY now claim UI success — shell is mounted and interactive.
      bootPhase = "UI ready";
      await reportUiReady();
      ready = true;
    } catch (e) {
      bootError = String(e);
      bootPhase = "Boot failed";
      ready = false;
      try {
        await invoke("report_ui_failed", { reason: bootError });
      } catch {
        /* not in tauri */
      }
    }
  }

  async function retryBoot() {
    ready = false;
    bootError = null;
    bootPhase = "Retrying…";
    try {
      await invoke("retry_ui_load");
    } catch {
      /* fall through to local boot */
    }
    await boot();
  }

  async function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    const next = { ...$settings, sidebar_collapsed: sidebarCollapsed };
    try {
      await persistSettings(next);
    } catch {
      /* non-fatal */
    }
  }

  async function setRightOpen(open: boolean) {
    rightOpen = open;
    const next = { ...$settings, right_panel_open: open };
    try {
      await persistSettings(next);
    } catch {
      /* non-fatal */
    }
  }

  async function toggleRight() {
    await setRightOpen(!rightOpen);
  }
</script>

{#if !ready}
  <div class="boot" role="status" aria-live="polite">
    <div class="boot-card">
      <div class="boot-title">Grok Desktop</div>
      <div class="boot-phase">{$launchStatus.phase || bootPhase}</div>
      {#if bootError || $launchStatus.load_failed}
        <p class="boot-error">
          {bootError || $launchStatus.last_error || "UI failed to load."}
        </p>
        <ul class="boot-help">
          <li>Prefer stable launch: <code>Grok-Desktop.bat</code> or <code>npm start</code></li>
          <li>
            Do not open a bare <code>grok-desktop.exe</code> from a previous <code>tauri dev</code>
          </li>
          <li>For hot reload use <code>npm run start:dev</code> and leave the console open</li>
        </ul>
        <button type="button" class="boot-retry" onclick={() => void retryBoot()}>Retry</button>
      {:else}
        <p class="boot-hint">Loading the interface…</p>
      {/if}
    </div>
  </div>
{:else}
  <div class="shell">
    <Sidebar collapsed={sidebarCollapsed} ontoggle={toggleSidebar} />

    <div class="main">
      <header class="topbar">
        <div class="left">
          <h1 class="title">Grok Desktop</h1>
        </div>
        <div class="right" role="toolbar" aria-label="Session controls">
          <UsageMeter />
          <VerboseToggle />
          <button
            type="button"
            class="tb"
            onclick={toggleRight}
            title="Context panel"
            aria-pressed={rightOpen}
          >
            {rightOpen ? "Hide context" : "Context"}
          </button>
          <button
            type="button"
            class="tb"
            onclick={() => (docsOpen = true)}
            title="Documentation (F1)"
          >
            Docs
          </button>
          <button
            type="button"
            class="tb"
            onclick={() => (settingsOpen = true)}
            title="Settings (Ctrl+,)"
          >
            Settings
          </button>
        </div>
      </header>
      <ChatArea />
    </div>

    <RightPanel open={rightOpen} onclose={() => void setRightOpen(false)} />
  </div>

  <SettingsModal open={settingsOpen} onclose={() => (settingsOpen = false)} />
  <DocumentationModal open={docsOpen} onclose={() => (docsOpen = false)} />

  {#if $errorToast}
    <div class="toast" role="alert" aria-live="assertive">{$errorToast}</div>
  {/if}
{/if}

<style>
  .boot {
    height: 100vh;
    display: grid;
    place-items: center;
    color: var(--muted);
    background: var(--bg);
    padding: 1.5rem;
  }
  .boot-card {
    max-width: 420px;
    text-align: center;
  }
  .boot-title {
    font-weight: 800;
    font-size: 1.2rem;
    color: var(--text);
    margin-bottom: 0.5rem;
  }
  .boot-phase {
    color: var(--accent);
    font-size: 0.95rem;
    margin-bottom: 0.75rem;
  }
  .boot-hint {
    font-size: 0.85rem;
  }
  .boot-error {
    color: #ffb0a0;
    font-size: 0.9rem;
    line-height: 1.45;
  }
  .boot-help {
    text-align: left;
    font-size: 0.82rem;
    line-height: 1.45;
    margin: 0.75rem auto 1rem;
    padding-left: 1.1rem;
  }
  .boot-help code {
    font-size: 0.85em;
    background: var(--surface-2);
    padding: 0.05rem 0.3rem;
    border-radius: 4px;
  }
  .boot-retry {
    border: none;
    border-radius: 8px;
    padding: 0.5rem 1.1rem;
    font-weight: 600;
    cursor: pointer;
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    font-family: inherit;
  }
  .shell {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    background: var(--bg);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.55rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--sidebar);
    min-height: 52px;
  }
  .title {
    margin: 0;
    font-weight: 700;
    font-size: 0.95rem;
    letter-spacing: 0.02em;
  }
  .right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .tb {
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 8px;
    padding: 0.35rem 0.7rem;
    font-size: 0.82rem;
    cursor: pointer;
    font-family: inherit;
  }
  .tb:hover {
    border-color: var(--accent-dim);
  }
</style>
