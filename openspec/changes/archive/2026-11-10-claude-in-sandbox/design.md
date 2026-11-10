# Design

## Context

A Claude Code session today (`crates/core/src/session.rs`, `launch`):

- starts `claude` directly, with `--settings <file>` and, with Claude's browser, `--mcp-config <file>`. Both files and the status line relay script (`statusline-….sh`) live in the session's data folder on the host (`hooks.rs`, `status.rs`);
- puts the token in `SPECDECK_HOOK_TOKEN`; the settings refer to it as `$SPECDECK_HOOK_TOKEN` (`allowedEnvVars`), so the token is in no file and no argument;
- runs the endpoint on `127.0.0.1` with a random port, for `/hooks`, `/status` and `/mcp`.

The app starts each session as a process of its own and sends it `Command::Start` (`crates/session/src/messages.rs`) with the agent, the folder and the login shell's environment as the app detected it (`Environment`). New fields there take `#[serde(default)]`, as `browser` does, so a session process can be started by a previous release's app.

A Docker Sandbox (`sbx`) runs Claude Code in a microVM. From what `sbx` 0.51 offers:

- `sbx run [flags] -- <claude args>`: arguments after `--` go to `claude`. A project script usually wraps this and passes on its own arguments (`sbx run --name "$name" -- "$@"`).
- `sbx run -e KEY` copies `KEY` from the caller's environment into the sandbox session. Nothing else from the host's environment crosses.
- Workspaces are mounted at the same path as on the host by default. The VM's home (`~/.claude`, transcripts, Claude Code's login) is its own.
- Network is deny-all with allow rules (`sbx policy allow network <host[:port]> [--sandbox NAME]`). `host.docker.internal:<port>` goes through the sandbox's proxy to the host's `localhost:<port>`, which a rule `localhost:<port>` allows (spike, `notes.md`).
- The kit's own `~/.claude/settings.json` sets hooks and a `statusLine` (`sbx-claude-hook`), and logs Claude Code in from `sbx`'s stored Anthropic secret.
- When its session ends, a sandbox stops by itself within a minute; `sbx run --name` re-attaches to it later.

Container engines reach the host differently:

| Engine | Host address | Reaches the host's `127.0.0.1` |
|---|---|---|
| Docker Sandboxes (`sbx`) | `host.docker.internal` | yes, tested; allow rule `localhost:<port>` |
| Docker through Colima (macOS) | `host.docker.internal` | yes, tested; `--network host` does not work (host is the VM) |
| Docker Desktop (macOS, Windows) | `host.docker.internal` | yes, Desktop forwards it |
| Podman machine (macOS) | `host.containers.internal` (`host.docker.internal` works too) | yes, tested; `--network host` does not work |
| Podman on Linux, rootless | `host.containers.internal` | only with loopback mapping (`pasta` `--map-host-loopback`, or `slirp4netns:allow_host_loopback=true`) |
| Docker Engine on Linux | none by default (`--add-host=host.docker.internal:host-gateway`) | no: traffic arrives on the bridge; works with `--network host` and address `127.0.0.1` (not tested) |

## Goals / Non-Goals

**Goals:**
- One generic mechanism, a start command, that works for `sbx` and for container scripts with Docker or Podman that pass arguments on to `claude`.
- The timeline, Claude's browser and the status line work in a sandbox with a one-time change to the project's script: forward one or two environment variables and allow one address.
- The token stays out of arguments and the session folder.
- The endpoint stays on loopback only.

**Non-Goals:**
- SpecDeck creating or managing sandboxes or containers itself, or calling `sbx`, `docker` or `podman`. The project's script owns the sandbox (kit, credentials, policy, cleanup).
- Listening on a bridge or any other non-loopback address so that engines without loopback forwarding work. Those use host networking or loopback mapping, as the docs page says.
- Reading the transcript from inside the sandbox: usage is unavailable there (the existing *Usage cannot be read* behaviour).
- Start commands for agents other than Claude Code. Others have no hooks, so nothing would change for them.
- Windows: start commands run through `/bin/sh` on macOS and Linux first; Windows follows if sandboxes on Windows are used.

## Decisions

### 1. A start command per folder, not sandbox detection

