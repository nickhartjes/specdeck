# Tasks

## 1. Protocol

- [x] 1.1 Add `ErrorCode::ServerBusy` (`"server-busy"`) to `crates/protocol/src/signalling.rs`, with a doc comment, and nothing else in that file. Verify the JSON-shape test gains a line for `{"type":"error","code":"server-busy"}` and that `cargo test --offline -p specdeck-protocol -- --test-threads=3` passes.

## 2. The sharing service (`crates/server`)

- [x] 2.1 Key per-address limits by client network: the IPv4 address, or the `/64` of an IPv6 address (design decision 4). Use it for the existing failed-join limiter too. Verify with a unit test in `lib.rs`:
  - two addresses in one `/64` share a key;
  - two addresses in different `/64`s do not;
  - IPv4 addresses are their own key.
- [x] 2.2 Count connections in total and per client network, check and reserve a slot before the WebSocket upgrade, and release it on disconnect. Over a cap, upgrade, send `server-busy`, and close. Add `max_connections` (2,000) and `max_connections_per_ip` (100) to `Config`. Verify with tests in `crates/server/tests/rooms.rs`, using a small `Config`:
  - the connection over the per-network cap gets `server-busy` and is closed, and a connection from another address is accepted;
  - the total cap works the same way;
  - after one connection closes, a new one from that network is accepted.
- [x] 2.3 Add the room limits from design decision 4:
  - a total room cap (`max_rooms`, 4,000) and 4 rooms per host connection, both answered with `server-busy`;
  - room ids over 64 bytes refused with `bad-message` in `host`, `close-room` and `join`, before any map or rate limiter is touched;
  - 40 viewers per host connection, with a `join` over it answered `server-busy`.

  Verify with tests in `rooms.rs`:
  - a fifth room is refused and the first four stay open;
  - three code rotations in a row (open the new room, close the old one) all succeed;
  - a 65-byte room id is refused and leaves the room count and the per-room limiter empty;
  - the 41st viewer of one host gets `server-busy`;
  - the total room cap refuses the next room.
- [x] 2.4 Give TURN credentials only to members, at most one per connection, with the 2-second grace (design decisions 1 and 2). Verify with tests in `rooms.rs`, using a `TurnConfig` and tokio's paused clock:
  - a host sending `host` then `turn-credentials` gets credentials;
  - a host sending them the other way round (the older app order) also gets credentials;
  - a viewer sending `turn-credentials` and then joining after the credentials (the older order) gets `turn-unavailable` after 2 s;
  - a viewer joining first and then asking gets credentials;
  - asking 3 times gives the same username;
  - a connection that never joins gets `turn-unavailable` and no credentials.

  Update `crates/core/tests/turn_relay.rs` and `crates/core/tests/diagnostics_leak.rs` to the new order where they ask for credentials, and check they still compile with `cargo test --offline -p specdeck-core --no-run`.
- [x] 2.5 Close connections that have not hosted or joined within 30 s (design decision 6). Verify with tests using a paused clock:
  - a silent connection is closed after 30 s;
  - a host that opened a room stays open past 30 s;
  - a viewer that joined stays open past 30 s.
- [x] 2.6 Bound the work one connection can cause (design decisions 4 and 5):
  - add the token bucket to `limits.rs`, kept per sender and viewer connection pair: a burst of 100, then 20 per second, with over-limit `signal`s dropped and answered `rate-limited`;
  - make the outbound queue an `mpsc::channel(256)` that closes the connection when full;
  - set the WebSocket frame size to 64 KiB.

  Verify:
  - with a unit test of the bucket in `limits.rs`;
  - with tests in `rooms.rs`: 500 signals in a burst relay exactly 100 and answer the rest `rate-limited`; a host that never reads while a viewer floods it is disconnected, and its viewer gets `peer-left`; a 100 KiB frame closes the connection.

  Then measure resident memory with 2,000 idle connections against a release build, once with default and once with 8 KiB read and write buffers. Pick the buffers, or a lower default connection cap, so it fits well within 128 Mi. Record the numbers in the PR.
- [x] 2.7 Read `SPECDECK_MAX_CONNECTIONS`, `SPECDECK_MAX_CONNECTIONS_PER_IP` and `SPECDECK_MAX_ROOMS` in `main.rs`, documented in its module comment like the other variables. Log each limit hit as a `warn` with the limit's name and count, at most once per limit per minute, without client addresses (design decision 7). Verify:
  - `cargo test --offline -p specdeck-server -- --test-threads=3` passes;
  - running the binary with `SPECDECK_MAX_CONNECTIONS_PER_IP=1` refuses a second `websocat` connection with `server-busy`.
- [x] 2.8 Document the service's limits in `docs/site/src/content/docs/self-host/configuration.md`: the three new variables with defaults, the fixed limits, and for each, what it protects against (design decision 4's table). Mention the office-NAT case under the per-address limit. Verify `just docs-build` succeeds.

## 3. The TURN relay

