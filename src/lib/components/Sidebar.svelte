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
  import { open } from "@tauri-apps/plugin-dialog";

  interface Props {
    collapsed: boolean;
    ontoggle: () => void;
  }
  let { collapsed, ontoggle }: Props = $props();

  let expandedNotes = $state<Record<string, boolean>>({});
  let openProjectMenu = $state<string | null>(null);
  let openChatMenu = $state<string | null>(null);
  let now = $state(Date.now());

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

  async function onAddProject() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select project folder",
      });
      if (!selected || Array.isArray(selected)) return;
      const project = await addProject(selected);
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
  }}
  onkeydown={(event) => {
    if (event.key === "Escape") {
      openProjectMenu = null;
      openChatMenu = null;
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
    <button type="button" class="new-chat" onclick={onNewChat}>New chat</button>
    <button type="button" class="add-proj" onclick={onAddProject}>Open Project...</button>

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

    <div class="section">
      <div class="label">Pinned Projects</div>
      {#if $pinnedProjects.length === 0}
        <div class="empty-hint">Pinned folders appear here after you open and pin a project.</div>
      {:else}
        {#each $pinnedProjects as p (p.id)}
          <div class="project-row" class:active={$activeProjectId === p.id}>
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
      {#each $recentProjects as p (p.id)}
        <div
          class="project-row"
          class:active={$activeProjectId === p.id}
          class:archived={p.archived}
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
        <div class="empty-hint">No projects yet. Use Open Project... to add a folder.</div>
      {/each}
    </div>

    <div class="section chats">
      <div class="label">Chats</div>
      {#each $chatList as c (c.id)}
        <div class="chat-row" class:active={$currentChat?.id === c.id}>
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
        <div class="empty-hint">No chats for this project.</div>
      {/each}
    </div>
  {/if}
</aside>

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
  .new-chat,
  .add-proj {
    width: 100%;
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.55rem 0.75rem;
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
  }
  .new-chat {
    background: var(--accent-gradient);
    color: var(--accent-contrast);
    border: none;
  }
  .add-proj,
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
    color: #ffb3b3;
    font: inherit;
    font-size: 0.76rem;
    cursor: pointer;
    text-align: left;
  }
  .chat-menu button:hover,
  .chat-menu button:focus-visible {
    background: var(--surface-2);
    outline: none;
  }
</style>
