# Design

## Context

See proposal.md for the motivation, and the `sharing-service-limits` delta for the behavior.

What is there today:

- **`crates/server/src/lib.rs`**
  - One `Mutex<Inner>` holds `rooms: HashMap<room, host connection>` and `connections: HashMap<id, Connection { tx, ip, role }>`.
  - `Role` is `New`, `Host { rooms, viewers }` or `Viewer { host }`.
  - `tx` is an `mpsc::unbounded_channel`, drained by a writer task per WebSocket.
- **Limits that exist:**

  | Limit | What it stops |
  |---|---|
  | 64 KiB per WebSocket message (`max_message_size`) | one huge message filling memory |
  | 10 failed joins per client IP per minute | guessing codes from one address |
  | 60 join attempts per room per minute | guessing one room's code from many addresses |
  | Rate-limiter maps pruned above 10,000 keys | the limiter itself growing without bound |
  | Pod memory limits (server 128 Mi, coturn 256 Mi) | one pod starving the node. A limit hit still kills every session on the pod |
  | 20 relay ports (k8s), 100 (Compose) | an implicit allocation cap. When reached, everyone fails |
  | `--no-tcp-relay`, `--no-multicast-peers`, 7 denied IPv4 ranges, `--allowed-peer-ip=$(POD_IP)` (k8s) | relaying into private IPv4 networks |

- **`ClientMessage::TurnCredentials` is answered for any connection** (`Service::handle`), with a new random username each time.
- **Every client asks for credentials before it is a member:**
  - the host sends `turn-credentials` then `host` (`crates/core/src/sharing.rs`, `connect_signalling`);
  - the desktop viewer (`crates/core/src/viewer.rs`, `attempt`) and the web viewer (`app/ui/src/web/connection.ts`) send `turn-credentials` on connect, and send `join` only after the credentials or `turn-unavailable` arrive.
- **Connections stay open:**
  - viewers keep their signalling connection for the whole session;
  - the host sends keepalive pings every 30 s;
  - the host holds at most 3 rooms at once: the rejoin room, the new code's room, and the old code's room until its `close-room` arrives.
- **Released apps skip a server message they cannot parse** (`signalling_client.rs`), so an unknown `ErrorCode` is ignored, not fatal.
- **The parked `harden-session-protocol` change:**
  - adds a `claim` to `host`, `ErrorCode::HostAway`, and a 60-second reservation of an away host's rooms;
  - adds one `pake` signal each way per join.

## Goals / Non-Goals

**Goals:**

- Every resource the service holds is bounded by a limit, and every limit has a stated attack or failure it stops (decision 9).
- Relay credentials are tied to a connection that counts toward the limits, so their number is bounded too.
- Released apps keep hosting with TURN against the new service.

**Non-Goals:**

- Making credentials impossible to get for someone willing to open a room. Anyone can host a room. Membership turns "unlimited credentials from one message" into "one credential per counted connection". The quotas bound what each credential can do.
- Per-user accounts or authentication on the service.
- Persisting limits or counters across restarts.
- Metrics endpoints. Limits are logged (decision 7); a metrics endpoint is a later change if needed.

## Decisions

### 1. Membership and one credential per connection

- **A connection is a member** while its role is `Host` (it holds a room) or `Viewer` (it has joined one).
- `Connection` gets:
  - `turn: Option<ServerMessage>`, the credential issued to it;
  - `turn_wanted: bool`.
- **On `turn-credentials`:**
  - a member gets its stored credential, or a newly issued one that is then stored;
  - a `New` connection gets `turn_wanted = true`, and a 2-second grace timer starts (decision 2);
  - with no TURN configured, the answer is `turn-unavailable` at once, as today.
- **`open_room` and `join` send the credential** when `turn_wanted` is set, and clear the flag.
- **The username stays `<expiry>:<random>`.** Each connection has its own random part, so coturn's `user-quota` counts allocations per connection. The secret and TTL are unchanged.

**The new app order:**

- **Host:** `host` (code room), `host` (rejoin room), then `turn-credentials`. Viewers only arrive after `viewer-joined`, by which time the credential has long arrived.
- **Desktop viewer:** `join`, wait for `joined`, then `turn-credentials`. The peer and offer are created when the credential or `turn-unavailable` arrives, instead of on `joined`. That adds one round trip to a join.
- **Web viewer:** the same order, in `connection.ts`.

