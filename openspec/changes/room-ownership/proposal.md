# Proposal

## Why

From the security review of 2026-11-10. A room on the sharing service belongs to whichever connection asked for it first, and the service forgets it the moment that connection drops. Three problems follow:

- **Anyone with the code can take the room.** When the host's connection to the service drops (sleep, Wi-Fi change, a proxy closing a quiet connection), the service frees the host's rooms. A viewer, also one the host removed, can open the code's room first. They hold the code, so people who join later with the shared code or link verify them as the host and see what they choose to show.
- **The host reacts to the wrong thing.** `room-taken` does not say which room. The host answers every `room-taken` by making a new invite code, also when it was the rejoin room that was taken. The code people were given stops working, and the rejoin room is never opened again.
- **The service never notices a dead connection.** It checks a connection once, 30 seconds after it opened. A host that drops off the network without a close keeps its rooms on the service. When it comes back it is refused its own rooms, and replaces its code for nothing. Dead connections also hold connection slots for as long as the proxy in front lets them.

Around these:

- **`host` is a way to probe for rooms.** `host` for an existing room answers `room-taken`, for a missing one something else, and no rate limit counts it. The join limits the security pages describe do not apply.
- **An unresponsive service stalls the session.** The host connects to the service inside its main loop, without a timeout. While the service accepts the connection but does not answer, connected viewers get no output, and Remove and End wait.
- **The service ignores SIGTERM.** Every restart waits for the kill timeout and cuts connections without a close.

The archived `server-limits` change already left room for this: its room limit counts "rooms the service keeps for a host that is away", for a host-reclaim change that was never written. This is that change.

## What Changes

- **A room has an owner.**
  - A host makes one random secret per session, its **claim**, and sends it with every `host` message.
  - The service lets a connection take a room that another connection holds only when it presents the same claim. That covers a host returning while its dead connection is still open.
- **The service keeps a room while its host is away.**
  - When a host's connection drops, its rooms opened with a claim stay reserved, for 5 minutes by default (`SPECDECK_ROOM_HOLD_SECONDS`; 0 turns it off).
  - Only the claim opens a reserved room again. Without it, `host` is answered `room-taken`.
  - A viewer who joins a reserved room waits on the service. When the host returns, the join goes through as usual. When the reservation ends first, the viewer gets `room-not-found`. No viewer app changes: released viewers already wait for `joined` and retry.
  - Reserved rooms count toward `SPECDECK_MAX_ROOMS`. When the service is full, a new room replaces the oldest reservation, so reservations never keep a live host out.
- **`room-taken` names the room**, and the host acts on it:
  - **the code's room is taken by someone else:** the host makes a new code, and the host's window says why;
  - **the rejoin room is taken:** the code stays as it is, and the host keeps asking for its rejoin room;
  - **an older service, which names no room:** the host matches the answer to its `host` messages in the order it sent them.
- **The service closes connections that are gone.** It pings every connection every 30 seconds and closes one from which nothing arrived for 90 seconds. Browsers and the apps answer pings by themselves, so released apps and the web viewer need no change.
- **Probing costs attempts.**
  - A `host` for a room someone else holds counts against the network's failed-join limit; over it, `host` and `join` are answered `rate-limited`.
  - Every connection gets a rate limit on all its messages, next to the existing one for set-up messages.
- **The host connects to the service on the side.** Connecting runs outside the session's main loop, with a 10-second limit. Viewers already connected keep getting output whatever the service does.
- **The service stops gracefully on SIGTERM and SIGINT**: it stops accepting, closes its connections with a close frame, and exits within 5 seconds. Reservations are in memory and do not survive a restart.

**Compatibility**, both ways, no breaking change:

- **Older host, new service:** a `host` without a claim opens a room as today. That room is not reserved when the host drops, and nobody can take it over.
- **New host, older service:** the service ignores the claim and names no room in `room-taken`. The host behaves as today, except that it no longer replaces its code because of the rejoin room.
- **Viewers, app and web, any release:** unchanged.

**Out of scope:**

- **The PAKE.** The claim is random and unrelated to the invite code, so it gives the service nothing to test codes against, and the PAKE can be added later without touching it.
- **A stolen code after the reservation ends.** Someone who has the code can still open its room once the host has been away for longer than the reservation. The host now notices on return and replaces the code, but people who use the old code meanwhile reach that other session. Closing this needs a host identity that is not derived from the code. It goes on the known-limits list.
- **Telling connected viewers a new rejoin room.** That needs a new session-protocol message.
- **More than one replica of the service**, and the web viewer's memory use per download.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-sharing`: the host comes back to its own code after losing the service; it replaces the code only when someone else holds the code's room, and says so; an unresponsive service does not stall connected viewers.
- `sharing-service-limits`: rooms have an owner and are kept while the host is away; unresponsive connections are closed; all messages are rate-limited and room probing counts as failed joins; reservations yield to live rooms; the hold time is a setting; the service stops gracefully.
- `sharing-service-logging`: events for rooms going away, being taken back and released, viewers waiting, unresponsive connections, and the service stopping.

## Impact

- **Protocol (`crates/protocol/src/signalling.rs`):** `host` gains an optional `claim`; `error` gains an optional `room`. Both are additive JSON fields. `docs`: `crates/protocol/README.md` if it lists the messages.
- **Service (`crates/server`):** room ownership and reservations in `lib.rs`; the liveness ping in the connection task; a message bucket in `limits.rs`; new events in `log.rs`; `SPECDECK_ROOM_HOLD_SECONDS` and signal handling in `main.rs`. Tests in `crates/server/tests/rooms.rs` and `logging.rs`.
- **Host (`crates/core/src/sharing.rs`, `signalling_client.rs`):** the claim, the pending-`host` queue, the split between code room and rejoin room, a new sharing event, and connecting in a task of its own. Tests in `crates/core/tests/sharing.rs`.
- **App (`crates/session`, `app/src-tauri`, `app/ui`):** the notice in the host's window that the code was replaced and why, passed on like the other sharing events.
- **Deployments:** `SPECDECK_ROOM_HOLD_SECONDS` in `deploy/compose` and `deploy/k8s/base/server.yaml` comments; a termination grace period that matches the 5 seconds.
- **Documentation.** This is not a new feature to show, so no README overview line and no video: a host sees one new notice, and the rest is the service behaving better.
  - `docs/site` `self-host/configuration.md`: the new setting under Limits, the liveness and message limits, and the new events and reasons under Logs.
  - `docs/site` `security/sharing-service.md`: who can open a room, what the service keeps while a host is away, and that probing with `host` is limited.
  - `docs/site` `security/known-limits.md`: a new item for a code used by someone else after the reservation ends.
  - `docs/site` `guide/host.md`: the "code was replaced" notice, next to New code.
  - `docs/site` `guide/troubleshooting.md`: a viewer who joins while the host is away waits instead of seeing "session ended".
