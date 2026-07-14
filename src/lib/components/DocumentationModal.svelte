<script lang="ts">
  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();

  function onBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={onBackdrop}
    onkeydown={(e) => {
      if (e.key === "Escape") onclose();
    }}
  >
    <div class="modal" role="dialog" aria-modal="true" aria-label="Documentation" tabindex="-1">
      <header>
        <div>
          <h2>Documentation</h2>
          <p>Grok Desktop working notes</p>
        </div>
        <button type="button" class="x" onclick={onclose} aria-label="Close documentation">x</button
        >
      </header>

      <nav aria-label="Documentation sections">
        <a href="#quick-start">Quick start</a>
        <a href="#projects">Projects</a>
        <a href="#chat">Chat</a>
        <a href="#agents">Agents</a>
        <a href="#privacy">Privacy</a>
        <a href="#settings">Settings</a>
        <a href="#troubleshooting">Troubleshooting</a>
        <a href="#roadmap">Roadmap</a>
      </nav>

      <section id="quick-start">
        <h3>Quick start</h3>
        <ol>
          <li>
            Select <strong>New Project</strong>, then create a new project or use an existing
            folder.
          </li>
          <li>Choose the model, reasoning depth, and approval profile in the composer.</li>
          <li>
            Send a prompt. Hidden mode shows clean answers; Verbose mode shows raw agent output.
          </li>
          <li>Use <strong>Stop</strong> to cancel a long turn.</li>
        </ol>
      </section>

      <section id="agents">
        <h3>Parallel agents</h3>
        <ul>
          <li>
            Open <strong>Agents</strong> or press <strong>Ctrl+Shift+A</strong> to dispatch independent
            tasks that continue in background tabs.
          </li>
          <li>
            The agent picker includes definitions Grok discovers from built-ins, the current
            project, your user profile, and plugins.
          </li>
          <li>
            Create reusable project agents in <code>.grok/agents</code> or user agents in
            <code>~/.grok/agents</code> without leaving the app.
          </li>
          <li>Up to eight tasks can run concurrently, and each task can be stopped separately.</li>
        </ul>
      </section>

      <section id="projects">
        <h3>Projects</h3>
        <ul>
          <li><strong>Pinned Projects</strong> keeps important folders at the top.</li>
          <li><strong>Recent Projects</strong> tracks folders opened from the sidebar.</li>
          <li>
            Hover a project row to reveal Pin, Archive, Notes, and Remove; the three-dot button
            keeps the same menu keyboard-accessible.
          </li>
          <li>Press <strong>Ctrl+K</strong> to search projects and chats together.</li>
          <li>
            Switching projects changes the Grok working directory and loads that folder's chat list.
          </li>
        </ul>
      </section>

      <section id="chat">
        <h3>Chat and attachments</h3>
        <ul>
          <li>
            Paste images or drop/select images, video, audio, documents, code, and archives;
            messages keep rich previews.
          </li>
          <li>Attachments are copied into managed app storage before Grok receives their paths.</li>
          <li>
            Use a chat's three-dot menu to export it as Markdown. Hover or focus a message to copy
            it or retry a prior user prompt; fenced code blocks have their own Copy button.
          </li>
          <li>
            Ask for <strong>show raw output</strong> when you need the full agent/tool stream.
          </li>
          <li>
            Each turn currently launches a headless Grok CLI process with
            <code>--output-format plain</code> and continues prior context when possible.
          </li>
        </ul>
      </section>

      <section id="privacy">
        <h3>Privacy Center</h3>
        <ul>
          <li>
            Open <strong>Menu → Privacy Center</strong> at the bottom of the sidebar to audit repository-upload
            evidence stored in Grok's local log and review the account retention status reported by Grok.
          </li>
          <li>
            Privacy Guard adds telemetry-off environment settings to every app-launched Grok task
            and stops the task if a repository upload event appears while it is running.
          </li>
          <li>
            <strong>Protect Grok CLI config</strong> creates a timestamped backup and disables
            telemetry, trace uploads, and Mixpanel in <code>~/.grok/config.toml</code>.
          </li>
          <li>
            Export an audit report or archive and clear the local log. Local clearing does not
            delete data already held by xAI. Use the account-retention control to enable Zero Data
            Retention; xAI states that opting out also deletes previously synced data.
          </li>
          <li>Broad project folders such as a home directory trigger a warning before opening.</li>
        </ul>
      </section>

      <section id="settings">
        <h3>Settings and context</h3>
        <ul>
          <li>
            Settings controls model and reasoning defaults, plan mode, memory, web search,
            subagents, and advanced permissions.
          </li>
          <li>
            The Context panel shows status, project path, active flags, detected MCP servers, and
            plugins.
          </li>
          <li>
            It also shows read-only Grok CLI capabilities, recent CLI sessions, and tracked
            worktrees.
          </li>
          <li>
            MCP servers managed by config can be toggled here; plugin or connector-owned servers are
            read-only.
          </li>
          <li>
            Grok Desktop updates are checked against signed GitHub Releases. Grok CLI updates use
            Grok Build's official self-updater. Both install only after confirmation.
          </li>
        </ul>
      </section>

      <section id="troubleshooting">
        <h3>Troubleshooting</h3>
        <ul>
          <li>If Grok is not found, set the Grok binary path in Settings or add Grok to PATH.</li>
          <li>
            If responses feel slow, remember this GUI still spawns a fresh headless Grok process per
            turn. Persistent agent transport is the right performance fix.
          </li>
          <li>
            If output looks noisy, switch Hidden mode on. Verbose mode intentionally exposes raw
            streams.
          </li>
          <li>
            If an image fails, reattach it so the app can copy it into its managed temp directory.
          </li>
        </ul>
      </section>

      <section id="roadmap">
        <h3>Roadmap gaps</h3>
        <ul>
          <li>Persistent agent sessions instead of process-per-turn headless calls.</li>
          <li>Session/worktree actions: resume, export, trace, delete, create, and remove.</li>
          <li>Git diff/review pane with stage, revert, commit, push, and PR helpers.</li>
          <li>
            Integrated terminal and project actions for build, test, run, and preview commands.
          </li>
          <li>Full worktree workflow for isolated parallel tasks.</li>
          <li>In-app browser or Chrome control for visual verification.</li>
          <li>Automations and scheduled checks.</li>
        </ul>
      </section>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 55;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.58);
  }
  .modal {
    width: min(820px, 100%);
    max-height: min(860px, calc(100vh - 2rem));
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1rem;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.45);
    display: grid;
    gap: 0.9rem;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
  }
  h2,
  h3,
  p {
    margin: 0;
  }
  h2 {
    font-size: 1.05rem;
  }
  h3 {
    font-size: 0.82rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  header p {
    color: var(--muted);
    font-size: 0.78rem;
  }
  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.1rem;
    cursor: pointer;
  }
  nav {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    padding-bottom: 0.2rem;
  }
  nav a {
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.22rem 0.55rem;
    color: var(--text);
    text-decoration: none;
    background: var(--surface-2);
    font-size: 0.76rem;
  }
  nav a:hover {
    border-color: var(--accent-dim);
  }
  section {
    display: grid;
    gap: 0.45rem;
    padding-top: 0.25rem;
  }
  ol,
  ul {
    margin: 0;
    padding-left: 1.15rem;
    color: var(--text);
    line-height: 1.5;
    font-size: 0.88rem;
  }
  li + li {
    margin-top: 0.24rem;
  }
</style>
