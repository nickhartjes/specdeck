# Tasks

Write each task's tests first and see them fail. Rust tests run with `cargo test --offline -p <crate> -- --test-threads=3`.

## 1. Protocol

- [x] 1.1 In `crates/protocol/src/signalling.rs`, add `claim: Option<String>` to `ClientMessage::Host` and `room: Option<String>` to `ServerMessage::Error`, both left out of the JSON when `None` and defaulting to `None` when absent. Update every place that builds or matches them (`crates/server`, `crates/core`, `crates/web` if it names them, and the hand-written `ServerMessage` type in `app/ui/src/web/connection.ts`). Verify with the JSON-shape test:
  - `{"type":"host","room":"r"}` decodes with no claim, and encodes back without a `claim` key;
  - `{"type":"host","room":"r","claim":"ab"}` round-trips;
  - `{"type":"error","code":"room-taken"}` decodes with no room, and `{"type":"error","code":"room-taken","room":"r"}` round-trips;
  - a `host` message with an extra unknown field still decodes (pins design decision 11).

## 2. The sharing service: ownership and reservations (`crates/server`)

- [x] 2.1 Room ownership (design decisions 1 and 2): keep the SHA-256 of the claim with the room, compare in constant time, refuse a claim over 64 bytes as `bad-message`, and move a held room to a connection that presents its claim. `room-taken` carries the room. Verify with tests in `crates/server/tests/rooms.rs`:
  - a second connection with the same claim gets `room-opened`, and a viewer who joins afterwards reaches the second connection, not the first;
  - a second connection with no claim, and one with another claim, get `room-taken` with the room's id;
  - a room opened without a claim cannot be taken with any claim;
  - a viewer that was in set-up with the first connection gets `peer-left` when the room moves;
  - a 65-byte claim is refused and leaves no room.
- [x] 2.2 Reservations (design decision 3): on disconnect, keep rooms that have a claim for `Config::room_hold` (5 minutes; 0 turns it off); remove them on `close-room`; expire them from the `report` task on a 5-second tick. Verify with tests in `rooms.rs` on tokio's paused clock:
  - the host returns after 30 seconds with its claim and gets `room-opened`; the room keeps its log number;
  - another connection without the claim gets `room-taken` while the room is reserved;
  - after the hold time, any connection can open the room;
  - a host that sent `close-room` before leaving reserves nothing, and a join gets `room-not-found`;
  - with `room_hold` 0, and for a room without a claim, the room is gone at once.
- [x] 2.3 Waiting viewers (design decision 4): park a `join` for a reserved room, at most 40 per room, and finish it when the host returns or the reservation ends. Verify with tests in `rooms.rs`:
  - a viewer that joins a reserved room gets nothing at first, then `joined`, and the host gets `viewer-joined`, when the host returns;
  - a waiting viewer that asked for relay credentials gets them after it is joined;
  - when the hold time ends, the waiting viewer gets `room-not-found`;
  - the 41st waiting viewer gets `server-busy`;
  - a waiting viewer that disconnected is not joined, and the host gets no `viewer-joined` for it;
  - the joins-per-room and failed-join limits apply to joins for a reserved room as for an open one.
- [x] 2.4 Reservations yield to live rooms (design decision 3): at the room total, opening a room removes the reservation with the earliest deadline; with no reservation left, `server-busy` as today. Verify with tests in `rooms.rs` with a small `max_rooms`: both cases, and that the evicted room's waiting viewer gets `room-not-found`.
- [x] 2.5 Read `SPECDECK_ROOM_HOLD_SECONDS` in `crates/server/src/main.rs`. Verify with a unit test of the parsing next to the existing settings (a number, 0, and not a number falling back to the default).

## 3. The sharing service: liveness, rates, probing, stopping (`crates/server`)

