---
title: Settings and notifications
description: Your name, notifications, the sharing service and the window.
---

## Settings

Open **Settings** with **SpecDeck › Settings…** or **⌘,** on macOS, or **File › Settings…** or **Ctrl+,** on Windows and Linux. Changes apply at once.

| Setting | What it does |
|---|---|
| Your name | The name others see in sessions you start or join from now on |
| Show notifications | Turns all of SpecDeck's notifications on or off |
| Screen reader support | Lets screen readers (VoiceOver, Narrator, Orca) read the terminal. Off unless you turn it on: while it is on, typing in the terminal of a session you host can repeat text |
| Give Claude a browser | Claude Code sessions you start from now on get [a browser of their own](/specdeck/guide/browser/). On by default; only on macOS |
| Verbose logging | The [diagnostic log](/specdeck/guide/troubleshooting/#see-what-the-connection-is-doing) also records how much each connection sends, summed per second. Never what is sent. Off unless you turn it on |
| Sharing service | The service that connects hosts and viewers. Leave it empty for the one built into SpecDeck |
| Check for updates automatically | Checks for a new SpecDeck in the background and downloads it, ready for **Restart to update**. Only on installs that [update themselves](/specdeck/install/#update) |

### Sharing service

If your team [runs its own sharing service](/specdeck/self-host/overview/), enter its domain, such as `specdeck.example.com`, or its full address (`wss://…/signal`). SpecDeck checks whether it can be reached and saves it either way. It applies to the next session you start or join, so it cannot be changed during a session. An address that starts with `ws://` is marked **Not encrypted**.

Everyone on a team that runs its own sharing service sets it here, or joins once through a host's link and ticks **Use this service from now on**. When SpecDeck was started with `SPECDECK_SIGNALLING_URL` set, that address is used and the setting cannot be changed. See [Connect the apps](/specdeck/self-host/connect-apps/) for every way to set it.

## Notifications

While SpecDeck is not the window in front, or its window is closed, it tells you what needs you:

| Who | When |
|---|---|
| Host | Someone joins or leaves |
| Host | Claude Code finishes a prompt, or asks your permission |
| Viewer | The host ends the session, or removes you |

Click a notification to bring the session to the front. Nothing is shown while you are looking at SpecDeck. To turn notifications off, use **Show notifications** in Settings.

## The window

- SpecDeck reopens where you left it, at the same size, unless that screen is no longer connected.
- It follows your computer's light or dark appearance, also when you switch while it is open. On macOS and Windows 11 the window lets your desktop show through, as the system's own apps do.
- Only one SpecDeck runs at a time. Opening it again, or clicking a join link, brings the running one to the front.
