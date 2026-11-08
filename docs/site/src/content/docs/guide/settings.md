---
title: Settings and notifications
description: Your name, appearance, notifications, the sharing service and the window.
---

## Settings

Open **Settings** with **SpecDeck › Settings…** or **⌘,** on macOS, or **File › Settings…** or **Ctrl+,** on Windows and Linux. Changes apply at once.

| Setting | What it does |
|---|---|
| Your name | The name others see in sessions you start or join from now on |
| Appearance | **System** follows your computer's light or dark appearance; **Light** and **Dark** keep SpecDeck that way. Also under **View › Appearance**. See [The window](#the-window) |
| Show notifications | Turns all of SpecDeck's notifications on or off |
| While you host a session | Which [notifications](#while-you-host-a-session) you get about the answers when you asked everyone to read or to estimate: a viewer has a question, everyone has read or the time is up, everyone has voted |
| While you watch a session | Which [notifications](#notifications) you get about a session you watch: a plan is ready for review, a document changed, the agent finished, the host asks you to read, the host asks for an estimate, someone shares a file, the host shares a link, someone adds to the braindump. The bell in a session's toolbar has the same switches |
| Screen reader support | Lets screen readers (VoiceOver, Narrator, Orca) read the terminal. Off unless you turn it on: while it is on, typing in the terminal of a session you host can repeat text |
| Give Claude a browser | Claude Code sessions you start from now on get [a browser of their own](/specdeck/guide/browser/). On by default; only on macOS |
| Show screenshots in the timeline | Pictures that Claude's browser and other MCP tools return, such as [screenshots](/specdeck/guide/browser/#screenshots-in-the-timeline), show in the timeline, for viewers too. Applies to sessions you start from now on. On by default |
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
| Host | A viewer has a question, everyone has read, or everyone has voted, as you chose under **While you host a session** |
| Viewer | The host ends the session, or removes you |
| Viewer | A plan is ready for review, a document changed, the agent finished, the host asks you to read or to estimate, or something is shared, as you chose under **While you watch a session** |

Click a notification to bring the session to the front. Nothing is shown while you are looking at SpecDeck. To turn notifications off, use **Show notifications** in Settings.

### While you host a session

Choose what calls you back while you wait for the people watching, after you [asked everyone to read or to estimate](/specdeck/guide/read-and-estimate/):

| Switch | You hear when | On at first |
|---|---|---|
| A viewer has a question | A viewer answers *I have a question*. The notification names them and shows their question, or what they read when they wrote none. Once per viewer for each request | Yes |
| Everyone has read, or the time is up | Nobody is still reading: everyone is ready or has a question. Or the time ran out while someone was still reading | Yes |
| Everyone has voted | Every viewer has voted in this round. It says when your own vote is still missing, and never shows a vote | Yes |

Only viewers who are there and can answer count: viewers who left, and viewers on an older SpecDeck, don't hold anything up. When someone joins and answers later, you hear again. Clicking a question's notification shows the questions in the read request, while it is still open.

### While you watch a session

Choose what calls you back to a session you watch, here or under the [bell](/specdeck/guide/follow/#the-bell) in the session's toolbar: they are the same switches, and a change in one shows in the other.

| Switch | You hear when | On at first |
|---|---|---|
| A plan is ready for review | Claude presents a plan to the host, or writes or revises an [OpenSpec plan](/specdeck/guide/documents/) | Yes |
| A document changed | The agent creates or changes a Markdown document | Yes |
| The agent finished | The agent finished a prompt and waits for the host's next one | No |
| The host asks you to read | The host [asks everyone to read](/specdeck/guide/read-and-estimate/) something | Yes |
| The host asks for an estimate | The host starts a vote, or a new round of one | Yes |
| Someone shares a file | The host or another viewer [shares a file](/specdeck/guide/follow/#share-a-file-or-add-to-the-braindump) | No |
| The host shares a link | The host [shares a link](/specdeck/guide/follow/#links); the notification names its title, never its full address | No |
| Someone adds to the braindump | The host or another viewer adds a file or a note to the braindump; a note's text is never shown | No |

You get at most one notification each time the agent finishes a prompt, naming everything in it: *Plan add-login is ready for review*, with *README.md changed too* below it. Claude's plan in plan mode is the exception: you hear about it at once, since Claude stops for it. Ticking off tasks while the agent works through a plan does not notify you. What happened before you joined, and a prompt the agent was already working on when you joined, never notifies you.

The host's read requests and votes, and what others share, notify you on their own, outside that one per prompt. What you shared yourself never notifies you. A request or vote that was already open when you joined, or that comes back after you reconnect, doesn't notify you.

Clicking one of these opens what its title names, in the preview tab:

| Notification | Opens |
|---|---|
| *Plan add-login is ready for review*, or *was updated* | The first document of the plan that changed: the proposal of a new plan, the design of a revised one |
| *README.md changed*, or *README.md and 2 other documents changed* | `README.md` |
| *Claude Code has a plan for you to review* | Claude Code's plan, in a tab of its own, which shows whether the host approved it |
| *Claude Code finished* | Nothing; the session comes to the front |
| *Henk asks you to read*, *Henk asks for an estimate* | Nothing; the session comes to the front, where the request waits |
| *Henk shared wireframe.png*, *Mo added a note to the braindump* | That file or note, while it is still shared |
| *Henk shared a link* | Nothing; the session comes to the front, with the link in the timeline |

It also works for an older notification, and after the host ended the session: you see the latest version you received, even of a document deleted since. Once you left the session, a click opens nothing.

A click only reaches SpecDeck when your system reports it:

- On Windows, click the notification while it is on screen. Once it moved to the notification center, clicking it opens nothing.
- On Linux, it depends on your notification server. GNOME and KDE report clicks; some lightweight servers don't, and then clicking opens nothing.

Whatever your system does with a click, every notification is also in the [bell's list](/specdeck/guide/follow/#the-bell), and selecting it there opens the same thing.

In a browser, turn notifications on under the bell instead. See [Notifications in the browser](/specdeck/guide/follow/#notifications-in-the-browser).

## The window

- SpecDeck reopens where you left it, at the same size, unless that screen is no longer connected.
- It follows your computer's light or dark appearance, also when you switch while it is open, unless you choose **Light** or **Dark** under **Appearance** in Settings or **View › Appearance**. A change applies to every window at once, title bar included. On macOS and Windows 11 the window lets your desktop show through, as the system's own apps do.
- The terminal stays dark in every appearance. The agent picks its colours for a dark background, so everyone in the session sees them the same way, whatever each of you chose. Your appearance changes only your own app.
- Only one SpecDeck runs at a time. Opening it again, or clicking a join link, brings the running one to the front.
