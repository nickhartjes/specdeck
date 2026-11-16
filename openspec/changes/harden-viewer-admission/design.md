# Design

## Context

See proposal.md for the motivation, and the `session-sharing` and `viewer-requests` deltas for the behavior.

What is there today, all in `crates/core/src/sharing.rs` unless a file is named:

- **A slot per connection.** `ViewerJoined` from the service inserts `Slot { peer, auth, admitted: None, relayed }` into `slots`, keyed by the connection id the service chose. `insert` replaces a slot that is already there, and nothing counts the slots.
- **`auth` is fixed when the slot is made:** `Auth::Code(key)` holds a copy of the code's key at that moment; `Auth::Returning(None)` becomes `Returning(Some(token))` when an offer verifies against one of `identities.keys()`.
- **`handle_viewer` admits on every `Hello`.** It checks the version, the rejoin token, `removed` and the viewer limit, but not `slot.admitted`. Without a token it inserts a new `Identity` under a new random token and overwrites `slot.admitted`, dropping the old `Admitted` and its `upload` without `release_upload`.
- **`rotate_code`** makes a new code and room and closes the old room on the service. It does not look at `slots`.
- **`remove`** returns when the viewer has no admitted slot. Otherwise it adds that slot's token to `removed`, takes it out of `identities`, calls `refuse`, and rotates the code.
- **`refuse`** takes the slot out of `slots` without `drop_slot`, so an upload on its way keeps its reservation in the shared-files store. `Store::release_viewer` (`shared_files.rs`) exists and is never called.
- **`identities` only grows.** A viewer who leaves keeps their entry, so they can return; nothing takes an entry out except `remove`.
- **A rejoin offer signed with a removed token does not verify,** because the token is no longer in `identities`. The slot is dropped without a word, and the viewer's app keeps trying (`crates/core/src/viewer.rs`, `attempt`: `Outcome::Retry`).
- **`Requests.suggestions` and `links`** are `Vec`s without a bound. Rate limits are per viewer id.
- **`Peer::add_remote_candidate`** (`peer.rs`) pushes onto `gathered.pending` without a bound until local gathering is complete.
- **Sequence numbers:** `last_output_seq.map_or(0, |seq| seq + 1)` and `seq as usize + 1` in `handle_viewer`; `frames.rs:200` and `viewer_state.rs:245,258` on the viewer's side.

Constraints:

- No change on the wire. `Hello` and `Welcome` are positional structs, and a host cannot tell an old web viewer from a new one before `Hello`.
- One maintainer: each decision below is the smallest one that closes its gap.

## Goals / Non-Goals

**Goals:**

- After **Remove**, nothing that viewer holds lets them back in: no spare token, no parked connection.
- After **New code**, only people connected at that moment, and people who get the new code, are in the session.
- One connection is one viewer, for limits and quotas too.
- What the host holds for connections, identities and waiting requests has a bound that does not depend on the sharing service being honest.

**Non-Goals:**

- A slot state machine as a type of its own in `crates/protocol`, mirroring `ViewerState`. It would remove this class of bug, and it is the right next step, but it is a refactor of a 2,500-line file and not needed to close these gaps.
- A participant list that shows absent viewers, or a refusal reason of its own for "the code changed". Both need a new message or field.
- Splitting `sharing.rs` into modules.

## Decisions

### 1. A second `Hello` is malformed

`handle_viewer` starts its `Hello` arm with: if the slot is admitted, `disconnect(connection, LeaveReason::Malformed)`.

- This is the rule the file already has for anything it cannot make sense of.
- No honest app sends two: the native viewer and the web viewer both send `Hello` once per connection, from `ViewerState`.
- It closes four things at once: spare tokens, the rate-limit reset, the quota reset, and the lost upload reservation.

*Alternative:* treat a second `Hello` as a resume of the same identity. Rejected: there is no use for it, and it keeps a replay restart that a viewer can trigger at will. `Resend` already exists for that.

### 2. Two deadlines for an unadmitted slot, checked on the host's own timer

`Slot` gets `deadline: Instant`: 30 seconds from `ViewerJoined`, shortened to 10 seconds from now when the peer reports `Connected`. The host loop already wakes for timers (`asks.deadline()`, the traffic interval); it gets one more arm that sleeps until the earliest deadline among unadmitted slots and drops those that passed it, with `LeaveReason::Failed`.

- 30 seconds is longer than the viewer's own `CONNECT_TIMEOUT` of 20 seconds (`crates/core/src/viewer.rs`, and `CONNECT_TIMEOUT_MS` in `app/ui/src/web/connection.ts`), so a join the host drops has already failed on the viewer's side.
- 10 seconds after `Connected` is generous: `Hello` is the first thing a viewer sends when the control channel opens.

*Alternative:* one deadline of 30 seconds. Rejected: a connected but silent slot is the parked connection of the review, and 30 seconds is long enough to act in.

### 3. At most 40 slots, duplicates ignored, 64 waiting candidates

- `ViewerJoined` returns at once when `slots` already has that connection id, or when `slots.len() >= MAX_SLOTS` (40).
- 40 is what an honest service already allows per host (`MAX_VIEWERS_PER_HOST` in `crates/server/src/lib.rs`): 20 viewers, each of which may be reconnecting once.
- `Peer::add_remote_candidate` drops a candidate when `pending` already holds 64. A real viewer sends about ten.
- With decision 2, a slot that never joins frees its place within 30 seconds, so a service that fills the 40 delays joins but cannot stop them for good. A dishonest service can refuse joins anyway.

### 4. A code change drops what the old code started

`rotate_code` drops every slot with `admitted: None` and `auth` equal to `Auth::Code(old key)`, before it takes the new keys.

