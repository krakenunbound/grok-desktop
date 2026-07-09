# Security

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.2.x   | Yes       |

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

## Reporting

Open a GitHub issue with reproduction steps and relevant environment details. Do not include tokens, credentials, private chat logs, or private MCP configuration.
