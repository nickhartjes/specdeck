# Design

## Context

See proposal.md for motivation, and the `session-hosting` delta for the behavior.

What is there today:

- nma-platform's `tools/sandbox/sandbox.sh run [claude args]` (behind `just sandbox`) creates the checkout's sandbox if needed and runs `exec sbx run --name <name> -- <args>`, attached to the terminal it was started in. It works out the repository from its own location, mounts the checkout (and for a worktree the main `.git`), keeps `tools/sandbox`, the justfiles, `mise.toml`, `.claude/settings*.json`, `.mcp.json`, `.git/config` and the git hooks read-only, and runs sbx's deny-all network policy with the kit's allowlist. It is macOS only (Windows untested). Its preconditions are mise, jq, the AWS CLI with the `entrnce` profile, `just sandbox-setup` and a Claude login through `just sandbox-login`; `just sandbox-doctor` checks them.
- `launcher::launch` starts `agent::program(agent, env)` in a pty in the folder, with the adapter's `extra_args` and `extra_env`.
- Before this change, `HostSession::launch` (`crates/core/src/session.rs`) decided on `config.agent.has_timeline()` whether to start the hook endpoint, write the `--settings`, status-line relay and MCP files into `data_dir`, read plans and usage, and allow whole files. `Activity::new(agent)` started in `Starting` with a timeline and read Program Status Protocol reports without one.
- `HostMessage::Welcome` carries `agent` and `timeline_supported` separately; `crates/session/src/runtime.rs` and `crates/core/src/sharing.rs` fill `timeline_supported` from `agent.has_timeline()`. Viewers already show a session without a timeline as terminal only.
- The start screen starts a session as soon as the host picks a folder (a recent folder or the folder picker), with the agent chosen above.

## Goals / Non-Goals

**Goals:**

- One way to start Claude in an nma-platform sandbox: the checkout's own `sandbox.sh`, so SpecDeck never drifts from nma-platform's setup.
- No change to the session protocol, and no change for anyone who leaves the setting off.

**Non-Goals:**

- A timeline for sandboxed sessions. That needs the hooks, settings, MCP and status-line files to reach Claude in the VM and the hook endpoint to accept requests from it, which means a host port in the sandbox's deny-all policy: a change to nma-platform's security model, left for its own change.
- Running other repositories, or other agents, in a sandbox.
- Setting up or checking the sandbox (`sandbox.sh` already explains what is missing, and `just sandbox-doctor` checks it).

## Decisions

### 1. A folder has a sandbox when it is a checkout root with nma-platform's launcher

`sandbox::launcher(folder)` (`crates/core/src/sandbox.rs`) returns `folder/tools/sandbox/sandbox.sh` on macOS when `folder/.git` exists (a directory in a main checkout, a file in a worktree), `tools/sandbox/sandbox.sh` is an executable file and `tools/sandbox/sbxenv.yaml` exists. File checks only, no git or sbx call, so the start screen can mark recent folders cheaply.

- **Alternative: also accept subfolders of a checkout.** `sandbox.sh` always mounts and starts in the checkout root, so the session would silently run somewhere other than the folder the host chose.
- **Alternative: check the `origin` remote for `nma-platform`.** Forks, mirrors and renamed remotes would lose the sandbox for no gain; the launcher files are what matters.

### 2. The sandbox is a session option, not a new agent

`HostConfig` and `Command::Start` get `sandbox: bool` (`#[serde(default)]` on the command, so a session process and app from different releases agree). Which agents can be sandboxed is decided in one place, `sandbox::runs(agent)` (Claude Code, on macOS), and `sandbox::program(agent, folder)` gives the program to run: the folder's launcher (decision 1, checked again in the session process) with `run`. The session starts sandboxed when `sandbox` is set and that program exists; it goes to the launcher as `LaunchOptions.program`, a general override of the agent's own program, and `extra_args` and `extra_env` stay empty.

`HostSession::launch` works out the session's `has_timeline` once (`agent.has_timeline() && !sandboxed`) and uses it for hooks, plans, usage and `Activity::with_timeline`. `SessionView::has_timeline()` and `HostSession::has_timeline()` / `sandboxed()` expose it, and the runtime and sharing fill `timeline_supported` and `whole_files_possible` from the session instead of the agent. `Activity::new(agent)`, which takes the agent's timeline, is left for tests only.

- **Alternative: a new `AgentId::ClaudeCodeSandbox`.** Viewers would see the sandbox in the agent name, but it needs protocol version 8 (postcard encodes the variant index in `Welcome`), so every older viewer is refused, and it puts a folder-dependent choice into the agent picker.
- **Alternative: start it as a Terminal session.** Viewers would be told it is a plain shell, which is the situation this change ends.

### 3. A setting, not a question per session

The start screen starts on folder pick, so the choice is a setting, `claude_sandbox` in `settings.json` (default false), shown under the agent picker when the chosen agent is `sandboxable` (a per-agent field of the profile, from `sandbox::runs`): "Run Claude Code in a Docker Sandbox when the folder has one". The profile tells the UI, per recent folder, whether it has a launcher; with the setting on and Claude Code chosen, those folders show a "Docker Sandbox" mark. The app sends `sandbox` with every Claude Code start; the session process decides with decision 1.

### 3a. The host sees the sandbox in the session window

The session process's `Event::Snapshot` gets `sandboxed: bool` (`#[serde(default)]`, false from a session process of an earlier release), set from whether the session actually started sandboxed, not from the setting. The host's toolbar shows a "Docker Sandbox" badge after the agent's name when it is true. Viewers are not told (decision 2): that would need a protocol change.

### 4. Status without hooks

Without hooks, `Activity` reads Program Status Protocol reports as for other agents. Whether Claude Code sends them in the sandbox is checked in task 3.2; if it does not, the status stays unknown, as in a Terminal session, and the docs say so.

Outcome (checked with a real nma-platform sandbox, 2026-11-09): the status changes between working and waiting in a sandboxed session.

### 5. Closing the session

Closing a session kills `sbx run` like any agent. The sandbox VM is sbx's to stop and keeps the checkout's state, so the next session in that checkout attaches to the same sandbox; `just sandbox-rm` removes it. SpecDeck does not stop or remove sandboxes.

Confirmed with a real nma-platform sandbox (2026-11-09): a second session in the same checkout reuses its sandbox.

## Risks / Trade-offs

- [The first session in a checkout takes minutes while sbx builds the kit] → The terminal shows `sandbox.sh`'s progress; the docs mention it.
- [`sandbox.sh` is code from the chosen folder] → It is the host's own checkout, the same script `just sandbox` runs, and it is read-only inside every sandbox, so a sandboxed agent cannot change what the host runs next.
- [nma-platform renames or changes `sandbox.sh run`] → Decision 1 stops offering the sandbox when the files move; a changed `run` shows its error in the terminal.

## Open Questions

None. Decision 4's question was answered by task 3.2: the status follows Claude in the sandbox.
