---
title: On your computer
description: What SpecDeck keeps on your computer, what it sends, and how releases are signed.
---

## What SpecDeck stores

SpecDeck has no account and keeps no session history. On your computer it stores:

| What | Where (macOS) | Holds |
|---|---|---|
| `settings.json` | `~/Library/Application Support/nl.nickhartjes.specdeck/` | Your name, up to 5 recent session folders (full paths), the last agent you chose, your sharing service, and your choices for layout, appearance, zoom, notifications, updates, logging, Claude's browser and screenshots. No passwords, tokens or keys |
| Window positions | the same folder | Size and place of each window |
| Diagnostic log | `~/Library/Logs/nl.nickhartjes.specdeck/` | Connection events and timings, see below |
| A session's temporary files | `~/Library/Caches/nl.nickhartjes.specdeck/sessions/` | Only while a session you host runs, see [Your data](/specdeck/security/data/#what-the-hosts-computer-keeps-while-a-session-runs) |
| Running sessions | `~/Library/Application Support/nl.nickhartjes.specdeck/run/` | For each session you host: its number, agent, the name of its folder, and the local socket the app uses to reach it. Only your account can use it |

On Linux the same files are under `~/.config/`, `~/.local/share/` and `~/.cache/` in a folder `nl.nickhartjes.specdeck`, and on Windows under `%APPDATA%` and `%LOCALAPPDATA%`.

`settings.json` and the log files are written with your account's default permissions, not made private to you. On macOS your Library folder keeps other accounts out; on Linux and Windows that depends on your home folder. See [Known limits](/specdeck/security/known-limits/#settings-and-logs-use-default-permissions).

In the **web viewer**, the browser keeps only your name and your choices for layout, appearance and notifications, in its local storage for that site. While the tab is open, it also keeps that page's diagnostic log in the tab's own session storage; closing the tab removes it, and the page never uploads it.

## Logs and diagnostics

The diagnostic log records when connections start and end and why, sizes and timings, and whether a connection is direct or relayed. It never holds terminal output, the timeline, what the agent was asked or did, invite codes, room ids, names, file paths or network addresses. Only your account can open the log folder, files older than 7 days are removed, and the whole log stays under 50 MB.

It also records crashes, the agent starting and stopping, and failed update checks, each with a fixed reason and never the error's own text, which could hold a path or something you typed. See [What else the log records](/specdeck/guide/troubleshooting/#what-else-the-log-records).

**Export Diagnostics…** saves the log and the current connection state to a file where you choose, after saying what it holds. SpecDeck never uploads it. See [The log](/specdeck/guide/troubleshooting/#the-log).

## What SpecDeck sends, and to whom

SpecDeck has no telemetry, analytics or crash reporting. It connects to:

- **the sharing service and its relay**, while you host or watch a session, and to check a sharing service you enter in Settings;
- **the people in your session**, over the session's encrypted connections;
- **GitHub**, to check for updates, unless you turn that off.

It also opens web pages in your browser when you ask it to, such as a shared link, a link in a document or an agent's install page. Its own windows show only SpecDeck and never load another page.

## Notifications

Notifications show what SpecDeck writes into them, so they can appear on your lock screen or in the notification list:

- **While you host:** viewers' names as they chose them, when they join, leave or suggest a prompt, and when the agent waits for you, what it wants to do, such as *Claude wants to use Bash* with the command, a file path, an address it wants to open, or the question it asks (up to 200 characters).
- **While you watch:** the names of plans and documents that changed, and that the agent finished.

No terminal output is shown. If commands or paths should not appear on your screen, turn off **Show notifications** in [Settings](/specdeck/guide/settings/), or hide notification previews on the lock screen in your system's settings.

## Updates

The `.dmg`, Homebrew, the Windows installer and the AppImage [update themselves](/specdeck/install/#update):

- **Every update is signed.** SpecDeck installs an update only after checking its signature against the update key built into the app. A download that fails the check is thrown away.
- **A check sends nothing about you.** It asks GitHub for the newest release, when the app starts and every 6 hours, with no identifier, name, session or usage data. GitHub sees your IP address, as with any download.
- **Nothing is installed until you choose Restart to update.**
- **You can turn the checks off** with **Check for updates automatically** in [Settings](/specdeck/guide/settings/).

The `.deb`, Nix and builds from source never check; their package manager or you update them.

## How releases are signed

| Release | Signed |
|---|---|
| macOS app | Not yet with an Apple Developer ID, and not notarized, so macOS warns on first launch (see [The first launch](/specdeck/install/#the-first-launch)). Homebrew checks the download against the hash in the cask |
| Windows installer | Not yet, so SmartScreen warns |
| AppImage and `.deb` | No |
| Updates | Yes, with SpecDeck's update key, checked by the app |
| Sharing service image | Yes, a keyless Sigstore signature from the release workflow, in the public transparency log. See [The web viewer's code](/specdeck/self-host/overview/#the-web-viewers-code) for the `cosign verify` command |

Releases have no published checksums, software bill of materials or build provenance yet. To be sure of what you run, use Homebrew or Nix, or [build from source](/specdeck/install/#from-source-debian-ubuntu-and-wsl). See [Known limits](/specdeck/security/known-limits/#releases).
