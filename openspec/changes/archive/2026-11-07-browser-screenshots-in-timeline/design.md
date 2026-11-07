# Design

## Context

See proposal.md for why. The constraints that shape the design:

- **Where the timeline is built.** `TimelineBuilder` (`crates/core/src/timeline.rs`) turns hook payloads into `Entry` values inside the session process (`specdeck-session`). `PostToolUse` keeps `tool_response` as a raw `Value`, and today reads it only for questions, plans, helper reports and file edits. The session keeps every event in memory and replays them to late joiners.
- **One event, one frame.** `fit` (`crates/protocol/src/timeline.rs`) keeps each event's JSON within `MAX_EVENT_JSON_BYTES` (240 KB) by cutting its large parts and setting `truncated`.
- **An unknown `Entry` kind breaks older viewers.** A frame whose event doesn't decode is treated as malformed, and the viewer asks for a resend (`viewer_state.rs`, `on_stream`). `ToolResult.outcome`, though, is read with `lenient`: an outcome of an unknown kind becomes `None`. That is how answers, plan decisions and reports reached viewers without a protocol bump.
- **What the hooks carry.** From the #67 spike (Claude Code 2.1.311): an MCP tool's image arrives inline in `tool_response` as `{type: "image", source: {type: "base64", media_type, data}}`, followed by a text block with the path where Claude Code saved it. A large image was re-encoded by Claude Code to a ~480 KB JPEG and still sent inline. No recorded fixture has an MCP tool yet.
- **SpecDeck's own screenshots** are JPEG at a fixed width, at most 150 KB (`agent-browser`, *Screenshots*), so about 200 KB in base64. The result's text block is the page's `page()` text, which leads with the address.
- **Images on the page.** Both the app's CSP and the web viewer's (`crates/server/src/web.rs`) already allow `img-src 'self' data:`.
- **The session binary has no image crate.** `image` 0.25 is in `Cargo.lock` only through Tauri and arboard, so only in the app.

## Goals / Non-Goals

**Goals:**
- No protocol version change; viewers on 1.7 without this change keep working.
- Screenshots from any MCP server, without server-specific code except for the address from SpecDeck's own browser.
- Nothing from outside what MCP tools returned to Claude reaches viewers.

**Non-Goals:**
- Storing screenshots anywhere but in the running session's memory.
- Several images per tool result, or images inside helper reports.
- Recognising which non-SpecDeck tool "navigated", to give other servers' screenshots an address.

## Decisions

### 1. The image travels as a tool outcome

```rust
ToolOutcome::Screenshot {
    /// None when the host turned screenshots off, or it was too large or unreadable.
    image: Option<Image>,       // { mediaType, data (base64), width, height }
    address: Option<String>,    // SpecDeck's browser only, shown like a URL without scheme
    left_out: Option<LeftOut>,  // turned-off | too-large | unreadable
}
```

It rides on the `ToolResult` the `PostToolUse` already produces. Older apps read it as no outcome (`lenient`), which is the *Older viewers keep working* behaviour, and the frame still decodes.

*Alternatives:* a new `Entry::Screenshot` is cleaner to render, but an older viewer can't decode the frame and keeps asking for a resend. A protocol bump to 3 for this alone would lock 1.7 viewers out of 1.8 hosts; the PAKE change (`harden-session-protocol`) will bump it anyway.

### 2. Only `mcp__*` tools, and only the first image

The builder looks for an image in a `PostToolUse` only when `tool_name` starts with `mcp__`. It reads `tool_response` as an array of content blocks, or as an object with a `content` array (both shapes are handled; the fixtures in task 1.1 decide which comes first). It takes the first block with `type: "image"` and a base64 `source`. Claude Code's own tools are excluded on purpose: `Read` of an image file returns the image, and that file can be anywhere on the host's computer, which the timeline promises not to share (`session-timeline`, *Capture tool calls*).

