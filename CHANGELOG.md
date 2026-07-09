# Changelog

All notable changes to Grok Desktop are tracked here.

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
