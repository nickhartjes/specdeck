# Design

## Context

See proposal.md for why. What the code does today:

- **Service (`crates/server/src/lib.rs`).**
  - `open_room` gives a room to the first connection that asks; another connection gets `room-taken`. It has no rate limit and does not touch the failed-join limiter.
  - `disconnect` removes all of a host's rooms at once and logs `room.closed` with `host_left`.
  - The connection task checks once, 30 seconds after connecting, that the connection is a member. After that it only reads. It never pings and has no read deadline.
  - Only `signal` messages are rate-limited (100 at once, then 20 a second, per pair).
  - `serve` runs `axum::serve` without a shutdown signal; `main.rs` handles no signals.
- **Protocol (`crates/protocol/src/signalling.rs`).** `ClientMessage` and `ServerMessage` are internally tagged serde enums without `deny_unknown_fields`, so a field a side does not know is ignored. `ServerMessage::Error` carries only `code`.
- **Host (`crates/core/src/sharing.rs`).**
  - `connect_signalling` awaits `signalling_client::connect` inside `Host::run`'s loop, with no timeout. It then sends `host` for the code room, `host` for the rejoin room, and `turn-credentials`.
  - `handle_server` answers any `room-taken` with `rotate_code()`.
  - On a normal end, `end()` sends `close-room` for both rooms before the connection closes.
- **Clients and pings.**
  - The app's `signalling_client` sends a WebSocket ping every 30 seconds on a quiet connection. The host and the desktop viewer both use it.
  - The web viewer (`app/ui/src/web/connection.ts`) sends no pings: a browser's WebSocket cannot. Browsers answer a server's ping by themselves, and so does tungstenite in the apps.
  - Released apps skip a service message they cannot parse (`signalling_client.rs`), and the web viewer ignores error codes it does not know.
- **Viewers and a missing room.** A viewer that was admitted before treats `room-not-found` as the session having ended (`viewer.rs`, `connection.ts`). Today that is what a viewer gets when it rejoins while the host has no connection to the service.
- **Earlier plan.** The parked `harden-session-protocol` change, described in `server-limits` design decision 8, planned a `claim` on `host`, a `host-away` error and a 60-second reservation, together with the PAKE. It was never written. This design keeps the `claim`, and differs on the other two (decisions 3 and 4).

## Goals / Non-Goals

**Goals:**

- Only the host that opened a room can open it again, while the service holds or reserves it.
- A host that loses the service for a few minutes comes back to the same code, and viewers who tried meanwhile get in.
- Nothing changes for released apps, the web viewer, or services that are not updated yet.
- The service gives no cheaper way to find out whether a room exists than a join.

**Non-Goals:**

- The PAKE, and anything that makes the service learn less about the code than it does today.
- Protecting the code room after the reservation ends (proposal, out of scope).
- Reservations that survive a restart of the service, or more than one replica.
- A new session-protocol message, and with it a protocol version step.

## Decisions

### 1. The claim is a random secret, sent as it is

- The host makes 32 random bytes when the session starts, and sends them hex-encoded as `claim` in every `host` message, for the code room and the rejoin room alike. A new code keeps the same claim.
- The service keeps SHA-256 of the claim with the room and compares in constant time. It never logs the claim or its hash, and never sends either to anyone.
- A claim longer than 64 bytes is refused as `bad-message`, like a long room id.

**Why random, not derived from the code:** anything derived from the code would be one more value to test codes against offline. A random claim tells the service nothing, and the PAKE can replace the room id later without touching it.

**Why not a commitment (send a hash, reveal the secret to reclaim):** it would protect the claim only from someone who reads the host's traffic to the service, which means a `ws://` service. That reader still lacks the code's key, so with a stolen claim they could at most take the room away from the host, which they can also do by cutting the connection. A commitment needs a fresh secret after every reclaim and a second message shape. Not worth it.

### 2. Who gets a room

`open_room(from, room, claim)` decides in this order:

| The room is | `host` carries | Result |
|---|---|---|
| not known | anything | opened for `from`, with the claim if there is one |
| held by `from` | anything | `room-opened` again, as today |
| held by another connection, opened with a claim | the same claim | moved to `from`; the other connection loses it |
| held by another connection | no claim, another claim, or the room has none | `room-taken`, with the room |
| reserved | the same claim | opened for `from`; waiting viewers are joined |
| reserved | no claim or another claim | `room-taken`, with the room |

- The claim check comes before the per-connection room cap, as `server-limits` decision 8 says.
- **Moving a room** takes it out of the old connection's set. Viewers the old connection had in set-up get `peer-left`, as when a host disconnects. The old connection is left to the liveness check (decision 5); the service does not close it, because a host that rotates onto a second connection on purpose would be legal too.
- **Every `room-taken` counts as a failed join** for the client's network, and `host` is refused with `rate-limited` while that network is over the limit (decision 7).

### 3. Reservations

- On `disconnect`, each of the host's rooms that has a claim becomes reserved instead of removed: it keeps its number, its count of joins and its claim, loses its host, and gets a deadline of now plus the hold time. Rooms without a claim are removed as today.
- `close-room` removes a room for good, so a session that ends normally leaves nothing behind (`end()` already sends it for both rooms).
- The hold time is `Config::room_hold`, 5 minutes, from `SPECDECK_ROOM_HOLD_SECONDS`. 0 means no reservations.
- **Expiry** runs in the existing `report` task, which already ticks at least once a minute; it gets a tick of 5 seconds for this. Expired rooms are removed and logged as `room.closed` with `hold_expired`; their waiting viewers get `room-not-found`.
- **When the room total is reached**, `open_room` removes the reservation with the earliest deadline (logged `room.closed`, `evicted`) and opens the new room. Only when no reservation is left does it answer `server-busy`.

**Why 5 minutes, not the 60 seconds of the earlier plan:** the cases that matter are a laptop waking up and a network change, and both often take longer than a minute. A reservation costs a few hundred bytes and can never keep a live room out, so a longer time is cheap. It is still short enough that a code nobody is hosting stops being "taken" soon.

- **Per network**, at most `max_kept_rooms_per_ip` reservations (16, `SPECDECK_MAX_KEPT_ROOMS_PER_IP`): a host from a network over that has its rooms closed when it leaves, as for a room without a claim. And at most `rooms_per_ip` new rooms a minute (30, `SPECDECK_ROOMS_PER_IP`), answered `rate-limited` above it. Opening a room the connection holds, or taking one back with its claim, is not a new room.

**Why the per-network bounds:** eviction takes the earliest deadline, which is the host that went away first. Without a bound, someone who holds a code could fill the room table from one address, so the next room evicts that host's reservation, and then open the code's room: exactly what reservations are for. With the bounds, one network holds at most 4 rooms on each of its 100 connections plus 16 reservations, and opens 30 rooms a minute, far from the 4,000 total; filling the table takes many networks. An office behind one address keeps up to 16 hosts' rooms through an uplink blip, which covers the case the earlier plan worried about.

### 4. Viewers wait for a host that is away

A `join` for a reserved room, after the usual rate limits, is parked:

- the connection is added to the room's waiting list, at most 40 per room (`MAX_VIEWERS_PER_HOST`); over that, `server-busy`;
- it gets no answer yet, and its role stays `New`;
- when the host takes the room back, each waiting connection that is still there and still `New` is joined exactly as `join` does now: `joined` to the viewer, `viewer-joined` to the host, relay credentials if it asked;
- when the reservation ends first, each gets `room-not-found`;
- a waiting connection that disconnects is dropped from the list.

A waiting connection is not a member, so the 30-second idle rule still closes it. The viewer apps give up on an attempt after 20 seconds and try again, so they never reach it.

