---
title: Join a session
description: Join with an invite code or a link, and choose how you follow sessions.
---

## Join

- **With a link:** click the link the host shared.
  - A `specdeck://join/…` link opens SpecDeck, which joins.
  - A web link (`https://…/join#…`) opens in your browser with two choices: **Open in the SpecDeck app**, or **Watch in this browser** (see [below](#watch-in-a-browser)).
  - Hosts copy a web link when their sharing service serves the web viewer.
- **With a code:** under **Join a session** (or **File › Join a Session…**, **⌘J** on macOS, **Ctrl+Shift+J** on Windows and Linux), type or paste the invite code or link and select **Join**. Upper or lower case and dashes do not matter, and the code is formatted as you type.

If you are watching another session, SpecDeck asks before it leaves that one. If a link does not work, SpecDeck says so on the start screen; see [Troubleshooting](/specdeck/guide/troubleshooting/) for what each message means.

## Through another sharing service

A join link also names the host's sharing service. If it is not the one you use, for example because the host's team [runs its own](/specdeck/self-host/overview/), SpecDeck asks **Join through …?** before it connects:

- **Join** uses that service for this session only.
- Tick **Use this service from now on** to make it your sharing service in [Settings](/specdeck/guide/settings/).
- **Cancel** connects nowhere.

The other service cannot read the session, but it sees your network address, so join only through links you trust.

A code typed on its own always uses your own sharing service. When host and viewer use different services, share the link.

:::note[SpecDeck 1.0]
SpecDeck 1.0 cannot open links from newer versions by clicking them. On 1.0, paste the link into **Join a session** instead, or update SpecDeck.
:::

## Overview or Developer

The first time you join, SpecDeck asks how you want to follow sessions:

- **Overview:** the conversation and the plan. You see what was asked, what the agent did in plain words, its answers, and the documents it wrote. The terminal is hidden.
- **Developer:** the host's terminal, read-only, next to every tool call, command and diff.

SpecDeck remembers your answer for every session you join, and you can change it at any time under **View** (see [Follow a session](/specdeck/guide/follow/)). **Leave** takes you back to the start screen.

## Watch in a browser

If you cannot install SpecDeck, for example on a company laptop, you can watch in a browser: current Chrome, Edge, Firefox and Safari. Hosting still needs the app.

- **Open the web link** the host shared, or go to the sharing service's address (`https://specdeck.hartj.es/` for the built-in one) and type the code.
- **Enter your name** and select **Watch in this browser**.

You see the session as in the app: the terminal, the timeline, files, people, the status bar, and the Overview and Developer views.

- **Your name and view** stay in this browser only. There is no account.
- **The code** is removed from the address bar once the page has read it, and is never sent to the server: it sits after the `#`, which browsers keep to themselves.
- **Each page joins only sessions on its own sharing service.** An invite for another service gets a link to that service's own page, never a connection from this one.
- **Some keys differ**, because the browser keeps its own:

  | To | Press |
  |---|---|
  | Find in the terminal and timeline | **⌘F** or **Ctrl+F** |
  | Open Timeline, Files or People | **⌥1**, **⌥2**, **⌥3** (macOS) or **Alt+1**, **Alt+2**, **Alt+3** |
  | See all keyboard shortcuts | **?** |
  | Zoom | Your browser's zoom |

- **"This web viewer needs to be updated"** means the host uses a newer SpecDeck than the page the sharing service serves. Ask whoever runs the service to update it, or watch in the app.
- **"This browser blocks the connection"** means the browser has WebRTC turned off, as privacy-focused browsers such as LibreWolf can. Allow WebRTC in its settings (in LibreWolf and Firefox, `media.peerconnection.enabled` in `about:config`), use another browser, or watch in the app.
- **Company networks often block UDP.** On such a network, the page loads but cannot connect, and it says so; see [Troubleshooting](/specdeck/guide/troubleshooting/).

## When the connection drops

If your connection drops, SpecDeck reconnects on its own and catches up; you do not miss anything. When the host ends the session, you can keep reading what was shared until you leave.
