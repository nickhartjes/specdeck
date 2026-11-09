# chrome-batch

Recorded with Claude Code 2.1.311 and the Claude in Chrome extension 1.0.105 on 2026-11-08, with `claude -p --chrome` and hooks that save each payload: Claude opens the test login page of `chrome-steps` and reads it in one `browser_batch`, reads it again, and logs in with one more `browser_batch`. Paths are rewritten to `/workspace/project` and `/home/user`.

`browser_batch` carries `actions: [{name, input}]`, each input as the single tool takes it (`computer`, `form_input`, `navigate`, `read_page`); the answer has one text block per action, starting with `[<name>]` or `[computer:<action>]`. A read with `filter: "interactive"` leaves out the label lines; refs stay the same across reads of the same page.
