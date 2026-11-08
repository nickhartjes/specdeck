---
title: Follow a session
description: Choose what you see, scroll back, search and copy.
---

<video class="video" controls preload="none" poster="/specdeck/video/following.jpg" src="/specdeck/video/following.mp4" aria-label="Following a session: Overview and Developer, the agent's steps and the host's decisions, the view settings, search, Files and the status bar (1.5 minutes, narrated, with captions; recorded before the window had panels and tabs)"></video>

## The window

The session window has three parts:

- **Files**, on the left: the documents and the code the agent touched.
- **Tabs**, in the middle: the terminal, and the documents and diffs you open.
- **Timeline**, on the right: each prompt, and what the agent did for it.

## Choose a layout

The layout button at the top of the session sets up the window for how you follow it. Your layout is remembered for every session, and nobody else's window changes.

| Layout | What you get |
|---|---|
| **Overview** | Documents and the timeline in plain words, with the newest plan in front. No terminal tab |
| **Developer** | The code, the terminal and every step |
| **Present** | Only the middle, with both side panels closed, for sharing your screen |

Change the window by hand as you like: close a panel with its **×** or the buttons next to the layout button, drag the line between two panels to make one wider (or focus it and use the arrow keys), and double-click that line to put the panel back at its usual width. SpecDeck remembers your changes and shows the layout as *adjusted*; choose it again to go back to it. Each layout keeps its own panel widths.

Switch the timeline between **Plain words** and **Every step** at its top:

