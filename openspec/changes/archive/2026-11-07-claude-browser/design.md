# Design

## Context

See proposal.md for why. The constraints that shape the design:

- **Two processes.** A hosted session runs in its own process (`specdeck-session`) so it outlives the app (`session-hosting`, *Hosted sessions outlive the app*). Claude Code, its hook endpoint (`crates/core/src/hooks.rs`: axum on a random 127.0.0.1 port, bearer token in `SPECDECK_HOOK_TOKEN`) and its launch flags live in that process. A webview can only live in the app, which talks to each session process over the local protocol (`crates/local`, `crates/session/src/messages.rs`).
- **Compatibility across one release.** The app controls sessions of its own and the previous release; `adapter.rs` translates the previous release's messages, and anything else only gets `hello` and `end`.
- **Window labels grant IPC.** `capabilities/default.json` covers `main` and `window-*`, `settings.json` covers `settings`. Any other label gets no commands.
- **What the spike measured** (#67, branch `spike-browser`, macOS 26, Claude Code 2.1.311): a never-shown, minimised, hidden or off-screen WKWebView evaluates JavaScript with a result and takes a fresh snapshot in 3–13 ms; a 640 pt snapshot is a ~66 KB JPEG; `--mcp-config` expands `${VAR}` in `headers`; an MCP tool's image arrives inline in `PostToolUse`.

## Goals / Non-Goals

**Goals:**
- The browser works whether or not a session window is open, as long as the app runs.
- No new dependency beyond the objc2 crates Tauri already pulls in; no image crate.
- Nothing about the browser reaches viewers that the terminal does not already show them.

**Non-Goals:**
- A browser engine of our choice: on macOS it is the system's WebKit.
- Trusted input events (real mouse and keyboard through the OS).
- An MCP SDK or the full MCP feature set: only what tools need.

## Decisions

### 1. The MCP server sits on the hook endpoint, the browser in the app

```
claude (pty)                       specdeck-session                         app (Tauri)
  mcp__specdeck__click{ref:4} --> POST /mcp (Bearer)                         
                                   tools/call -> BrowserAction --Event::Browser{id, action}-->
                                                                             browser-<session> window
                                                                             (hidden WKWebView)
                                   <-- result -------------------- Command::BrowserResult{id, result}
  <-- tool result (text / image)
```

`/mcp` is a new route on the existing `HookEndpoint`, with the same token check, so there is no second port or secret. It turns a `tools/call` into a `BrowserAction` and hands it to the session runtime, which sends `Event::Browser { id, action }` to the attached app and waits for `Command::BrowserResult { id, result }`.

*Alternatives:* a WebKit view inside the session process would survive the app, but the session binary would have to link AppKit and WebKit and run a main-thread run loop, for a case (the app being gone) that lasts seconds during an update. A separate MCP process (stdio) would need its own channel to the app; the session already has one.

### 2. A minimal MCP server, hand-written

The endpoint speaks MCP over streamable HTTP with plain JSON answers (no SSE, no sessions): `initialize`, `notifications/initialized` (202), `ping`, `tools/list`, `tools/call`; anything else gets JSON-RPC "method not found" (Claude Code also sends `server/discover`); `GET` gets 405. The spike's 60-line server was enough for Claude Code. The protocol version answered is the one the client asks for, from a list the server knows.

*Alternative:* the `rmcp` SDK: more than we use, a new dependency tree, and it would set the protocol version for us.

### 3. The app offers the browser in `Start`; the session only then passes `--mcp-config`

`Command::Start` gets `browser: bool` (serde default `false`). The app sets it when the setting is on, the agent is Claude Code and the platform is macOS. Only then does the session write an MCP config next to the hook settings (a user-only temp file, deleted with the session, like `write_hook_settings`):

```json
{"mcpServers": {"specdeck": {"type": "http", "url": "http://127.0.0.1:<port>/mcp",
  "headers": {"Authorization": "Bearer ${SPECDECK_HOOK_TOKEN}"}}}}
```

and adds `--mcp-config <file>` to the launch. Not `--strict-mcp-config`: the host's own MCP servers stay. A session never sends `Event::Browser` unless `Start` said `browser: true`, so an app that cannot read it never gets one. The next release must keep answering it (or translate it in `adapter.rs`) for sessions this release started.

### 4. One hidden, incognito window per session, labelled `browser-<id>`

Created on the first `Event::Browser` for a session, with `WebviewWindowBuilder`: `visible(false)`, `focused(false)`, `skip_taskbar(true)`, `incognito(true)` (a fresh non-persistent WebKit data store per window), 1280×800. Kept in an app-level map by session id, independent of the session's window, and closed on `Event::Ended`, on End session and on Quit. `is_session_window` and the Window menu already ignore labels that are not `main` / `window-*`; a test pins that.

### 5. Native WebKit for results and snapshots; everything else in one script

Tauri's `eval` returns nothing, so the module reaches the `WKWebView` through `with_webview` (on the main thread) and uses `evaluateJavaScript:completionHandler:` and `takeSnapshotWithConfiguration:completionHandler:`, answering through a oneshot channel. The tools are JavaScript functions in one file (`browser.js`, `include_str!`), each run as a self-contained expression that returns JSON, so nothing depends on globals a page could replace:

| tool | does |
|---|---|
| `navigate {url}` | `webview.navigate`, waits for the page-load event (30 s), answers address and title |
| `read_page` | walks the DOM in reading order: text, and links, buttons, fields and elements with a click role numbered |
| `click {ref}` / `type {ref, text, submit?}` / `press {key}` | scrolls into view, focuses, dispatches pointer and mouse events then `click()`; sets `value` through the native setter and fires `input` / `change`; `keydown` / `keyup`, Enter submits the form |
| `screenshot` | snapshot at 640 pt, JPEG through `NSBitmapImageRep` at quality 0.7; above 150 KB, again at 0.5, then at 480 pt |
| `back` | `history.back()`, waits for the load |
| `console` | the buffer an initialization script fills from `console.*`, `error` and `unhandledrejection`, last 500 entries |

**Numbered elements and "the page changed":** `read_page` marks each element with `data-specdeck-ref="<generation>-<n>"`, where the generation is a random value kept per document. `click` and `type` look the element up by its full mark; after a navigation (a new document) or once the element is gone, the lookup fails and the tool answers "read the page again".

### 6. Errors are tool results, not protocol errors

A failure (page did not load, element gone, timeout, no app) is a `tools/call` result with `isError: true` and one plain-words text block, so Claude reads it and can act. When no app is attached the session answers at once; otherwise it waits 35 s for `navigate` and `back`, 10 s for the rest.

### 7. The setting

`Settings.browser: bool`, serde default `true`, shown in Settings as *Give Claude a browser* with a line that it applies to new Claude Code sessions on macOS. Hidden on other platforms.

## Risks / Trade-offs

- [Synthetic events are `isTrusted: false`] → file pickers, some drag and drop and sites that check trust won't work. Fine for checking an app under development; the answer says when an action had no visible effect only if the page tells us, so Claude falls back to a screenshot.
- [A page can lie about itself] (override DOM methods, hide text from the walker) → the page is what Claude asked to look at; screenshots come from WebKit, not from page scripts, so they cannot be faked by the page.
- [What Claude types shows in the timeline] → tool inputs are shown to viewers today (`session-timeline`, *Capture tool calls*), so a password Claude types into a test login is visible, as it already is in the terminal. Covered by *Reminder that viewers see everything*; not changed here.
- [WebKit, not Chrome] → layout or API differences from Chrome go unnoticed. Hosts who need Chrome keep Claude in Chrome or Playwright alongside.
- [Hidden windows throttle timers] (spike: about a third of normal speed) → pages still load and answer; animations and polling run slower. Screenshots wait for `afterScreenUpdates`.
- [Memory] → a WebKit view per session that used the browser, roughly 50–100 MB, only from the first tool call; closed with the session.
- [Claude Code's MCP config format changes] → the session logs a warning when Claude Code does not list the tools (no `initialize` within a minute of start) and the session itself is unaffected.

## Migration Plan

None. The setting defaults to on; a host who does not want it turns it off for new sessions. Rolling back a release removes the tools from new sessions; running sessions of the newer release can only be ended by the older app, as today.

## Open Questions

- Whether the console should also include failed network requests: an addition to the same tool later, without changing the spec.
