# Architecture

Grok Desktop is a local desktop shell around the Grok Build CLI.

## Stack

- Tauri 2 hosts the native desktop window, tray, file dialogs, and Rust command layer.
- Svelte 5 renders the chat UI, sidebar, parallel-agent workspace, settings, docs, and context panels.
- Grok Build CLI is spawned by Rust for each headless chat turn.

## Data Flow

```mermaid
flowchart LR
  User["User"] --> UI["Svelte UI"]
  UI --> IPC["Tauri IPC commands"]
  IPC --> Rust["Rust command layer"]
  Rust --> Grok["grok CLI"]
  Grok --> Rust
  Rust --> Events["Tauri events"]
  Events --> UI
  Rust --> AppData["App data JSON files"]
```

## Main Modules

| Area                         | Files                                                          |
| ---------------------------- | -------------------------------------------------------------- |
| App shell                    | `src/routes`, `src/lib/components`                             |
| Chat state                   | `src/lib/stores/chat.ts`                                       |
| Project state                | `src/lib/stores/projects.ts`                                   |
| Settings and capabilities    | `src/lib/stores/settings.ts`, `src/lib/stores/capabilities.ts` |
| Parallel agent state         | `src/lib/stores/agents.ts`                                     |
| Desktop and CLI updates      | `src/lib/stores/updater.ts`, `src-tauri/src/grok_cli.rs`       |
| Privacy audit state          | `src/lib/stores/privacy.ts`                                    |
| Tauri command bridge         | `src-tauri/src/commands.rs`                                    |
| Grok process execution       | `src-tauri/src/grok_process.rs`                                |
| ACP extension transport      | `src-tauri/src/grok_acp.rs`                                    |
| Grok inventory/context       | `src-tauri/src/capabilities.rs`, `src-tauri/src/grok_cli.rs`   |
| Parallel agent execution     | `src-tauri/src/agent_runs.rs`                                  |
| Privacy audit and safeguards | `src-tauri/src/privacy.rs`                                     |
| Local config and persistence | `src-tauri/src/config.rs`                                      |
| Images                       | `src-tauri/src/image_handler.rs`                               |
| Tray                         | `src-tauri/src/tray.rs`                                        |

## Chat Execution

Full-access, auto-edit, and plan turns run as bounded headless commands:

```text
grok -p <prompt> -m <model> --reasoning-effort <level> --cwd <project> --output-format plain
```

The app streams stdout/stderr to the frontend, keeps Hidden mode clean by default, and stores raw output for explicit reveal.

Ask-before-actions turns use `grok agent stdio` and ACP so protected tool calls can pause for an interactive decision. The client initializes as the upstream `desktop` type, selects the authentication method advertised by the agent, maintains a per-chat ACP session, and answers `session/request_permission` without restarting the turn. The same contained turn handles xAI's blocking question interviews and plan-approval requests, which remain visibly paused in the composer until the user responds.

The same connection consumes Grok's structured background-task and subagent lifecycle extensions. Running items are surfaced as tabs in the Agents workspace and counted in the application menu. After the main prompt response arrives, the contained process remains alive until reported work finishes; a short settle window handles late start events, and completion-before-start IDs are remembered so finished tasks cannot be accidentally re-armed.

## Parallel Agents

The Agents workspace asks `grok inspect --json` for the effective built-in, project, user, and plugin agent inventory. Each manually dispatched task receives a unique Grok session ID and runs independently with the selected agent and current project settings. Runs continue while their tabs are not selected and can be stopped individually. Grok-spawned subagents and background tool tasks join the same tab strip as read-only lifecycle views.

Project definitions are stored in `<project>/.grok/agents/*.md`; user definitions are stored in `~/.grok/agents/*.md`. Names are constrained to path-safe characters, files are created exclusively, and the app allows at most eight concurrent runs.

## Updates

The Tauri updater checks the latest GitHub Release after startup, then every six hours, or on demand. Published artifacts are signed in CI. The app embeds only the public verification key, prompts before installation, and restarts after a verified update finishes.

Grok CLI updates remain a separate official channel. Grok Desktop calls `grok update --check --json` for machine-readable installed/latest status and invokes `grok update` only after a second installation confirmation. Installation is blocked while chat or agent tasks are active and runs without a console window.

At startup, the usage meter opens a short-lived, contained ACP connection and requests the authenticated `x.ai/billing` extension directly. Current allocation, prepaid balance, on-demand usage, period, and subscription tier come from the structured response. If an older CLI does not expose the extension, the app reads its latest bounded billing telemetry snapshot and can trigger the legacy prompt-free refresh path when no snapshot exists.

## Privacy Guard

Privacy Guard applies telemetry-off environment variables to chat and parallel-agent processes. It tails Grok's local unified log from the task's start position; if a repository-state upload event appears, the app terminates the contained Grok process tree and raises a visible privacy alert. The Privacy Center audits historical local evidence, can write equivalent persistent CLI settings after creating a backup, and warns before unusually broad project roots are opened.

Account-level retention changes use Grok Build's authenticated `x.ai/privacy/setCodingDataRetention` ACP extension. The UI and backend still require an exact direction-specific typed confirmation and verify the local authenticated account cache after the server accepts the change. Older CLIs fall back to the contained `/privacy opt-out` and `/privacy opt-in` flows.

## App Data

The app writes user data under `%APPDATA%\com.the-kraken.grok-desktop\`:

- `settings.json`
- `projects.json`
- `chats/*.json`
- `temp_images/`

## Upstream Compatibility

The official CLI remains the execution engine. Grok Desktop implements only the wire-facing adapter required for its GUI, rather than embedding or forking Grok Build. Extension calls follow ACP 0.10 framing: the internal vendor method `x.ai/...` is sent with a leading underscore on the JSON-RPC wire. Compatibility fallbacks are retained because the public source snapshot and installed release can evolve on different schedules.

See [Upstream compatibility](UPSTREAM_COMPATIBILITY.md) for the audited contracts and remaining integration opportunities.

## Planned Performance Direction

The main performance limitation is process-per-turn startup outside Ask-before-actions mode. The intended upgrade path is a shared persistent Grok transport or leader connection with explicit lifecycle recovery.

## Browser Automation

The Context panel can show browser-capable MCP servers when Grok reports them, for example Playwright-backed servers. The current app does not embed a browser view or expose browser-specific controls; those are roadmap items.
