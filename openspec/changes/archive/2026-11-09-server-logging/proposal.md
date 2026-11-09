# Proposal

## Why

The sharing service (`crates/server`) logs almost nothing: that it is listening, that no TURN relay is set, and at most once a minute which limit was reached (issue #96). A self-hoster cannot see rooms opening, viewers joining or being turned away, or connections failing. With `deploy/compose`, `docker compose logs` is all they have, so a team that says "joining doesn't work" gets no answer from the service's side. The log has to say what the service does without becoming a record of who joined which session.

## What Changes

- The service logs a fixed set of events, each with a fixed name and typed fields:
  - `service.started`: its version, and whether TURN and the web viewer are on
  - `room.opened` and `room.closed { viewers, lasted_s, why }`
  - `room.refused { reason }`, when a host cannot open a room
  - `peer.joined`, `peer.left { lasted_s }` and `peer.refused { reason }`
  - `socket.error { kind }`
  - `limit.reached { limit, count }`, which replaces today's `limit reached`
  - `service.stats { connections, rooms, viewers, turn_issued }` every 5 minutes, only when something changed
- Rooms and viewers are named in the log by a sequence number that only this run of the service uses, never by room id or connection id. Events never hold room ids, invite codes, connection ids, set-up messages (SDP and ICE candidates), TURN credentials, error text, or client IP addresses, at any level. There is no setting to add addresses: operators who need them have their reverse proxy's or ingress's access log.
- TURN credentials are counted in `service.stats`, not logged one by one.
- Refusals and socket errors are logged right away the first time for each reason, then counted for the rest of the minute and logged as one line with a `count`. A flood of connections or wrong codes cannot flood the log.
- `SPECDECK_LOG_FORMAT=json` writes one JSON object per line. Otherwise, and by default, the log is plain text.
- `SPECDECK_LOG_LEVEL` (`error`, `warn`, `info` or `debug`; default `info`) chooses the level. When it is unset, `RUST_LOG` is still read if it is a plain level, so existing setups keep working. Only the service's own events are written. Library events (axum, hyper, tungstenite) are never written, because at verbose levels they can carry addresses and message content.
- Compose sets `SPECDECK_LOG_LEVEL=info`. Kubernetes sets `SPECDECK_LOG_LEVEL=info` and `SPECDECK_LOG_FORMAT=json`, for the cluster's log collector. The image no longer sets `RUST_LOG`.
- **BREAKING** for anyone who filters the log by its text: the `limit reached` message becomes the `limit.reached` event, and the `sharing service listening on …` line becomes `service.started`.
- Docs: the security pages stop saying the log holds only the start and limits. They say what it holds now, and that it never holds room ids, codes, addresses or set-up messages. The self-host pages list the events, the two settings, and how to read the log in Compose and Kubernetes.

Out of scope: a metrics endpoint, log files or retention inside the service (it writes to stdout; the container runtime keeps the log), coturn's own log, and the apps' diagnostic log (`diagnostics`, worked on separately).

## Capabilities

### New Capabilities

- `sharing-service-logging`: what the sharing service logs, in which format and at which level; what it never logs; and how its log is kept bounded.

### Modified Capabilities

None. `sharing-service-limits` keeps its requirements: the limits behave as before, and only how they are logged changes. `security-documentation` already requires saying what the service sees and logs; the pages' content changes, not the requirement.

## Impact

- `crates/server`: a new logging module (events, plain and JSON output, level, throttling), with `lib.rs` and `main.rs` emitting through it. No new dependencies: JSON output is written with `serde_json`, which the crate already uses, because tracing-subscriber's `json` feature needs a crate that is not available offline.
- `crates/server/tests`: a test that runs a host and a viewer through the service and checks the events and that no marker or address appears.
- `deploy/compose/compose.yaml`, `deploy/k8s/base/server.yaml`, `deploy/k8s/README.md` and `docker/server.Dockerfile`: the log settings.
- Docs: `self-host/configuration.md`, `docker-compose.md`, `kubernetes.md`, `overview.md`, and `security/sharing-service.md`, `overview.mdx` and `known-limits.md`.
- Privacy: the log on any service, the built-in one included, now records when sessions happen and how many viewers each has, without saying which session or who joined. The security pages say so, and, as for the relay's log, that how long the built-in service keeps it is not stated yet.