- [x] 3.1 Check every new coturn option and each `denied-peer-ip` range syntax against `turnserver --help` in both `coturn/coturn:4.18` (Compose) and `coturn/coturn:4.7` (k8s). Confirm in particular that `max-bps` and `bps-capacity` count bytes per second, and that IPv6 `first-last` ranges are accepted. Record the result in the PR, and verify each image starts with the full option list (`docker run --rm coturn/coturn:<tag> <options> --log-file=stdout` logs no option errors).
- [x] 3.2 Add the quotas and denied ranges from design decision 9 to `deploy/compose/compose.yaml`, with `TURN_TOTAL_QUOTA` (default 100) in `.env.example`, with a comment. No `--bps-capacity` (design decision 9). Verify `docker compose -f deploy/compose/compose.yaml config` renders them with the defaults and with values set.
- [x] 3.3 Add the same options to `deploy/k8s/base/coturn.yaml`, with `--total-quota=20` next to the port range, and say in `deploy/k8s/README.md` to raise them together. Verify `kubectl kustomize deploy/k8s/local` renders, and that `just local-up` brings coturn up with the new arguments (`kubectl logs` shows no option errors).
- [x] 3.4 Add `scripts/turn-denied-peers.mjs` (Node, no dependencies; Rust has no MD5 crate available offline for TURN's long-term credentials), run by `just local-test`. With credentials from the service, or signed with the shared secret, it allocates an IPv4 and an IPv6 relay and asks for permissions to `127.0.0.1`, `10.0.0.1`, `192.168.1.1`, `169.254.169.254`, `::1`, `::ffff:10.0.0.1`, `::a00:1`, `64:ff9b::a00:1`, `2002:a00:1::1`, `fd00::1` and `fe80::1`, checking each is refused (403), while `1.1.1.1` and `2606:4700:4700::1111` are allowed. Verify it passes against a dual-stack coturn 4.7 and 4.18 in Docker, and that `just local-test` passes.
- [x] 3.5 Update the self-host docs:
  - `configuration.md` (coturn section): the quotas, the bandwidth units, and the denied ranges with what they stop;
  - `docker-compose.md`: the new `.env` setting, and raising it with the relay range;
  - `kubernetes.md`: `--total-quota` with the port range;
  - `overview.md`: that the relay's credentials go only to session members.

  Verify `just docs-build` succeeds.

## 4. The apps

- [x] 4.1 Host (`crates/core/src/sharing.rs`, `connect_signalling`): send `turn-credentials` after both `host` messages. Treat `server-busy` like an unreachable service: `SharingEvent::Unavailable` and the existing backoff. Verify `cargo test --offline -p specdeck-core --test sharing -- --test-threads=3` passes, with a new test where a service with `max_rooms` of 1 makes the host report `Unavailable`.
- [x] 4.2 Desktop viewer (`crates/core/src/viewer.rs`, `attempt`): send `join` first, ask for credentials on `joined`, and create the peer once the credentials or `turn-unavailable` arrive. On `server-busy`, end with a new `ViewerEvent::ServiceBusy` before the first admission, and retry after it. Verify in `tests/sharing.rs`:
  - joining still works, with and without TURN configured;
  - a full service (`max_connections_per_ip` of 1, with the host on another address or the limit raised for it) ends a first join with `ServiceBusy`, not `CannotConnect`.
- [x] 4.3 Map `ViewerEvent::ServiceBusy` to `EndReason::ServiceBusy` (`service-busy`) in `app/src-tauri/src/viewer.rs`. In `app/ui` add the type and the text "The sharing service is busy. Try again in a few minutes." with retry in `ViewerScreen.tsx`. Verify with a test in `ViewerScreen.test.tsx`, then `mise exec node@24.22.2 -- pnpm typecheck` and `mise exec node@24.22.2 -- pnpm test`.
- [x] 4.4 Web viewer (`app/ui/src/web/connection.ts`): the same order as 4.2, and end with `service-busy` on `server-busy`. Verify with tests in `connection.test.ts`:
  - `join` is sent before `turn-credentials`;
  - the peer starts only after credentials;
  - `server-busy` ends with `service-busy`.

## 5. Check it end to end

- [x] 5.1 Add an ignored test to `turn_relay.rs`: one host and 20 viewers connect through the local coturn with direct connections disabled, and all 20 data channels open (the per-credential quota refuses none). Verify `just local-test` passes.
- [x] 5.2 In `just local-dev`, with the local service from this change:
  - a host and a viewer from this branch connect through the relay;
  - the latest released app hosts, and a viewer from this branch joins it through the relay;
  - a released viewer joins a host from this branch. It connects directly, and the service logs show its `turn-unavailable` after 2 s.

  Record the results in the PR.

  Results (2026-11-08, driven with Orca; this branch's server run locally on port 18092, because another workspace redeployed the shared cluster; the cluster's coturn as relay; released 1.6.0 with a scratch `HOME`):
  - branch host and branch viewer: credentials after `host`/`join`, relay on both sides, viewer admitted;
  - 1.6 host: TURN credentials 0.5 ms after connecting, through the grace; branch viewer: own credentials after `joined`, relay on both sides. The 1.6/protocol version check then refuses the admission, as it does for any 1.6/`main` pair;
  - 1.6 viewer with a branch host: `turn-unavailable` exactly 2.0 s after connecting, then joined without TURN. It could not connect: on one machine both apps only have private addresses, which the relay refuses to forward to, so neither the host-relay path nor a direct one exists. Explained in the design's risks; on the internet the viewer's public address is reachable through the host's relay. A 1.6 viewer and a host from this change are refused by the version check in any case.
- [x] 5.3 Run `just check`, and `cargo clippy --offline --workspace --all-targets -- -D warnings`. Verify both pass.

## Workflow follow-up

- Release notes: older viewers get no TURN from the new service until they update; self-hosters can set `TURN_TOTAL_QUOTA`.
- Archive the change after review; `sharing-service-limits` becomes a main spec.
