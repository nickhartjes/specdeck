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
| `SPECDECK_MAX_ROOMS` | `4000` | Open rooms in total. See [Limits](#limits) |
| `RUST_LOG` | none | Log level, such as `info` or `debug` |

It serves two paths:

| Path | What |
|---|---|
| `/signal` | The WebSocket the apps connect to |
| `/health` | Answers `ok`, for health checks |

The service keeps rooms in memory, so run exactly one instance.

## Limits

The service bounds everything it holds, so one client cannot use it up for everyone else. When a limit refuses a connection, a room or a join, the apps say the sharing service is busy and keep retrying where they already would. Each limit that is reached is logged as a warning with its name and count, at most once a minute, without client addresses.

| Limit | Default | Protects against |
|---|---|---|
| Connections in total (`SPECDECK_MAX_CONNECTIONS`) | 2,000 | Running the service out of memory by opening sockets. 2,000 connections take about 35 MiB |
| Connections per client network (`SPECDECK_MAX_CONNECTIONS_PER_IP`) | 100 | One client taking every connection. A client network is an IPv4 address, or the /64 of an IPv6 address, because one IPv6 client usually holds a whole /64 |
| Rooms in total (`SPECDECK_MAX_ROOMS`) | 4,000 | Running the service out of memory through rooms |
| Rooms per host | 4 | One connection opening unlimited rooms. The app holds at most 3 |
| Room id length | 64 bytes | Huge keys in the service's tables |
| Viewers per host | 40 | Flooding one host with viewers. The app admits at most 20 |
| Time to host or join | 30 seconds | Connections that hold a place and do nothing |
| Connection set-up messages | 100 at once, then 20 per second, per viewer connection | Using the service to flood a host or viewer |
| Backlog per connection | 256 messages, 256 KiB | A client that stops reading and makes the service buffer for it; the service closes that connection |
| Failed joins | 10 per client network per minute | Guessing invite codes |
| Join attempts per room | 60 per minute | Guessing one session's code from many addresses |

Every viewer and every host keeps a connection open for the whole session. If your team shares one office address and more than 100 people host or watch at once, raise `SPECDECK_MAX_CONNECTIONS_PER_IP`.

The service gives TURN credentials only to a connection that hosts a room or has joined one, and the same credentials each time it asks. Apps from earlier releases ask before they join; the service then waits 2 seconds for the room, so their hosts keep the relay, but their viewers join without it and need a direct connection until they update.

## coturn

The server signs TURN credentials with the shared secret (the TURN REST API scheme), so coturn needs:

| Option | Why |
|---|---|
| `--use-auth-secret`, `--static-auth-secret=<secret>` | Accept the credentials the server hands out |
| `--external-ip=<public address>` | The address coturn advertises in relayed addresses |
| `--min-port`, `--max-port` | The relay ports; open the same range in the firewall |
| `--no-tcp`, `--no-tls`, `--no-dtls`, `--no-tcp-relay` | The apps use TURN over UDP only |
| `--denied-peer-ip=<range>` for each private, loopback, link-local and other special-purpose range, IPv4 and IPv6 | Keeps the relay from reaching into your own network |
| `--user-quota=40` | Allocations per credential. One credential cannot take the whole relay; a host needs one per viewer |
| `--total-quota=<relay ports>` | Allocations in total, as many as there are relay ports, so a full relay says so instead of failing to find a port |
| `--max-bps=2500000` | Bandwidth per allocation: 20 Mbit/s, enough for a late joiner's catch-up |

coturn counts bandwidth in **bytes** per second, despite the option's name: multiply megabits by 125,000. The relay's total bandwidth is at most the allocation quota times 20 Mbit/s; to lower it, shrink the relay range and `--total-quota`. Do not add `--bps-capacity`: coturn reserves the per-allocation rate twice, for each allocation, against it, so 200 Mbit/s would admit only 5 allocations, about two viewers.

The denied ranges cover the IPv6 forms of IPv4 addresses too (IPv4-mapped, NAT64, 6to4 and Teredo). Without them, a relay on a server with IPv6 would reach `10.0.0.1` as `::ffff:10.0.0.1`. No range may start at `::`: coturn then refuses public IPv4 peers as well.

The [Compose file](https://github.com/nickhartjes/specdeck/blob/main/deploy/compose/compose.yaml) and [Kubernetes manifest](https://github.com/nickhartjes/specdeck/blob/main/deploy/k8s/base/coturn.yaml) have the complete list. To check a relay, run `node scripts/turn-denied-peers.mjs wss://<domain>/signal` from a checkout: it asks the relay to reach each kind of address and reports which it refuses.

## TURN URLs

TURN URLs must use `turn:` with `?transport=udp`. The app's WebRTC library has no TURN over TCP or TLS, so `turns:` URLs and TCP transports do not work, and viewers on networks that block UDP entirely cannot connect.
