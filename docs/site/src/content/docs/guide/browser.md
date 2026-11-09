---
title: Claude's browser
description: A browser of its own for Claude Code, to check the web app it builds.
---

<video class="video" controls preload="none" poster="/specdeck/video/claude-browser.jpg" src="/specdeck/video/claude-browser.mp4" aria-label="Claude's browser: Claude's browser steps in the timeline, in plain words and step by step, a screenshot opened full size, and the Settings to turn it off (1 minute, narrated, with captions)"></video>

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
- **More detail.** [Claude's browser](/specdeck/security/claude-browser/) in *Security and privacy* says how it is kept apart, step by step.
- **What viewers see.** Viewers see Claude's browser steps in the timeline, such as *Typed into “Email”* and *Clicked “Sign in”*, and the [screenshots](#screenshots-in-the-timeline) it takes. What Claude types into an ordinary field shows in the step's details. What it types into a password field, or into a field the page marks as a one-time code or card number, is left out of the timeline: the step only says *Filled in “Password”*. The terminal still shows every tool call as Claude Code prints it, including what Claude types, so use test accounts, not your own passwords. Steps in [other browsers](#playwright-mcp-and-claude-in-chrome) are described too.

## Screenshots in the timeline

When Claude takes a screenshot, everyone in the session sees it in the timeline, with the page's address: as a small picture on the step, which opens full size in a tab. People who join later see the earlier screenshots too.

This works for any MCP tool that returns a picture to Claude, such as Playwright MCP or Claude in Chrome, on every platform. Pictures from Claude Code's own tools, such as a file Claude reads, are never shown. A very large picture is made smaller first; one that is still too large shows as *Too large to show*.

A screenshot can show more than the terminal does, such as a page you are logged in to. To keep screenshots out of the timeline, turn off **Show screenshots in the timeline** in [Settings](/specdeck/guide/settings/). Sessions you start afterwards then only say that Claude took a screenshot, for you and for viewers. Screenshots are kept only while the session runs, on your computer, and sent to viewers over the session's own encrypted connections.

## Playwright MCP and Claude in Chrome

When Claude uses Playwright MCP or Claude in Chrome instead, on any platform, viewers see the same kind of steps: *Opened 127.0.0.1:5179/*, *Clicked “Sign in”*, *Filled in “Password”*, and one step for a run of browser calls. Each action of a Claude in Chrome batch is listed on its own. An element is named as Claude's last read of the page described it; without a read, by Claude's own words, such as *Clicked Sign in button*.

What Claude types there is hidden from the timeline unless SpecDeck knows the field is an ordinary one:

- **Playwright MCP.** Its page snapshots don't say whether a field is a password field, so everything Claude types or fills in through it is left out: the step says *Filled in “Email”*, and the details say *Text hidden*. Choices, such as a ticked checkbox, still show.
- **Claude in Chrome.** Text shows for a field its last read of the page listed as an ordinary field, such as an email field. It is left out for password fields, and for fields SpecDeck doesn't know, such as when Claude types without clicking a field first. Claude in Chrome doesn't mark one-time-code or card number fields, so text typed into those does show.

As with SpecDeck's own browser, the terminal still shows everything Claude types, so use test accounts and test data.

## When it isn't available

- While SpecDeck itself isn't running, for example during **Restart to update**, the tools answer that the browser isn't available, and the page that was open is gone. Your session keeps running. Once SpecDeck is back, Claude can open the page again in a new, empty browser.
- Only Claude Code sessions on macOS get it, for now. Other agents, terminal sessions and SpecDeck on Windows and Linux don't.
- The browser is WebKit, the engine Safari uses, not Chrome. A page that only goes wrong in Chrome won't show the problem here.

## Turn it off

Turn off **Give Claude a browser** in [Settings](/specdeck/guide/settings/). Claude Code sessions you start afterwards get no browser; sessions that already run keep theirs.
