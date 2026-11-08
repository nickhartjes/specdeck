# playwright-steps

Recorded with Claude Code 2.1.311 and Playwright MCP 0.0.84 (headless Chrome) on 2026-11-08, with `claude -p` and hooks that save each payload: Claude opens a local test login page, takes a snapshot, types an address, a test password and a code with `browser_type`, fills the card number and notes with `browser_fill_form`, and clicks the ✕ button and *Sign in*. The page (served by `python3 -m http.server` on 127.0.0.1:5179) has an email field, a password field, a text field marked `autocomplete="one-time-code"` (*Code*), a text field marked `autocomplete="cc-number"` labelled only by `aria-label` (*Card number*), a text area (*Notes*), a *Sign in* button and a ✕ button without a label. The page was served without a charset, so the ✕ reads as `âœ•` in Playwright's text. Paths are rewritten to `/workspace/project` and `/home/user`.

Click and type calls carry Claude's own description as `element` and the snapshot's ref as `target` (`e5`); `browser_fill_form` carries `fields: [{name, target, type, value}]`. Only `browser_snapshot` answers with the snapshot itself; other answers point to a `.playwright-mcp/*.yml` file. The snapshot shows every field as `textbox "<name>"`, password, one-time-code and card fields included.

`tools.json` holds the server's tool list as it gave it to Claude Code.
