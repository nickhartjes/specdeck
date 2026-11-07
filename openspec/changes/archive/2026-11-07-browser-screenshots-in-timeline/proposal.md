# Proposal

## Why

Claude Code now has a browser of its own (`agent-browser`, #70), and many hosts also use Playwright MCP or Claude in Chrome. Viewers only see "Used its browser: opened localhost:5173/login, took a screenshot" in the timeline. They don't see what Claude saw, such as the error on the login page, unless the host shares their screen. #67 asks for the screenshots in the timeline. The spike for #67 showed that the image arrives inline in the `PostToolUse` hook, as an image block in `tool_response`, so the session can take it from the hooks it already reads, for SpecDeck's browser and for other MCP servers alike.

## What Changes

- **Screenshots in the timeline.** When an MCP tool's result holds an image, the timeline keeps it with that tool call. A screenshot from SpecDeck's browser also carries the page's address. Every participant gets it, late joiners too, in the app and the web viewer.
- **Only MCP tools.** Images in the results of Claude Code's own tools (such as `Read` of a PNG file) are left out, since they can show files from anywhere on the host's computer.
- **Small enough to send.** An image that would make the entry too large is scaled down and re-encoded as JPEG before it goes into the timeline. If it still doesn't fit, it's left out and the step says the screenshot was too large to show.
- **Shown in the steps.** In plain words, a browser step shows a thumbnail of its latest screenshot. With every step, the tool call shows its own. Selecting a thumbnail opens the screenshot full size in a tab.
- **The host can turn it off.** A new setting, *Show screenshots in the timeline*, on by default. When off, the session keeps no images: everyone, the host included, sees that a screenshot was taken but not the image. It applies to sessions started afterwards, like *Give Claude a browser*.
- **Older viewers keep working.** The image travels as a new kind of tool outcome, which apps of the same protocol version that don't know it already ignore.

Out of scope: clicks and typing in more detail than today's plain words; the browser on Windows and Linux; a live view of the browser; screenshots in a recap (#57); turning screenshots off for a running session; more than one image per tool result.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-timeline`: a new requirement to capture images from MCP tool results, with their size limit and the address; a new requirement for the *Show screenshots in the timeline* setting; *Entries too large to send whole* covers screenshots; *Older viewers keep working* covers the new outcome.
- `viewer-experience`: a new requirement that screenshots show in the timeline's steps as thumbnails.
- `session-window`: *Timeline steps open tabs* opens a screenshot full size in a tab.

## Impact

- `crates/protocol` (`timeline.rs`): `ToolOutcome::Screenshot` with the image (media type, base64 data, width, height) and the address when known; `fit` drops the image before cutting anything else.
- `crates/core` (`timeline.rs`): find the first image block in an `mcp__*` tool's `tool_response`; scale it down when it is too large; take the address from SpecDeck's own screenshot result. New fixtures recorded from Claude Code for SpecDeck's browser and Playwright MCP.
- Dependencies: the `image` crate (JPEG and PNG only) in `specdeck-core`, so in the session binary. It is already in the app through Tauri. The installer stays under its 25 MB limit (16.3 MB today), checked in the tasks.
- `crates/session` (`messages.rs`): `Command::Start` gets `screenshots: bool`.
- `app/src-tauri`: the setting in `Settings` and the Settings screen, passed in `Start`.
- `app/ui`: the timeline model keeps screenshots on tool items; thumbnails in `TimelinePanel`; an image tab in the tab model.
- Memory: the session keeps its timeline for late joiners, so each screenshot adds up to about 200 KB while the session runs.
- No change to the sharing service or to the protocol version.
