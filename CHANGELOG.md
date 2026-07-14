# Changelog

All notable changes to Grok Desktop are tracked here.

## 0.7.0 - 2026-07-14

### Added

- Grok CLI Update Center backed by the official `grok update --check --json` and `grok update` flows.
- Automatic six-hour CLI update checks with installed/latest version and release-channel visibility.
- A two-step installation warning that preserves projects, chats, settings, and xAI login while replacing the CLI executable.

### Changed

- Usage now performs a prompt-free startup refresh and waits for a new billing snapshot instead of briefly presenting stale allocation data as `0% left`.
- The usage badge shows `Checking…` until fresh information is available and never substitutes a stale numeric value when refresh fails.
- Desktop and CLI update actions are labeled separately in the application menu.

## 0.6.1 - 2026-07-13

### Added

- In-app Zero Data Retention control backed by Grok Build's `/privacy opt-out` and `/privacy opt-in` commands.
- Direction-specific typed confirmation warnings: opting out explains that previously synced data is deleted; opting in warns that future code and trace data may be retained again.

### Security

- Data-retention changes require an exact, direction-specific confirmation phrase in both the interface and backend, and are blocked while other Grok tasks are active.

## 0.6.0 - 2026-07-13

### Added

- Privacy Center for reviewing local repository-upload evidence, account retention status, and Grok CLI privacy configuration.
- Privacy Guard for app-launched chats and agents, including telemetry-off process settings and immediate termination when Grok records a repository-state upload attempt.
- Privacy audit export plus archive-before-clear controls for Grok's local unified log.
- Broad and sensitive project-folder warnings before a project is opened.

### Security

- Grok config hardening creates a timestamped backup before disabling telemetry, trace uploads, and Mixpanel.
- Local log clearing is blocked while Grok Desktop tasks are active and distinguishes local cleanup from remote deletion requests.

### Changed

- Replaced the crowded top-bar action row with a conventional bottom-left application menu for agents, privacy, context, output visibility, updates, documentation, and settings.

## 0.5.0 - 2026-07-13

### Added

- Parallel Agents workspace with one tab per independent run, live output, stop controls, and up to eight concurrent tasks.
- Discovery of Grok's built-in, project, user, and plugin agent definitions through `grok inspect`.
- In-app creation of project `.grok/agents` and user `~/.grok/agents` definitions.
- Signed GitHub Releases updater with quiet startup and six-hour checks, a manual Updates control, download progress, and confirmation before restart.

### Changed

- Exposed Grok Build's background-agent workflow as a desktop-native tabbed workspace instead of requiring `/dashboard` in the CLI.
- Updated release automation to publish the signed updater manifest and platform artifacts.

### Security

- Agent definitions use validated path-safe names and exclusive file creation to prevent traversal and accidental overwrite.
- Parallel agent runs are capped and remain inside the existing Windows process-job containment.

## 0.4.0 - 2026-07-12

### Added

- Guided New Project flow for creating a project or using an existing folder.
- Composer selectors for available Grok models and low, medium, or high reasoning effort.
- Clear approval profiles for ask-before-actions, auto-approved edits, plan-only, and full access.
- Native release packaging for Linux (AppImage, Debian, and RPM) and macOS (Apple Silicon and Intel DMG).

### Changed

- Moved new-chat creation beside the Chats heading and made project/chat action menus reveal on hover.
- Updated project, settings, context, and documentation language around the redesigned workflow.

### Fixed

- Prevented background Grok capability scans from flashing a Command Prompt window on startup.

## 0.3.1 - 2026-07-12

### Changed

- Rebranded the executable, window, tray, shortcuts, and installer with the Kraken theme icon.
- Made the Windows NSIS installer create Start Menu and desktop shortcuts during installation.

## 0.3.0 - 2026-07-12

### Added

- Unified project and chat search with a `Ctrl+K` shortcut.
- Per-chat Markdown transcript export from the chat action menu.
- Message copy and retry actions, plus one-click copy controls on fenced code blocks.
- General file, image, video, and audio attachments with drag-and-drop previews.
- Composer-level model and access-mode controls and a local Grok usage/credit meter.

## 0.2.0 - 2026-07-08

### Added

- Project sidebar with pinned and recent projects, project actions, notes, archive state, and sorting.
- Context panel for Grok status, active flags, detected MCP servers, plugins, CLI sessions, worktrees, and capability notes.
- Settings controls for Grok options including model, YOLO, plan mode, memory, web search, subagents, output visibility, permission mode, and binary override.
- Image paste/drop support with thumbnails and managed temp-image storage.
- In-app documentation modal and keyboard shortcut reference.
- Sky-blue visual theme, logo asset, tray integration, and release launcher scripts.
- Local quality gate documentation and CI workflow.

### Fixed

- Release launch path now uses the production bundle instead of depending on the dev server.
- Hidden mode no longer shows duplicated "Thinking" indicators.
- ANSI/tool-noise cleanup for assistant output.
- Idle footer status no longer shows a pulsing/busy indicator.

### Known Limitations

- Chat turns still use one headless Grok CLI process per turn. Persistent `grok agent stdio` or leader-mode integration is the planned performance upgrade.
- Stop kills the main Grok process, but Windows process-tree cleanup still needs a job-object implementation.
- Chat history is local plaintext under the app data directory.
