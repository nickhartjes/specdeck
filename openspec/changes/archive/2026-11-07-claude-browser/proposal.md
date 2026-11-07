# Proposal

## Why

When Claude Code checks a web app it is building, it needs a browser, and only hosts who set one up (Claude in Chrome, Playwright MCP) give it one. SpecDeck already runs Claude Code for the host and starts its hooks, so it can give every Claude Code session a browser of its own: nothing to install, isolated from the host's own browser, and later visible to viewers (#67), since SpecDeck takes every screenshot itself. A spike on macOS (#67, branch `spike-browser`) showed that a hidden WebKit window keeps rendering, answers JavaScript and takes snapshots in milliseconds, and that Claude Code accepts an MCP server from `--mcp-config` with the token kept in the environment.

## What Changes

- **A browser for Claude Code.** On macOS, a Claude Code session gets a `specdeck` MCP server with browser tools: open an address, read the page (its text with numbered elements to act on), click, type, press a key, take a screenshot, go back, and read the console.
- **The browser is SpecDeck's own:** a WebKit view in a window that is never shown, created on the first tool call and closed with the session. It starts clean (private, no cookies or storage from anything else), one per session, and the pages it opens cannot reach SpecDeck itself.
- **The host's permissions apply:** SpecDeck approves nothing itself, so Claude Code asks the host before each browser tool, or decides on its own in auto mode, as for any MCP tool.
- **When the app is not running** (a crash, Restart to update), the browser tools answer that the browser is not available, and the page that was open is gone. The session itself keeps running, as today.
- **A setting, *Give Claude a browser*,** on by default, applies to sessions started after it changes.
- **Browser steps in plain words:** the timeline's summary and the notice when Claude Code needs the host say what Claude does in its browser ("Opened localhost:5173/ in its browser"), not the tool's name (`mcp__specdeck__navigate`).
- Screenshots are taken at a fixed width and sent to Claude as JPEG, small enough to also travel in the timeline later.

Out of scope: the screenshot step in the timeline and the image from other browsers in hooks (#67, the next change); Windows and Linux; showing the browser as a tab (#63) or live to viewers; opening the host's dev server for viewers (#68); agents other than Claude Code.

## Capabilities

### New Capabilities

- `agent-browser`: SpecDeck's own browser that a Claude Code session can drive: the tools it offers, where it runs, how it is isolated, when it is available, and the setting that turns it off.

### Modified Capabilities

None. Hosting, sharing and the timeline behave as before; browser tool calls are captured as any other tool call (`session-timeline`, *Capture tool calls*), and only their wording is this capability's.

## Impact

- `crates/core`: the hook endpoint (`hooks.rs`) gets an `/mcp` route speaking MCP over streamable HTTP with the same bearer token; `session.rs` writes an MCP config next to the hook settings and passes `--mcp-config` to Claude Code when the browser is on.
- `crates/session`: new messages on the local protocol (`messages.rs`): the session asks the app to run a browser action and the app answers. `Start` says whether the app offers a browser, so a session never sends requests an app cannot read.
- `app/src-tauri`: a browser module (macOS) that owns one hidden `browser-*` window per session, with native WebKit calls (JavaScript with a result, snapshots, JPEG through AppKit); a new setting. The `browser-*` label matches no capability, so the window gets no IPC.
- Dependencies: `objc2-web-kit`, `objc2-app-kit`, `objc2-foundation` and `block2` as direct macOS dependencies; all are already in the tree through Tauri. No image crate.
- No change to the session protocol between hosts and viewers, or to the sharing service.
