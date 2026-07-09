# Contributing

Grok Desktop is a Tauri 2 + Svelte 5 desktop app for the Grok Build CLI.

## Local Setup

```powershell
npm install
rustc --version
cargo --version
grok --version
```

For normal local launch:

```powershell
npm start
```

For hot reload:

```powershell
npm run start:dev
```

## Quality Gate

Before opening a PR, run:

```powershell
npm run format
cd src-tauri
cargo fmt --all
cargo test
cargo clippy --all-targets -- -D warnings
cd ..
npm run check
npm run build
npx tauri build --no-bundle --ci
```

## Development Notes

- Do not commit `node_modules`, `.svelte-kit`, `build`, `src-tauri/target`, logs, or local app data.
- Do not commit secrets, Grok credentials, chat transcripts from private work, or local MCP auth files.
- Keep Grok process spawning shell-free: pass arguments as argv entries.
- UI changes should be verified with a real launch screenshot when practical.
- Do not leave `grok-desktop.exe`, `grok.exe`, dev servers, or watchers running after verification.
