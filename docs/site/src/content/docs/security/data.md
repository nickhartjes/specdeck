---
title: Your data
description: Where session content lives, how it travels, and what viewers can see.
---

## Where a session lives

A session runs on the host's computer: the agent, the session folder and SpecDeck itself. Everything viewers see comes from there.

- **It travels straight to each viewer, encrypted.** The terminal, the timeline, documents, code files, screenshots, shared files and links go from the host's SpecDeck to each viewer over a WebRTC connection encrypted with DTLS, with keys made fresh for every connection. When a direct connection is not possible, the same encrypted traffic passes through the TURN relay, which cannot read it.
- **It never goes through the sharing service.** The service only matches viewers to the host and passes on the messages that set up the connection. See [The sharing service](/specdeck/security/sharing-service/).
- **It is stored nowhere else.** No server keeps a copy, and there is no account or cloud history. When the session ends, it is gone, except for what the agent itself wrote to your disk and what viewers kept themselves.

### What the host's computer keeps while a session runs

SpecDeck keeps a few files for each session in a folder of its own, `sessions/<number>`, in its cache folder (`~/Library/Caches/nl.nickhartjes.specdeck` on macOS, `~/.cache/nl.nickhartjes.specdeck` on Linux, `%LOCALAPPDATA%\nl.nickhartjes.specdeck` on Windows). On macOS and Linux only your account can read them; on Windows they have your user profile's permissions.

| File | Holds |
|---|---|
| `session-….log` | The session's whole terminal output, so viewers who join late can catch up |
| `snapshots-…` | In a git repository, copies of files in the session folder before and after Claude's shell commands, to show what a command changed |
| `hooks-….json`, `mcp-….json`, `statusline-….sh` | The settings SpecDeck gives Claude Code, which name the session's local endpoint. They refer to the session's secret token, but do not contain it. A session started through a [start command](/specdeck/guide/sandbox/) has none of these: Claude Code gets the same settings as arguments |

SpecDeck deletes them when the session ends. If the session's process is killed instead, for example by a crash or a power cut, they can stay behind; see [Known limits](/specdeck/security/known-limits/#leftover-session-files). Shared files and the braindump are kept in memory only, and are written to disk only when you put a file in the session folder or give the braindump yourself. Prompt suggestions, waiting or kept for later, are kept in memory only and never written to disk.

SpecDeck runs git in the session folder to show the branch, to see what a shell command changed and to ask whether a file is ignored. Those git calls run no command that the folder's own git settings name: no file-system monitor, no hook and no filter. They do not look into submodules, whose own settings could name one. So a folder with a prepared `.git/config` cannot run anything through SpecDeck. git commands you or the agent type in the terminal use the folder's settings as usual.

