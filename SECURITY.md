# Security

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.6.x   | Yes       |
| < 0.6   | No        |

## Local Data

Grok Desktop stores settings, project lists, chats, and attached temp images under:

```text
%APPDATA%\com.the-kraken.grok-desktop\
```

Chat history is plaintext. Treat the local machine as trusted and avoid attaching or prompting with secrets unless that is acceptable for your Grok CLI setup.

## Process and File Safety

- Grok is launched with discrete argv entries, not through a shell.
- Attached image paths must resolve under the managed temp-images directory.
- The Grok binary override must point to an executable named `grok` or `grok.exe`.
- Project and chat IDs are validated to avoid path traversal.
- Broad or sensitive project roots trigger a confirmation warning.
- Privacy Guard adds telemetry-off environment settings to app-launched Grok tasks and stops the contained process tree if Grok records a repository-state upload event.
- Persistent CLI config hardening creates a timestamped backup before changing `~/.grok/config.toml`.

Privacy Guard is defense in depth around the installed Grok Build CLI and cannot guarantee the behavior of third-party binaries. Account-level Zero Data Retention is a separate xAI setting managed through Grok Build's `/privacy` flow.

The Privacy Center can invoke Grok Build's supported `/privacy` flow directly. Enabling Zero Data Retention warns that xAI deletes previously synced data; disabling it warns that future code and traces may be retained again. Each direction requires its own exact typed confirmation phrase, which is checked again by the Rust backend before Grok is launched.

## Reporting

Open a GitHub issue with reproduction steps and relevant environment details. Do not include tokens, credentials, private chat logs, or private MCP configuration.
