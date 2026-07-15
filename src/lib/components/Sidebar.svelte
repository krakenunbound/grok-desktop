<script lang="ts">
  import { onMount } from "svelte";
  import {
    pinnedProjects,
    recentProjects,
    activeProjectId,
    addProject,
    removeProject,
    setPinned,
    setArchived,
    updateProjectNotes,
    touchProject,
    projectSort,
    showArchivedProjects,
    type Project,
  } from "$lib/stores/projects";
  import {
    chatList,
    createNewChat,
    openChat,
    deleteChat,
    currentChat,
    refreshChatList,
    showError,
    ensureSession,
    selectedModel,
    yoloEnabled,
  } from "$lib/stores/chat";
  import { invoke } from "@tauri-apps/api/core";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import ProjectDialog from "$lib/components/ProjectDialog.svelte";
  import GrokCliUpdateControl from "$lib/components/GrokCliUpdateControl.svelte";
  import UpdateControl from "$lib/components/UpdateControl.svelte";
  import VerboseToggle from "$lib/components/VerboseToggle.svelte";

  interface Props {
    collapsed: boolean;
    ontoggle: () => void;
    rightOpen: boolean;
    privacyProtected: boolean;
    privacyWarning: boolean;
    runningAgents: number;
    onagents: () => void;
    onprivacy: () => void;
    oncontext: () => void;
    ondocs: () => void;
    onsettings: () => void;
  }
  let {
    collapsed,
    ontoggle,
    rightOpen,
    privacyProtected,
    privacyWarning,
    runningAgents,
    onagents,
    onprivacy,
    oncontext,
    ondocs,
    onsettings,
  }: Props = $props();

  let expandedNotes = $state<Record<string, boolean>>({});
  let openProjectMenu = $state<string | null>(null);
  let openChatMenu = $state<string | null>(null);
  let now = $state(Date.now());
  let searchQuery = $state("");
  let searchInput = $state<HTMLInputElement>();
  let projectDialogOpen = $state(false);
  let appMenuOpen = $state(false);
  let cliUpdateOpen = $state(false);
  let cliUpdateAvailable = $state(false);
  let normalizedQuery = $derived(searchQuery.trim().toLowerCase());
  let filteredPinnedProjects = $derived(
    $pinnedProjects.filter((project) => matchesProject(project)),
  );
  let filteredRecentProjects = $derived(
    $recentProjects.filter((project) => matchesProject(project)),
  );
  let filteredChats = $derived(
    $chatList.filter(
      (chat) => !normalizedQuery || chat.title.toLowerCase().includes(normalizedQuery),
    ),
  );

  onMount(() => {
    const timer = window.setInterval(() => (now = Date.now()), 60_000);
    return () => window.clearInterval(timer);
  });

  function relativeTime(value: string | null): string {
    if (!value) return "never";
    const then = new Date(value).getTime();
    const diff = Math.max(0, now - then);
    const minute = 60_000;
    const hour = 60 * minute;
    const day = 24 * hour;
    const month = 30 * day;
    if (diff < minute) return "now";
    if (diff < hour) return `${Math.floor(diff / minute)}m ago`;
    if (diff < day) return `${Math.floor(diff / hour)}h ago`;
    if (diff < 7 * day) return `${Math.floor(diff / day)}d ago`;
    if (diff < month) return `${Math.floor(diff / (7 * day))}w ago`;
    if (diff < 12 * month) return `${Math.floor(diff / month)}mo ago`;
    return `${Math.floor(diff / (12 * month))}y ago`;
  }

  function matchesProject(project: Project): boolean {
    if (!normalizedQuery) return true;
    return [project.name, project.path, project.project_type, project.notes]
      .join(" ")
      .toLowerCase()
      .includes(normalizedQuery);
  }

  function toggleProjectMenu(id: string, event: MouseEvent) {
    event.stopPropagation();
    openChatMenu = null;
    openProjectMenu = openProjectMenu === id ? null : id;
  }

  function toggleChatMenu(id: string, event: MouseEvent) {
    event.stopPropagation();
    openProjectMenu = null;
    openChatMenu = openChatMenu === id ? null : id;
  }

  async function onPinProject(project: Project, pinned: boolean) {
    openProjectMenu = null;
    try {
      await setPinned(project.id, pinned);
    } catch (e) {
      showError(e);
    }
  }

  async function onNewChat() {
    try {
      const pid = $activeProjectId;
      const chat = await createNewChat(pid);
      if (pid) await touchProject(pid, chat.id);
    } catch (e) {
      showError(e);
    }
  }

  async function onAddProject(path: string) {
    try {
      const project = await addProject(path);
      await selectProject(project);
    } catch (e) {
      showError(e);
    }
  }

  async function selectProject(project: Project) {
    try {
      activeProjectId.set(project.id);
      await refreshChatList(project.id);
      const list = $chatList;
      let chatId = list[0]?.id ?? null;
      if (chatId) {
        await openChat(chatId);
      } else {
        const chat = await createNewChat(project.id);
        chatId = chat.id;
      }
      await touchProject(project.id, chatId);
      await ensureSession($selectedModel, $yoloEnabled, project.path);
    } catch (e) {
      showError(e);
    }
  }

  async function selectChat(id: string) {
    try {
      await openChat(id);
      const projectId = $currentChat?.project_id ?? null;
      if (projectId) await touchProject(projectId, id);
    } catch (e) {
      showError(e);
    }
  }

  async function switchToWorkspaceChat() {
    activeProjectId.set(null);
    await refreshChatList(null);
    const next = $chatList[0];
    if (next) {
      await openChat(next.id);
    } else {
      await createNewChat(null);
    }
  }

  function toggleNotes(id: string) {
    openProjectMenu = null;
    expandedNotes = { ...expandedNotes, [id]: !expandedNotes[id] };
  }

  async function onRemoveProject(project: Project) {
    const ok = window.confirm(
      `Remove "${project.name}" from the sidebar? The folder is not deleted.`,
    );
    if (!ok) return;
    try {
      const wasActive = $activeProjectId === project.id;
      await removeProject(project.id);
      if (wasActive) await switchToWorkspaceChat();
    } catch (e) {
      showError(e);
    }
  }

  async function onArchiveProject(project: Project) {
    try {
      const wasActive = $activeProjectId === project.id;
      await setArchived(project.id, !project.archived);
      if (wasActive && !project.archived) await switchToWorkspaceChat();
    } catch (e) {
      showError(e);
    }
  }

  async function onDeleteChat(id: string, title: string) {
    openChatMenu = null;
    const ok = window.confirm(`Delete chat "${title || "Untitled"}"?`);
    if (!ok) return;
    await deleteChat(id);
  }

  async function onExportChat(id: string, title: string) {
    openChatMenu = null;
    try {
      const safeTitle = (title || "Grok chat").replace(/[<>:"/\\|?*]/g, "_").slice(0, 80);
      const destination = await saveDialog({
        title: "Export Grok chat",
        defaultPath: `${safeTitle}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!destination) return;
      await invoke("export_chat_markdown", { chatId: id, destination });
    } catch (error) {
      showError(error);
    }
  }

  async function saveNotes(project: Project, event: Event) {
    const notes = (event.currentTarget as HTMLTextAreaElement).value;
    if (notes !== project.notes) {
      try {
        await updateProjectNotes(project.id, notes);
      } catch (e) {
        showError(e);
      }
    }
  }
</script>

<svelte:window
  onclick={() => {
    openProjectMenu = null;
    openChatMenu = null;
    appMenuOpen = false;
  }}
  onkeydown={(event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    }
    if (event.key === "Escape") {
      openProjectMenu = null;
      openChatMenu = null;
      appMenuOpen = false;
      if (document.activeElement === searchInput && searchQuery) searchQuery = "";
    }
  }}
/>

<aside class="sidebar" class:collapsed>
  <div class="head">
    <button type="button" class="icon-btn" onclick={ontoggle} title="Toggle sidebar">
      {collapsed ? ">" : "<"}
    </button>
    {#if !collapsed}
      <img class="brand-logo" src="/grok-gui-logo.webp" alt="" aria-hidden="true" />
      <div class="brand-block">
        <span class="brand">Grok</span>
        <span class="subbrand">Build workspace</span>
      </div>
    {/if}
  </div>

  {#if !collapsed}
    <button type="button" class="new-project" onclick={() => (projectDialogOpen = true)}>
      <span aria-hidden="true">＋</span> New Project
    </button>

    <div class="toolbar">
      <select bind:value={$projectSort} title="Sort projects">
        <option value="last-opened">Last worked</option>
        <option value="last-modified">Modified</option>
        <option value="name">Name</option>
        <option value="type">Type</option>
      </select>
      <label class="archive-toggle" title="Show archived projects">
        <input type="checkbox" bind:checked={$showArchivedProjects} />
        Archived
      </label>
    </div>

    <label class="search">
      <span aria-hidden="true">⌕</span>
      <input
        bind:this={searchInput}
        bind:value={searchQuery}
        type="search"
        placeholder="Search projects and chats"
        aria-label="Search projects and chats"
      />
      {#if searchQuery}
        <button type="button" onclick={() => (searchQuery = "")} aria-label="Clear search">×</button
        >
      {/if}
    </label>

    <div class="section">
      <div class="label">Pinned Projects</div>
      {#if filteredPinnedProjects.length === 0}
        <div class="empty-hint">
          {normalizedQuery
            ? "No matching pinned projects."
            : "Pinned folders appear here after you open and pin a project."}
        </div>
      {:else}
        {#each filteredPinnedProjects as p (p.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions: hover reveals the same menu available from the keyboard-accessible action button -->
          <div
            class="project-row"
            class:active={$activeProjectId === p.id}
            onmouseenter={() => {
              openChatMenu = null;
              openProjectMenu = p.id;
            }}
            onmouseleave={() => (openProjectMenu = null)}
          >
            <button
              type="button"
              class="project-main"
              onclick={() => selectProject(p)}
              title={p.path}
            >
              <span class="project-name">{p.name}</span>
              <span class="project-meta">{p.project_type}</span>
            </button>
            <time class="project-age" datetime={p.last_opened}>{relativeTime(p.last_opened)}</time>
            <button
              type="button"
              class="project-more"
              aria-label={`More actions for ${p.name}`}
              aria-haspopup="menu"
              aria-expanded={openProjectMenu === p.id}
              onclick={(event) => toggleProjectMenu(p.id, event)}>•••</button
            >
            {#if openProjectMenu === p.id}
              <div class="project-menu" role="menu" aria-label={`${p.name} actions`}>
                <button type="button" role="menuitem" onclick={() => onPinProject(p, false)}>
                  Unpin
                </button>
                <button type="button" role="menuitem" onclick={() => onArchiveProject(p)}>
                  {p.archived ? "Unarchive" : "Archive"}
                </button>
                <button type="button" role="menuitem" onclick={() => toggleNotes(p.id)}>
                  {expandedNotes[p.id] ? "Hide notes" : "Notes"}
                </button>
                <button
                  type="button"
                  role="menuitem"
                  class="danger"
                  onclick={() => onRemoveProject(p)}>Remove from sidebar</button
                >
              </div>
            {/if}
          </div>
          {#if expandedNotes[p.id]}
            <textarea
              class="notes"
              placeholder="Project notes or last activity..."
              onblur={(e) => saveNotes(p, e)}>{p.notes}</textarea
            >
          {/if}
        {/each}
      {/if}
    </div>

    <div class="section projects">
      <div class="label">Recent Projects</div>
      {#each filteredRecentProjects as p (p.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions: hover reveals the same menu available from the keyboard-accessible action button -->
        <div
          class="project-row"
          class:active={$activeProjectId === p.id}
          class:archived={p.archived}
          onmouseenter={() => {
            openChatMenu = null;
            openProjectMenu = p.id;
          }}
          onmouseleave={() => (openProjectMenu = null)}
        >
          <button
            type="button"
            class="project-main"
            onclick={() => selectProject(p)}
            title={p.path}
          >
            <span class="project-name">{p.name}</span>
            <span class="project-meta">{p.archived ? "Archived" : p.project_type}</span>
          </button>
          <time class="project-age" datetime={p.last_opened}>{relativeTime(p.last_opened)}</time>
          <button
            type="button"
            class="project-more"
            aria-label={`More actions for ${p.name}`}
            aria-haspopup="menu"
            aria-expanded={openProjectMenu === p.id}
            onclick={(event) => toggleProjectMenu(p.id, event)}>•••</button
          >
          {#if openProjectMenu === p.id}
            <div class="project-menu" role="menu" aria-label={`${p.name} actions`}>
              <button type="button" role="menuitem" onclick={() => onPinProject(p, true)}>
                Pin
              </button>
              <button type="button" role="menuitem" onclick={() => onArchiveProject(p)}>
                {p.archived ? "Unarchive" : "Archive"}
              </button>
              <button type="button" role="menuitem" onclick={() => toggleNotes(p.id)}>
                {expandedNotes[p.id] ? "Hide notes" : "Notes"}
              </button>
              <button
                type="button"
                role="menuitem"
                class="danger"
                onclick={() => onRemoveProject(p)}>Remove from sidebar</button
              >
            </div>
          {/if}
        </div>
        {#if expandedNotes[p.id]}
          <textarea
            class="notes"
            placeholder="Project notes or last activity..."
            onblur={(e) => saveNotes(p, e)}>{p.notes}</textarea
          >
        {/if}
      {:else}
        <div class="empty-hint">
          {normalizedQuery
            ? "No matching recent projects."
            : "No projects yet. Use New Project to get started."}
        </div>
      {/each}
    </div>

    <div class="section chats">
      <div class="section-head">
        <div class="label">Chats</div>
        <button
          type="button"
          class="section-add"
          onclick={onNewChat}
          title="New chat"
          aria-label="New chat">＋</button
        >
      </div>
      {#each filteredChats as c (c.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions: hover reveals the same menu available from the keyboard-accessible action button -->
        <div
          class="chat-row"
          class:active={$currentChat?.id === c.id}
          onmouseenter={() => {
            openProjectMenu = null;
            openChatMenu = c.id;
          }}
          onmouseleave={() => (openChatMenu = null)}
        >
          <button type="button" class="chat" onclick={() => selectChat(c.id)}>
            <span>{c.title || "Untitled"}</span>
          </button>
          <time class="chat-age" datetime={c.updated_at}>{relativeTime(c.updated_at)}</time>
          <button
            type="button"
            class="chat-more"
            aria-label={`More actions for ${c.title || "Untitled"}`}
            aria-haspopup="menu"
            aria-expanded={openChatMenu === c.id}
            onclick={(event) => toggleChatMenu(c.id, event)}
          >
            •••
          </button>
          {#if openChatMenu === c.id}
            <div class="chat-menu" role="menu" aria-label={`${c.title || "Untitled"} actions`}>
              <button type="button" role="menuitem" onclick={() => onExportChat(c.id, c.title)}>
                Export Markdown
              </button>
              <button
                type="button"
                role="menuitem"
                class="danger"
                onclick={() => onDeleteChat(c.id, c.title)}>Delete chat</button
              >
            </div>
          {/if}
        </div>
      {:else}
        <div class="empty-hint">
          {normalizedQuery ? "No matching chats." : "No chats for this project."}
        </div>
      {/each}
    </div>
  {/if}

  <div class="app-menu">
    {#if appMenuOpen}
      <div class="app-menu-popover" role="menu" aria-label="Grok Desktop menu">
        <button type="button" role="menuitem" onclick={() => ((appMenuOpen = false), onagents())}>
          <span aria-hidden="true">✦</span>
          <span>Parallel agents</span>
          {#if runningAgents > 0}<small>{runningAgents} running</small>{/if}
        </button>
        <button
          type="button"
          role="menuitem"
          class:protected={privacyProtected}
          class:warning={privacyWarning}
          onclick={() => ((appMenuOpen = false), onprivacy())}
        >
          <span aria-hidden="true">{privacyProtected ? "◆" : "◇"}</span>
          <span>Privacy Center</span>
          <small>{privacyProtected ? "Guard on" : "Guard off"}</small>
        </button>
        <button type="button" role="menuitem" onclick={() => ((appMenuOpen = false), oncontext())}>
          <span aria-hidden="true">▤</span>
          <span>{rightOpen ? "Hide context" : "Show context"}</span>
        </button>
        <div class="component-action" role="presentation"><VerboseToggle /></div>
        <div class="component-action" role="presentation"><UpdateControl /></div>
        <button
          type="button"
          role="menuitem"
          class:protected={cliUpdateAvailable}
          onclick={() => {
            appMenuOpen = false;
            cliUpdateOpen = true;
          }}
        >
          {#if cliUpdateAvailable}
            <span class="update-dot" aria-hidden="true"></span>
          {:else}
            <span aria-hidden="true">↻</span>
          {/if}
          <span>Grok CLI updates</span>
          {#if cliUpdateAvailable}<small>Available</small>{/if}
        </button>
        <button type="button" role="menuitem" onclick={() => ((appMenuOpen = false), ondocs())}>
          <span aria-hidden="true">?</span>
          <span>Documentation</span>
          <small>F1</small>
        </button>
        <button type="button" role="menuitem" onclick={() => ((appMenuOpen = false), onsettings())}>
          <span aria-hidden="true">⚙</span>
          <span>Settings</span>
          <small>Ctrl+,</small>
        </button>
      </div>
    {/if}
    <button
      type="button"
      class="app-menu-trigger"
      aria-label="Open Grok Desktop menu"
      aria-haspopup="menu"
      aria-expanded={appMenuOpen}
      title="Grok Desktop menu"
      onclick={(event) => {
        event.stopPropagation();
        appMenuOpen = !appMenuOpen;
      }}
    >
      <span aria-hidden="true">☰</span>
      {#if !collapsed}<span>Menu</span><small>v0.7.3</small>{/if}
    </button>
  </div>
</aside>

<ProjectDialog
  open={projectDialogOpen}
  onclose={() => (projectDialogOpen = false)}
  onselect={onAddProject}
/>

<GrokCliUpdateControl
  open={cliUpdateOpen}
  onclose={() => (cliUpdateOpen = false)}
  onstatus={(available) => (cliUpdateAvailable = available)}
/>

<style>
  .sidebar {
    width: 300px;
    min-width: 300px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.75rem;
    transition:
      width 0.18s ease,
      min-width 0.18s ease;
    overflow: hidden;
  }
  .sidebar.collapsed {
    width: 48px;
    min-width: 48px;
    align-items: center;
  }
  .app-menu {
    position: relative;
    margin-top: auto;
    flex: 0 0 auto;
  }
  .app-menu-trigger {
    width: 100%;
    min-height: 36px;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0.55rem;
    border: 1px solid transparent;
    border-radius: 8px;
    padding: 0.45rem 0.6rem;
    background: transparent;
    color: var(--muted);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .collapsed .app-menu-trigger {
    width: 32px;
    display: grid;
    place-items: center;
    padding: 0;
  }
  .app-menu-trigger:hover,
  .app-menu-trigger[aria-expanded="true"] {
    color: var(--text);
    background: var(--surface-2);
    border-color: var(--border);
  }
  .app-menu-trigger small,
  .app-menu-popover small {
    font-size: 0.67rem;
    color: var(--muted);
  }
  .app-menu-popover {
    position: absolute;
    bottom: calc(100% + 0.45rem);
    left: 0;
    z-index: 20;
    width: 250px;
    display: grid;
    gap: 0.15rem;
    padding: 0.4rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface);
    box-shadow: 0 16px 42px rgba(0, 0, 0, 0.5);
  }
  .app-menu-popover > button {
    display: grid;
    grid-template-columns: 20px 1fr auto;
    align-items: center;
    gap: 0.5rem;
    min-height: 36px;
    border: none;
    border-radius: 7px;
    padding: 0.45rem 0.55rem;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
    text-align: left;
    cursor: pointer;
  }
  .app-menu-popover > button:hover,
  .app-menu-popover > button:focus-visible {
    background: var(--surface-2);
    outline: none;
  }
  .app-menu-popover button.protected {
    color: var(--accent);
  }
  .app-menu-popover button.warning {
    color: #ffb38a;
  }
  .app-menu-popover .update-dot {
    width: 8px;
    height: 8px;
    justify-self: center;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent-glow);
  }
  .component-action > :global(button) {
    width: 100%;
    min-height: 36px;
    border: none;
    background: transparent;
    border-radius: 7px;
    justify-content: flex-start;
    padding: 0.45rem 0.55rem 0.45rem 2.55rem;
    font-size: 0.78rem;
  }
  .component-action > :global(button:hover),
  .component-action > :global(button:focus-visible) {
    background: var(--surface-2);
    outline: none;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.2rem;
  }
  .brand-block {
    display: grid;
    min-width: 0;
  }
  .brand-logo {
    width: 28px;
    height: 28px;
    object-fit: contain;
    border-radius: 7px;
  }
  .brand {
    font-weight: 750;
    color: var(--text);
  }
  .subbrand {
    color: var(--muted);
    font-size: 0.72rem;
  }
  .icon-btn {
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 8px;
    width: 32px;
    height: 32px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .new-project {
    width: 100%;
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.55rem 0.75rem;
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
  }
  .new-project {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    border: none;
  }
  .toolbar select {
    background: var(--surface);
    color: var(--text);
  }
  .toolbar {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.45rem;
    align-items: center;
  }
  .toolbar select {
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.35rem 0.45rem;
    font-size: 0.78rem;
  }
  .archive-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--muted);
    font-size: 0.72rem;
    white-space: nowrap;
  }
  .search {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.35rem;
    min-height: 34px;
    padding: 0 0.55rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--muted);
  }
  .search:focus-within {
    border-color: var(--accent-dim);
  }
  .search input {
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  .search button {
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    padding: 0.1rem;
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-height: 0;
  }
  .section.projects,
  .section.chats {
    overflow-y: auto;
  }
  .section.projects {
    max-height: 34%;
  }
  .section.chats {
    flex: 1;
  }
  .label {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    padding: 0.45rem 0.35rem 0.15rem;
  }
  .section-head {
    display: flex;
    align-items: end;
    justify-content: space-between;
  }
  .section-add {
    width: 25px;
    height: 25px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .section-add:hover,
  .section-add:focus-visible {
    border-color: var(--border);
    background: var(--surface-2);
    color: var(--accent);
  }
  .empty-hint {
    font-size: 0.78rem;
    color: var(--muted);
    padding: 0.25rem 0.4rem;
    opacity: 0.82;
  }
  .project-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 30px;
    align-items: center;
    gap: 0.25rem;
    border-radius: 8px;
    padding: 0.2rem;
  }
  .project-row.active {
    background: var(--surface-2);
  }
  .project-row.archived {
    opacity: 0.72;
  }
  .project-main {
    min-width: 0;
    text-align: left;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.45rem 0.5rem;
    border-radius: 8px;
    cursor: pointer;
    display: grid;
    gap: 0.15rem;
  }
  .project-main:hover,
  .chat:hover {
    background: var(--surface-2);
  }
  .project-name,
  .chat span {
    display: block;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.86rem;
  }
  .project-meta {
    color: var(--muted);
    font-size: 0.7rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .project-age {
    color: var(--muted);
    font-size: 0.68rem;
    white-space: nowrap;
    text-align: right;
  }
  .project-more {
    width: 30px;
    height: 28px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-weight: 800;
    letter-spacing: 0.04em;
    line-height: 1;
  }
  .project-more:hover,
  .project-more[aria-expanded="true"] {
    color: var(--text);
    background: var(--surface);
  }
  .project-menu {
    grid-column: 1 / -1;
    justify-self: end;
    width: min(180px, calc(100% - 0.5rem));
    display: grid;
    padding: 0.3rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.35);
    z-index: 4;
  }
  .project-menu button {
    border: none;
    background: transparent;
    color: var(--text);
    border-radius: 6px;
    padding: 0.48rem 0.55rem;
    font: inherit;
    font-size: 0.76rem;
    cursor: pointer;
    text-align: left;
  }
  .project-menu button:hover,
  .project-menu button:focus-visible {
    background: var(--surface-2);
    outline: none;
  }
  .project-menu .danger {
    color: #ffb3b3;
  }
  .notes {
    width: calc(100% - 0.5rem);
    min-height: 58px;
    resize: vertical;
    margin: 0 0.25rem 0.25rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    padding: 0.45rem 0.55rem;
    font-size: 0.78rem;
  }
  .chat-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 30px;
    align-items: center;
    gap: 0.3rem;
    border-radius: 8px;
  }
  .chat-row.active {
    background: var(--surface-2);
  }
  .chat {
    width: auto;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text);
    border-radius: 8px;
    padding: 0.42rem 0.5rem;
    cursor: pointer;
    text-align: left;
  }
  .chat-age {
    color: var(--muted);
    font-size: 0.68rem;
    white-space: nowrap;
    text-align: right;
  }
  .chat-more {
    width: 30px;
    height: 28px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-weight: 800;
    letter-spacing: 0.04em;
    line-height: 1;
  }
  .chat-more:hover,
  .chat-more[aria-expanded="true"] {
    color: var(--text);
    background: var(--surface);
  }
  .chat-menu {
    grid-column: 1 / -1;
    justify-self: end;
    width: min(160px, calc(100% - 0.5rem));
    display: grid;
    padding: 0.3rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.35);
    z-index: 4;
  }
  .chat-menu button {
    border: none;
    border-radius: 6px;
    background: transparent;
    padding: 0.48rem 0.55rem;
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
    cursor: pointer;
    text-align: left;
  }
  .chat-menu .danger {
    color: #ffb3b3;
  }
  .chat-menu button:hover,
  .chat-menu button:focus-visible {
    background: var(--surface-2);
    outline: none;
  }
</style>
