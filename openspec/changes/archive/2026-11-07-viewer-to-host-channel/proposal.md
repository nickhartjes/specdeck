## Why

Viewers can only watch. Prompt suggestions (#52), read and confirm (#53), estimation (#54), the braindump (#61), files for the project (#62) and the whole project (#64) all need viewers to send something to the host. This change builds that channel once, with the host in control (#51), and proves it with its first user: prompt suggestions.

It also ends a constraint that would make every one of those features expensive: today host and viewer must speak exactly the same session protocol, so any new message locks out every older app and every web viewer page a sharing service has not updated yet.

## What Changes

- **Protocol version range.** The session protocol goes to version 3. A host accepts viewers that speak version 2 or 3. A version 2 viewer, app or web page, is admitted read-only: the host never sends it a message it cannot read and it cannot send anything. A viewer newer than the host is still refused, as today, because older hosts cannot change.
- **A channel from viewers to the host** on the existing `control` data channel: a viewer sends a request with its own id, and the host answers that viewer alone with what became of it. Requests are typed, one variant per feature, and decoded as strictly as other viewer messages.
- **The host decides.** Each kind of request has a switch for the session, on when a session starts (prompt suggestions never in a terminal session). The host's switches are sent to viewers so they show only what is allowed, and enforced in the session process regardless. Each viewer has a rate limit that reconnecting does not reset. Removing a viewer stays as it is.
- **Nothing a viewer sends reaches the agent.** Only the host's own action types into the session.
- **Prompt suggestions (the core of #52).** A viewer suggests a prompt from a *Prompts* tab, in the app or the browser. The host sees suggestions waiting and sends one as written, edits it first, or declines it. The viewer sees what became of each of their suggestions. Only the host and the viewer who suggested it see a suggestion.
- **Diagnostics** record that a request arrived, its kind, size and outcome, never its text.
- **BREAKING** for the wire only: protocol version 3 viewers cannot join hosts on version 2. Version 2 viewers keep joining version 3 hosts.

Not in this change: keeping suggestions for later or after the session, showing pending suggestions to other viewers, marking a sent prompt as suggested in the timeline (the rest of #52), and sending files (#61, #62).

## Capabilities

### New Capabilities
- `viewer-requests`: what viewers may send to the host and how the host stays in control: the per-session switches, rate limits, answers to the viewer, the guarantee that nothing reaches the agent without the host, and prompt suggestions as the first kind of request.

### Modified Capabilities
- `session-sharing`: *Matching app versions* becomes a range: a newer host admits older viewers read-only.
- `viewer-experience`: *Read-only for viewers* changes: viewers may send what the host allows, but still have no control that sends input to the session.
- `web-viewer`: *Browser-specific failures* changes: a page older than the host's app joins read-only instead of being refused.
- `diagnostics`: viewer requests are logged without their content.

## Impact

- `crates/protocol`: `control.rs` (version range, `Send`, `Reply`, `Permissions`, `Suggestions`, the larger cap for requests, validation of suggestion text), `viewer_state.rs`, `README.md`.
- `crates/core/src/sharing.rs`: per-viewer protocol version, gating what is sent, the gate and rate limit, the suggestions held for the session; `crates/core/src/viewer.rs`.
- `crates/session`: new commands and events between the app and the session process (`messages.rs`, `runtime.rs`); `app/src-tauri/src/adapter.rs` for sessions of the previous release, which cannot take them.
- `crates/web` and `app/ui/src/web`: sending from the browser.
- `app/ui`: the host's switch and suggestions inbox, the viewer's *Prompts* tab.
- Tests in `crates/core/tests/sharing.rs` for versions, gating and the agent never receiving viewer text.
