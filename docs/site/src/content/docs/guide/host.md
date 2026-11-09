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

Settings are the same for every window: your name, sharing service, layout and zoom. Each window's title names its session, so you can tell them apart in the Window menu or the taskbar.

## What viewers see

- **The whole session so far.** Anyone who joins later can scroll back to the start of the terminal and the timeline, as the note under the invite code says.
- **The terminal**: everything the agent or shell prints. Claude Code itself prints part of what it reads and writes, such as the first lines of a file it creates, and that is shared like any other output.
- **With Claude Code, the timeline**: your prompts, Claude's responses, the tools it uses, and diffs of the files it creates or changes in the session folder. Markdown files in the session folder can also be read whole.
- **Whole code files, only if you choose so.** By default viewers see only the changes Claude makes to code. In the **sharing settings** (the sliders button at the top), **Code viewers can open** can be set to **Changed files, whole, with changes marked**. From then on, each code file Claude creates or changes in the session folder is sent whole with its change, up to 200 KB, and viewers can open it with the changes marked. Turning it off again stops sending them; what was already sent stays with the viewers who got it. Every session starts with only the changes.
- **Never sent whole**, whatever you choose: files outside the session folder (also through a symlink), files git ignores, private files such as `.env`, keys and certificates (`*.pem`, `*.key`, `id_rsa`), `.npmrc` and `.netrc`, files that are not text, and files Claude only read. Their changes are still shown as diffs, as before, except for files outside the session folder.
- **In the timeline, files outside the session folder only by name.** When the agent reads or changes a file elsewhere, such as Claude Code's own notes under `~/.claude/`, the timeline shows that it happened and the file's path, starting with `~` in your home folder, but no diff and no text. What the agent prints about it in the terminal is still shared.
- **Shell commands as Claude typed them**, including any paths in them.
- **Files people share, and nothing else from your computer.** A file you share is one you chose or dropped yourself. SpecDeck never reads any other file to share it, whatever a viewer's app asks for. See [Share files](#share-files).

Start the session in the folder you mean to share, not a folder above it: the timeline does not show changes elsewhere in a git repository.

For where all of this travels and what stays on your computer, see [Your data](/specdeck/security/data/).

:::caution[Anything in the terminal is shared]
SpecDeck cannot filter the terminal. Keep secrets out of what the agent or a command prints while people watch, for example by not asking the agent to show `.env` files. This holds whatever **Code viewers can open** is set to: a file Claude reads or prints in the terminal is shared as it appears there.
:::

## While you host

