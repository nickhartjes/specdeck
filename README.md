# SpecDeck

[![CI](https://github.com/nickhartjes/specdeck/actions/workflows/ci.yml/badge.svg)](https://github.com/nickhartjes/specdeck/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nickhartjes/specdeck)](https://github.com/nickhartjes/specdeck/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

SpecDeck lets one person (the **host**) run a coding agent (Claude Code, OpenCode, omp, Codex, Grok or pi) on their computer while teammates (the **viewers**) follow along from anywhere. Viewers see the terminal and, for Claude Code, a timeline of prompts, answers, tool calls and file diffs. They join with an invite code and are read-only: only the host types into the agent, on the host's own login.

SpecDeck is a native app built on Tauri 2 with a Rust session engine.

Documentation, for using the app and for running your own sharing service: **[nickhartjes.github.io/specdeck](https://nickhartjes.github.io/specdeck/)** (source in [`docs/site`](docs/site)). What the app does is specified in [`openspec/specs`](openspec/specs); the design decisions are in the [archived changes](openspec/changes/archive), starting with [`native-shared-session-mvp`](openspec/changes/archive/2026-10-31-native-shared-session-mvp/design.md), and the checks still to do on real machines are in [`openspec/changes/release-and-verification`](openspec/changes/release-and-verification).

## Videos

Short narrated walkthroughs, with captions; select one to watch it in the documentation.

| [![Getting started](docs/site/public/video/getting-started.jpg)](https://nickhartjes.github.io/specdeck/guide/first-steps/) | [![Following a session](docs/site/public/video/following.jpg)](https://nickhartjes.github.io/specdeck/guide/follow/) | [![Diagnostics](docs/site/public/video/diagnostics.jpg)](https://nickhartjes.github.io/specdeck/guide/troubleshooting/#see-what-the-connection-is-doing) |
|---|---|---|
| **Getting started** (1 min): host a session, share the code, a teammate joins | **Following a session** (1½ min): Overview or Developer, steps, search and files | **Diagnostics** (1½ min): Connection Status and a file for bug reports |

## Install

| Platform | How |
|---|---|
| macOS 11 or later (Apple silicon, Intel) | `brew install --cask nickhartjes/tap/specdeck`, or the `.dmg` from [Releases](https://github.com/nickhartjes/specdeck/releases) |
| Windows | `SpecDeck-x64-setup.exe` from [Releases](https://github.com/nickhartjes/specdeck/releases) |
| Linux | `nix run github:nickhartjes/specdeck` (builds from source, x86_64 and aarch64), or the AppImage or `.deb` from [Releases](https://github.com/nickhartjes/specdeck/releases) |
| Debian, Ubuntu, or Windows through WSL 2 (from source) | `scripts/install-from-source.sh` in a clone of this repository ([details](https://nickhartjes.github.io/specdeck/install/#from-source-debian-ubuntu-and-wsl)) |

With Nix, add it to a system or home configuration through the flake's `packages.<system>.specdeck` or `overlays.default`. The Nix package uses the public sharing service, `wss://specdeck.hartj.es/signal`, like the release builds; override it with `specdeck.override { signallingUrl = "wss://…/signal"; }`, or set `SPECDECK_SIGNALLING_URL` when starting the app.

**Can't install?** Viewers can watch in a browser instead: the sharing service serves a web viewer at its own address (for the built-in service, `https://specdeck.hartj.es/`), and hosts' join links open there. Hosting needs the app.

**Something not working?** **Help › Connection Status** (⇧⌘D on macOS, Ctrl+Shift+D on Windows and Linux) shows what the connection is doing, and **Help › Export Diagnostics…** saves a file to attach to a bug report. The file holds connection events and timings, never terminal output, names or addresses. See [Troubleshooting](https://nickhartjes.github.io/specdeck/guide/troubleshooting/).

Releases built before the signing certificates are in place are unsigned, so the system warns on first launch. On macOS, allow SpecDeck once under System Settings > Privacy & Security (Open Anyway), or run `xattr -dr com.apple.quarantine /Applications/SpecDeck.app`; the Homebrew cask says the same when it installs an unsigned release. On Windows, choose More info > Run anyway in the SmartScreen prompt.

## Repository layout

| Path | Contents |
|---|---|
| [`crates/protocol`](crates/protocol) | Wire protocol shared by hosts and viewers: messages, frames, signing, invite codes (no I/O) |
| [`crates/core`](crates/core) | Session engine: runs the agent in a pty, output log, timeline, WebRTC connections |
| [`crates/server`](crates/server) | The sharing service (`specdeck-server`): matchmaking and TURN credentials; serves the web viewer when built with `web-viewer` |
| [`crates/web`](crates/web) | The viewer logic of `crates/protocol` as WebAssembly, for the web viewer |
| [`app/src-tauri`](app/src-tauri) | Tauri shell: windows, menus, tray, commands |
| [`app/ui`](app/ui) | Web UI (React, TypeScript, Vite), for the app and the web viewer (`web.html`, `src/web`) |
| [`openspec`](openspec) | Specs and change proposals |
| [`deploy/compose`](deploy/compose) | Docker Compose setup for the server, coturn and Caddy on one server |
| [`deploy/k8s`](deploy/k8s) | Kubernetes manifests for the server and coturn (`base`, `local`) ([README](deploy/k8s/README.md)) |
| [`docs/site`](docs/site) | Documentation site (Astro Starlight), published to GitHub Pages by [`docs.yml`](.github/workflows/docs.yml) |
| [`docs/video`](docs/video) | How-to videos, made from the UI's demo mode with HyperFrames and a local voice ([README](docs/video/README.md)) |
| [`docker`](docker) | Server image (`specdeck-server`, published to `ghcr.io/nickhartjes/specdeck-server`) |
| [`scripts`](scripts) | Helper scripts used by the `just` recipes and CI |

## Development

Tools are pinned in [`mise.toml`](mise.toml): Rust (with the `wasm32-unknown-unknown` target), Node, pnpm, just, cmake (needed to build libdatachannel) and the `wasm-bindgen` CLI. With [mise](https://mise.jdx.dev) installed, run `mise install`. On Linux, Tauri also needs the WebKitGTK development packages (see the CI workflow for the list), and libdatachannel's bindings need libclang (`libclang-dev`, already present on GitHub's runners). Builds from source use a local sharing service (`ws://127.0.0.1:18081/signal`) unless `SPECDECK_SIGNALLING_URL` is set when building.

```sh
just install   # install JavaScript dependencies
just dev       # start the desktop app in development mode
just check     # check formatting, lint, type-check and test (Rust and UI)
just capacity  # measure how 20 viewers affect the host's terminal (run alone)
just fmt       # format the Rust code
just package   # build installers for this platform (target/release/bundle)
just docs      # documentation site with live reload
just video     # the how-to videos (docs/video/<name>/renders)
just check-web # web viewer: WebAssembly core and tests, the page (app/ui/dist-web), the service with it
```

To run the web viewer against a local service, run `just web-wasm` and `pnpm --dir app/ui build:web`, then `cargo run -p specdeck-server --features web-viewer` and open `http://127.0.0.1:8080/`.

Run `just` to list all recipes. To try sharing on your own machine (Colima with Kubernetes, `colima start --kubernetes`):

```sh
just local-dev     # the server and coturn on Colima's k3s, plus a host app and a viewer app
just local-up      # only deploy the server and coturn to Colima's k3s
just local-test    # check that two peers connect through the relay only
just local-down    # remove the deployment
```

`just local-dev` opens two windows, **SpecDeck (host)** and **SpecDeck (viewer)**, each with its own settings (`SPECDECK_PROFILE`, debug builds only). Host a session in one and join with its invite code in the other. To look at the UI without a session, run `pnpm --filter @specdeck/ui dev` and open `http://localhost:5173/?demo=start`, `?demo=host` or `?demo=viewer` in a browser.

See [`deploy/k8s/README.md`](deploy/k8s/README.md) for ports, secrets and the differences from a real cluster.

## CI

[`ci.yml`](.github/workflows/ci.yml) runs `just check` and `just package` on macOS, Windows and Ubuntu, and fails when an installer is larger than 25 MB ([`scripts/check-installer-size.sh`](scripts/check-installer-size.sh)). In between it runs `just capacity` on its own and reports the result without failing on it: on Ubuntu runners the 20-viewer measurement still varies too much from run to run.
[`nix.yml`](.github/workflows/nix.yml) builds the Nix flake on every push to main, and on pull requests that change what the Nix build depends on (lockfiles, manifests, `nix/`). Rust caches are saved from main only; pull requests reuse them.
[Renovate](renovate.json) keeps dependencies up to date, and on Monday mornings also refreshes the indirect ones in `Cargo.lock` and `pnpm-lock.yaml`. When it changes the npm dependencies, [`renovate-nix-hash.yml`](.github/workflows/renovate-nix-hash.yml) updates their hash in `nix/package.nix` on its branch. It pushes that commit with `RENOVATE_NIX_HASH_TOKEN`, a fine-grained personal access token for this repository with Contents read and write only, so CI and Nix run for it on the pull request; the token expires and has to be renewed. Without that secret the workflow starts CI and Nix by hand, and the pull request's own checks for the commit need approval.

## Releases

Push a tag that matches the version in `Cargo.toml` and `app/src-tauri/tauri.conf.json`, for example `v1.0.0`. [`release.yml`](.github/workflows/release.yml) builds the installers on each platform, uploads them to a GitHub release under stable names (`SpecDeck-arm64.dmg`, `SpecDeck-x64.dmg`, `SpecDeck-x64-setup.exe`, `SpecDeck-x86_64.AppImage`, `SpecDeck-amd64.deb`), publishes it, and has [`homebrew.yml`](.github/workflows/homebrew.yml) update the cask in `nickhartjes/homebrew-tap` ([`scripts/render-cask.sh`](scripts/render-cask.sh)). The Nix flake follows the repository, so it needs nothing per release. With the update signing key, a final release also carries the signed update files and a `latest.json`, which installed apps from 1.2.0 on check for updates (prereleases are never offered). The secrets for the sharing service URL, signing, update signing and the tap are listed at the top of `release.yml`. Without the signing secrets the installers are unsigned (ad-hoc on macOS) and the cask explains how to open the app; without the update signing key installed apps find no update; without the tap deploy key the cask is not updated.

## License

[MIT](LICENSE)