The host sets the command in the start screen for Claude Code; it is stored with the recent folder in the app's settings and sent to the session process as an optional field of `Command::Start`. The session runs it as `/bin/sh -c '<command> "$@"' sh <args…>`, with the login shell's environment the app already detects, so `just`, `mise` and other tools on the login shell's `PATH` resolve as in the host's terminal, and the appended arguments arrive quoted.

`/bin/sh` and not `$SHELL`: `"$@"` and the `sh` placeholder are POSIX; under fish or nushell as login shell they mean something else. Aliases and shell functions do not resolve; the docs say to use a script or a `just` recipe.

*Alternatives:* `$SHELL -lc` (breaks with non-POSIX shells, and runs the login profile, which may print or be slow, on every start); detect `sbx` in a Terminal session (no way to inject settings into a process SpecDeck did not start); call `sbx` from SpecDeck (bypasses the project's kit, credentials and policy, and ties SpecDeck to one engine); a project file such as `.specdeck.json` (lets a cloned repo choose what SpecDeck runs on the host without the host's say, so a host setting is safer).

The "agent not installed" check is skipped for a session with a start command: `claude` may exist only inside the sandbox. A command that fails to start shows the terminal's output, as an agent that exits does today.

### 2. Inline settings and MCP config

With a start command, `--settings` and `--mcp-config` get JSON strings, not file paths (Claude Code accepts both). The JSON is the same as the files today, so it still holds `$SPECDECK_HOOK_TOKEN` and not the token. Without a start command nothing changes: files as today.

The status line relay becomes an inline command in the settings, `sh -c '…' `, posting to the `/status` URL with the token from the environment, instead of a script path in the data folder. It uses `curl`, as the script does today; the `sbx` image has it.

Settings given with `--settings` merge their hooks with the sandbox's own, and `$SPECDECK_HOOK_TOKEN` is expanded inside the sandbox (spike). The `statusLine` does not merge: ours replaces the kit's `sbx-claude-hook claude-status` for SpecDeck sessions. Wrapping the kit's command would tie SpecDeck to one kit, so the docs name the trade-off instead.


### 3. Token and port through the environment

The start command's environment gets `SPECDECK_HOOK_TOKEN` (as today) and `SPECDECK_HOOK_PORT`. The script forwards them: `sbx run -e SPECDECK_HOOK_TOKEN …` (or `docker run -e` / `podman run -e`), and with `sbx` allows the address, for example `sbx policy allow network --sandbox "$name" "localhost:$SPECDECK_HOOK_PORT"`: the proxy matches the host's `localhost`, not `host.docker.internal`. The docs page gives this snippet. A script that does neither still runs Claude; the timeline then explains (decision 6).

*Alternative:* the token inline in the settings argument. Rejected: on macOS every user can read every process's arguments, and the token also guards `/mcp`, which drives Claude's browser.

### 4. The endpoint's address: a setting, loopback only

The settings and MCP config name the endpoint as `http://<address>:<port>`. With a start command, `<address>` is the host address set for the folder next to the start command, `host.docker.internal` when not set; without one it is `127.0.0.1` as today. The address goes to the session process with the start command in `Command::Start`.

The endpoint keeps listening on `127.0.0.1` only, for every engine. An engine that does not forward the host address to the host's loopback cannot reach it; the docs page names those (Docker Engine on Linux, rootless Podman without loopback mapping) and the fix for each (host networking with address `127.0.0.1`, or loopback mapping).

*Alternative:* also listening on the address the sandbox's traffic arrives on, such as the Docker bridge. Rejected: the bridge address differs per engine and network, other containers on that bridge could reach the endpoint, and it adds a listener to keep safe for one setup that has a simpler fix.

The spike (`notes.md`) confirmed it for `sbx` and Docker through Colima: traffic to `host.docker.internal:<port>` arrives on `127.0.0.1`, and a sandbox-scoped rule with the port lets only that port through. Podman on macOS reaches it too, by either name. The spike's findings go in the change's `notes.md`, the docs page's engine table and the security docs.

### 5. Paths