*Alternative:* an allow-list of browser servers (`specdeck`, `playwright`, `chrome-devtools`, `claude-in-chrome`). It would miss renamed servers and new tools; an MCP tool returning an image already sent that image to Claude, and the terminal says so.

### 3. Scaled in the session process with the `image` crate

The builder keeps an image as it arrived when its base64 is at most `MAX_SCREENSHOT_BASE64` (200 KB), which covers SpecDeck's own screenshots. Larger ones are decoded (JPEG and PNG), scaled to at most 1280 px wide, and saved as JPEG at quality 70, then again at 960 px and quality 60 if needed. If none fits, or the image doesn't decode, `image` is `None` with the reason. The width and height are those of the kept image, so the UI can reserve space before it loads.

`specdeck-core` gets `image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }`, the same version Tauri pulls in, so the app gains nothing. The session binary does gain it.

*Alternatives:* ask the app to scale (it has AppKit and `image`), but the session outlives the app and screenshots would be lost while it is gone. Leaving large images out would drop every Playwright screenshot. Scaling in the viewer doesn't help, since the limit is on what is sent.

### 4. `fit` drops the image first

In `shrink`, a `ToolResult` with `ToolOutcome::Screenshot` loses its `image` (set to `None`, `left_out: too-large`) before its error is cut. After decision 3 this only happens when the error text is also very large.

### 5. The address from SpecDeck's own browser

For `mcp__specdeck__screenshot`, the address is the URL at the start of the result's text block, shown without its scheme like the plain-words steps do (`localhost:5173/login`). Other servers get no address. The step's plain words already name the last opened address.

### 6. The setting goes in `Start`

`Command::Start` gets `screenshots: bool` with serde default `true` (the setting's default). `Settings.screenshots` (serde default `true`) is shown in Settings on every platform, since Playwright MCP works everywhere. When `false`, the builder still emits `ToolOutcome::Screenshot` with `image: None, left_out: turned-off`, so everyone sees that a screenshot was taken. Nothing is filtered per participant: the host's own timeline comes from the same events, which keeps late joiners and the host consistent.

*Alternative:* the host sees images and viewers don't. That needs per-participant timelines in the session process, and the host can see the screenshots in the terminal's saved paths anyway.

### 7. In the UI: on the tool item, a thumbnail and an image tab

`buildTimeline` already copies `outcome` onto the `ToolItem`; nothing changes there. `steps.ts` picks the latest screenshot outcome in a browser run, or in any step with an MCP image. `TimelinePanel` renders a thumbnail (`<img src="data:...">`, max 240 px wide, with width and height set) and the address. A click opens an `image` tab (`tabs.ts`, new `TabKind`, id `image:<toolUseId>`), which shows the image at full size, scrollable. The tab is a preview tab like a file opened with a single click.

## Risks / Trade-offs

- [Memory in long sessions] Each screenshot keeps up to ~200 KB in the session's timeline; 100 screenshots is ~20 MB. → Acceptable for now. If it shows up in `ui.size` diagnostics, drop old images from the replay first.
- [The session binary grows] The `image` crate with JPEG and PNG adds a few hundred KB. → Measure the DMG in task 2.3. The limit is 25 MB and it's at 16.3 MB today.
- [The hook payload shape changes] Claude Code could stop sending the image inline, or change the block shape. → Recorded fixtures with the Claude Code version, a test per shape, and a missing image just means no thumbnail.
- [Screenshots show more than the terminal] A logged-in page or personal data. → The setting, the sentence in Settings that viewers see screenshots, and the security docs (`security-hardening-plan`, item 5) say so.
- [Decoding untrusted images] An MCP server returns a crafted PNG. → `image` decodes in the session process, which has no more rights than the host's shell. Set decoder limits (`image::Limits`, max 8192×8192 and 64 MB) so a decompression bomb fails instead of using up memory.

## Migration Plan

None. A host with this change sends screenshots; viewers without it ignore them. The new `Start` field defaults to on, so a session started by this release's app gets the setting it chose. Rollback is a release without the change.
