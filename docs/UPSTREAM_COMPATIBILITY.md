# Grok Build Upstream Compatibility

Grok Desktop uses the official Grok Build CLI as its engine. This document records contracts verified against the public `xai-org/grok-build` source so GUI behavior does not depend on terminal-output guesses.

## Audited baseline

- Repository: <https://github.com/xai-org/grok-build>
- Source snapshot audited: `b189869b7755d2b482969acf6c92da3ecfeffd36`
- Installed runtime verified: Grok Build `0.2.101` (`5bc4b5dfad`, stable)
- ACP protocol: version `"1"`; upstream Rust SDK `agent-client-protocol 0.10.4`

The public repository is periodically synchronized from xAI's internal monorepo. Grok Desktop therefore treats source-defined extensions as versioned capabilities and retains safe fallbacks where practical.

## Implemented contracts

### ACP initialization and authentication

The client sends protocol version `"1"`, declares read/write filesystem and terminal capabilities as unavailable, identifies itself with `_meta.clientType = "desktop"`, and selects the agent's `_meta.defaultAuthMethodId` when advertised. Legacy agents fall back to `cached_token`, then their first advertised method.

### Extension framing

ACP 0.10 represents xAI extensions internally as `x.ai/...`, but JSON-RPC transports extension names with a leading underscore. For example:

```text
internal: x.ai/billing
wire:     _x.ai/billing
```

The adapter owns this translation so product modules never construct wire names.

### Billing

`x.ai/billing` supplies structured allocation percentage, current period, prepaid balance, on-demand usage/cap, subscription tier, and billing-history data. Grok Desktop consumes the current-period fields directly. The prior telemetry reader remains a bounded compatibility cache.

### Privacy

`x.ai/privacy/setCodingDataRetention` changes authenticated server-side coding-data retention and updates Grok's local account cache. Grok Desktop preserves its direction-specific typed warnings, sends the extension request, validates the response, and verifies the cache. The `/privacy` path remains only for older CLIs.

### Interactive permissions

`session/request_permission` is a blocking client request. Grok Desktop displays the tool call and agent-provided options, then returns either the selected option identifier or a distinct cancellation outcome. Cancellation is not treated as user rejection or successful completion.

### Questions and plan approval

`x.ai/ask_user_question` and `x.ai/exit_plan_mode` are blocking reverse requests, just like permissions. Grok Desktop renders their structured payloads in the composer and returns the exact upstream tagged outcome. Question responses support accepted answers, cancellation, and the plan-only `chat_about_this` and `skip_interview` paths. Plan responses support approval, cancellation with revision feedback, and abandonment. Stopping a turn resolves any parked interaction safely before process cleanup.

### CLI updates

`grok update --check --json` can legitimately return `null` for `latestVersion` and `installer`, particularly when a registry check fails. The parser accepts that contract and preserves the upstream error.

## Next integrations

1. Move all chat modes onto a resilient shared ACP/leader transport after reconnect and crash-recovery behavior is designed.
2. Add controls for upstream task kill, subagent cancellation, and monitor lifecycle once persistent transport ownership is available.
3. Add upstream contract fixtures for session loading and task cancellation.
4. Add reconnect replay so an Agents tab can recover lifecycle state after the desktop application itself restarts.

## Deliberate boundary

Grok Desktop does not vendor the complete Grok runtime. Keeping the official executable preserves xAI authentication, updater behavior, and runtime compatibility. The adapter is intentionally small so upstream protocol changes remain isolated and testable.
