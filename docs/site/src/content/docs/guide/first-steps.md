---
title: First steps
description: What you need, and setting your name.
---

SpecDeck lets one person (the **host**) run a coding agent, or just a terminal, on their computer while teammates (the **viewers**) follow along from anywhere. Viewers see the terminal and, for Claude Code, a timeline of every prompt, answer, tool call and file change. Each person scrolls and searches at their own pace.

Viewers are read-only. Only the host types into the agent, and the agent runs on the host's own login.

<video class="video" controls preload="none" poster="/specdeck/video/getting-started.jpg" src="/specdeck/video/getting-started.mp4" aria-label="Getting started with SpecDeck: hosting a session, sharing the invite code and a teammate joining (1 minute, narrated, with captions)"></video>

## What you need

**Everyone** needs [SpecDeck](/specdeck/install/) for macOS, Windows or Linux, and an internet connection. SpecDeck connects through your team's sharing service, so no VPN is needed. It uses UDP; on networks that block UDP entirely (some company networks), joining does not work and SpecDeck says so.

**The host** can share a coding agent, installed and logged in, or a plain terminal:

| Choice | What viewers see |
|---|---|
| [Claude Code](https://code.claude.com/docs/en/setup) (`claude`) | The terminal, plus the timeline, file diffs and the list of touched files |
| [OpenCode](https://opencode.ai/docs/) (`opencode`) | The terminal |
| [omp](https://omp.sh) (`omp`) | The terminal, and whether the agent is working or waiting |
| [Codex](https://developers.openai.com/codex/cli) (`codex`) | The terminal, and whether the agent is working or waiting |
| [Grok](https://x.ai/cli) (`grok`) | The terminal, and whether the agent is working or waiting |
| [pi](https://github.com/earendil-works/pi) (`pi`) | The terminal, and whether the agent is working or waiting |
| Terminal | Your own shell: your login shell on macOS and Linux, PowerShell on Windows. Nothing to install |

Check an agent by running it in a terminal: it should start without asking you to log in. On **Windows**, Claude Code also needs **Git for Windows** (Git Bash), and pi needs a bash: Git Bash, or one from MSYS2, Cygwin or WSL.

## Set your name

The first time you open SpecDeck, it asks what others should call you. To change it later, select **Change** next to your name on the start screen, or open [Settings](/specdeck/guide/settings/).

Two people may use the same name; the second is shown as `Anna (2)`. Invisible and text-direction characters are left out of names, so `Anna` with a hidden character is still the second `Anna`, and a name of only such characters is refused. Names are labels, not logins: anyone with the invite code can join under any name.

## Next

- [Host a session](/specdeck/guide/host/) to share your agent or terminal.
- [Join a session](/specdeck/guide/join/) with the code or link a host sent you.
