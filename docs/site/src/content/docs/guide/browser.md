---
title: Claude's browser
description: A browser of its own for Claude Code, to check the web app it builds.
---

On macOS, a Claude Code session you host gets a browser of its own. Claude can use it to check the web app it is building: open a page, read it, fill in a form, click a button, take a screenshot and read the console. You don't install or set up anything.

## What Claude can do

| Tool | What it does |
|---|---|
| `navigate` | Opens an address, such as `http://localhost:5173/`, and waits for the page to load |
| `read_page` | Reads the page's text, with its links, buttons and fields numbered |
| `click`, `type`, `press` | Clicks a numbered element, types into a field, or presses a key such as Enter |
| `screenshot` | Takes a picture of the visible part of the page |
| `back` | Goes back to the previous page |
| `console` | Reads what the page logged, and its errors |

SpecDeck doesn't approve these tools for you: your Claude Code permissions decide, as for any other tool. In the default mode Claude Code asks you the first time it wants to use each one; in auto mode it decides on its own. In permission prompts and rules the tools are called `mcp__specdeck__navigate`, `mcp__specdeck__screenshot` and so on.

## How it is kept separate

- **Its own browser.** It is not your browser and doesn't see your logins, cookies, history or passwords. Every session's browser starts empty, and everything it stored is gone when the session ends.
- **Never on screen.** The browser has no window you see, and it never takes the focus while you work.
- **No way into SpecDeck.** Pages Claude opens can't use SpecDeck's functions or reach your sessions. Only the session's own Claude Code can use its browser: it gets the tools from a server on your computer (`127.0.0.1`) that answers only with that session's secret token.
- **What viewers see.** Viewers see Claude's browser steps in the timeline and the terminal as they see any other tool, including what Claude types. Use test accounts, not your own passwords.

## When it isn't available

- While SpecDeck itself isn't running, for example during **Restart to update**, the tools answer that the browser isn't available, and the page that was open is gone. Your session keeps running. Once SpecDeck is back, Claude can open the page again in a new, empty browser.
- Only Claude Code sessions on macOS get it, for now. Other agents, terminal sessions and SpecDeck on Windows and Linux don't.
- The browser is WebKit, the engine Safari uses, not Chrome. A page that only goes wrong in Chrome won't show the problem here.

## Turn it off

Turn off **Give Claude a browser** in [Settings](/specdeck/guide/settings/). Claude Code sessions you start afterwards get no browser; sessions that already run keep theirs.
