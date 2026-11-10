---
title: Known limits
description: Security limits of the current release, what they mean, and what you can do.
---

This list holds everything we know of in the current release that is weaker than you might expect. Each item says what it means for you and, where there is one, what you can do about it. Items leave the list when a release fixes them. Found something that is not here? See [SECURITY.md](https://github.com/nickhartjes/specdeck/blob/main/SECURITY.md).

## Invite codes

### The sharing service could guess a code offline

The room id the sharing service sees is made from the invite code, so whoever runs the service could test codes against it on their own computers. With enough computing power they could find a code, and then intercept viewers who join afterwards with it. Viewers already connected are not affected. A fix (a PAKE) is planned, without a date.

**What you can do:** run your own sharing service; make a new code once everyone has joined; end sessions when you are done. See [The weakness: guessing a code offline](/specdeck/security/sharing-service/#the-weakness-guessing-a-code-offline).

### Anyone with the code joins

There is no step where the host lets each viewer in. Whoever has the code joins, up to the viewer limit, and sees the whole session so far before the host can remove them.

**What you can do:** share the code only with the people you mean, in a private channel; make a new code once everyone has joined; lower the viewer limit to the number of people you expect.

### Codes do not expire

A code works until the host makes a new one or ends the session, however long that is.

**What you can do:** use **New code** once everyone has joined, and end sessions you no longer need.

## The sharing service

### The service sees IP addresses, including local ones

The server sees the IP addresses of everyone in a session. The connection setup it passes on also lists the addresses each app can be reached at, which from the app include addresses in your local network, such as `192.168.1.20`.

**What you can do:** run your own sharing service, so only your team sees them.

### The relay logs addresses

The relay logs each connection it relays, with the IP addresses of both ends. How long the built-in service keeps those logs is not stated yet.

**What you can do:** run your own sharing service and choose your own log retention.

### The server logs when sessions happen

The server's log records when each room opens and closes, how many viewers join it, and joins it turned away. It names rooms and viewers only by numbers it counts from 1, never by room id, code or address, but it does show when sessions take place and how big they are. How long the built-in service keeps this log is not stated yet.

**What you can do:** run your own sharing service, and set `SPECDECK_LOG_LEVEL=warn` if you want it to log only limits and errors.

### Relay credentials last 12 hours and are not tied to a session

The server gives relay credentials to anyone who hosts or joins a session, and anyone can host one. The credentials work for 12 hours, also after the session ends, for relaying to any public address, within the relay's quotas.

**What you can do:** on your own service, shorten `SPECDECK_TURN_TTL_SECONDS` (see [Configuration](/specdeck/self-host/configuration/)).

### The web viewer trusts whoever runs the service

A browser loads the web viewer's code from the sharing service on every visit. Whoever runs the service could change that code and read the sessions of browser viewers. On the built-in service, that is its maintainer and Cloudflare, which delivers the page.

**What you can do:** watch in the app instead; or run your own service from the signed image or your own build. See [The web viewer's code](/specdeck/self-host/overview/#the-web-viewers-code).

### The Compose relay keeps its admin console

The Docker Compose setup does not turn off coturn's admin console. It listens on the server's own loopback address only, so only programs on the server itself can reach it. The Kubernetes setup turns it off.

**What you can do:** add `--no-cli` to the coturn command in your `compose.yaml`.

## The app

### Links in documents open inside the app window

Clicking a link in a document, a plan or one of Claude's answers loads that page in the SpecDeck window itself, without an address bar, instead of in your browser. The page cannot use SpecDeck's functions, but it could pretend to be part of SpecDeck, and the session view is gone from that window. In the web viewer, the tab leaves the session.

**What you can do:** copy links into your browser instead of clicking them. To get back, close the window: a session you host keeps running and opens again from **Running sessions** on the start screen; as a viewer, join again with the code. Shared links are not affected: they always open in your browser.

### Settings and logs use default permissions

`settings.json`, the log files and each session's folder are written with your account's default permissions, not made private to you. They hold no secrets, but they do hold your recent folder paths and your name. The session's temporary files themselves, and the log folder, are private to your account.

**What you can do:** on a shared Linux computer, make your home folder private (`chmod 700 ~`).

### Leftover session files

When a session ends normally, SpecDeck deletes its temporary files, which hold the session's terminal output and copies of changed files. If the session's process is killed instead, for example by a crash or a power cut, they stay in the cache folder, readable only by your account, until you delete them.

**What you can do:** when no session runs, delete the `sessions` folder in SpecDeck's cache folder (`~/Library/Caches/nl.nickhartjes.specdeck/sessions` on macOS).

### The hook token is visible to the agent's commands

Claude Code gets the session's token in its environment, and every command it runs inherits it. A script in your repository that the agent runs could read it, and then add made-up steps to the timeline your viewers see, or use Claude's browser.

**What you can do:** nothing specific: those commands already run as you, with your files. Review what the agent runs, as you would anyway.

The same holds [in a sandbox](/specdeck/guide/sandbox/): the start command passes the token in, so commands there can read it too. A sandbox keeps the agent away from your files, not from the session's timeline or Claude's browser.

### Notifications can show commands, paths and addresses

When the agent waits for you, the notification can show the command it wants to run, a file path, an address, or its question. Notifications also show viewers' names as they chose them, and a viewer could choose a misleading one.

**What you can do:** turn off **Show notifications**, or hide notification previews on your lock screen in your system's settings.

### Shared links may point into your local network

A shared link must be `https`, or `http` for an address that is not the sharer's own computer. A link to a private address, such as a router at `http://192.168.1.1`, is allowed, and would open on the viewer's own network.

**What you can do:** check the site on the link's card before you open it. Links open only when you choose to.

### The diagnostics export trusts the window for its path

Export Diagnostics… writes the file wherever the app's window says, normally the place you chose in the save dialog. Only a window that was itself compromised could misuse this.

### An unencrypted sharing service is allowed

You can enter a `ws://` sharing service in Settings. SpecDeck marks it as not encrypted, but uses it. The session itself is still encrypted, but room ids and the connection setup then cross the network readable, so anyone on the way could attempt the offline guessing above.

**What you can do:** use only `wss://` services.

## Releases

### Installers are not signed by Apple or Microsoft

The macOS app has no Apple Developer ID and is not notarized, and the Windows installer is not signed, so your system warns on first launch. The AppImage and `.deb` are not signed either. Updates are signed, with SpecDeck's own key.

**What you can do:** install with Homebrew, which checks the download against the hash in its cask, or with Nix, or build from source.

### No checksums or build provenance

Releases have no published checksums, software bill of materials or build attestations, so you cannot check a download against the build that made it. The sharing service image is the exception: it is signed by the release workflow.

**What you can do:** as above.

### The release workflow trusts tagged actions

The build uses third-party GitHub Actions by version tag, not by fixed commit. A compromised action could change what a release contains.

**What you can do:** build from source or with Nix.

## By design

These are not bugs, but they matter for what you show:

- **Everything in the terminal is shared**, including secrets the agent or a command prints. See [What viewers can see](/specdeck/security/data/#what-viewers-can-see).
- **Viewers who join late see the whole session so far.**
- **Screenshots can show more than the terminal**, such as a page you are logged in to. Turn off **Show screenshots in the timeline** to keep them out.
- **Viewers keep what they saw.** They can copy text, save shared files and take screenshots of their own.
