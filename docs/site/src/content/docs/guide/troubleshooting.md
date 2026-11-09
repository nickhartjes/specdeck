---
title: Troubleshooting
description: What SpecDeck's messages mean and what to do about them.
---

## Joining does not work

SpecDeck says why on the start screen:

| Message | What it means |
|---|---|
| That code does not work | Check the code with the host. Codes stop working when the host ends the session or makes a new code |
| This session is full | The host reached their viewer limit |
| Too many attempts | Several wrong codes were tried from your network. Wait a minute |
| SpecDeck cannot reach its sharing service | Check your internet connection, and the [sharing service in Settings](/specdeck/guide/settings/#sharing-service) |
| Your network does not allow the connection | Your network blocks UDP. Try another network, such as your phone's hotspot |
| Update SpecDeck to join / The host needs to update | You and the host use versions that do not match. Versions that offer Codex, Grok and pi as agents cannot connect to older versions, in either direction |
| This web viewer needs to be updated | In the [web viewer](/specdeck/guide/join/#watch-in-a-browser): the host uses a newer SpecDeck than the page the sharing service serves. Ask whoever runs the service to update it, or watch in the app |
| This browser blocks the connection | In the [web viewer](/specdeck/guide/join/#watch-in-a-browser): the browser has WebRTC turned off, as LibreWolf can. Allow WebRTC (`media.peerconnection.enabled` in `about:config` for LibreWolf and Firefox), use another browser, or watch in the app |

A code typed on its own always uses your own sharing service. If the host uses another one, the code does not work for you: ask for the link instead (see [Through another sharing service](/specdeck/guide/join/#through-another-sharing-service)).

## Clicking a link does nothing

SpecDeck 1.0 cannot open links from newer versions. Paste the link into **Join a session**, or [update](/specdeck/install/#update). Pasting a link always works, also when your system does not open `specdeck://` links.

## A session says "Started by an older SpecDeck"

Sessions you host keep running through an update in the version that started them. SpecDeck shows and controls sessions started by the version before it. A session started by an even older version, for example when you updated twice while it ran, has no window: it is listed in the menu-bar or tray menu with **End Session…** only, and under **Running sessions** on the start screen with **End…** only. Its viewers stay connected until you end it there or quit SpecDeck.

## Sharing is unavailable

When the host sees a banner that sharing is unavailable, SpecDeck cannot reach its sharing service. The session keeps running on the host's computer, and the invite code appears as soon as the service is back. If it does not come back, check the address in [Settings](/specdeck/guide/settings/#sharing-service).

## See what the connection is doing

When a connection keeps dropping or the terminal behaves oddly, open **Help › Connection Status** (**⇧⌘D** on macOS, **Ctrl+Shift+D** on Windows and Linux). It shows your own side only: as the host, your sharing and each viewer's connection as your app sees it; as a viewer, your connection to the host.

<video class="video" controls preload="none" poster="/specdeck/video/diagnostics.jpg" src="/specdeck/video/diagnostics.mp4" aria-label="Diagnostics: the Connection status panel, Export Diagnostics and Verbose logging (1.5 minutes, narrated, with captions)"></video>

The line at the top says what is going on:

| It says | What it means |
|---|---|
| Connected directly to the host | The fastest path: your computers reach each other without a relay |
| Connected to the host through the relay | Your network does not allow a direct path, so the connection goes through the sharing service's relay. It works, a little slower |
| Reconnecting, attempt 3. The last attempt ended because … | The connection dropped and SpecDeck is getting it back. The reason says why the last try failed |
| The sharing service cannot be reached. Trying again in 8 s | As the host: viewers cannot join meanwhile, but your session runs on, and viewers who are connected stay connected |
| The log file cannot be written | SpecDeck keeps the recent events in memory only, until it quits |

Below it are the details: the connection's state, how much was sent or received, and the terminal's state. The terminal part answers questions such as why the mouse wheel does not scroll: when **Mouse** says the program uses it, the wheel goes to the program, not to the scrollback.

As a viewer, **Reconnect** drops the connection and connects again at once. You keep your place: nothing is replayed from the start. It is unavailable while SpecDeck is already reconnecting.

### Send diagnostics with a bug report

**Export Diagnostics…** saves one file, `specdeck-diagnostics-<date>-<time>.jsonl`, where you choose. You can open it in any text editor. It holds:

- the SpecDeck version, your operating system, and whether you use the default sharing service or one of your own (not its address);
- what the panel shows right now;
- the diagnostic log of the last 7 days: when connections started and ended and why, sizes and timings, and whether they were direct or relayed. This includes the log of each session you hosted, also ones that have ended.

It never holds terminal output, the timeline, what the agent was asked or did, invite codes, names, file paths or network addresses. SpecDeck sends it nowhere: attach it to your bug report yourself.

If the window stops responding, so the panel does not open, choose **Help › Export Diagnostics…** from the menu bar instead: it works without the window. The log then also says when the window stopped responding and for how long.

#### From a browser

The web viewer keeps its own diagnostic log for as long as the tab is open, also across a reload, and forgets it when you close the tab. Choose **Export diagnostics…** in the layout menu while watching, or below the message when a session ended or a join failed. The browser downloads a file in the same format, with your browser, its version and your operating system's family in place of the app's version and system. It holds the same kind of events and never more.

### What else the log records

Besides the connections, the log records what goes wrong around the agent:

- **Crashes**: when SpecDeck or a session it hosts crashes, the thread and the place in SpecDeck's code where it happened, and a fixed kind of crash. Never the crash's message.
- **The agent**: how SpecDeck found your environment (your login shell or its own) and how many entries `PATH` has, which agent started (and whether in a Docker Sandbox) or why it could not (such as not installed, or the folder is gone), and the exit code when it stops. A session that stops without saying goodbye is recorded too.
- **Claude Code's hooks and the timeline**: when the hook server starts and stops, and per minute how many of Claude Code's messages the timeline could not place, by event and tool name. Tools from MCP servers count as `mcp`, without the server's name. Screenshots left out of the timeline, and changes of shell commands that could not be tracked, with the step that failed.
- **Updates**: each check, and why a check or download failed (offline, an HTTP error, a signature that does not match, or other), with the versions.

None of these hold error messages, environment variables, prompts, file contents or paths. When the same thing goes wrong more than 30 times a minute, the log keeps the first 30 and then says how many it left out.

### The log

SpecDeck keeps this log all the time, a few lines a minute: `specdeck.log.<date>` for the app, and `specdeck-session-<number>.log.<date>` for each session you host, which [runs on its own](/specdeck/guide/host/#when-specdeck-restarts-or-stops). Files older than 7 days are removed, and the oldest go first when the logs together pass 50 MB. Only your user account can read the folder:

| System | Folder |
|---|---|
| macOS | `~/Library/Logs/nl.nickhartjes.specdeck` |
| Windows | `%LOCALAPPDATA%\nl.nickhartjes.specdeck\logs` |
| Linux | `~/.local/share/nl.nickhartjes.specdeck/logs` |

When someone joins, the host's log says how long sending them the session so far took, how much it was and how often their connection made the host wait (`host.replay`), and the viewer's log says when they had all of it (`viewer.caught_up`). Together they tell a long session from a slow connection when joining takes a while.

You can delete the folder at any time. **Verbose logging** in [Settings](/specdeck/guide/settings/) also records how much each connection sends, summed per second, for problems that only show under load. Turn it off again when you are done.

## macOS: viewers on the same network cannot connect

SpecDeck connects directly to teammates on the same network when it can, which needs macOS's Local Network permission. If you chose **Don't Allow** when macOS asked, turn SpecDeck on under **System Settings › Privacy & Security › Local Network**.

## The app does not open the first time

Releases are not signed yet. See [The first launch](/specdeck/install/#the-first-launch).
