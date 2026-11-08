# chrome-steps

Recorded with Claude Code 2.1.311 and the Claude in Chrome extension 1.0.105 on 2026-11-08, with `claude -p --chrome` and hooks that save each payload: in a new tab, Claude opens a local test login page, reads it, clicks the email and password fields and types into them with `computer`, sets the code and card number with `form_input`, and clicks the ✕ button and *Sign in*. The page (served by `python3 -m http.server` on 127.0.0.1:5179) has an email field, a password field, a text field marked `autocomplete="one-time-code"` (*Code*), a text field marked `autocomplete="cc-number"` labelled only by `aria-label` (*Card number*), a text area (*Notes*), a *Sign in* button and a ✕ button without a label. Paths are rewritten to `/workspace/project` and `/home/user`.

Clicks are `computer {action: "left_click", ref, action_summary, tabId}`; typing is `computer {action: "type", text, action_summary, tabId}` without a ref, into the element the last click focused; `form_input` carries `{ref, value, action_summary, tabId}`, and its value can be a number. `read_page` lists elements as `<role> ["<name>"] [ref_n] [type="…"]`, a field's label on the line above it, indented one step less; a password field shows `type="password"`, but one-time-code and card fields show `type="text"`.

`tools.json` holds the server's tool list as it gave it to Claude Code.
