---
title: Follow a session
description: Choose what you see, scroll back, search and copy.
---

<video class="video" controls preload="none" poster="/specdeck/video/following.jpg" src="/specdeck/video/following.mp4" aria-label="Following a session: Overview and Developer, the agent's steps and the host's decisions, View, search, Files and the status bar (1.5 minutes, narrated, with captions)"></video>

## Choose what you see

Select **View** at the top of the session to change how you follow it. Your choice is remembered, and nobody else's view changes.

| Setting | What it does |
|---|---|
| **Show terminal** | Shows or hides the host's terminal. Hidden, it keeps running out of sight, so it comes back at once with everything that happened |
| **Steps: Summary** | Each thing the agent did as one line in plain words, such as *Ran the test suite* (in the agent's own words), *Looked at 3 files*, *Changed 2 files* or *Updated the plan*, marked as done, failed or running. Select a line to see the commands, files and diffs behind it |
| **Steps: Details** | Every tool call with its input and outcome, and the diffs of changed files |
| **Show the first-join question again** | Asks how you want to follow sessions once more |

With the terminal hidden, a line at the bottom of the conversation says what the agent is doing right now and for how long, such as *Claude is running the test suite · 12 s*, or what it is waiting for when it needs the host.

The host's terminal is always shown, because the host types into it; the host can still choose summary or details for the steps. Sessions with OpenCode, omp, Codex, Grok, pi or a plain terminal have no steps to show, so the terminal stays shown there, with a note why.

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

## Read the session

| To | Do this |
|---|---|
| Read earlier output | Scroll up in the terminal. New output keeps arriving without moving your view; select **Jump to live** to follow it again |
| Find something | Press **⌘F** (macOS) or **Ctrl+F** (Windows, Linux) and type. **Enter** goes to the next match, **Shift+Enter** to the previous one. The search covers the terminal and the timeline. In the host's own terminal on Windows and Linux, use **Ctrl+Shift+F**, because Ctrl+F belongs to the terminal |
| See what happened per prompt | Open **Timeline**. Each prompt shows the agent's answer, its tool calls (succeeded, failed or did not run) and the diffs of files it changed |
| Go to a prompt | Use **Jump to** at the top of the timeline, or select a prompt: the terminal scrolls to where it happened |
| See which files changed | Open **Files**. Select a changed file to see all its diffs in order |
| Switch between Timeline, Files and People | Select the tab, or press **⌘1**, **⌘2**, **⌘3** (macOS) or **Ctrl+1**, **Ctrl+2**, **Ctrl+3** (Windows, Linux) |
| Copy | Select text in the terminal, the timeline or a diff and copy it: **⌘C** on macOS; on Windows and Linux **Ctrl+C** copies while text is selected (in your own terminal, without a selection it interrupts the agent, as in any terminal) and **Ctrl+Shift+C** always copies. Copying never sends anything to the session |
| Make the side panel wider or narrower | Drag the line between the terminal and the side panel, or focus it and use the left and right arrow keys. SpecDeck remembers the width |
| Read the terminal larger or smaller | The terminal keeps the host's size and scales its text to fill the width of your terminal pane, following the latest output. Give the pane more room (a wider window, a narrower side panel) for larger text. [Zoom](/specdeck/guide/keyboard/) makes the rest of the window larger; the terminal keeps filling its pane |

What you scroll, search or open only changes your own view.

## The status bar

The bar at the bottom of the window shows what the agent works with and has used: the folder, its git branch (● means uncommitted changes, ↑↓ commits ahead of and behind its upstream) and, for Claude Code, the model and effort, how full the context window is, the tokens used in the session, and the host's 5-hour and 7-day limits with the time until they reset. Hover over a figure for details.

Each prompt in the **Timeline** shows the tokens it used. Most of the token total is context the agent reads again from its cache on every call.
