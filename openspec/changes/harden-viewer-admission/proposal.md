# Proposal

## Why

The security review of 2026-11-10 found that the host's bookkeeping around admission lets a viewer with a changed app undo what the host decides. The join handshake itself is sound; the gaps are in what the host does before and after it (`crates/core/src/sharing.rs`):

- **A removed viewer can come back.**
  - The host accepts a second `Hello` on a connection it already admitted. Each one makes a new viewer with a new rejoin token, and `remove` forgets only the newest. The viewer returns through the rejoin room with an older token and gets the whole session.
  - A second connection that finished the handshake but never sent `Hello` is not in the participant list, keeps the old code's key through **New code** and **Remove**, and can send `Hello` afterwards.
  - A viewer who is not connected at the moment cannot be removed at all, and their rejoin token works for the rest of the session.
- **One viewer can get around the limits.**
  - Each extra `Hello` gets a fresh rate limit and a fresh 20 MB file quota.
  - Removing a viewer during an upload never frees the room that upload reserved, so shared files can end up full for everyone.
  - Nothing bounds the waiting suggestions and links of the session as a whole.
- **The host trusts the sharing service for its own bookkeeping.** The service can make the host open any number of peer connections, or replace a live viewer's connection, by repeating `viewer-joined`.

SpecDeck is meant for teams whose security officer has to agree. "Remove" and "New code" have to mean what the documentation says.

## What Changes

All of it is in the host. Nothing changes on the wire, so released apps and the web viewer keep working, and so do self-hosted sharing services.

- **One `Hello` per connection.** A second one is a protocol error and the connection is closed.
- **Joining has a deadline.** A connection that has not been admitted 30 seconds after it was announced, or 10 seconds after it connected, is dropped.
- **The host bounds what is still joining.** At most 40 connections at once, admitted or not; an announcement for a connection the host already has is ignored; at most 64 waiting candidates per connection.
- **A new code ends everything the old code started.** **New code** and **Remove** drop every connection that authenticated with the old code and was not admitted yet.
- **A new code also forgets viewers who are not connected.** Their rejoin token stops working; they are told so when they try to return, and join again with the new code. This is how the host removes someone who is away.
- **Removing is final.** A removed viewer's rejoin attempts are refused with the same message, connections of theirs that are still joining are dropped, and the room an unfinished upload held is freed.
- **Waiting requests are bounded for the session:** at most 50 waiting suggestions and 50 waiting links. Over that, the viewer is told to wait, as with the rate limit.
- **Sequence numbers from the other side are added with a check**, so a value of `u64::MAX` is a protocol error instead of a panic in debug builds.

**Not in this change:**

- who may open a room on the sharing service after the host's connection dropped (change `room-ownership`);
- a rate limit on messages from a viewer, and back-pressure for a viewer that reads slowly (a later change);
- the key exchange that stops offline guessing of a code (PAKE).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-sharing`: revoking the code also ends joins in progress and forgets absent viewers; removing a viewer is final; a connection is one viewer; joining has a deadline and a bound.
- `viewer-requests`: waiting suggestions and links are bounded for the session.
- `session-streaming`: a viewer whose connection dropped reconnects without the code only while the host has not made a new code since; otherwise they are refused and need the new code.

## Impact

- **Code:** `crates/core/src/sharing.rs` (admission, removal, code rotation, `Requests`), `crates/core/src/peer.rs` (the cap on waiting candidates), `crates/core/tests/sharing.rs`. For the checked sequence numbers also `crates/protocol/src/frames.rs` and `crates/protocol/src/viewer_state.rs`.
- **Wire protocol and sharing service:** unchanged. No new message, field or refusal reason; `PROTOCOL_VERSION` stays as it is.
- **Viewers:** a viewer who was away while the host made a new code now sees *The host removed you from the session.* and needs the new code. Before, they came back by themselves.
- **Documentation:**
  - README: no new overview line. This changes what **Remove** and **New code** do, and both are already covered by the *Invite* line and its page.
  - `docs/site/src/content/docs/guide/host.md`: the rows *Stop people from joining with the current code* and *Remove someone*, and the limit on waiting suggestions.
  - `docs/site/src/content/docs/guide/join.md` (*If your connection drops*) and `guide/troubleshooting.md`: what a viewer sees when the code changed while they were away.
  - `docs/site/src/content/docs/security/sharing-service.md`: the paragraph *It works until the host replaces it or ends the session*.
  - `docs/site/src/content/docs/security/known-limits.md`: *Anyone with the code joins* and *Codes do not expire* advise a new code once everyone has joined; both get the sentence that people who are away then need the new code.
  - Video: none. Nothing new appears on screen; the existing videos show neither removal nor a rejoin.