**Why not the `host-away` error of the earlier plan:** released viewers cannot parse a new error code. The desktop viewer skips the message and the web viewer ignores it, so for them `host-away` and silence are the same, and both end in their 20-second timeout. Waiting lets them succeed as soon as the host is back, needs no change in three viewer implementations, and replaces today's "the session ended" for a viewer that rejoins while the host is away. The cost is that a viewer's window says it is connecting, not that the host is away. A `host-away` notice can be added later for apps that understand it.

### 5. Liveness is the service's job

- The connection task sends a WebSocket ping every 30 seconds through the connection's own writer, and keeps the time of the last frame it read, of any kind.
- When that is 90 seconds ago, it logs `socket.error` with `unresponsive`, closes the socket and runs the usual `disconnect`.
- The outbox channel carries an enum of text or ping instead of a `String`, so pings go through the same bounded queue and writer.

**Why the service pings:** the proposal's scope said "the apps ping every 30 s". The host and the desktop viewer do. The web viewer does not and cannot, so a rule that only counted client frames would cut web viewers off after 90 seconds. Pongs to the service's own pings work for every client, also released ones.

The existing 30-second idle rule for connections that never became members stays as it is.

### 6. Message rate

- Each connection gets one `TokenBucket` for every `ClientMessage` except `signal`, which keeps its own per-pair bucket: 20 at once, then 2 a second.
- A message over the limit is dropped before `handle` acts on it. The sender gets `rate-limited` once per run of dropped messages, as for signals, and `limit.reached` names `messages`.
- A host needs 3 messages to connect and 2 per new code. A viewer needs 2.

### 7. Probing

- `room-taken` records a failed join for the network, in the same `failed_joins` limiter as a `join` for an unknown room.
- `open_room` checks `failed_joins.exceeded` first and answers `rate-limited`, as `join` does.
- So someone asking "does room X exist?" with `host` gets the same 10 *hits* a minute per network as with `join`. A miss opens the room, which counts toward the 30 new rooms a minute per network (decision 3), so misses cost too, at three times the rate of `join`. The message bucket bounds both per connection.
- A released host that returns before the service noticed its dead connection gets two `room-taken` answers per attempt. With liveness that lasts at most 90 seconds, and its backoff is 10 seconds, so it can reach the limit of 10. That is accepted: the limit then also stops its retries from being answered differently, and it clears within a minute.

### 8. `room-taken` names the room, and the host keeps a queue for services that do not

- `ServerMessage::Error` becomes `{ code, room: Option<String> }`, with `room` left out of the JSON when it is `None`. The service sets it for `room-taken` only.
- The host keeps the rooms it asked for and has no answer for yet, in order. `room-opened { room }` removes that room. An error that can answer a `host` (`room-taken`, `server-busy`, `bad-message`, `rate-limited`) removes the named room, or the first in the queue when no room is named. The service answers a connection's messages in order, so the first unanswered `host` is the right one.
- What the host does with `room-taken`:
  - **the current code room:** `rotate_code()`, then a new `SharingEvent::CodeReplaced { reason: InUse }` before the `Ready` with the new code;
  - **the rejoin room:** nothing changes; the host sends `host` for it again every 30 seconds while the connection lasts, and records it in the diagnostic log (a fixed event, no ids);
  - **a room it no longer wants** (an old code): ignored.
- The notice reaches the host's window like the other sharing events: `crates/session` passes it to the app as a notice, and the UI shows it in the place it shows sharing problems, until the host dismisses it. Text: "Someone else was using your invite code on the sharing service, so SpecDeck made a new one. Share the new code; the old code and link may lead to another session."

**Why keep the same rejoin room:** connected viewers were given it in `Welcome`. A new rejoin room would need a new session-protocol message to tell them, and with it a protocol version step, which this change avoids.

### 9. Connecting to the service off the main loop

- `Host::run` no longer awaits the connection. It spawns a task that runs `signalling_client::connect` under `tokio::time::timeout(10 s)` and sends the result through a oneshot. The loop selects on that receiver with its other branches.
- State is `Idle` (waiting for `next_attempt`), `Connecting(receiver)` or `Connected(server_rx)`. A timeout counts as a failed attempt, with the same backoff and the same `Unavailable` event.
- When the session ends while connecting, the task's result is dropped, which closes the socket.