`sbx` mounts the session folder at the same path, and the docs ask container scripts to do the same (`-v "$PWD:$PWD" -w "$PWD"`), so file paths in hook payloads match the host's and the existing rules (`session_folder.rs`, `documents.rs`, `code_files.rs`, `shell_changes.rs`) apply unchanged. Paths in the sandbox's home, such as `~/.claude/plans/…`, show as they are; the parts that read such a file on the host (usage, documents read from disk) already treat an unreadable file as missing. Tests cover a payload whose `transcript_path` and plan path do not exist on the host.

### 6. Explaining missing events

The existing degraded mode (*Degraded mode without structured events*) already marks the timeline unavailable. For a session with a start command, its notice gets a sandbox-specific text and a link to `guide/sandbox`, and two things make it more precise:

- **Which cause.** The endpoint already answers a request without the session's token with 401 (`hooks.rs`). It now also notes that such a request arrived. With a rejected request seen, the notice says the address is reached but the token is not passed, and shows the `-e SPECDECK_HOOK_TOKEN` line. With nothing seen, it says the sandbox does not reach SpecDeck (address not allowed, wrong host address for the engine, or an engine that cannot reach the host's loopback, linking the engine table) or Claude Code in the sandbox is not logged in yet (a new sandbox has its own login).
- **Before the first prompt.** Claude Code runs its status line as soon as it shows its screen, so with a start command the inline relay's first post to `/status` proves the address and token work. The spike saw the first post about 1 s after the first terminal output, 2 s after start. When neither a status post nor a hook event arrives within 15 s of the first terminal output, the notice shows without waiting for a prompt. A first `sbx run` downloads the kit's image before Claude Code starts and can take longer; the notice then shows early and clears with the first status post, so its text says SpecDeck has not been reached *yet*. The existing trigger, a submitted prompt without hook events, stays for setups where the relay cannot run (no `curl`). A status post or hook event clears the notice. `SessionStart` cannot serve as the signal: it supports only command and MCP tool hooks.

A rejected request is only a hint: another local process sending a wrong token would show the token text. It changes only which text is shown, never what the endpoint accepts.

*Alternative:* a reachability check on the start screen. Rejected: SpecDeck would have to run something inside the sandbox, which the project's script owns.

### 7. Ending a session

Ending a session stops the start command's process group, as it stops `claude` today. Whether that stops the sandbox or container is the script's matter. For `sbx` the spike found nothing to do: the hangup ends `sbx run`, the sandbox stops by itself within a minute (not removed, even with `--rm`), and the next `sbx run --name` re-attaches to it, so a new session in the same folder does not clash. The docs snippet needs no `exec` or `trap`.

## Risks / Trade-offs

- [A future `sbx` stops reaching host loopback] → the change relies on what the spike found for 0.51; the docs snippet is tested by hand per `sbx` version.
- [Some engines cannot reach loopback] → documented with their fix; SpecDeck adds no listener for them.
- [The endpoint is reachable from every sandbox on the machine that forwards to loopback, not only this session's] → the token is still required; this is the same trust as another process on the machine, which the security docs already describe.
- [The token in the sandbox is readable by the agent] → already a known limit (*The hook token is visible to the agent's commands*); the item is extended to sandboxes.
- [A start command runs whatever the host typed] → it is the host's own setting, run as the host, like typing it in a Terminal session; it is never taken from the project folder.
- [No `curl` in the sandbox image] → the status line stays empty and the rest works; the `sbx` image has it; the docs name the dependency for other images.
- [Claude Code in a new sandbox is not logged in] → with `sbx`'s stored Anthropic secret it is logged in from the start; without it the first prompt gets no hook events, the notice names it as a cause and the docs say to log in once.
- [Our status line replaces the kit's] → `sbx`'s own status line integration is lost in SpecDeck sessions; the docs say so.
- [The early notice shows during a first image download] → it says SpecDeck is not reached yet and clears with the first status post.
- [The sandbox outlives the session] → `sbx` stops it within a minute; other engines' scripts own their cleanup.
- [`sbx` changes its flags often] → SpecDeck depends only on the start command passing arguments on; the `sbx` part lives in the docs snippet, tested by hand per `sbx` version.
- [Inline JSON hits argument length limits] → the settings are a few KB, far below `ARG_MAX`; a test checks the size.

## Migration Plan

No migration: folders without a start command behave as today, and a session process started by a previous release's app gets no start command. Rollback is clearing the start command.
