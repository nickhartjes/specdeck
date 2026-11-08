# browser-steps

Recorded with Claude Code 2.1.311 on 2026-11-08 by `cargo run -p specdeck-core --example record_browser_steps`: in SpecDeck's browser, Claude opens a login page, reads it, types an address and a test password, and clicks *Sign in*. The browser is a stand-in that answers as `browser.js` does for that page. Paths are rewritten to `/workspace/project` and `/home/user`.

Click and type calls carry the element's number as `element` in `tool_input`; the answers arrive in `PostToolUse` as an array of text blocks in `tool_response`.