New apps against an older service still work, because the older service answers anyone.

- **Alternative: give credentials with `room-opened` and `joined` automatically.** That saves a message, but older apps would get credentials they never asked for and ignore them, and the grace would still be needed.
- **Alternative: give hosts credentials only once a viewer has joined.** An attacker can join their own room, so this adds no protection, and it delays every first viewer.

### 2. The grace for older apps

The connection task spawns `sleep(2 s)`. It then calls `Service::credential_grace_over(id)`, which answers `turn-unavailable` if the connection is still `New` with `turn_wanted` set.

- **Older hosts** send `host` right behind their request, so they become members within milliseconds and get credentials.
- **Older viewers** wait 2 s, get `turn-unavailable`, then join as they do against a service without TURN. On networks that need the relay, they fail with "network blocked" until they update.

The 2 s is well below the 20 s connect timeout and longer than any round trip.

- **Alternative: refuse at once.** Older hosts would lose TURN for all their viewers.
- **Alternative: hold until membership with no deadline.** Older viewers wait for credentials before joining, so they would never join.

### 3. `server-busy` and where it is sent

`ErrorCode::ServerBusy` (`"server-busy"`) is added to `crates/protocol/src/signalling.rs`. It is a small addition, with a line in the JSON-shape test.

| Refused | When | The connection |
|---|---|---|
| a new connection | at the total or per-address connection cap | upgraded, sent `server-busy`, and closed; it never enters `connections` |
| `host` | at the per-host-connection room cap, or the total room cap | stays open, keeps its existing rooms |
| `join` | when the host connection has its maximum number of viewers | stays open |

The service checks and reserves a connection slot before the upgrade, and releases it on disconnect, so concurrent upgrades cannot overshoot the caps.

**In the apps:**

- **Desktop viewer:**
  - before its first admission: `ViewerEvent::ServiceBusy`, which becomes `EndReason::ServiceBusy` (`service-busy`), shown as "The sharing service is busy. Try again in a few minutes.", with retry;
  - after admission: `Outcome::Retry` with the existing backoff.
- **Host:** treats `server-busy` like an unreachable service: `SharingEvent::Unavailable` and the existing backoff. The host's existing "sharing unavailable" text stays.
- **Web viewer:** ends with `service-busy`, with the same text.

- **Alternative: reply HTTP 503 instead of upgrading.** The apps would see a failed handshake and report "service unavailable". Upgrading costs one WebSocket handshake, which is bounded by the per-address cap.

### 4. Limits on the service and their defaults

| Limit | Default | Setting | Where it is checked | Stops |
|---|---|---|---|---|
| Connections in total | 2,000 | `SPECDECK_MAX_CONNECTIONS` | before upgrade | memory exhaustion by opening sockets |
| Connections per client network | 100 | `SPECDECK_MAX_CONNECTIONS_PER_IP` | before upgrade | one client taking all slots. 100 fits an office NAT watching one host |
| Rooms in total, reserved ones included | 4,000 | `SPECDECK_MAX_ROOMS` | `open_room` | memory exhaustion through rooms |
| Rooms per host connection | 4 | constant | `open_room` | one connection opening unlimited rooms. Apps hold at most 3 |
| Room id length | 64 bytes | constant | `host`, `close-room`, `join` (`bad-message`) | 64 KiB keys in the room map and the per-room rate limiter. Today's ids are 32 hex characters; PAKE's are 7 |
| Viewers per host connection | 40 | constant | `join` | flooding one host with viewer slots. The app's limit is 20, and the margin covers rejoins |
| Idle deadline before host or join | 30 s | constant | connection task | sockets that hold a slot and do nothing |
| `signal` relays per sender and viewer connection | burst of 100, then 20 per second | constant | `relay` (`rate-limited`, once per run of dropped messages) | using the service to flood a host or viewer, which costs CPU and queue space on both |
| Outbound queue per connection | 256 messages and 256 KiB | constant | `Inner::send` | a client that stops reading, growing server memory (decision 5) |
| WebSocket frame size | 64 KiB (as the message size) | constant | upgrade | today's default frame size of 16 MiB being buffered before the message check |

**Client network:** an IPv4 address, or the `/64` of an IPv6 address. One IPv6 client typically owns a whole `/64`, so a per-address limit on IPv6 would be no limit at all. The existing failed-join limiter gets the same key.

