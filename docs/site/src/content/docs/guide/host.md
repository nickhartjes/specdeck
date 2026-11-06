---
title: Host a session
description: Share a coding agent or your terminal with your team.
---

## Start a session

1. Under **Host a session** (or **File › Host a Session…**, **⌘N** on macOS, **Ctrl+Shift+N** on Windows and Linux), pick the **agent**, or **Terminal** to share your shell without an agent. Each choice shows what your viewers get with it (a timeline with diffs and plans, the agent's status, or the terminal only), and for an installed agent its version and where it was found; Terminal shows the shell it starts. Agents that are not installed are listed below the choices under *Not installed*; select one to open its install instructions in your browser. SpecDeck remembers what you chose last.
2. Select one of your **recent folders**, or **Choose a folder and start…** and pick the project folder the agent or shell should start in. Recent folders list the last five folders you hosted in, newest first; they are also under **File › Open Recent**. Select **×** next to one to remove it from the list (the folder itself is not touched).
3. The agent or your shell starts in the terminal. Use it as you normally would. In a terminal session, typing `exit` ends the shell; the session stays open for viewers to read until you end it.
   - The first time in a new folder, Claude Code asks whether you trust it. Answer it in the terminal.
4. Share the **invite code** at the top, for example `7K2M-9QXH-4TPA`. **Copy** copies the code; the link button copies a `specdeck://join/…` link, which opens SpecDeck and joins when a teammate clicks it.

:::caution[Terminal sessions show everything]
With **Terminal**, viewers see all output, including anything secret a command prints, such as tokens in environment variables or `.env` files. Passwords you type at a prompt are not shown, because the shell does not echo them.
:::

The link also names your sharing service, so it works for viewers whose SpecDeck uses another one. When you and your viewers use [different sharing services](/specdeck/guide/join/#through-another-sharing-service), share the link rather than the bare code.

## Several sessions at once

Each window shows one session. To host another agent or folder, or to watch a teammate while you host, open another window with **File › New Window** (**⇧⌘N** on macOS). **Host a Session…** and **Join a Session…** open a new window by themselves when the one in front already shows a session. Each session has its own invite code and viewers, and ending one leaves the others running.

Settings are the same for every window: your name, sharing service, view and zoom. Each window's title names its session, so you can tell them apart in the Window menu or the taskbar.

## What viewers see

- **The whole session so far.** Anyone who joins later can scroll back to the start of the terminal and the timeline, as the note under the invite code says.
- **The terminal**: everything the agent or shell prints. Claude Code itself prints part of what it reads and writes, such as the first lines of a file it creates, and that is shared like any other output.
- **With Claude Code, the timeline**: your prompts, Claude's responses, the tools it uses, and diffs of the files it creates or changes in the session folder. Markdown files in the session folder can also be read whole.
- **In the timeline, files outside the session folder only by name.** When the agent reads or changes a file elsewhere, such as Claude Code's own notes under `~/.claude/`, the timeline shows that it happened and the file's path, starting with `~` in your home folder, but no diff and no text. What the agent prints about it in the terminal is still shared.
- **Shell commands as Claude typed them**, including any paths in them.

Start the session in the folder you mean to share, not a folder above it: the timeline does not show changes elsewhere in a git repository.

:::caution[Anything in the terminal is shared]
SpecDeck cannot filter the terminal. Keep secrets out of what the agent or a command prints while people watch, for example by not asking the agent to show `.env` files.
:::

## While you host

| To | Do this |
|---|---|
| See who is watching | Open **People** in the side panel |
| Stop people from joining with the current code | Select the **new code** button next to the invite code. People already watching stay; share the new code with anyone still to join |
| Remove someone | In **People**, select **Remove** next to their name and confirm. They are disconnected immediately, and the invite code changes so they cannot rejoin with it |
| Limit the number of viewers | In **People**, set **Max viewers** (1 to 20). Lowering it never disconnects anyone already watching |
| End the session for everyone | Select **End session** and confirm. The agent or shell stops and the invite code stops working |

The status next to the folder name shows what the agent is doing: **working**, **waiting for input**, or **exited**. For omp, Codex, Grok and pi it shows working or waiting when the agent reports it, and **Live** otherwise; OpenCode always shows **Live**, because its activity is not shared; a terminal session also shows **Live**, and **Shell exited** once you exit the shell. When Claude Code asks your permission, for example to run a command, the status shows **waiting** and the timeline shows the question, so viewers see why it paused.

If you set up your own Claude Code status line, it keeps showing in the terminal. SpecDeck reads the same figures from it for the bar at the bottom of the window, which everyone in the session sees, including your 5-hour and 7-day limits.

## Closing the window

**Closing the window does not end the session.** The agent keeps running and viewers stay connected. While you host, SpecDeck shows an icon in the menu bar (macOS) or the system tray (Windows, Linux). Its menu shows what the agent is doing and how many people watch, and offers the items below. With several sessions, the menu has one submenu per session, named after its folder, with the same items.

| Menu item | What it does |
|---|---|
| **Open Window** | Brings the session back, with everything that happened while the window was closed. On macOS, clicking SpecDeck in the Dock does the same |
| **Copy Invite Code** | Copies the code, also while the window is closed |
| **End Session…** | Ends the session for everyone, after asking |

Quitting SpecDeck (**⌘Q** on macOS, **File › Quit** elsewhere) while you host asks first, because it ends the session for everyone. With several sessions open, it asks once for all of them.

## When sharing is unavailable

If SpecDeck's sharing service cannot be reached, the session still runs on your computer. A banner says that sharing is unavailable, and the invite code appears as soon as the service is back.
