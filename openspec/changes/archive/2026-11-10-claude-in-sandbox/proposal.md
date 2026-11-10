# Proposal

## Why

Some projects run Claude Code in a sandbox or container: a project script starts `claude` inside a [Docker Sandbox](https://docs.docker.com/ai/sandboxes/) (`sbx`, a microVM with a deny-all network), or in a container with Docker or Podman. To host such a project, the host has to start a Terminal session and run the script by hand, and so loses the timeline and everything built on it (#106): prompts, responses, tool calls, file changes, documents, screenshots, Claude's browser and the agent status from hooks. Even started as a Claude Code session it could not work today: SpecDeck's hook settings, token and MCP config never reach the sandbox, and the sandbox cannot reach SpecDeck's endpoint on the host's loopback.

## What Changes

- A Claude Code session can start through a command the host sets per folder, such as `just sandbox`, instead of `claude`. SpecDeck appends its Claude Code arguments to that command, so a script that passes its arguments on to `claude` needs no change for them. The command runs through `/bin/sh` with the login shell's environment, whatever the host's shell is.
- Next to the start command, the host can set the address the sandbox reaches the host by, `host.docker.internal` when not set (Podman uses `host.containers.internal`).
- When a session starts through a command, SpecDeck's hook settings and MCP config go to Claude Code inline, as arguments, not as paths to files on the host, and they name the endpoint by that address. The session token still never appears in arguments: it is in the command's environment, for the script to pass into the sandbox.
- The command also gets the endpoint's port in its environment, so the script can allow exactly that address in the sandbox's network policy.
- The endpoint keeps listening on loopback only. Engines that cannot reach the host's loopback, such as Docker Engine on Linux without host networking, are documented, not worked around.
- The status line relay works from inside the sandbox.
- Ending the session stops the start command, as it stops `claude` today.
- When a session started through a command does not reach SpecDeck, the timeline says so, before the first prompt when it can, and links to the setup page. It tells apart a sandbox that reaches SpecDeck without the token from one that does not reach it at all (or whose Claude Code is not logged in).
- A docs page describes the setup for sandboxes and containers, with a working example for Docker Sandboxes and the address per engine; the security pages describe the endpoint as reachable from a sandbox on the same machine and the token in the command's environment.

## Capabilities

### New Capabilities
- `agent-sandbox`: running Claude Code through a host-set start command, such as a sandbox or container script, with the timeline, Claude's browser and the status line still working.

### Modified Capabilities
<!-- None: the new requirements stand next to session-hosting and session-timeline without changing theirs. -->

## Impact

- Code: `crates/core` (`session.rs` launch, `hooks.rs` settings, MCP config and endpoint address, `status.rs` relay, `launcher.rs` start through a command), `crates/session` (`messages.rs`: the start command and address as optional fields of `Command::Start`, so a previous release's app still starts sessions), the host's start screen and settings in `app/` (per-folder start command and address). `crates/protocol` does not change: viewers see the agent as Claude Code.
- Security: the hook endpoint stays on loopback; a sandbox on the same machine whose engine forwards to the host's loopback can reach it. The token stays required and is passed by environment. A spike confirms how `sbx` and Podman traffic arrives before any of this is built.
- Documentation: a README overview line linking to the new page; a new `docs/site` page `guide/sandbox` (what it does, who: hosts; how: the start command and address settings, an example `sbx` script snippet, and the address per engine), listed in `docs/site/sidebar.mjs`; updates to `security/claude-browser.mdx`, `security/data.md` and `security/known-limits.md`. No video: the feature is a setup step with nothing new to watch, the timeline looks as it does without a sandbox.
