---
title: Configuration reference
description: The SpecDeck server's settings and the coturn options it relies on.
---

## Server

`specdeck-server` reads its settings from the environment:

| Variable | Default | Meaning |
|---|---|---|
| `SPECDECK_LISTEN` | `0.0.0.0:8080` | Address and port to listen on |
| `SPECDECK_TURN_SECRET` | none | coturn's `static-auth-secret`. Without it, the service hands out no TURN credentials and only direct connections work |
| `SPECDECK_TURN_URLS` | none | Comma-separated TURN URLs as the apps reach them, for example `turn:turn.example.com:3478?transport=udp` |
| `SPECDECK_TURN_TTL_SECONDS` | `43200` (12 hours) | How long TURN credentials stay valid. A relayed session keeps using them, so cover a long working day |
| `SPECDECK_TRUSTED_PROXIES` | `0` | How many reverse proxies sit in front of the service. The client address for rate limiting is then read from `X-Forwarded-For`, counting this many entries from the right |
| `SPECDECK_MAX_CONNECTIONS` | `2000` | Open connections in total. See [Limits](#limits) |
| `SPECDECK_MAX_CONNECTIONS_PER_IP` | `100` | Open connections from one client network. See [Limits](#limits) |
| `SPECDECK_MAX_ROOMS` | `4000` | Open rooms in total, rooms kept for a host that is away included. See [Limits](#limits) |
| `SPECDECK_ROOM_HOLD_SECONDS` | `300` (5 minutes) | How long a room is kept for its host after the host's connection ended. `0` keeps no rooms. See [Rooms kept for a host](#rooms-kept-for-a-host) |
| `SPECDECK_LOG_LEVEL` | `info` | `error`, `warn`, `info` or `debug`. When it is unset, a `RUST_LOG` that is a plain level, such as `debug`, is used instead. See [Logs](#logs) |
| `SPECDECK_LOG_FORMAT` | `plain` | `plain` for text lines, `json` for one JSON object per line. See [Logs](#logs) |

It serves two paths:

| Path | What |
|---|---|
| `/signal` | The WebSocket the apps connect to |
| `/health` | Answers `ok`, for health checks |

The service keeps rooms in memory, so run exactly one instance.

On SIGTERM or SIGINT the service stops accepting connections, closes each open one with a WebSocket close, and exits within 5 seconds. The provided Compose file and Kubernetes manifest give it 10 seconds before they kill it (`stop_grace_period`, `terminationGracePeriodSeconds`). Rooms are not kept across a restart: the apps open their rooms again when the service is back.

## Limits

The service bounds everything it holds, so one client cannot use it up for everyone else. When a limit refuses a connection, a room or a join, the apps say the sharing service is busy and keep retrying where they already would. Each limit that is reached is logged as a `limit.reached` warning with its name and count, at most once a minute, without client addresses; see [Logs](#logs).

| Limit | Default | Protects against |
|---|---|---|
| Connections in total (`SPECDECK_MAX_CONNECTIONS`) | 2,000 | Running the service out of memory by opening sockets. 2,000 connections take about 35 MiB |
| Connections per client network (`SPECDECK_MAX_CONNECTIONS_PER_IP`) | 100 | One client taking every connection. A client network is an IPv4 address, or the /64 of an IPv6 address, because one IPv6 client usually holds a whole /64 |
| Rooms in total (`SPECDECK_MAX_ROOMS`) | 4,000 | Running the service out of memory through rooms. Rooms kept for a host that is away count too |
| Room hold time (`SPECDECK_ROOM_HOLD_SECONDS`) | 5 minutes | Someone who knows a code opening its room while the host's connection is down. A host that lost the service also comes back to the same code. See [Rooms kept for a host](#rooms-kept-for-a-host) |
| Rooms per host | 4 | One connection opening unlimited rooms. The app holds at most 3 |
| Room id length | 64 bytes | Huge keys in the service's tables |
| Viewers per host | 40 | Flooding one host with viewers. The app admits at most 20 |
| Viewers waiting per room | 40 | Piling up viewers for a room whose host is away |
| Time to host or join | 30 seconds | Connections that hold a place and do nothing |
| Unresponsive connections | closed after 90 seconds | Connections that are gone, and the places and rooms they hold. The service pings every connection every 30 seconds, and closes one from which nothing arrived for 90 seconds, not even an answer. Browsers and the apps answer pings by themselves |
| Messages per connection | 20 at once, then 2 per second | One connection flooding the service. Connection set-up messages are not counted here; they have their own limit. Extra messages are dropped, and the client is told so once |
| Connection set-up messages | 100 at once, then 20 per second, per viewer connection | Using the service to flood a host or viewer |
| Backlog per connection | 256 messages, 256 KiB | A client that stops reading and makes the service buffer for it; the service closes that connection |
| Failed joins | 10 per client network per minute | Guessing invite codes. Asking to open a room that another connection holds, or that is kept for its host, counts too. Over the limit, the network's joins and requests to open a room are refused, whether the room exists or not |
| Join attempts per room | 60 per minute | Guessing one session's code from many addresses |

Every viewer and every host keeps a connection open for the whole session. If your team shares one office address and more than 100 people host or watch at once, raise `SPECDECK_MAX_CONNECTIONS_PER_IP`.

Limits without a variable are fixed.

The service gives TURN credentials only to a connection that hosts a room or has joined one, and the same credentials each time it asks. Apps from earlier releases ask before they join; the service then waits 2 seconds for the room, so their hosts keep the relay, but their viewers join without it and need a direct connection until they update.

### Rooms kept for a host

When a host's connection to the service ends, for example because the laptop went to sleep or changed networks, the service keeps the host's rooms for the hold time. Only that host can open them again. So nobody who knows the code can take its room while the host is away, and the host returns to the same code.

- **Only the host's app can take a room back.** The app makes a random value for each session, its claim, and sends it with every request to open a room. The service keeps only a SHA-256 hash of it with the room, in memory, and never logs or passes it on. A room that another connection holds, or that is kept, goes to a connection only with the same claim. A room opened without a claim cannot be taken over by anyone.
- **Only rooms opened with a claim are kept.** Current apps send one; apps from earlier releases do not, and their rooms close when their connection ends, as before.
- **Viewers wait.** A viewer who joins a kept room gets no answer until the host returns, and then joins as usual. When the hold time ends first, the service answers as for a wrong code. At most 40 viewers wait for one room.
- **Kept rooms never keep a live host out.** They count toward `SPECDECK_MAX_ROOMS`, but when the service is full, a new room releases the kept room whose hold time ends first.
- **A restart forgets them.** Kept rooms live in memory only.

A shorter hold time frees codes sooner after a host crashed; a longer one covers longer absences. With `0`, rooms close as soon as their host's connection ends, and anyone with the code can open its room again.

## Logs

The service writes its log to standard output, one line per event: `docker compose logs server` or `kubectl logs` shows it. Each line has the time (UTC), the level, the event's name and its fields:

```text
2026-11-09T10:00:00.123456Z  INFO peer.joined room=3 peer=7 viewers_on_host=2
```

With `SPECDECK_LOG_FORMAT=json`, the same event is one JSON object, for a log collector:

```json
{"event":"peer.joined","level":"INFO","peer":7,"room":3,"time":"2026-11-09T10:00:00.123456Z","viewers_on_host":2}
```

The provided Compose file logs plain text and the Kubernetes manifest JSON, both at `info`.

### Events

| Event | Level | Fields | When |
|---|---|---|---|
| `service.started` | info | `version`, `turn`, `web_viewer` | The service starts; whether it hands out TURN credentials and serves the web viewer |
| `service.no_turn` | warn | | No TURN relay is set, so only direct connections work |
| `config.invalid` | warn | `setting` | `SPECDECK_LOG_LEVEL`, `SPECDECK_LOG_FORMAT` or `RUST_LOG` has a value the service does not understand; it uses the default |
| `room.opened` | info | `room`, `rooms_open` | A host opens a room for an invite code |
| `room.closed` | info | `room`, `viewers`, `lasted_s`, `why` | A room closes: `why` is `closed` when the host replaced its code or ended the session, `host_left` when its connection ended and the room was not kept, `hold_expired` when it was kept and the host did not return in time, `evicted` when it was kept and released to make place for a new room. `viewers` joined through it; `lasted_s` is how long it was open |
| `room.away` | info | `room`, `viewers`, `rooms` | A host's connection ended and its room is [kept for it](#rooms-kept-for-a-host). `viewers` joined through it so far; `rooms` are open or kept now |
| `room.reclaimed` | info | `room`, `away_s`, `waiting` | A host takes its room back with its claim. `away_s` is how long it was away, 0 when it took the room from a connection that was still open; `waiting` viewers now join |
| `room.refused` | info | `reason` | A host cannot open a room: `room_taken` (another connection holds it or it is kept, and the claim does not match), `rate_limited` (too many failed attempts from its network), `busy` or `bad_message` |
| `peer.joined` | info | `room`, `peer`, `viewers_on_host` | A viewer joins |
| `peer.waiting` | info | `room`, `waiting` | A viewer joins a room whose host is away, and waits; `waiting` viewers wait for it now |
| `peer.left` | info | `peer`, `lasted_s` | A viewer's connection to the service ends |
| `peer.refused` | info | `reason` | A join or a connection is turned away: `unknown_room` (wrong or old code), `rate_limited` (too many wrong codes), `busy` (a limit), `bad_message` |
| `socket.error` | info | `kind` | A connection fails: `reset` (it broke off), `too_large`, `protocol`, `bad_message` (not a SpecDeck message), `backlog_full` (it stopped reading), `unresponsive` (nothing arrived from it for 90 seconds) |
| `socket.idle_closed` | debug | | A connection that neither hosted nor joined within 30 seconds is closed |
| `limit.reached` | warn | `limit`, `count` | A [limit](#limits) refused something: `connections`, `connections_per_network`, `rooms`, `rooms_per_host`, `viewers_per_host`, `waiting_per_room`, `signals_per_viewer`, `messages` or `backlog` |
| `service.stats` | info | `connections`, `rooms`, `viewers`, `turn_issued` | Every 5 minutes, when something changed: what is open now, kept rooms included, and how many connections got TURN credentials since the last one |
| `service.stopping` | info | `connections`, `rooms` | The service was asked to stop, with this many connections and rooms open or kept |

`room` and `peer` are numbers the service gives out in order from 1 each time it starts, so you can follow one session through the log. They are not the room id or the connection id, and say nothing about them.

**Bursts.** `peer.refused`, `room.refused` and `socket.error` are logged the first time for each reason in a minute. The rest of that minute is counted and logged as one more line with `count`, so a flood of wrong codes or connections cannot flood the log. At `debug`, each one is logged.

**What is never logged**, at any level and in either format: room ids, invite codes, claims or their hashes, connection ids, the messages that set up a connection (session descriptions and ICE candidates), TURN credentials, error text, and client IP addresses. Only the service's own events are written, not those of the libraries it uses. There is no setting to add addresses; if you need them, your reverse proxy's or ingress's access log has them.

**Levels.** `info` logs everything above. `warn` logs only limits, a missing relay and setting problems, so it no longer says when sessions happen. `debug` adds idle connections and logs every refusal and error on its own line.

The service keeps no log files: how long the log is kept is up to Docker, your cluster or your log collector.

## coturn

The server signs TURN credentials with the shared secret (the TURN REST API scheme), so coturn needs:

| Option | Why |
|---|---|
| `--use-auth-secret`, `--static-auth-secret=<secret>` | Accept the credentials the server hands out |
| `--external-ip=<public address>` | The address coturn advertises in relayed addresses |
| `--min-port`, `--max-port` | The relay ports; open the same range in the firewall |
| `--no-tcp`, `--no-tls`, `--no-tcp-relay` | The apps use TURN over UDP only. coturn 4.18 and later leave DTLS off by default and refuse `--no-dtls`; add it only on older versions |
| `--denied-peer-ip=<range>` for each private, loopback, link-local and other special-purpose range, IPv4 and IPv6 | Keeps the relay from reaching into your own network |
| `--user-quota=40` | Allocations per credential. One credential cannot take the whole relay; a host needs one per viewer |
| `--total-quota=<relay ports>` | Allocations in total, as many as there are relay ports, so a full relay says so instead of failing to find a port |
| `--max-bps=2500000` | Bandwidth per allocation: 20 Mbit/s, enough for a late joiner's catch-up |

coturn counts bandwidth in **bytes** per second, despite the option's name: multiply megabits by 125,000. The relay's total bandwidth is at most the allocation quota times 20 Mbit/s; to lower it, shrink the relay range and `--total-quota`. Do not add `--bps-capacity`: coturn reserves the per-allocation rate twice, for each allocation, against it, so 200 Mbit/s would admit only 5 allocations, about two viewers.

The denied ranges cover the IPv6 forms of IPv4 addresses too (IPv4-mapped, NAT64, 6to4 and Teredo). Without them, a relay on a server with IPv6 would reach `10.0.0.1` as `::ffff:10.0.0.1`. No range may start at `::`: coturn then refuses public IPv4 peers as well.

The [Compose file](https://github.com/nickhartjes/specdeck/blob/main/deploy/compose/compose.yaml) and [Kubernetes manifest](https://github.com/nickhartjes/specdeck/blob/main/deploy/k8s/base/coturn.yaml) have the complete list. To check a relay, run `node scripts/turn-denied-peers.mjs wss://<domain>/signal` from a checkout: it asks the relay to reach each kind of address and reports which it refuses.

## TURN URLs

TURN URLs must use `turn:` with `?transport=udp`. The app's WebRTC library has no TURN over TCP or TLS, so `turns:` URLs and TCP transports do not work, and viewers on networks that block UDP entirely cannot connect.
