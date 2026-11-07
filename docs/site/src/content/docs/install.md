---
title: Install
description: Install SpecDeck on macOS, Windows or Linux.
---

The host needs SpecDeck, and so does every viewer who wants the app. Download it from the [latest release](https://github.com/nickhartjes/specdeck/releases/latest), or use a package manager.

:::tip[Can't install? Watch in your browser]
Viewers who cannot install apps, for example on a company laptop, can [watch in a browser](/specdeck/guide/join/#watch-in-a-browser) instead: open the host's link, or the sharing service's address, in current Chrome, Edge, Firefox or Safari. Hosting needs the app.
:::

| Platform | How |
|---|---|
| macOS 11 or later (Apple silicon) | `brew install --cask nickhartjes/tap/specdeck`, or `SpecDeck-arm64.dmg` |
| macOS 11 or later (Intel) | `brew install --cask nickhartjes/tap/specdeck`, or `SpecDeck-x64.dmg` |
| Windows | `SpecDeck-x64-setup.exe` |
| Linux | `SpecDeck-x86_64.AppImage`, `SpecDeck-amd64.deb`, [Nix](#nix), or [from source](#from-source-debian-ubuntu-and-wsl) |

The release files keep the same names from version to version, so a link such as `https://github.com/nickhartjes/specdeck/releases/latest/download/SpecDeck-arm64.dmg` always gets the newest one.

## The first launch

Releases are not signed yet, so your system warns the first time you open SpecDeck:

- **macOS**: allow SpecDeck once under **System Settings › Privacy & Security** (**Open Anyway**), or run `xattr -dr com.apple.quarantine /Applications/SpecDeck.app`. The Homebrew cask says the same when it installs.
- **Windows**: choose **More info › Run anyway** in the SmartScreen prompt.

On macOS, the first time you host or join, macOS also asks whether SpecDeck may *find and connect to devices on your local network*. Select **Allow**: SpecDeck connects directly to teammates on the same network when it can.

Installing also registers `specdeck://` links, so clicking a join link opens SpecDeck.

## Nix

The flake builds SpecDeck from source, for x86_64 and aarch64 Linux:

```sh
nix run github:nickhartjes/specdeck
```

To add it to a NixOS or Home Manager configuration, use the flake's `packages.<system>.specdeck` or `overlays.default`.

The Nix package uses the public sharing service, `wss://specdeck.hartj.es/signal`, like the release builds. To use your team's own service, override it:

```nix
specdeck.override { signallingUrl = "wss://specdeck.example.com/signal"; }
```

Or set the service in [Settings](/specdeck/guide/settings/), which works for every install.

## From source (Debian, Ubuntu and WSL)

When the release files cannot be used, for example on a work laptop that blocks installers, build SpecDeck from source with one script. On Windows this works in Ubuntu under [WSL 2](https://learn.microsoft.com/windows/wsl/install): the SpecDeck window opens on the Windows desktop. Check first that your organisation allows running software in WSL.

```sh
git clone https://github.com/nickhartjes/specdeck.git ~/specdeck
~/specdeck/scripts/install-from-source.sh
```

The script asks for your sudo password once to install the system libraries, installs Rust, Node and the other build tools for your user only with [mise](https://mise.jdx.dev), and builds the app with the public sharing service, like the release builds. The first build takes several minutes.

Then start SpecDeck with `specdeck`, or from the application menu. `specdeck` keeps the terminal attached, with the app's output there, until you close the window or press Ctrl+C; `specdeck --background` frees the terminal and logs to `~/.cache/specdeck.log`. In WSL it also appears in the Windows Start menu under **Ubuntu**, and `wsl -- bash -lc specdeck` starts it from PowerShell.

- **Coding agents run inside Linux**: install the agent (Claude Code, Codex, …) in Ubuntu as well; the Windows one is not used.
- **Update**: run `git -C ~/specdeck pull` and the script again. A build from source does not update itself.
- **Your team's own sharing service**: set it in [Settings](/specdeck/guide/settings/), or build it in with `SPECDECK_SIGNALLING_URL=wss://specdeck.example.com/signal ~/specdeck/scripts/install-from-source.sh`.

## Update

From version 1.2.0, SpecDeck updates itself when it was installed from the `.dmg`, with Homebrew, with the Windows installer, or as the AppImage. It checks for a new release when it starts and every 6 hours, downloads it in the background, and then shows **SpecDeck 1.2.1 is ready** (with the new version) and **Restart to update**. Nothing is installed until you choose that, and SpecDeck never restarts by itself. Sessions you host [keep running through the restart](/specdeck/guide/host/#when-specdeck-restarts-or-stops), with your viewers connected, and open again in their windows afterwards; versions before 1.7.0 end them. While you watch a session, SpecDeck asks first, because restarting leaves it; you can join again with the invite code.

- **Check now**: **SpecDeck › Check for Updates…** on macOS, **Help › Check for Updates…** on Windows and Linux.
- **Turn the background checks off**: **Check for updates automatically** in [Settings](/specdeck/guide/settings/). Check for Updates… still works.
- **Versions before 1.2.0** cannot update themselves: install 1.2.0 by hand once, as below.
- **Only final releases** are offered, never test releases, and every update is checked against SpecDeck's signature before it is installed. A check only asks GitHub for the newest release; it sends nothing about you or your sessions.
- **macOS asks again for Local Network access** after each update, because releases are not signed with an Apple Developer ID yet. Select **Allow** again.

Where SpecDeck does not update itself:

- **`.deb`**: download the new release and install it over the old one. Your settings stay.
- **Nix**: update the flake input.
- **From source**: pull and run `scripts/install-from-source.sh` again.
- **Homebrew** leaves SpecDeck to update itself; `brew upgrade --greedy --cask specdeck` updates it through Homebrew instead.

When the host's and a viewer's versions cannot work together, SpecDeck says **Update SpecDeck to join** or **The host needs to update**.