A session keeps running when SpecDeck restarts to update or crashes, and ends by itself if SpecDeck is not back within 10 minutes (see [When SpecDeck restarts or stops](/specdeck/guide/host/#when-specdeck-restarts-or-stops)).

## What viewers can see

**Everything in the terminal is shared. The timeline adds no content of a file outside the session folder, of a private file or of a file git ignores.**

SpecDeck cannot filter the terminal. Whatever the agent or a command prints is shared as it appears, including secrets: a token in an environment variable, a `.env` file the agent shows, a password a program echoes. Claude Code itself prints part of the files it writes, and of files changed by commands, in its own terminal. So a file outside the session folder can still reach viewers, through the terminal.

One rule decides whether the timeline carries a file's content, in any form: a diff, the lines of a new file, a document's text or a code file's whole text. It does only when the file is **inside the session folder**, with symlinks resolved, **is not private by name**, and **is not ignored by git**. Any other file, Markdown included, appears only by its path, as created, changed or deleted, also when it is edited. The rule is the same for Claude's file tools and its shell commands, and no setting changes it.

- **Private by name**, compared without case: environment files (`.env`, `.env.*`, `.envrc`), keys and certificates (`*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.keystore`, `*.jks`, `id_rsa*`, `id_dsa*`, `id_ecdsa*`, `id_ed25519*`), credentials (`.npmrc`, `.netrc`, `.pypirc`, `.git-credentials`, `.htpasswd`, `.dockercfg`, `credentials*`, `secrets.*`, `*.secret`) and Terraform variables and state (`*.tfvars`, `*.tfvars.json`, `*.tfstate`, `*.tfstate.backup`). So is every file below a folder named `.ssh`, `.aws`, `.kube`, `.gnupg`, `.docker` or `.git` inside the session folder. The list is a backstop, not a promise: a secret can be in any file.
- **Ignored by git** is asked of the repository the file is in, which can be a submodule or a clone below the session folder. A file git tracks is not ignored, nor is a file in no repository. When git cannot answer for a file in a repository, the file is named only.
- **A path the agent names** is resolved in the session folder, and the file SpecDeck reads is the one that passed the rule.

What the timeline adds, with Claude Code:

- **Prompts, answers and tool calls**, including shell commands as Claude typed them, and the questions and permission requests Claude Code shows you, with your answers.
- **Diffs** of files Claude creates or changes inside the session folder, and, in a git repository, of files inside it that its commands change, under the rule above.
- **Markdown documents** inside the session folder that Claude creates or changes, whole, up to 200 KB, under the rule above.
- **Other files only by path**, starting with `~` in your home folder, without text or diff: files outside the session folder, private files and files git ignores.
- **Whole code files only if you choose so**, with **Code viewers can open** in the sharing settings. Every session starts with only the changes. The rule above applies to them too.
- **Screenshots** from Claude's browser and other MCP tools, unless you turn off **Show screenshots in the timeline**. A screenshot can show more than the terminal, such as a page you are logged in to.
- **Usage figures** from Claude Code's status line, such as your 5-hour and 7-day limits, in the bar at the bottom.

The terminal is not filtered by this rule: what Claude Code prints of a private file there is still shared.

The exact list is in [What viewers see](/specdeck/guide/host/#what-viewers-see).

### Viewers who join late

Anyone who joins gets the whole terminal and timeline from the start of the session, not only what happens after they join. The note under the invite code says so. If something was shown earlier that a new viewer should not see, end the session and start a new one.

### What viewers take with them

Viewers can copy text, save shared files and take their own screenshots, and a viewer's app keeps what it was sent until they leave. Treat what you show as seen by everyone in the session.

## Shared files and links

- **Shared files** are images, Markdown or plain text, checked by their content, up to 10 MB each. They travel over the same encrypted connections, never through the sharing service, and a viewer gets one only when they open or save it. Nothing in them runs as a script. Viewers can share files only while the host allows it.
- **Shared files never reach the agent by themselves.** Only the host can put a shared file in the session folder (in `shared/`), and only then can the agent read it.
- **The braindump** holds notes (text up to 32 KB) and files that people collect for the agent. Its files follow the rules of shared files, and its notes are shown as text, so nothing in them runs. It is held only in the host's memory and is gone when the session ends. Viewers can add to it only while the host allows it.
- **The braindump reaches the agent only when the host gives it.** Giving writes the items not given yet to `shared/braindump/` in the session folder, then types a prompt into the agent that ends with their paths. The host sees and can edit that prompt first, and nothing is typed in a terminal session. The copies in `shared/braindump/` stay after the session, like any file in the session folder. Read what viewers added before you give it.
- **SpecDeck shares no other file.** It reads only files the host or a viewer chose to share, whatever a viewer's app asks for.
- **Links** are shown as text with their site in plain ASCII, so a look-alike name stands out. SpecDeck never loads a shared page itself: a link opens in your own browser only when you choose to open it. Links from viewers wait for the host's approval before anyone else sees them.

See [Share files](/specdeck/guide/host/#share-files) and [Share a link](/specdeck/guide/host/#share-a-link).

## What reaches the agent

Only the host types into the agent, on their own computer and login. Nothing a viewer's app sends is typed into the session, not even the automatic answers a terminal gives to programs. Prompt suggestions, links and files from viewers wait for the host, who decides what to do with them in their own window. A suggestion, waiting or kept for later, is seen only by the host and its author. Once the host sends it, everyone's timeline shows the prompt as suggested by its author.
