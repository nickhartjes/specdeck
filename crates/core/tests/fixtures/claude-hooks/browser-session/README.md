# browser-session

Recorded with Claude Code 2.1.311 on 2026-11-07 by `cargo run -p specdeck-core --example record_browser_hooks`: SpecDeck's browser (answering every screenshot with a small JPEG) opens a page and takes a screenshot, then Playwright MCP 0.0.84 does the same with Chrome. Paths are rewritten to `/workspace/project` and `/home/user`, and Playwright's PNG is replaced by a small one.

Both servers' screenshots arrive in `PostToolUse` as an array of content blocks in `tool_response`: a text block, the image block (`{type: "image", source: {type: "base64", media_type, data}}`), and a text block with the path where Claude Code saved the image. SpecDeck's first text block starts with the page's address.
