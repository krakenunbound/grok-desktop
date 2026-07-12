# Changelog

All notable changes to Grok Desktop are tracked here.

## 0.4.0 - 2026-07-12

### Added

- Guided New Project flow for creating a project or using an existing folder.
- Composer selectors for available Grok models and low, medium, or high reasoning effort.
- Clear approval profiles for ask-before-actions, auto-approved edits, plan-only, and full access.

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
