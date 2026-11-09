# Proposal

## Why

nma-platform developers run Claude Code with permissions bypassed in a Docker Sandbox (`sbx`), started with `just sandbox` from their checkout (`tools/sandbox/sandbox.sh` in nma-platform). SpecDeck can only start `claude` straight on the host, so hosting a session means giving up the sandbox, or starting it by hand in a Terminal session, where viewers are told it is a plain shell. SpecDeck should start Claude through the checkout's own sandbox setup, without a copy of it.

## What Changes

- A new setting, **Run Claude Code in a Docker Sandbox when the folder has one** (macOS only, off by default). With it on, a Claude Code session in the root of an nma-platform checkout or worktree runs that checkout's `tools/sandbox/sandbox.sh run` instead of `claude`. In any other folder, Claude Code starts as before.
- SpecDeck reuses the sandbox as nma-platform sets it up. The host must have nma-platform cloned and have run `just sandbox-setup` (and `just sandbox-login`); SpecDeck does not install sbx, store credentials or create kits. The first session in a checkout creates its sandbox, which the terminal shows.
- A sandboxed session shares the terminal only: no timeline, diffs, plans, browser, usage or hook-based status, because SpecDeck's hooks listen on the host's `127.0.0.1` and its settings and MCP files are in a host temp folder, neither of which the sandbox can reach. Viewers see Claude Code with no timeline, as the session tells them today through `timeline_supported`.
- Recent folders that will start sandboxed are marked as such on the start screen, and the host's session window shows a "Docker Sandbox" badge for a sandboxed session.
- No protocol change: the agent stays `claude-code` on the wire and `Welcome.timeline_supported` is false.
- Not in this change: a hook bridge into the sandbox (timeline for sandboxed sessions), sandboxes for other repositories or agents, Windows and Linux.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-hosting`: adds "Claude Code in a Docker Sandbox".

## Impact

- `crates/core/src/sandbox.rs` (new): whether a folder has an nma-platform sandbox, and the program that starts it.
- `crates/core/src/session.rs`, `launcher.rs`, `activity.rs`: `HostConfig.sandbox`; a sandboxed session starts the sandbox program and has no timeline, whatever its agent.
- `crates/session/src/messages.rs`, `runtime.rs`: `Command::Start.sandbox` (`#[serde(default)]`, so a session process and app from different releases still agree); `timeline_supported` and `whole_files_possible` follow the session, not the agent.
- `crates/core/src/diag.rs`: `agent.started` records whether the session is sandboxed.
- `app/src-tauri/src/settings.rs`, `lib.rs`: the setting, and which recent folders have a sandbox.
- `app/ui/src`: the setting on the start screen and the mark on recent folders.
- `docs/site/src/content/docs/guide/host.md`: the setting, its preconditions and what viewers get.