- Admitted slots keep `Auth::Code(old key)` too; they are left alone, which is the documented behavior.
- A viewer who was halfway through an honest join when the host pressed **New code** has to join again with the new code. That is what a new code means.

### 5. A code change forgets viewers who are not connected

`rotate_code` also moves every token in `identities` that has no admitted slot into `removed`.

This is how the host removes someone who is away, and it is the choice among three:

| Option | Cost | Why not |
|---|---|---|
| Show absent viewers in the participant list, with **Remove** | A new field in `Participant` or a new message, a protocol version, UI in three places (host, viewer, web viewer) | Too large, and it puts people in the list who are not there |
| Let rejoin tokens expire some minutes after the viewer disconnects | A timer and a setting; a laptop that slept through lunch has to ask for the code | The host still cannot act: they wait for a timer they cannot see |
| **A code change forgets absent viewers** | About ten lines; no new UI | Chosen |

- It gives **New code** one meaning the host can hold in their head: *from now on, only who is here and who I give the new code to.*
- **Remove** rotates the code, so it forgets absent viewers too. A viewer who was reconnecting at that second has to join with the new code. Accepted: removal is rare and deliberate, and one rule is easier to document than two.
- A forgotten viewer who joins again with the new code is a new viewer: a new id, a new name suffix if needed, a fresh rate limit. The service's join limits and decision 8 bound what that is worth.

### 6. Removed and forgotten viewers get an answer, not silence

The rejoin offer check tries the tokens in `removed` as well as those in `identities`. An offer signed with one of them verifies, the connection is established, and `Hello` then reaches the existing `removed.contains(&token)` check and is refused with `RefusalReason::Removed`.

- Today the offer of a removed viewer fails to verify and is dropped, and their app retries until its own time-out. With decision 5, that would happen to honest viewers, so it has to be fixed here.
- The refused viewer receives only `Refused`: `refuse` runs before `Welcome` and before any replay.
- The message they see is the existing *The host removed you from the session.* For a viewer who was only away, *The invite code changed; ask the host for the new one* would be better, but that needs a new `RefusalReason`, which old apps cannot decode. **Left as it is, on purpose; a later protocol version can add the reason.** The documentation says what the message means.
- `removed` gets a bound of 200 tokens, oldest first out. A token that fell out is refused in silence, as today.

### 7. The rejoin room is not rotated on removal

After decisions 1, 5 and 6, a removed viewer holds exactly one token, and it is in `removed`. Knowing the rejoin room id gives them nothing: an offer there has to be signed with a token the host accepts.

- Rotating it would need a message that tells every connected viewer the new room. Old apps and old web viewers would not understand it and would lose the ability to reconnect.
- Whether someone else can *take* the rejoin room on the service while the host is away is a different gap, and belongs to `room-ownership`.

### 8. Removal ends everything of that viewer

`remove` and the forgetting in decision 5 go through one helper, `revoke(token, viewer_id)`:

1. the token moves from `identities` to `removed`;
2. every unadmitted slot with `Auth::Returning(Some(token))` is dropped;
3. `view.release_viewer(viewer_id)` frees every reservation of that viewer.

`refuse` releases the slot's own upload the way `drop_slot` does, so no path out of `slots` skips it.

Their waiting suggestions and links stay, as `viewer-requests` requires (*Removed viewer's suggestions*).

### 9. A bound on identities of viewers who are away

`identities` keeps at most 100 entries without an admitted slot. When a viewer disconnects and that makes 101, the one who has been away longest is forgotten as in decision 5. `Identity` gets `away_since: Option<Instant>`.

- A session has at most 20 viewers; 100 absences without a code change is a script, not a team.
- It bounds the loop in the rejoin offer check, which tries every token.

### 10. Waiting requests: 50 of each, answered as rate-limited

`Requests::accept` answers `Outcome::RateLimited` when the request is a suggestion and 50 suggestions are in state `Waiting`, or a link and 50 links are `Waiting`. The check comes after the per-viewer bucket, so a request over both limits costs the viewer a token as before.

- `RateLimited` is what every released viewer already shows as *wait a moment*. A new outcome would need a protocol version.
- Only `Waiting` counts: kept, sent, shared and declined items were each answered by the host, one click at a time.
- 50 is more than a host reads; the point is that the list cannot grow without end.

### 11. Checked sequence numbers, no spec change

- Host: `last_output_seq` and `last_event_seq` use `checked_add(1)`; `None` is `Malformed`.
- Viewer (`frames.rs`, `viewer_state.rs`): the same, ending in the error those functions already return for a frame out of order.

Release builds wrap today and do no harm, so no behavior a user can see changes; this only stops a debug build from panicking. It has tests but no requirement.

## Risks / Trade-offs

- **An honest viewer who was away during a code change has to ask for the code** → It is the meaning of a new code, the docs say so in the row that describes the button, and they get a clear refusal instead of endless reconnecting.
- **The refusal says "removed" to someone who was only away** → Documented in `guide/troubleshooting.md`; a reason of its own comes with the next protocol version.
- **A 10-second deadline cuts off a viewer on a very slow link between connecting and `Hello`** → `Hello` is under 200 bytes and sent at once; the viewer's app retries by itself, and the diagnostics record the drop as `Failed` before admission.
- **Old web viewers or apps that send `Hello` twice** → None do; checked in `viewer_state.rs` (one `Hello` per `ViewerState::connected`). Task 1.1 keeps a test with the native viewer reconnecting, which sends one `Hello` per connection.
- **The caps are constants, not settings** → They are far from what real sessions reach. A setting can follow if someone needs it.

## Migration Plan

Host-only, no data to migrate. Hosts get it by updating; viewers and services need nothing. Rolling back is installing the previous release.