- [x] 3.1 Liveness (design decision 5): ping every connection every 30 seconds through its writer, and close one from which no frame arrived for 90 seconds, with `socket.error` `unresponsive`. Make both intervals `Config` fields so tests can shorten them. Verify with tests in `rooms.rs`:
  - a raw socket that never reads or answers is closed after the deadline, and its connection slot and rooms are handled as for any disconnect (a room with a claim becomes reserved);
  - a client that only answers pings stays open for many intervals;
  - `signalling_client::connect` from `crates/core` (as a dev-dependency test, or in `crates/core/tests/sharing.rs`) stays connected to a service with 1-second pings for 10 seconds without sending anything itself.
- [x] 3.2 Message rate (design decision 6): one bucket per connection for every message but `signal`, 20 at once then 2 a second; drop over the limit, answer `rate-limited` once per run, and report `limit.reached` `messages`. Verify with tests in `rooms.rs`:
  - a connection that sends 100 `host` messages at once has at most 20 acted on and gets one `rate-limited`;
  - a host that connects and replaces its code three times in a row is never limited;
  - `signal` messages are not counted against this bucket.
- [x] 3.3 Probing (design decision 7): record every `room-taken` as a failed join for the network, and answer `host` with `rate-limited` while the network is over the limit. Verify with tests in `rooms.rs`:
  - after 10 `room-taken` answers from one network in a minute, `host` for an existing room and for a missing one both get `rate-limited`, and so does `join`;
  - a host on another network opens a room meanwhile;
  - a host that takes its room back with its claim records no failed join.
- [ ] 3.4 Graceful stop (design decision 10): `serve` takes a shutdown future; connection tasks send a close frame on it; `main.rs` waits for SIGTERM or SIGINT, logs `service.stopping`, and exits within 5 seconds. Verify with a test in `rooms.rs` that triggers the shutdown with a host and a viewer connected: both receive a close frame and `serve` returns within 5 seconds. Then check by hand that `docker stop` on the built image returns in under 5 seconds.

## 4. The sharing service: logging (`crates/server/src/log.rs`)

- [x] 4.1 Add the events and reasons of design decision 12: `room.away`, `room.reclaimed`, `peer.waiting`, `service.stopping`; `room.closed` reasons `hold_expired` and `evicted`; `socket.error` kind `unresponsive`; limits `messages` and `waiting_per_room`. Count reserved rooms as rooms in the load summary. Verify with tests in `crates/server/tests/logging.rs` that follow the scenarios in the `sharing-service-logging` delta, one per scenario.
- [x] 4.2 Extend the marker test in `logging.rs` ("Markers do not leak"): open a room with a claim that contains a known marker, let the host go away and return, and let a viewer wait. Verify that neither the claim, the room id nor a connection id appears, in plain text or JSON, at the debug level.

## 5. The host (`crates/core`)

- [ ] 5.1 The claim (design decision 1): make 32 random bytes per session and send them hex-encoded with every `host`, also after a new code and after reconnecting. Verify with a test in `crates/core/tests/sharing.rs` against the real service: drop the host's service connection, let it reconnect, and check that the invite code is unchanged, a new viewer joins with it, and a viewer that lost its connection rejoins.
- [ ] 5.2 The pending-`host` queue and the split between rooms (design decision 8). Verify with unit tests of `handle_server` in `sharing.rs`, or tests in `crates/core/tests/sharing.rs` with a scripted service:
  - `room-taken` naming the code room: a new code, `CodeReplaced { reason: InUse }`, then `Ready` with the new code;
  - `room-taken` naming the rejoin room: the code is unchanged, no `CodeReplaced`, and `host` for the rejoin room is sent again after 30 seconds;
  - `room-taken` without a room, as the second answer after `room-opened` for the code room: treated as the rejoin room;
  - `room-taken` without a room, as the first answer: treated as the code room;
  - `room-taken` for an old code's room: ignored.
