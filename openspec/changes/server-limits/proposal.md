# Proposal

## Why

Step 3 of the security plan agreed on 2026-11-03. The sharing service and its TURN relay have almost no limits:

- **Anyone gets TURN credentials.** A fresh WebSocket that sends `turn-credentials` gets a new 12-hour credential, as often as it likes. With a script, our relay becomes a free, anonymous UDP proxy.
- **The server's memory has no bounds.** Nothing limits:
  - the number of connections, in total or per address;
  - the number of rooms a connection opens;
  - how long a connection may stay open without hosting or joining;
  - how fast a connection relays `signal` messages.

  Each connection's outbound queue is also unbounded. A single client can grow the server until the pod is OOM-killed, which ends every session on it.
- **coturn has no quotas.** One credential can open any number of allocations, and one relayed session can use the whole uplink.
- **coturn denies private IPv4 ranges, but no IPv6 ones.** On a host with IPv6, a relayed client can reach `::1`, the local IPv6 network, link-local addresses, and IPv4 loopback and private addresses through their IPv4-mapped forms (`::ffff:127.0.0.1`).

The data-transparency docs (step 5) will describe the service as hardened. It has to be, first.

## What Changes

- **TURN credentials only for session members.**
  - The service gives credentials only to a connection that holds a room (a host) or has joined one (a viewer), and at most one credential per connection. Asking again returns the same credential.
  - Hosts ask after opening their rooms, and viewers after `joined`: the desktop app and the web viewer both.
  - **Older apps keep working.** The service holds a request from a connection that is not a member yet for up to 2 seconds:
    - if the connection opens or joins a room in that time, it gets its credential;
    - otherwise the service answers `turn-unavailable`.

    Old hosts, which send `host` right behind their request, keep TURN. Old viewers then join without TURN until they update.
- **Capacity limits on the service, each with a default and a `SPECDECK_*` variable to change it:**
  - connections in total and per client address;
  - rooms in total (including rooms the host-reclaim change will keep for an away host);
  - rooms per host connection;
  - viewers per host connection;
  - the length of a room id.
- **Abuse limits on each connection:**
  - a connection that has not hosted or joined within 30 seconds is closed;
  - `signal` messages are rate-limited per connection;
  - the outbound queue is bounded, and a client that does not read its messages is disconnected instead of growing memory.
- **A new `server-busy` error.** The service sends it when a capacity limit refuses a connection, room or join.
  - A viewer joining for the first time sees "The sharing service is busy" with a way to try again.
  - An admitted viewer, and a host, retry with their existing backoff, as when the service is unreachable.
  - Older apps skip the unknown code. Their attempt times out instead.
- **coturn quotas:**
  - allocations per credential (`user-quota`) and in total (`total-quota`);
  - 20 Mbit/s per allocation (`max-bps`). The total is bounded by the allocation quota times that rate. coturn's own total (`bps-capacity`) is not used: it reserves the per-allocation rate twice for every allocation, so 200 Mbit/s would admit only 5 allocations (design decision 9).

  These apply to both the Compose and the Kubernetes deployment.
- **coturn denies IPv6 special-purpose ranges** next to the IPv4 ones:
  - loopback, IPv4-compatible and IPv4-mapped (`::1` to `::ffff:ffff`, and `::ffff:0:0/96`);
  - NAT64 (`64:ff9b::/32`);
  - discard-only (`100::/64`);
  - Teredo and other IETF protocol ranges (`2001::/23`);
  - documentation (`2001:db8::/32`);
  - 6to4 (`2002::/16`);
  - unique local (`fc00::/7`);
  - link-local and site-local (`fe80::/9`);
  - multicast (`ff00::/8`).

  The IPv4 list also gains the remaining special-purpose ranges (`192.0.0.0/24`, the documentation and benchmarking ranges, and `224.0.0.0` and up).
- **Docs:** the self-host pages list the new settings, the coturn options, and what each limit protects against.
- **Not in this change**, each a later change of its own:
  - TURN over TLS on 443;
  - the PAKE and the host's room reservation (`harden-session-protocol`, parked on `worktree-harden-session-protocol`). This change is written to fit it, see design decision 8;
  - the app-side hardening.

## Capabilities

### New Capabilities

- `sharing-service-limits`: what the sharing service and its TURN relay refuse, and why:
  - who gets relay credentials;
  - capacity and abuse limits, and what apps show when the service is full;
  - the relay's quotas and the addresses it never relays to;
  - the settings self-hosters use to change the limits.

### Modified Capabilities

None. `session-sharing` keeps its requirements; joining, hosting and remote connectivity work as before for members.

## Impact

- `crates/server`:
  - `lib.rs`: membership, the credential grace, caps, the idle deadline, the signal rate, and a bounded outbound queue;
  - `limits.rs`: a token bucket next to the sliding window;
  - `turn.rs`: one credential per connection;
  - `main.rs`: the new `SPECDECK_*` variables.
- `crates/protocol/src/signalling.rs`: a small addition, `ErrorCode::ServerBusy`.
- `crates/core/src/viewer.rs` and `sharing.rs`: the order of the credential request, and handling `server-busy`. Small changes.
- `app/src-tauri/src/viewer.rs` and `app/ui`: one new end reason (`service-busy`) with its text. Small additions.
- `app/ui/src/web/connection.ts`: the same order and `server-busy` handling for the web viewer.
- `deploy/compose/compose.yaml`, `.env.example`, `deploy/k8s/base/coturn.yaml`, `deploy/k8s/README.md`: coturn quotas and denied ranges.
- `docs/site/src/content/docs/self-host/`: configuration, overview, Docker Compose and Kubernetes pages.
- Compatibility:
  - new apps work with older services, which answer credentials to anyone;
  - older apps work with the new service through the grace, but their viewers lose TURN.