- **Plain words:** each thing the agent did as one line, such as *Ran the test suite* (in the agent's own words), *Looked at 3 files*, *Changed 2 files* or *Updated the plan*, marked as done, failed or running. Select a line to see the commands, files and diffs behind it.
- **Every step:** every tool call with its input and outcome, and the diffs of changed files.

**Show the first-join question again**, under the layout button, asks how you want to follow sessions once more.

When the terminal is not shown, a line at the bottom of the timeline says what the agent is doing right now and for how long, such as *Claude is running the test suite · 12 s*, or what it is waiting for when it needs the host.

The host always has the terminal tab, because the host types into it. Sessions with OpenCode, omp, Codex, Grok, pi or a plain terminal have no steps to show, so the terminal tab is always there, with a note why.

On a narrow window, such as a browser on a phone, you see one part at a time and switch with **Files**, **Session** and **Timeline** at the top.

## When Claude asks the host

Claude Code sometimes stops to ask the host something. The timeline shows what was asked and what the host decided, in both summary and details:

| When Claude | You see |
|---|---|
| Asks a question with options | *Asked: Which database? → Postgres*, shown open with every option, the host's choice marked, and anything the host typed or noted |
| Proposes a plan | *Proposed a plan* while it waits, then *Plan approved* or *Plan rejected*, shown open with the plan. A rejected plan is usually followed by a revised one |
| Needs permission to run a tool | The step is marked *Allowed by the host* or *Denied by the host*. A denial shows once the host types their next prompt |
| Hands work to a helper | Select *Asked a helper: …* to read what the helper reported back, also when it worked in the background and finished later |
| Updates its to-do list | Select *Updated its to-do list* to see the list, each item marked to do, in progress or done |

While Claude waits for an answer or for a plan to be approved, the line at the bottom of the conversation says so.

You only see these decisions: answering questions and approving plans stays with the host. Viewers on an older SpecDeck version see the steps without the decisions.

When Claude asks, the timeline also shows a notice of what it asks, such as *Claude wants to use Bash: npm test*, *Claude asks: Which database?* or *Claude has a plan for you to review*.

## What the agent is doing

Next to the session's name, a label shows whether the agent is working or waiting for the host.

| Session | The label shows |
|---|---|
| Claude Code | Working or waiting, also while Claude shows the host a question or a plan, and while the host compacts its context with `/compact` |
| omp | Working or waiting, as omp reports it. When omp needs the host, its own message is shown with the label, such as *Waiting for input: omp needs permission: approve Write: src/main.rs* |
| Codex, Grok, pi | Working or waiting, as the agent reports it, with its message when it needs the host, such as Grok asking whether it may trust the folder |
| OpenCode | *Live*: OpenCode does not report what it is doing |
| Terminal | *Live*. While a program in the terminal that reports its state runs, such as Claude Code or Codex started by hand, the label shows whether that program is working or waiting, with its message when it needs the host, and goes back to *Live* when the program ends |

Agents report their state through the terminal (the [Program Status Protocol](https://www.superlogical.com/rex/docs/build/program-status)), so a program shows up here only if it supports it. The host is notified when an agent or program waits for them while the SpecDeck window is in the background.

## Get called back

You don't have to keep watching. While the session's window is in the background, SpecDeck tells you when a plan is ready for review or a document changed, and, if you want, every time the agent finishes. You get at most one notification each time the agent finishes a prompt. It also tells you when the host [asks you to read something or to estimate](/specdeck/guide/read-and-estimate/). Choose which ones under **While you watch a session** in [Settings](/specdeck/guide/settings/#while-you-watch-a-session). Clicking a notification opens the plan or document it is about.

### Notifications in the browser

In the web viewer, notifications are off until you turn them on: open the layout button and tick **While this tab is in the background** under **Notifications**. Your browser asks whether the page may show notifications. Once you allow it, choose the same kinds as in the app. The browser remembers your choice, so you are not asked again. They show only while the tab is hidden or the browser is not in front. Clicking one shows the page and opens the plan or document it is about, as in the app, as long as the page is still open. If the browser does not allow notifications, the menu says so; allow them for the page in your browser's site settings and try again.

## Read the session

| To | Do this |
|---|---|
| Read earlier output | Scroll up in the terminal. New output keeps arriving without moving your view; select **Jump to live** to follow it again |
| Find something | Press **⌘F** (macOS) or **Ctrl+F** (Windows, Linux) and type. **Enter** goes to the next match, **Shift+Enter** to the previous one. The search covers the terminal and the timeline. In the host's own terminal on Windows and Linux, use **Ctrl+Shift+F**, because Ctrl+F belongs to the terminal |
| See what happened per prompt | Look at the **Timeline**. Each prompt is numbered, with the agent's answer and its steps (done, failed, running or did not run) on a line beside it |
| Go to a prompt | Use **Jump to** at the top of the timeline, or select a prompt: the terminal scrolls to where it happened |
| See a step's change | Select the step. A step that changed a file opens that file's diffs in a tab, at that change; any other step shows the terminal at its line, highlighted for a moment |
| See which files changed | Look in **Files**, under **Documents** or **Code**. Each file says whether the agent created, changed or only read it. Select a changed file to see all its diffs in order; select a file it only read to see where in the terminal it read it |
| See a whole code file | Open a changed file from **Code** and switch from **Changes** to **Whole file, changes marked**: the file as it is now, with the lines changed in this session marked and where lines were removed. It works when the host shares whole files (otherwise the tab says the host shares only the changes), and for changes made since. Each file keeps the view you chose for the rest of the session; a step in the timeline always shows its change |
| See files people shared | Look in **Files › Shared**, under **Shared with everyone**: wireframes, notes or logs the host or other viewers shared, newest first. Select one to open it; **Save a copy** saves it on your computer. A share also shows in the timeline. See below |
| See the braindump | Look in **Files › Shared**, under **Braindump for** the agent: the notes and files people collected for the agent, each marked **Given** once the host gave it. See below |
| Open a file for a while | A single click opens a file in a preview tab, in italics, which the next file you open replaces. Double-click the file or the tab to keep it open; scrolling or selecting text in a preview keeps it too |
| Show or hide the files, the terminal or the timeline | Press **⌘1**, **⌘2**, **⌘3** (macOS) or **Ctrl+1**, **Ctrl+2**, **Ctrl+3** (Windows, Linux), from left to right. **⌘2** or **Ctrl+2** goes to the terminal tab, and adds it in Overview |
| Copy | Select text in the terminal, the timeline or a diff and copy it: **⌘C** on macOS; on Windows and Linux **Ctrl+C** copies while text is selected (in your own terminal, without a selection it interrupts the agent, as in any terminal) and **Ctrl+Shift+C** always copies. Copying never sends anything to the session |
| See who is in the session | Point at the avatars at the top; select them for the list |
| Read the terminal larger or smaller | The terminal keeps the host's size and scales its text to fill the width of the tab area, following the latest output. Give it more room (a wider window, narrower or closed side panels) for larger text. [Zoom](/specdeck/guide/keyboard/) makes the rest of the window larger; the terminal keeps filling its tab |

What you scroll, search or open only changes your own view.

## Joining late

If you join after prompts have finished, the timeline starts with a card about what happened before you came in: how long the session has run, how many prompts it had, the plans with their tasks done, the documents the agent wrote, the files and links people shared, and how many code files changed. Select a plan, document, file or link to open it. The card does not change after you join, and **×** closes it for the rest of the session.

The prompts that finished before you joined are folded to the prompt and its outcome, such as *4 files changed · 3 of 6 tasks done*, or the first sentence of the agent's reply. Select **Show the N steps** to unfold one. A search that finds something in a folded prompt unfolds it. The prompt that was running or latest when you joined stays open, as does everything after it.

To get around, use **Jump to a prompt** in the card, or **Go to the latest prompt** to go to the end of the timeline and follow it again. Jumping to a folded prompt leaves it folded.

Losing the connection and coming back does not count as joining again. In the web viewer, reloading the page does. The host never sees prompts folded.

## Share a file or add to the braindump

When the host lets viewers add files and notes, **Files › Shared** has **Add files or a note**: choose files, or drop them there, and choose **Where it goes**.

- **To the braindump** collects input for the agent: what you need, edge cases, a screenshot. Write a note, add files, or both. A note is text up to 32 KB, saved as a Markdown file with your name, such as `sanne-note-1.md`. You can **Edit** your own note in its tab until the host gives it to the agent. When the braindump is complete, the host gives it all to the agent at once, with a prompt the host writes. Each item is then marked **Given**, and the timeline shows the giving as a step.
- **Share with everyone** is for people to look at, and never reaches the agent unless the host puts the file in the session folder.

Images (PNG, JPEG, GIF, WebP), Markdown and text files up to 10 MB can be added, up to 20 MB of your files in a session. Everyone in the session sees them, with your name. You can remove your own files and notes.

What you add goes to the host's SpecDeck first, which checks it and passes it on. When the host has not turned this on, the **Shared** tab says so. With a host on an older SpecDeck you can share files but there is no braindump.

## Links

The host, or a viewer through the host, can share a link: a preview deploy, staging, a ticket or a whiteboard. It shows as a card in the timeline, at the moment it was shared, with its title, its site, its address and who shared it. Select **Open in your browser** to open it in your own default browser, with your own logins; SpecDeck never loads the page itself. In the browser viewer it opens in a new tab, which can't reach the SpecDeck tab.

A link marked **For this session only** hides the part of its address after `#`, which can hold a key, as in an Excalidraw room. It still opens and copies whole; share it only with people in the session. When the host removes a link, its card says so and the address is gone.

To share a link yourself, select **Send a link to** *host* above the timeline. Only the host sees it, with your name, until they share it with everyone or decline it; you see which. The host can turn this off. Links to your own computer can't be sent, since nobody else can reach them.

## The status bar

The bar at the bottom of the window shows what the agent works with and has used: the folder, its git branch (● means uncommitted changes, ↑↓ commits ahead of and behind its upstream) and, for Claude Code, the model and effort, how full the context window is, the tokens used in the session, and the host's 5-hour and 7-day limits with the time until they reset. Hover over a figure for details.

Each prompt in the **Timeline** shows the tokens it used. Most of the token total is context the agent reads again from its cache on every call.