- [ ] 5.3 Add `SharingEvent::CodeReplaced` and a diagnostic event for a rejoin room that is taken (fixed fields only, in `crates/core/src/diag.rs`). Verify that `crates/core/tests/diagnostics_leak.rs` still passes with the new event exercised.
- [ ] 5.4 Connect off the main loop (design decision 9): spawn the connection attempt under a 10-second timeout and select on its result. Verify with tests in `crates/core/tests/sharing.rs` on the paused clock, with a listener that accepts TCP and never answers the WebSocket handshake:
  - a connected viewer keeps receiving terminal output while the attempt hangs;
  - `Remove` and `Close` take effect while the attempt hangs;
  - `Unavailable` is sent after 10 seconds, and the next attempt follows the usual backoff.
- [ ] 5.5 Against an older service: run the tests of 5.1 and 5.2 against a service built without this change's server side, or a scripted one that ignores `claim` and names no room. Verify that the host opens its rooms and that no code is replaced because of the rejoin room.

## 6. The app: the notice (`crates/session`, `app/src-tauri`, `app/ui`)

- [ ] 6.1 Pass `CodeReplaced` from the session process to the app as a notice, next to the other sharing events in `crates/session/src/messages.rs` and `runtime.rs`, and add it to `app/ui/src/api/types.ts`. An app of the previous release drops it. Verify with a test in `crates/session/tests/process.rs` that the notice arrives, and that the transcript tests in `app/src-tauri/src/adapter.rs` still pass.
- [ ] 6.2 Show the notice in the host's window where sharing problems are shown, with the text of design decision 8, until the host dismisses it; announce it to screen readers as the other notices are. Add it to `app/ui/src/api/demo.ts`. Verify with a test next to `HostScreen` that the notice appears on the event, names no code, and goes away when dismissed, and that the axe pass stays clean.

## 7. Deployments

- [x] 7.1 Add `SPECDECK_ROOM_HOLD_SECONDS` as a commented setting to `deploy/compose/compose.yaml` and `deploy/k8s/base/server.yaml`, and set a termination grace period of 10 seconds for the server in both. Verify with `docker compose config` and `kubectl kustomize deploy/k8s/base`.

## 8. Documentation

No README overview line and no video: there is no new feature to use or show, only one notice and a service that behaves better (proposal, Impact).

- [x] 8.1 `docs/site` `self-host/configuration.md`: `SPECDECK_ROOM_HOLD_SECONDS` under Limits with what it protects against; the liveness rule and the message limit; the new events, reasons and limit names under Logs. Verify that every event and reason in `crates/server/src/log.rs` is in the page.
- [x] 8.2 `docs/site` `security/sharing-service.md`: only the host that opened a room can open it again while the service holds or keeps it; what the service keeps while a host is away (a hash of a random value, in memory, for the hold time); probing with room requests is limited like wrong codes. Correct "Guessing on the service is limited" accordingly.
- [x] 8.3 `docs/site` `security/known-limits.md`: add "A code can be used by someone else after the host was away": what it means, that the host's app replaces the code and says so, and what to do (make a new code after a long absence; end sessions that are done).
- [x] 8.4 `docs/site` `guide/host.md`: the "code was replaced" notice, next to New code. `guide/troubleshooting.md`: a viewer who joins or rejoins while the host is away keeps connecting and gets in when the host is back.
- [x] 8.5 `crates/protocol/README.md`: the `claim` and `room` fields, if the page lists the service messages.
- [x] 8.6 Run `just docs-build` and see it pass.

## 9. Whole-system check

- [ ] 9.1 Run `just check` and see it pass.
- [ ] 9.2 By hand, with a local service and a host and a viewer app: start a session, stop the host's network for a minute (or pause the service's port), bring it back, and check that the code is the same, a second viewer joins with it, and the first viewer, disconnected meanwhile, rejoins. Then restart the service and check that the host comes back with the same code.
- [ ] 9.3 By hand, with a released app of the previous version as host against the new service, and the new app against a service of the previous version: host, join, drop the host's connection and return. Check that both work as design decision 11 says.