### 10. Graceful stop

- `main.rs` waits for SIGTERM or SIGINT (Ctrl-C only on Windows) and logs `service.stopping` with the open connections and rooms.
- `serve` takes a shutdown future and passes it to `axum::serve(...).with_graceful_shutdown`. The `Service` holds a `Notify` that every connection task selects on; on shutdown each sends a close frame and ends.
- `main` waits at most 5 seconds for the server to finish, then exits.
- Reservations are not written anywhere. After a restart, hosts open their rooms as new, with their claim, which is the "not known" row in decision 2.

### 11. Compatibility rules

| Host | Service | Behaviour |
|---|---|---|
| older (no claim) | new | Rooms open as today, are closed when the host drops, and cannot be taken over. `error.room` is an unknown field the host ignores. Liveness closes its dead connections within 90 seconds, so its own rooms come free sooner than today. |
| new | older | The service ignores `claim` (unknown field in an internally tagged enum without `deny_unknown_fields`; a test pins this). No reservation. `room-taken` has no room, so the host uses its queue. |
| new | new | Everything above. |

- Viewers of any release, app and web: no message they send or receive changes. A new `error.room` field is ignored by serde and by the web viewer's JSON handling.
- No session-protocol or local-protocol version changes. The new sharing event travels from the session process to the app as a new notice kind; an app of the previous release drops a notice it does not know, which only loses the notice.

### 12. New log events

All fields are numbers or fixed words, as `sharing-service-logging` requires. No room id, claim, connection id or address.

| Event | Level | Fields |
|---|---|---|
| `room.away` | info | room number, viewers joined so far, rooms now held |
| `room.reclaimed` | info | room number, whole seconds away (0 when taken from an open connection), viewers that were waiting |
| `peer.waiting` | info | room number, viewers now waiting for it |
| `room.closed` | info | new reasons `hold_expired` and `evicted` |
| `socket.error` | as today | new kind `unresponsive` |
| `limit.reached` | warn | new limit names `messages` and `waiting_per_room` |
| `service.stopping` | info | connections, rooms |

`room.refused` with `room_taken` and `rate_limited` already exists and now also covers refused claims and probing. The load summary counts reserved rooms as rooms.

## Risks / Trade-offs

- **A code holder can still open the room after the reservation ends** → the host replaces its code on return and says why; documented as a known limit; a real fix needs a host identity outside the code.
- **A host that crashed keeps its code "taken" for 5 minutes**, and viewers who join wait instead of hearing that the code is wrong → their apps time out after 20 seconds per attempt and say they cannot connect; after 5 minutes they get the usual answer. A self-hoster can shorten the time.
- **Bulk reservations**: connect, open 4 rooms with a claim, disconnect, repeat → bounded to 16 reservations and 30 new rooms a minute per network (decision 3); filling the table to evict a particular host's reservation takes many networks. On a shared service that is full for real, a reservation can still be evicted early.
- **Liveness depends on clients answering pings** → browsers and tungstenite do; the host and the desktop viewer also send their own pings. A test runs the current app client against the service with short intervals; released apps are covered by the same library behaviour.
- **Released hosts and the failed-join limit** (decision 7) → at most a minute of `rate-limited`, only while their own dead connection lives.
- **Viewers cannot tell "host away" from "connecting"** → accepted for now (decision 4).
- **One mutex, more state under it** → the reservation and waiting lists are touched only in `open_room`, `join`, `disconnect` and the expiry tick, all short.

## Migration Plan

1. Release the service and the apps together as usual; either can go first (decision 11).
2. Self-hosters update the image. Nothing to configure; `SPECDECK_ROOM_HOLD_SECONDS` is optional.
3. Rollback: an older service ignores claims, and new hosts fall back to the queue. No stored state to undo.

## Open Questions

- Whether the built-in service should use a hold time other than the default. It can be decided when deploying.