**The signal rate:** a token bucket per sender and viewer connection, the pair a `signal` names. It lives in `limits.rs` next to `RateLimiter`, and is stored on the sender's `Connection`, so it goes away with it.

- A join uses an offer, an answer and some tens of candidates, well inside the burst.
- A host with 20 viewers joining at once has 20 pairs, each with its own bucket, so it is never throttled by the number of its viewers.
- A host has at most 40 pairs (the viewer cap), so its buckets are bounded too.
- A flood is answered with `rate-limited` once, until a message gets through again. Answering every dropped message would make the service reply to a flood with a flood, and fill the flooder's own backlog.

**Memory:** each connection costs its socket buffers, its queue and a map entry. Measured with a release build on macOS, 2,000 idle connections that each host a room (task 2.6):

| WebSocket read and write buffers | Resident memory |
|---|---|
| axum's default, 128 KiB each | 271 MiB |
| 8 KiB each (chosen) | 32–36 MiB |

The service starts at about 3 MiB. With 8 KiB buffers, the default cap fits well within the k8s limit of 128 Mi, and the memory limit stays as it is. A message longer than the buffer simply takes more reads.

- **Alternative: make every limit a setting.** Most limits follow from the protocol and the app, not from deployment size. Only the three that scale with a team's size get settings.

### 5. A bounded outbound queue that disconnects

`Connection.tx` becomes an outbox: an `mpsc::Sender<String>` with capacity 256, and a count of the bytes queued. Counting only messages would still allow 256 relayed messages of up to 64 KiB each, 16 MiB per connection.

- `Inner::send` encodes the message, adds its length to the count, and uses `try_send`. The writer subtracts the length once the message is written.
- **Over 256 KiB queued, or the channel `Full`:** the service records the connection for closing and signals the connection task through a `oneshot` kept in `Connection`. The task closes the socket and runs the usual `disconnect`, so rooms, viewers and `peer-left` are handled as for any drop.
- **`Closed`:** ignored, as today.

Dropping single messages instead would leave a host or viewer half-set-up in ways neither side can detect. Closing is a state both sides already handle.

### 6. Idle deadline

The connection task selects on `sleep(30 s)` while the connection is `New`. When the sleep fires, it asks `Service::is_member(id)`, and closes the socket if not. Once a connection is a member, no deadline applies. Hosts send keepalives, and the service keeps both sides open as today.

### 7. Logging when a limit is hit

Each limit logs a `warn` with its name and current count. The log is rate-limited to once per limit per minute, so a flood does not become a log flood. Client addresses are not logged, which keeps the service's footprint as described in the self-host overview.

### 8. Fit with `harden-session-protocol`

- **Reserved rooms count toward `SPECDECK_MAX_ROOMS`.** They do not count toward a connection's room cap, because they have no connection.
- **A reclaiming host** takes its rooms back through `open_room`. The per-connection cap counts them on the new connection, and the claim check comes before the cap.
- **`host-away`** and **`server-busy`** are separate codes. A viewer retries on the first and reports the second.
- **The two `pake` signals per join** are far below the signal rate.
- Both changes add a variant to `ErrorCode` and edit `open_room`. Whichever lands second rebases. The changes do not conflict in behavior.

### 9. coturn

Both deployments (`deploy/compose/compose.yaml` and `deploy/k8s/base/coturn.yaml`) get the same options:

| Option | Value | Stops |
|---|---|---|
| `--user-quota` | 40 | one credential opening unlimited allocations. A host needs one per viewer (20), plus allocations that linger after viewers leave |
| `--total-quota` | relay port count (Compose: `TURN_TOTAL_QUOTA`, default 100; k8s: 20) | exhausting the relay. Over the quota, coturn answers "quota reached" instead of failing to find a port |
| `--max-bps` | 2500000 | one allocation taking the whole uplink. coturn counts bytes per second, so this is 20 Mbit/s |
| `--denied-peer-ip` | the IPv6 ranges in the proposal, in coturn's `first-last` form (none may start at `::`, see below), plus `192.0.0.0/24`, `192.0.2.0/24`, `198.18.0.0/15`, `198.51.100.0/24`, `203.0.113.0/24` and `224.0.0.0`–`255.255.255.255` | relaying into private and special-purpose networks over IPv6, or through IPv6 forms of IPv4 addresses (mapped, NAT64, 6to4, Teredo) |

