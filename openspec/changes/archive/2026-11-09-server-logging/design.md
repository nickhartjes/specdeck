# Design

## Context

`crates/server` has three `tracing` calls: the start line and the missing-TURN warning in `main.rs`, and `limit_reached` in `lib.rs`, which already throttles to once a minute per limit and leaves addresses out. `main.rs` installs `tracing_subscriber::fmt()` with `EnvFilter::from_default_env()`, so `RUST_LOG` decides everything, library crates included. Compose, Kubernetes and the Dockerfile set `RUST_LOG=info`.

The state the events need is already in `Inner`: `rooms` (room id → host), `connections` with their `Role`, and the counters behind the limits. All of it changes under one `Mutex`, in `open_room`, `close_room`, `join`, `disconnect`, `reserve` and `send`.

The app solves the same problem in `crates/core/src/diag.rs`: one module owns every `tracing` call, each event is a function that takes only numbers, enums and booleans, and a test fails if any other file names `tracing`. The server follows the same pattern, but stays separate from `specdeck-diag`, which pulls in `specdeck-core` and libdatachannel.

tracing-subscriber's `json` feature needs `tracing-serde`, which is not in the offline cargo cache, so JSON has to be formatted here. `serde_json` and `tracing-subscriber` (with `env-filter`, which also provides `registry`) are already dependencies.

## Goals / Non-Goals

**Goals:**
- One place that decides what can be logged, and makes leaking an id, an address or SDP a type error, not something a review has to catch.
- Plain text and JSON from the same events, with no new crates.
- Tests that need neither a real clock nor a global subscriber.

**Non-Goals:**
- Changing any limit, protocol message or client-visible behaviour.
- Writing log files, rotating them, or sending them anywhere: the service writes to stdout.
- Tracing spans or request ids per HTTP request.

## Decisions

### 1. A `log` module owns every event

`crates/server/src/log.rs` has one function per event (`room_opened(room: RoomNo, rooms_open: usize)`, `peer_refused(reason: Refusal)`, `socket_error(kind: SocketError)`, …). Arguments are `u64` numbers, `Duration`s and `Copy` enums whose `Display` is a fixed snake_case word. No function takes a `&str`, a `String`, an `IpAddr` or an error. The event name is the `tracing` message, and all events use one target, `specdeck_server`.

A unit test, like `only_this_module_uses_tracing` in core, fails when any other file under `crates/server/src` contains `tracing::` or a `tracing` macro import. `main.rs` reports startup through `log::started(…)` and `log::no_turn()`.

*Alternative:* `tracing` calls spread through `lib.rs` with a code review rule. Rejected: the current `limit_reached` comment ("Client addresses are not logged") is exactly that kind of rule, and nothing tests it.

### 2. Numbers instead of ids

`Inner` gets two counters, `next_room` and `next_peer`, starting at 1. Each open room keeps its number, its open time and how many viewers joined through it, so `rooms` becomes `HashMap<String, Room { host, no, opened, joined }>`. A viewer's `Role::Viewer` keeps its peer number, its join time and its room's number. Host connections are not numbered: a host is seen through its rooms.

The numbers are `u64` and are only ever compared within one run. They are not derived from the ids, so they say nothing about them.

*Alternative:* a keyed hash of the room id, with a key that changes on each restart. Rejected: it gives nothing a counter doesn't, and it invites the question of whether the key can leak.

### 3. Where events are emitted

All under the existing lock, from the code paths that already decide the outcome:

| Event | Level | Emitted in | Fields |
|---|---|---|---|
| `service.started` | info | `main` | `version`, `turn`, `web_viewer` (booleans) |
| `service.no_turn` | warn | `main` | none |
| `room.opened` | info | `open_room`, only for a room that is new | `room`, `rooms_open` |
| `room.closed` | info | `close_room` (`why=closed`), `disconnect` of a host (`why=host_left`) | `room`, `viewers` (joined through it), `lasted_s` |
| `room.refused` | info | `open_room` errors | `reason`: `room_taken`, `busy`, `bad_message` |
| `peer.joined` | info | `join` | `room`, `peer`, `viewers_on_host` |
| `peer.left` | info | `disconnect` of a viewer: its own connection ended (a viewer whose host left stays connected until it leaves too) | `peer`, `lasted_s` |
| `peer.refused` | info | `join` errors; `upgrade` when no `Slot` is free | `reason`: `unknown_room`, `rate_limited`, `busy`, `bad_message` |
| `socket.error` | info | `handle` (read error, oversized message, unparseable message), `send` (backlog full) | `kind`: `reset`, `too_large`, `protocol`, `bad_message`, `backlog_full` |
| `socket.idle_closed` | debug | `handle`, idle timeout | none |
| `limit.reached` | warn | `limit_reached`, unchanged throttle | `limit`: `connections`, `connections_per_network`, `rooms`, `rooms_per_host`, `viewers_per_host`, `signals_per_viewer`, `backlog`; `count` |
| `config.invalid` | warn | `main`, after `init` | `setting`: the variable's name |
| `service.stats` | info | stats task | `connections`, `rooms`, `viewers`, `turn_issued` |

`lasted_s` is whole seconds, rounded down. A `Host` for a room the same connection already holds is not an event. A `close_room` for an unknown room is not logged; it is answered as before.

Socket errors are sorted by kind without reading their text: axum's `Error::into_inner()` is walked through `source()` for a `tungstenite::Error` (`Capacity` → `too_large`, `Protocol` → `protocol`, `Io`/`ConnectionClosed` → `reset`) or an `io::Error` (`reset`), and anything else is `reset`. `tungstenite` is already in the dependency tree through axum, so the crate adds it as a direct dependency at the version axum uses (`tungstenite = "0.29"`, matching axum 0.8.9's tokio-tungstenite 0.29), and the lockfile gains no new package. If the downcast proves brittle across versions, the fallback is to treat every read error as `reset` and keep `too_large` from the message size check.

### 4. Throttling bursts

A `Throttle` in `log.rs` keys on (event, reason) and holds, per key, the start of its current minute and a count. It takes `now: Instant` as a parameter, like `RateLimiter`, so tests drive it without sleeping:

- the first occurrence in a minute is logged at once and opens the window;
- later ones within the window only increase the count;
- the next occurrence after the window, or the report task's tick (decision 5), flushes a pending count as the same event with `count=n`, then starts a new window.

So a count is never lost, but it can arrive late: up to about two minutes after the burst starts, since the report task ticks once a minute. At `debug`, the throttle is bypassed and every occurrence is logged. `limit.reached` keeps its own once-a-minute rule; it moves from `Inner::limit_reached` into `log.rs` as well.

*Alternative:* flush exactly at the end of each minute with a timer per key. Rejected: one task already wakes regularly, and a count that comes a minute late is fine for a log.

### 5. Stats task

`serve` spawns one report task that ticks every `Config::stats_interval` or every minute, whichever is shorter (default 5 minutes; tests set it in milliseconds), and holds only a `Weak` to the service, so it ends with it. On each tick, under the lock, it flushes the throttle. Once `stats_interval` has passed, it also takes `turn_issued` (a counter `give_turn` increases the first time a connection gets credentials) and compares `(connections, rooms, viewers, turn_issued)` with the last values it logged. It logs `service.stats` only when they differ, starting from all zeros, so a service no client ever reached logs no summary. "viewers" is the number of connections with `Role::Viewer`.

### 6. Format and output

`log::init(settings)` builds `Registry` + a filter that passes events whose target is exactly `specdeck_server` at the chosen level, and nothing else, + one of two layers, both writing to stdout. (`Targets` matches by prefix, so it would also let in a crate named, say, `specdeck_server_extra`.)

