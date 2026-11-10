# Spike notes

Run on 2026-11-10 on macOS with `sbx` 0.51.0 (Claude Code 2.1.319 in the `claude` kit), Docker 29.5.2 through Colima, and Podman 6.1.4 (rootless, `podman machine` with libkrun). A listener on the host's `127.0.0.1:47811` logged each request's source address, path and `Authorization` header. The throwaway sandboxes (`specdeck-spike`, `specdeck-spike-claude`, `specdeck-spike-rm`) and their rules were removed afterwards.

## 1.1 Reaching the host from a Docker Sandbox

- `host.docker.internal` resolves in the sandbox; traffic goes through the sandbox's proxy (`gateway.docker.internal:3128`), which treats it as the host's **`localhost:<port>`**.
- Without a rule: `403`, "Approval required for localhost:47811".
- A sandbox-scoped rule **`localhost:47811`** lets it through: `sbx policy allow network --sandbox NAME "localhost:$PORT"`. A rule for `host.docker.internal:47811` does **not** match. Other ports stay blocked (`47812` still asked for approval).
- The request arrives at the listener on **`127.0.0.1`**, from `127.0.0.1`, with the `Authorization` header intact. Design decision 4 holds: the endpoint stays on loopback only.
- Scoped rules are removed with the sandbox (`sbx rm`).

## 1.2 Claude Code in the sandbox

- `sbx exec -e SPECDECK_HOOK_TOKEN` and `sbx run -e SPECDECK_HOOK_TOKEN` (a bare key copies it from the caller's environment) make the token visible to `claude`.
- `claude --settings '<json>'` with HTTP hooks to `http://host.docker.internal:<port>/hooks` works: `UserPromptSubmit`, `Stop` and `SessionEnd` arrived with `Authorization: Bearer <token>`, so `$SPECDECK_HOOK_TOKEN` is expanded inside the sandbox. Settings for four events were 1.1 KB.
- **Hooks merge:** the sandbox's own `~/.claude/settings.json` (the kit's `sbx-claude-hook` hooks plus a marker `UserPromptSubmit` hook added for the test) still ran next to the inline ones.
- **The status line does not merge:** the kit sets `statusLine` to `/usr/local/bin/sbx-claude-hook claude-status`; ours, given by `--settings`, replaces it. Our relay worked, so whatever `sbx` shows from its own status line is lost for SpecDeck sessions.
- `curl` is in the base image (`/usr/bin/curl`), so the inline relay runs.
- The status line runs before any prompt: the first `/status` post came **2.2 s** after `sbx run` started on a running sandbox and **1.8 s** after on a stopped one, about 1 s after the first terminal output. Creating a sandbox the first time also downloads the kit image (214 MB here) before Claude Code starts, printing progress in the terminal.
- **Login:** the sandbox has `~/.claude/.credentials.json` from `sbx`'s stored Anthropic OAuth secret (`sbx secret ls`: anthropic, oauth configured); `claude -p` answered without a login. A not-logged-in Claude Code was not seen; it needs `sbx` without that secret.
- Hook payloads: `cwd` is the host path (the workspace is mounted at the same path); `transcript_path` is in `/home/agent/.claude/projects/…`, which the host cannot read, as design decision 5 expects.

## 1.3 Ending a session

- SpecDeck ends a session with `ChildKiller::kill` (SIGHUP). A SIGHUP to the pty's process group ends `sbx run` at once; nothing of it is left on the host.
- The sandbox keeps running, then **stops by itself** 20–60 s after the session ended. It is not removed.
- `sbx run --rm` does **not** remove the sandbox when the session ends by hangup; it too only stopped.
- `sbx run --name NAME` on an existing sandbox, running or stopped, re-attaches to it: a new session in the same folder does not clash. No `exec` or `trap` is needed in the script.

## 1.4 Engines

| Engine | Host address | Reaches the host's `127.0.0.1` | Notes |
|---|---|---|---|
| Docker Sandboxes (`sbx` 0.51, macOS) | `host.docker.internal` | yes, tested | allow rule `localhost:<port>` |
| Docker through Colima (macOS) | `host.docker.internal` | yes, tested; arrives from `127.0.0.1` | works with or without `--add-host=host.docker.internal:host-gateway`; `--network host` does **not** work (host is the Colima VM) |
| Docker Desktop (macOS, Windows) | `host.docker.internal` | expected, not tested | |
| Podman machine (macOS) | `host.containers.internal` or `host.docker.internal` | yes, tested; arrives from `127.0.0.1` | both names work; `--network host` does **not** work (host is the Podman VM); starting the machine does not touch Colima's `docker` context |
| Podman on Linux, rootless | `host.containers.internal` | not tested | needs loopback mapping |
| Docker Engine on Linux | none by default | not tested | `--network host` with address `127.0.0.1` |

## Consequences for the change

- The `sbx` snippet allows `localhost:$SPECDECK_HOOK_PORT`, not `host.docker.internal:…` (design decision 3, docs task 5.1).
- No cleanup snippet: `sbx` stops the sandbox itself, and a new session re-attaches (design decision 7).
- The early notice must not fire while `sbx` downloads its image on a first run: count the grace period from the end of that, or keep it long enough and word the notice so a later status post clears it (design decision 6).
- Our status line replaces the kit's; record it as a trade-off and in the docs.
- `--network host` is Linux-only advice.
- Podman on macOS also reaches `host.docker.internal`, so the default address works there too.

## 6.3 Hand test with Podman

A container script (`podman run --rm -it -e SPECDECK_HOOK_TOKEN -v "$PWD:$PWD" -w "$PWD" …`) started a stand-in for Claude Code in a `curlimages/curl` container: it read SpecDeck's inline settings and posted a status line, `UserPromptSubmit`, a `Write` and `Stop`, as Claude Code does. Real Claude Code was not run in Podman: it needs its own login there, and npm was unreachable to install it. With the host address `host.containers.internal` set in the dialog, the host saw the status line, the prompt, the response and `notes.md` written in the container as a document; ending the session stopped the container.