| To | Do this |
|---|---|
| See who is watching | Select the avatars at the top of the session |
| Stop people from joining with the current code | Select the **new code** button next to the invite code. People already watching stay; share the new code with anyone still to join |
| Remove someone | Select the avatars, then **Remove** next to their name, and confirm. They are disconnected immediately, and the invite code changes so they cannot rejoin with it |
| Limit the number of viewers | Select the avatars and set **Max viewers** (1 to 20). Lowering it never disconnects anyone already watching |
| Let viewers open whole code files | Select the **sharing settings** button (sliders) and set **Code viewers can open**. It applies to changes Claude makes from then on |
| Let viewers suggest prompts, or stop them | Select the **sharing settings** button and switch **Viewers can suggest prompts**. Nothing a viewer suggests reaches the agent until you send it. See [Prompt suggestions](#prompt-suggestions) |
| Answer a suggested prompt | Open **Prompts**. Under **Waiting for you**, select **Send**, **Edit** to change it first, **Keep for later**, or **Decline**. A kept suggestion moves to **Kept for later**, where you can send it, edit it first, or **Remove** it; its author sees that you kept it, and sees a removed one as declined. Kept suggestions stay when you turn suggestions off, and are gone when the session ends. A prompt you send from a suggestion shows in everyone's timeline as *suggested by* its author, with *edited* when you changed it |
| Share a file with everyone | Open **Files › Shared** and select **Add files or a note**, or drop files there, and choose **Share with everyone**. See [Share files](#share-files) |
| Collect notes and files for the agent | Open **Files › Shared**, select **Add files or a note**, and keep **To the braindump**. Select **Give** when it is complete. See [The braindump](#the-braindump) |
| Let viewers add files and notes, or stop them | Select the **sharing settings** button and switch **Viewers can add files and notes**. It covers sharing files and the braindump, and is off when a session starts |
| Share a link | Select **Share a link** above the timeline. See [Share a link](#share-a-link) |
| Let viewers send links, or stop them | Select the **sharing settings** button and switch **Viewers can share links**. You see each link first |
| End the session for everyone | Select **End session** and confirm. The agent or shell stops and the invite code stops working |

The status next to the folder name shows what the agent is doing: **working**, **waiting for input**, or **exited**. For omp, Codex, Grok and pi it shows working or waiting when the agent reports it, and **Live** otherwise; OpenCode always shows **Live**, because its activity is not shared; a terminal session also shows **Live**, and **Shell exited** once you exit the shell. When Claude Code asks your permission, for example to run a command, the status shows **waiting** and the timeline shows the question, so viewers see why it paused.

If you set up your own Claude Code status line, it keeps showing in the terminal. SpecDeck reads the same figures from it for the bar at the bottom of the window, which everyone in the session sees, including your 5-hour and 7-day limits.

## Prompt suggestions

Viewers can [suggest a prompt](/specdeck/guide/follow/#suggest-a-prompt) for the agent. Suggestions are on when an agent session starts; a terminal session has none, because every line typed into a shell runs. Nothing a viewer suggests reaches the agent until you send it.

When a suggestion arrives, SpecDeck tells you who suggests a prompt, without taking the focus from your terminal, and the **Prompts** tab next to **Timeline** shows how many are waiting. Open it to see them under **Waiting for you**, oldest first, each with who suggested it:

- **Send** types the text into the agent's prompt, after anything you already typed there, and submits it, as if you typed it yourself. A suggestion of several lines is one prompt.
- **Edit** lets you change the text first; then **Send** it, or **Cancel**.
- **Keep for later** sends nothing yet and moves it to **Kept for later**. From there you can send it, edit it first, or **Remove** it. A kept suggestion stays kept; it does not go back to waiting.
- **Decline** sends nothing.

The viewer who suggested it sees what you did: sent, edited and sent, kept for later, or declined; a suggestion you remove shows as declined. Other viewers never see a suggestion while it waits or is kept. Once you send it, everyone's timeline shows the prompt as *suggested by* its author, with *edited* when you changed it. Answered suggestions stay under **Handled**.

Waiting and kept suggestions stay while the session's window is closed, and are gone when the session ends; they are never written to disk. When you turn suggestions off, or remove a viewer, the suggestions waiting or kept stay, and you can still answer them. Each viewer can send three at once and then one every 10 seconds; SpecDeck asks them to wait beyond that.

:::caution[Read a suggestion before you send it]
A suggestion can contain anything, including instructions you would not give the agent yourself. Sending it is the same as typing it.
:::

## Share files

Share a wireframe, acceptance criteria or a CI log so everyone in the session can look at it together. Open **Files**, select **Shared**, then **Add files or a note**, or drop files on it, and choose **Share with everyone**. Everyone sees the file under **Shared with everyone**, with who shared it, when and its size, and in the timeline as a step, such as *Nick shared wireframe.png*. Anyone can open it in a tab, or **Save a copy**.

- **What can be shared:** images (PNG, JPEG, GIF, WebP), Markdown and text files, such as logs, JSON or code, up to **10 MB** each. SpecDeck checks the content, not only the name: SVG, PDF and other files are refused, and so is a program renamed to `notes.txt`. A session holds up to **50 MB** of shared files, and each viewer's files up to 20 MB. Remove some to make room.
- **Viewers share only when you let them.** In the **sharing settings**, switch on **Viewers can add files and notes**. It is off at the start of every session, and it also decides whether viewers can add to [the braindump](#the-braindump). Turning it off keeps the files already shared.
- **You can remove any file**, and a viewer can remove their own. A removed file is gone for everyone, and its step in the timeline says it was removed.
- **Files last for the session.** People who join later or reconnect get them too. SpecDeck keeps them in memory only, never on disk, and when the session ends they are gone, except copies people saved.
- **The agent does not see shared files.** To give one to the agent, open it and select **Put in the session folder**. SpecDeck writes it to `shared/` in the session folder, without replacing anything: a second `wireframe.png` becomes `wireframe (2).png`. Then tell the agent where it is. Only you can do this.
- **Files travel between you and each viewer directly**, over the session's own encrypted connections, never through the sharing service. A viewer gets a file only when they open it or save it. Viewers' files travel to you first, and from you to the others.

:::caution[Read a viewer's file before you give it to the agent]
A file a viewer shares reaches you and everyone in the session, but never the agent unless you put it in the session folder. It can contain anything, including instructions meant for the agent. Read it first.
:::

## The braindump

Before you start, people often have input for the agent in different places: what the product owner needs, edge cases a developer thought of, a screenshot. The braindump collects it in one place in the session, and you give it to the agent at once instead of retyping it.

Open **Files › Shared** and select **Add files or a note**, or drop files there. **Where it goes** starts on **To the braindump**. Write a note, choose files, or both, and select **Add**. Everyone sees the items under **Braindump for** the agent, above **Shared with everyone**, with who added each one, when and its size.

- **Notes** are text up to **32 KB**. Each becomes a Markdown file named after its author, such as `sanne-note-1.md`. The author can **Edit** their note in its tab until it is given to the agent; the edited note replaces the old one. Nobody else can edit it, you neither.
- **Files** follow the rules of [shared files](#share-files): images, Markdown and text, up to 10 MB, counted in the same 50 MB a session and 20 MB a viewer. Files can't be edited: remove one and add it again.
- **Viewers add only when you let them**, with the same **Viewers can add files and notes** switch. You can always add.
- **Removing:** you can remove any item, a viewer their own. Removing an item that was already given leaves its copy in the session folder.
- **Giving:** when the braindump is complete, select **Give N to** the agent, where N counts the items not given yet. The dialog shows the prompt the agent gets, which you can edit, and the items. **Give** copies each item into `shared/braindump/` in the session folder, never replacing a file: a second `edge-cases.txt` gets a number, as with **Put in the session folder**. Then SpecDeck types your prompt into the agent, followed by the exact paths it wrote, and sends it. Given items are marked **Given**. Add more and give again: a second round gives only what was added since.
- **In the timeline** each giving is a step, such as *Gave the braindump to Claude Code: 3 items from Sanne and Mo*, which lists the items to open. Adding an item is not a step.
- **In a terminal session** everyone can add to the braindump too, but **Copy N to the session folder** only writes the files and types nothing, because every line typed into a shell runs. Point your tool to `shared/braindump/` yourself.
- **For this session only.** The braindump is held in memory, like shared files, and is gone when the session ends. Only the copies given to the agent stay, in `shared/braindump/`.
- **Viewers on an older SpecDeck** keep watching, without the braindump.

:::caution[Read the braindump before you give it]
Notes and files from viewers can contain anything, including instructions meant for the agent. Nothing reaches the agent until you give it, and you see the prompt first. Read the items before you select **Give**.
:::

## Share a link

Share a preview deploy, staging, a ticket or a live whiteboard with everyone in the session. Select **Share a link** above the timeline, paste the address, and add a title if you like. Everyone sees a card with the title, the site, the address, and who shared it when, in the timeline at that moment, so people who join later find it too. It works in every session, also a terminal session, where the links are listed under the timeline's note.

- **It opens in each person's own browser**, with their own logins, when they select **Open in your browser**. SpecDeck only carries the address: it never loads the page, its title or a preview, and nothing opens by itself.
- **Only web addresses others can reach.** Links start with `https://`, or `http://` for an address on your network. Links to your own computer (`localhost`, `127.0.0.1` and the like) are refused, because viewers can't reach it; opening your app to viewers is not available yet. Addresses with a user name or password in them are refused too. SpecDeck checks the address as written; it does not look up where a name points.
- **For this session only.** Some links carry their key after `#`, such as an Excalidraw room (`https://excalidraw.com/#room=…,<key>`): anyone with the link can join the room. **For this session only** is on for such links, and you can turn it off before sharing. The card then hides the part after `#`, and copying the link warns that it holds a key. Once SpecDeck can save a recap of a session, such links will be left out of it. It can't be changed once the link is shared.
- **Links from viewers.** Viewers can send you a link to share. It waits above the timeline, with who sent it and the whole address, until you select **Share** or **Decline**; nobody else sees it before you share it. Turn this off with **Viewers can share links** in the sharing settings; links already waiting stay.
- **Removing a link.** Select **Remove…** on its card. Its card then says you removed it, and people who join later never get the address. People who already saw it still have it, so for a link with a key, also close the room or change its key where you made it.

**Drawing together.** Start a live collaboration room on [excalidraw.com](https://excalidraw.com) and share its link. Everyone draws in their own browser, and Excalidraw encrypts the drawing end to end; SpecDeck never sees it.

## Closing the window

**Closing the window does not end the session.** The agent keeps running and viewers stay connected. While you host, SpecDeck shows an icon in the menu bar (macOS) or the system tray (Windows, Linux). Its menu shows what the agent is doing and how many people watch, and offers the items below. With several sessions, the menu has one submenu per session, named after its folder, with the same items.

| Menu item | What it does |
|---|---|
| **Open Window** | Brings the session back, with everything that happened while the window was closed. On macOS, clicking SpecDeck in the Dock does the same |
| **Copy Invite Code** | Copies the code, also while the window is closed |
| **End Session…** | Ends the session for everyone, after asking |

The start screen also lists every session you host that has no window, under **Running sessions**, with what the agent is doing and how many people watch. **Open** shows the session in that window. This helps where there is no tray icon, such as GNOME without the AppIndicator extension.

Quitting SpecDeck (**⌘Q** on macOS, **File › Quit** elsewhere) while you host asks first, because it ends the session for everyone. With several sessions open, it asks once for all of them.

## When SpecDeck restarts or stops

From SpecDeck 1.7.0, each session you host runs on its own, apart from the app. It keeps running, with the agent working and your viewers connected, when SpecDeck:

- restarts to install an update (**Restart to update**, see [Update](/specdeck/install/#update)),
- crashes, or
- is force quit, for example from Activity Monitor or Task Manager.

Your viewers notice nothing. Start SpecDeck again and it opens a window for each session, with the whole terminal and timeline so far and the same invite code. If you close one of those windows, the session shows under **Running sessions** on the start screen.

If SpecDeck is not started again within **10 minutes**, the session ends itself as if you had ended it: the agent stops, viewers see that the session ended, and the invite code stops working. Quitting SpecDeck still ends every session you host, after asking, and so does logging out or shutting down your computer.

## When sharing is unavailable

If SpecDeck's sharing service cannot be reached, the session still runs on your computer. A banner says that sharing is unavailable, and the invite code appears as soon as the service is back.