- **plain**: `tracing_subscriber::fmt::layer()` with ANSI off, target off, so a line reads `2026-11-09T10:00:00.123Z  INFO peer.joined room=3 peer=7 viewers_on_host=2`.
- **json**: a small `Layer` that visits the event's fields into a `serde_json::Map`, adds `time` (RFC 3339 UTC, from tracing-subscriber's own timer), `level` and `event` (the message), and writes the line through the `MakeWriter`. Keys come out in alphabetical order, since `serde_json` is built without `preserve_order`. It reuses the visitor approach of `specdeck-diag`'s `JsonVisitor`, about 60 lines. Numbers and booleans become JSON numbers and booleans; `Display` enums become strings.

Both layers take a `MakeWriter`, so tests pass a shared buffer and install the subscriber with `tracing::subscriber::set_default` on a current-thread runtime, where the spawned server tasks run on the same thread and see it. No global subscriber is needed.

Since only the `specdeck_server` target passes the filter (every event in `log.rs` names it explicitly), library events never reach either layer, whatever the level. That keeps tungstenite's and hyper's trace output, which can hold frames and peer addresses, out by design.

### 7. Choosing the level and format

`main` reads `SPECDECK_LOG_LEVEL`. If that is unset, it reads `RUST_LOG` and uses it only if the whole value parses as a plain level (`error`, `warn`, `info`, `debug`, `trace`; `trace` is treated as `debug`, since the service has no trace events). A directive such as `specdeck_server=debug,hyper=trace` is not a plain level, so the default applies with a warning. An invalid `SPECDECK_LOG_LEVEL` also falls back to `info`, and the warning is logged after `init`, so it is the first line. `SPECDECK_LOG_FORMAT` is `json` or `plain` (default); anything else is plain with a warning. The parsing is a pure function, `log::settings(level: Option<&str>, rust_log: Option<&str>, format: Option<&str>)`, so it is tested without touching the environment.

### 8. Deployments

- `docker/server.Dockerfile`: drop `RUST_LOG=info` from `ENV`. The default is `info` now, and keeping `RUST_LOG` in the image would hide a missing `SPECDECK_LOG_LEVEL` in the docs' examples.
- `deploy/compose/compose.yaml`: `SPECDECK_LOG_LEVEL: info` in place of `RUST_LOG`, plain format, because `docker compose logs` is read by people.
- `deploy/k8s/base/server.yaml`: `SPECDECK_LOG_LEVEL=info` and `SPECDECK_LOG_FORMAT=json`, because cluster log collectors (Loki, Elastic, Cloud Logging) parse JSON lines.
- The `main.rs` module comment lists both variables, like the other settings.

## Risks / Trade-offs

- [The log now shows when sessions happen and how big they are, on every service, the built-in one included] → No ids, so a line cannot be tied to a code or a person by the log alone. The security pages say so, together with the retention still being unstated. An operator who wants less sets `SPECDECK_LOG_LEVEL=warn`.
- [Timing correlation: someone with both the server's log and the relay's log (which has addresses) could match a `peer.joined` time to a relay allocation] → That operator already sees addresses and room ids in memory, so this gives them nothing new. The docs already say the relay logs addresses.
- [Throttled counts arrive up to 5 minutes late] → Acceptable for a log. At `debug` there is no delay.
- [Renaming `limit reached` breaks text alerts built on it] → Listed as breaking in the proposal and called out in the release notes. The new name is stable from now on.
- [Socket-error kinds depend on downcasting axum's error to tungstenite's] → A unit test pins the mapping. If the downcast stops matching after an axum upgrade, everything shows as `reset` and the test fails, rather than leaking text.
- [Another emission point added later could take a `String`] → The static test keeps `tracing` inside `log.rs`, and `log.rs` functions take no text.

## Migration Plan

Nothing to migrate: the service keeps no state. Deployments that set `RUST_LOG=info` keep working through the fallback. The release notes say to switch to `SPECDECK_LOG_LEVEL`, and that `limit reached` is now `limit.reached`. Rollback is the previous image; its log is just quieter.
