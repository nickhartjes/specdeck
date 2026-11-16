# Proposal

## Why

From the security review of 2026-11-10. SpecDeck's windows can be led away from SpecDeck, and the app does not say which window may call which of its functions:

- **A clicked link replaces the session view, and the page it opens keeps receiving the session.** Links in documents, plans, answers and shared Markdown files are plain anchors (`app/ui/src/components/Markdown.tsx`), and no window has a navigation rule. The linked page loads in the SpecDeck window. Tauri delivers the terminal and the session's events by running script in whatever page the window shows, so that page can read them, including a new invite code. A viewer can plant such a link in a shared Markdown file, and a repository can through the agent's answers. The known limit "Links in documents open inside the app window" says the page cannot use SpecDeck's functions, which is true, but not that it can read the session.
- **Claude's browser may be able to open SpecDeck itself.** The browser tools accept only `http` and `https` addresses, but a page can send the hidden window on to any address, and nothing stops `tauri://localhost`. App functions are not granted per window: with no app manifest, every window that shows the app's own page may call all 81 of them. If WebKit allows that redirect, an agent in a sandbox could drive SpecDeck's own screen and start a terminal on the host's computer. This is untested (see design.md, Open Questions); the fix does not depend on the answer.
- **A join link joins without asking.** `specdeck://join/CODE` from any web page makes the app join an attacker's session at once, showing the user's name and IP address to its host and attacker-made content to the user.
- **Viewer names can mislead.** Names may hold text-direction and zero-width characters, and Linux notification servers render markup in *NAME is watching your session.*

## What Changes

- **SpecDeck's windows show only SpecDeck.** Session windows and the settings window refuse to load any other page.
- **Links in session content open in the participant's own browser**, in the app after the same check as shared links, and in the web viewer in a new tab that cannot reach the SpecDeck tab and gets no referrer. A link that fails the check does not open; its address can still be read and copied.
- **Claude's browser loads only `http` and `https` pages**, however it gets there: a tool call, a redirect, a link or a script.
- **App functions are granted per window.** Only session windows and the settings window may call them; a window of Claude's browser may call none, whatever page it shows.
- **A join link opened from outside the app asks first**: *Join session 7K2M-9QXH-4TPA as nick?* A link pasted into the app's join field does not ask again.
- **Viewer names are shown without invisible and text-direction characters**, and notifications show names as plain text.
- **Docs:** the known limit "Links in documents open inside the app window" is removed, and the pages on Claude's browser, documents, joining and following say what happens now.

Not in this change:

- Refusing a host's functions from a window that only views a session. All session windows keep the same grants; see design.md, Non-Goals.
- Tauri's isolation pattern.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-experience`: adds that the app's windows show only the app's own page, that app functions are granted per window, and that names in notifications are plain text.
- `session-window`: adds how links in documents, plans, answers and shared Markdown files open.
- `web-viewer`: adds how those links open in the browser.
- `agent-browser`: *Pages cannot reach SpecDeck* also covers a page that sends the browser to SpecDeck's own address; adds that the browser loads only web pages.
- `session-sharing`: *Join link* asks before joining from a link opened outside the app; *Participant list* shows names without invisible characters.

## Impact

- **App (`app/src-tauri`)**: `build.rs` (app manifest), `capabilities/*.json`, window creation in `lib.rs` (session and settings windows), `browser/webkit.rs`, a new command to open a content link, `notify.rs`. A test keeps the manifest, the capabilities and the command handler in step.
- **UI (`app/ui`)**: `components/Markdown.tsx`, the `Bridge` (`api/bridge.ts`, `api/tauri.ts`, `api/demo.ts`, `web/bridge.ts`), `App.tsx` and a join confirmation dialog.
- **Protocol and core**: the name filter of `crates/core/src/program_status.rs` moves to `crates/protocol`, used at admission in `crates/core/src/sharing.rs`. No new message and no protocol version change: older viewers and hosts keep working.
- **Documentation**: this is a security fix, not a new feature, so the README overview gets no new line. The pages that change are `docs/site/src/content/docs/security/known-limits.md` (limit removed), `security/claude-browser.mdx` (how the browser is kept apart), `guide/documents.md` (links open in your browser), `guide/join.md` (the question a join link asks), `guide/follow.md` and `guide/host.md` where they mention names. No video: nothing new to show in motion; the existing videos show no link being clicked and no join from a link, so none needs a new take.
- **Dependencies**: none added. `tauri-plugin-opener` is already used from Rust.
