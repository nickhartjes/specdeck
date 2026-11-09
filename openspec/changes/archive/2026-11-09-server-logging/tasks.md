# Tasks

Run Rust through `cargo --offline` and tests with `-- --test-threads=3`.

## 1. Log module and output

- [x] 1.1 Add `crates/server/src/log.rs` with the event enums (`Refusal`, `RoomRefusal`, `SocketError`, `ClosedWhy`, `Limit`), their snake_case `Display`, and one function per event from design decision 3, taking only numbers, booleans, `Duration`s and those enums; verify with unit tests that each enum's `Display` is a fixed word and `lasted_s` rounds down
- [x] 1.2 Add `log::settings(level, rust_log, format)` (design decision 7) returning the level, the format and any warning to log; verify with unit tests: default `info`, `SPECDECK_LOG_LEVEL` wins over `RUST_LOG`, a plain `RUST_LOG=debug` is used, `RUST_LOG=hyper=trace` and an invalid level fall back to `info` with a warning, `trace` maps to `debug`, unknown format falls back to plain with a warning
- [x] 1.3 Add the plain and JSON layers and `log::init`/a test constructor taking a `MakeWriter`, with a filter that passes only the exact `specdeck_server` target (design decision 6); verify with unit tests that a JSON line parses with `time` (RFC 3339 UTC), `level`, `event` and numeric fields, that a plain line has the event name and `key=value` fields, and that an event from another target is not written at `debug`
- [x] 1.4 Add the `Throttle` (design decision 4) with an explicit `now`; verify with unit tests: first occurrence logged at once, later ones counted, the count flushed with `count=n` on the next occurrence after the minute or on `flush`, no count line when nothing was counted, and every occurrence logged at `debug`
- [x] 1.5 Move `limit_reached`'s once-a-minute rule into `log.rs` as `limit.reached`, and add the static test that no file under `crates/server/src` other than `log.rs` uses `tracing`; verify `cargo test --offline -p specdeck-server` passes with the test in place

## 2. Events in the service

- [x] 2.1 Number rooms and viewers in `Inner` (design decision 2): `rooms` holds a `Room { host, no, opened, joined }`, `Role::Viewer` holds its peer number, join time and room number; verify the existing `crates/server/tests/rooms.rs` still passes unchanged
- [x] 2.2 Emit `room.opened`, `room.closed` (`closed` and `host_left`), `room.refused`, `peer.joined`, `peer.left` and `peer.refused` from `open_room`, `close_room`, `join`, `disconnect` and `upgrade`, through the throttle for refusals; verify with a test in `crates/server/tests/logging.rs` that captures the log (current-thread runtime, `set_default`) and checks the session-from-start-to-end, code-replacement, wrong-code, rate-limited and full-service scenarios from the spec
- [x] 2.3 Classify socket errors in `handle` and `send` (design decision 3) and emit `socket.error` and `socket.idle_closed`; add `tungstenite = "0.29"` as a direct dependency and check `Cargo.lock` gains no package; verify with a unit test of the mapping (`Capacity` → `too_large`, `Protocol` → `protocol`, I/O reset → `reset`) and an integration test that an oversized message and an unparseable message log `too_large` and `bad_message`
- [x] 2.4 Add `Config::stats_interval` (default 5 minutes), the `turn_issued` counter in `give_turn`, and the stats task spawned by `serve` that flushes the throttle and logs `service.stats` only on change (design decision 5); verify with an integration test at a short interval: stats after a session with the right counts and 1 credential, then no further stats line while idle
- [x] 2.5 Add the leak test from the spec: a host and a viewer at `debug`, in both formats, with markers in the room id, the SDP, an ICE candidate and the TURN secret, plus a wrong code; assert no marker, no connection id and no `127.0.0.1` in the captured log; verify it passes and fails if a marker is deliberately logged
- [x] 2.6 Switch `main.rs` to `log::settings` and `log::init`, emit `service.started` and `service.no_turn`, and list `SPECDECK_LOG_LEVEL` and `SPECDECK_LOG_FORMAT` in its module comment; verify by running `cargo run --offline -p specdeck-server` with `SPECDECK_LOG_FORMAT=json` and seeing a JSON `service.started` line, and with `RUST_LOG=debug` alone seeing debug events

## 3. Deployments

- [x] 3.1 Remove `RUST_LOG=info` from `docker/server.Dockerfile`'s `ENV`; verify with `grep RUST_LOG docker/server.Dockerfile` returning nothing
- [x] 3.2 Replace `RUST_LOG` with `SPECDECK_LOG_LEVEL: info` in `deploy/compose/compose.yaml`; verify with `docker compose -f deploy/compose/compose.yaml config` (with dummy `.env` values) showing the variable
- [x] 3.3 Set `SPECDECK_LOG_LEVEL=info` and `SPECDECK_LOG_FORMAT=json` in `deploy/k8s/base/server.yaml` and mention both in `deploy/k8s/README.md`; verify with `kubectl kustomize deploy/k8s/local` rendering both, and, if the local cluster is up (`just local-dev`), that `kubectl logs` shows a JSON `peer.joined` after a viewer joins

## 4. Docs

- [x] 4.1 In `docs/site/src/content/docs/self-host/configuration.md`, replace the `RUST_LOG` row with `SPECDECK_LOG_LEVEL` and `SPECDECK_LOG_FORMAT`, add a "Logs" section listing each event with its level and fields, what is never logged, the throttling and the stats line, and update the sentence on how limits are logged; verify the docs site builds (`mise exec node@24.22.2 -- pnpm --dir docs/site build`)
- [x] 4.2 In `docker-compose.md`, show `docker compose logs -f server` with an example line, and in `kubernetes.md` show `kubectl logs` with a JSON line and a `jq` filter; link both to the Logs section; verify the build and links
- [x] 4.3 In `docs/site/src/content/docs/security/sharing-service.md`, replace "Its log holds only that it started and …" with what the log holds and never holds, and that how long the built-in service keeps it is not stated yet; update the server sentence in `security/overview.mdx` and `self-host/overview.md` to match; verify each page against the spec's "Logging documented for security reviewers" scenario
- [x] 4.4 In `security/known-limits.md`, extend "The relay logs addresses" (or add a sibling entry) so it says the server's log records when sessions happen and how many viewers they have, without ids or addresses, with retention on the built-in service not stated; verify the build

## 5. Integration check

- [x] 5.1 Run `cargo clippy --offline -p specdeck-server --all-targets` and `cargo test --offline -p specdeck-server --features web-viewer -- --test-threads=3`; verify both pass
- [x] 5.2 Run the service locally, connect a host and a viewer from two apps (or the test clients), try a wrong code, and read the log in plain text and JSON; verify every spec scenario's events appear and no id or address does
