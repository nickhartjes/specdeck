# Tasks

## 1. Spike: reaching the host from a sandbox

- [x] 1.1 In a throwaway Docker Sandbox (`sbx run shell` with a deny-all policy), run a listener on the host's `127.0.0.1:<port>` and `curl http://host.docker.internal:<port>` from inside; record whether it arrives on loopback, from which source address, and which `sbx policy allow network` rule (sandbox-scoped, with port) lets it through. Verify by writing the findings in `openspec/changes/claude-in-sandbox/notes.md`; if it does not arrive on loopback, stop and reconsider the change (design decision 4).
- [x] 1.2 In the same sandbox, check that `sbx run -e SPECDECK_HOOK_TOKEN` makes the variable visible to `claude`; that `claude --settings '<json>'` with an HTTP hook to `host.docker.internal` reaches the listener with the expanded token header; that `--settings` merges with the sandbox's own `~/.claude/settings.json` hooks; that `curl` is in the Claude image for the status line relay; that the status line runs as soon as Claude Code shows its screen, before any prompt, and how long that takes (for the grace period in design decision 6); and what a not-yet-logged-in Claude Code shows on first start. Verify by adding the results to `notes.md`.
- [x] 1.3 In the same sandbox, end `sbx run` the way SpecDeck ends a session (stop the process group) and check whether the sandbox keeps running and whether a new `sbx run --name` with the same name starts. Verify by adding the results and the snippet that stops it cleanly (`exec`, `trap`) to `notes.md`.
- [x] 1.4 On a Podman machine on macOS, run the listener on `127.0.0.1:<port>` and `curl http://host.containers.internal:<port>` from a container (Docker through Colima is done; Podman was not installed during the spike); if a Linux machine is at hand, also rootless Podman with and without loopback mapping and Docker Engine with `--network host`. Verify by filling the engine table in `notes.md`.

## 2. Start command and address per folder

- [x] 2.1 Store an optional start command and host address with each recent folder in `app/src-tauri/src/settings.rs`, kept when the folder moves to the top of the list and dropped with it; verify with settings unit tests for save, load, reorder and remove.
- [x] 2.2 Show and edit the start command and the host address (placeholder `host.docker.internal`) for Claude Code on `app/ui/src/screens/StartScreen.tsx`, in a per-folder dialog from each recent folder and from "Start through a command…" for a new one, hidden for other agents and Terminal; verify with `StartScreen.test.tsx` cases for set, remembered from a recent folder, cleared, and hidden for OpenCode.
- [x] 2.3 Add an optional `start` (the command and its host address, `launcher::StartCommand`) to `Command::Start` in `crates/session/src/messages.rs` with `#[serde(default)]`, and send them from `app/src-tauri/src/client.rs`; verify with a `messages.rs` test that a `start` without them, as a previous release's app sends it, parses with neither set.
- [x] 2.4 Start a Claude Code session with a start command as `/bin/sh -c '<command> "$@"' sh <args…>` with the login shell's environment, in `crates/core/src/launcher.rs`, skipping the "not installed" check; verify with a launcher test whose start command is a script that prints its arguments and `$SPECDECK_HOOK_PORT`, run with `SHELL` set to a non-POSIX shell path.
- [x] 2.5 Make ending a session stop the start command's process group; verify with a launcher test whose start command starts a child that sleeps, and that no process of it is left after the session ends.

## 3. Settings, MCP config and status line inline

- [x] 3.1 With a start command, pass `--settings` and `--mcp-config` as JSON strings naming the endpoint by the folder's host address (`host.docker.internal` when not set), and without one keep the files and `127.0.0.1`; verify with `hooks.rs` unit tests that the inline JSON holds `$SPECDECK_HOOK_TOKEN` and never the token, names the set address, and that the size stays far below `ARG_MAX`.
- [x] 3.2 Replace the status line relay script path with an inline command when a start command is set, in `crates/core/src/status.rs`; verify with a test that runs the inline command with a payload and sees it at `/status`.
- [x] 3.3 Give the start command `SPECDECK_HOOK_TOKEN` and `SPECDECK_HOOK_PORT` in its environment, and nothing with the token in its arguments; verify with a `host_session.rs` test whose fake `claude` script checks its arguments for the token and posts a hook with the token from its environment, ending in a timeline entry.
- [x] 3.4 Keep the endpoint on `127.0.0.1` with a start command; verify with a test that the endpoint of a session with a start command is bound to loopback only and that requests without the token still get 401.

## 4. Paths and missing events

- [x] 4.1 Add a `host_session.rs` test with hook payloads whose `transcript_path` and plan file exist only "inside the sandbox" (paths that do not exist on the host) while file changes are in the session folder; verify the timeline shows the file change, usage shows unavailable, and the session keeps running.
- [x] 4.2 Note in the session when the endpoint answers a request with 401, in `crates/core/src/hooks.rs`, without changing what it accepts; verify with a `hooks.rs` test that a request without the token is still refused and is noted, and one with the token is not.
- [x] 4.3 For a session with a start command, show the degraded-mode notice when neither a status post nor a hook event arrives within 15 s of the first terminal output, as well as after a prompt without hook events, and clear it on the first status post or hook event; verify with `host_session.rs` tests for: no requests (notice before a prompt), only rejected requests, a status post in time (no notice), and no status post but hook events after a prompt (no notice).
- [x] 4.4 Give the notice its sandbox texts, token not passed (with the `-e SPECDECK_HOOK_TOKEN` line) or address not reached / not logged in (linking the engine table), with a link to the setup page, in the timeline UI and the web viewer; verify with UI tests for each text and for the notice without a start command.

## 5. Documentation

- [x] 5.1 Write `docs/site/src/content/docs/guide/sandbox.md`: what it does, who can use it (hosts), how (the start command and host address on the start screen, per folder), the `sbx` snippet that forwards `SPECDECK_HOOK_TOKEN` and allows `localhost:$SPECDECK_HOOK_PORT` (not `host.docker.internal`), that `sbx` stops the sandbox itself after the session, that SpecDeck's status line replaces the kit's, a container example mounting the folder at the same path, the engine table from `notes.md` with the address and fix per engine, logging in once inside a new sandbox, what does not work (usage), and the explanation shown when the script is not set up; list it in `docs/site/sidebar.mjs`.
- [x] 5.2 Add the README overview line for running Claude in a sandbox or container, linking to `guide/sandbox`. No video: the timeline looks as it does without a sandbox.
- [x] 5.3 Update `security/claude-browser.mdx` (the endpoint stays on loopback; which sandboxes on the machine can reach it), `security/data.md` (inline settings, no relay script file) and `security/known-limits.md` (the token inside a sandbox) to match the code after group 3.
- [x] 5.4 Run `just docs-build` and verify it passes with the new page, sidebar entry and README link.

## 6. End to end

- [x] 6.1 Host a Claude Code session in a project that uses a Docker Sandbox script, with the start command set and the script forwarding the token and allowing the port; verify by hand that prompts, responses, tool calls, file changes, the status line and Claude's browser (with a screenshot) appear for the host and a viewer, and that ending the session leaves no process on the host and the sandbox stops by itself.
- [x] 6.2 Run the same session with a script that does not forward the token, and again with one that does not allow the port; verify each time that the terminal works and, before any prompt, the timeline shows the matching sandbox explanation with a working link.
- [x] 6.3 Host a session through a Podman container script with the address `host.containers.internal`; verify by hand that the timeline works (done with a stand-in for Claude Code in the container; see `notes.md`).