- **k8s `--allowed-peer-ip=$(POD_IP)`** stays; allow rules take precedence over deny rules in coturn.
- **Compose and k8s use different coturn images** (4.18 and 4.7). Task 3.1 checked every option against both:
  - `--user-quota`, `--total-quota`, `--max-bps` and `--bps-capacity` exist in both, and both count bandwidth in bytes per second, per TURN session (allocation) for `max-bps`.
  - **`--bps-capacity` is not used.** It is an admission limit: coturn reserves `max-bps` for input and output of every allocation against it, and refuses allocations with 486 once it is used up. With 20 Mbit/s and 200 Mbit/s, the sixth allocation was refused (coturn 4.7 in Docker), which would have been about two viewers on the whole relay. The total bandwidth is bounded instead by `--total-quota` × `--max-bps`, and by shrinking the relay range (decided 2026-11-08, replacing the 200 Mbit/s total).
  - **Alternatives:** derive `bps-capacity` from the quota (`total-quota` × 2 × `max-bps`), which never refuses before the quota and so only adds a setting to keep in step; or lower `max-bps` to 1 Mbit/s so 100 allocations fit in 200 Mbit/s, which makes catch-up and file transfers over the relay about 20× slower.
  - Both accept IPv6 `first-last` ranges. `scripts/turn-denied-peers.mjs`, run against a dual-stack coturn of each version, shows every denied address refused with 403 and public IPv4 and IPv6 peers allowed.
  - **A range that starts at `::` makes coturn refuse public IPv4 peers too** (`::-::ffff:ffff:ffff` blocked `1.1.1.1`). The list therefore starts at `::1`, and covers mapped addresses with their own range. coturn already refuses `::1` without a list, but not `::ffff:10.0.0.1`, which the IPv4 list alone let through.
  - **coturn 4.18 no longer accepts `--no-dtls`** and exits; `--no-cli` only logs an error. DTLS and the CLI are off by default there. The Compose file, which pins 4.18 since 1.5.0, still passes both. That is fixed in a separate hotfix, not in this change.

- **Alternative: stop coturn from relaying over IPv6 at all** (`--relay-ip` set to the IPv4 address). The apps only advertise IPv4 today, so it would work. But with network mode host, the address is the server's own, and it differs per deployment. The deny list protects the same way without per-server settings.

## Risks / Trade-offs

- **[Older viewers lose TURN against the new service]** → A relayed connection needs a relay on one side only. An older viewer still reaches the host's relay address, and coturn forwards to the viewer's public (server-reflexive) address, also behind strict NATs. It only fails when the viewer's network blocks UDP to the relay, where credentials would not help either. The case is narrow besides:
  - a 1.6 viewer and a host from this change are refused by the version check anyway;
  - the case that remains is an older viewer and an older host on an updated service, and the older host still gets TURN through the grace (decision 2);
  - auto-update moves most apps to the new order, and the PAKE change moves everyone to protocol 3.

  The release notes say so. A test on one machine cannot show the host-relay path: both apps only have private addresses, which the deny list refuses to relay to (task 5.2).
- **[Older apps ignore `server-busy`, so their attempt times out with "network blocked"]** → It only happens at capacity, and the service logs it.
- **[One office behind one IPv4 address with more than 100 connections]** → `SPECDECK_MAX_CONNECTIONS_PER_IP` raises it. The docs name that case.
- **[`user-quota` too low when viewers churn quickly]** → 40 leaves twice the viewer limit. The relay test (task 5.1) runs 20 viewers through one host.
- **[`max-bps` slows a late joiner's catch-up on the relay]** → 20 Mbit/s matches the replay estimate in `docs/replay-measurements.md`. Direct connections are not affected.
- **[Anyone can still get a credential by opening a room]** → Stated as a non-goal. Each credential is bounded by `user-quota`, `max-bps` and the connection caps, and is counted in the per-network limit.
- **[A bounded queue disconnects a legitimate but slow client]** → 256 messages is far beyond a join's burst. Clients reconnect with backoff, as after any drop.

## Migration Plan

1. Release the service and the apps together, as usual. The service accepts both old and new app orders.
2. Self-hosters on Compose add `TURN_TOTAL_QUOTA` to `.env` if their relay range differs from the default. Without it, the default applies.
3. Kubernetes users with a wider relay range raise `--total-quota` with `--min-port`/`--max-port`.
4. **Rollback:** the previous image and manifests. Apps from this change keep working against the older service (decision 1).
