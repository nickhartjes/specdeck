---
title: Claude in a sandbox
description: Host Claude Code that runs in a Docker Sandbox or a container, with the timeline still working.
---

Some projects run Claude Code in a sandbox: a script starts it in a [Docker Sandbox](https://docs.docker.com/ai/sandboxes/) (`sbx`), or in a container with Docker or Podman, so it can't touch the rest of your computer. You can host such a project as a Claude Code session, with the timeline, Claude's browser and the status line. You set this up once per folder; viewers see an ordinary Claude Code session.

This is for hosts on macOS and Linux. Windows doesn't support start commands yet.

## Set a start command

1. On the start screen, choose **Claude Code**.
2. Next to a recent folder, select the **container** button, or select **Start through a command…** and pick a folder.
3. Type the command that starts Claude Code in your sandbox, such as `just sandbox` or `./sandbox.sh`.
4. Select **Save and start**, or **Save** to start later.

The folder shows its command under its name. SpecDeck runs the command in the folder instead of `claude`, through `/bin/sh`, with the `PATH` of your login shell, so tools such as `just` and `mise` are found. Shell aliases and functions are not: use a script or a `just` recipe. To go back to plain `claude`, open the dialog and select **Clear**.

SpecDeck adds its own arguments at the end of the command, so the script must pass its arguments on to `claude`, such as with `claude "$@"`.

## What the script must do

SpecDeck gives the command two environment variables:

| Variable | What it is | What the script does with it |
|---|---|---|
| `SPECDECK_HOOK_TOKEN` | The session's secret token | Passes it into the sandbox, for Claude Code to send with its events |
| `SPECDECK_HOOK_PORT` | The port SpecDeck listens on, on your computer | Allows the sandbox to reach it, where its network is closed by default |

Inside the sandbox, Claude Code reaches SpecDeck at `host.docker.internal` by default. Change it under **Host address** in the dialog, such as `host.containers.internal` for Podman.

### Docker Sandboxes

A script for `sbx`, tested with `sbx` 0.51 on macOS:

```sh
#!/bin/sh
# Starts Claude Code in a Docker Sandbox for this folder. SpecDeck adds its arguments.
name="claude-$(basename "$PWD")"
sbx ls | grep -q "^$name " || sbx create --name "$name" claude "$PWD" || exit 1
if [ -n "$SPECDECK_HOOK_PORT" ]; then
  sbx policy allow network --sandbox "$name" "localhost:$SPECDECK_HOOK_PORT" >/dev/null || exit 1
fi
sbx run --name "$name" -e SPECDECK_HOOK_TOKEN -- "$@"
```

- **The rule names `localhost`, not `host.docker.internal`.** The sandbox's proxy sends `host.docker.internal` to your computer's `localhost`, and only a rule for that matches. The rule is for this sandbox only, and for this one port.
- **Each session has a new port**, so rules for older ports stay until you remove the sandbox (`sbx rm`), or one by one with `sbx policy rm network --sandbox NAME --resource localhost:PORT`. A rule for an old port lets the sandbox reach whatever uses that port later.
- **Ending the session** ends `sbx run`. The sandbox stops by itself within a minute, and the next session in the folder starts it again.
- **The sandbox logs Claude Code in** with the Anthropic login `sbx` keeps (`sbx secret ls`). Without it, log in once inside the sandbox.

### Docker and Podman containers

A container works the same way when it mounts the folder at the same path, so the files Claude changes show in the timeline:

```sh
docker run --rm -it -e SPECDECK_HOOK_TOKEN -v "$PWD:$PWD" -w "$PWD" my-claude-image claude "$@"
```

Whether the container reaches SpecDeck depends on the engine, because SpecDeck only listens on your own computer (`127.0.0.1`):

| Engine | Host address | Works |
|---|---|---|
| Docker Sandboxes (`sbx`) | `host.docker.internal` | Yes, tested |
| Docker with Colima (macOS) | `host.docker.internal` | Yes, tested |
| Docker Desktop (macOS, Windows) | `host.docker.internal` | Expected, not tested |
| Podman (macOS) | `host.containers.internal` | Yes, tested; `host.docker.internal` works too |
| Podman on Linux, rootless | `host.containers.internal` | Only with loopback mapping, such as `--network pasta:--map-host-loopback` |
| Docker Engine on Linux | `127.0.0.1` | Only with `--network host` |

`--network host` only helps on Linux: on macOS, the container's host is the virtual machine Docker or Podman runs in.

## When the timeline doesn't start

If nothing from the sandbox reaches SpecDeck within about 15 seconds of Claude Code starting, the timeline says why, with a link to this page. The terminal keeps working either way.

- **"…reaches SpecDeck without its token"**: the sandbox reaches SpecDeck, but the script doesn't pass `SPECDECK_HOOK_TOKEN` into it. Add `-e SPECDECK_HOOK_TOKEN` to the command that starts the sandbox.
- **"SpecDeck has not been reached from the sandbox yet"**: the sandbox's network doesn't allow SpecDeck's port, the host address doesn't suit the engine, or Claude Code inside isn't logged in. The first time a sandbox starts, it may still be downloading its image: the message goes away when Claude Code reaches SpecDeck.

After fixing the script, start a new session.

## What works differently

- **Usage** isn't shown: Claude Code keeps its transcript inside the sandbox, where SpecDeck can't read it.
- **Your own status line** doesn't run inside the sandbox, and neither does the one the `sbx` kit sets up: SpecDeck's takes its place, to relay Claude Code's status to viewers.
