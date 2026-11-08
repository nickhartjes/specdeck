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
| `RUST_LOG` | none | Log level, such as `info` or `debug` |

It serves two paths:

| Path | What |
|---|---|
| `/signal` | The WebSocket the apps connect to |
| `/health` | Answers `ok`, for health checks |

The service keeps rooms in memory, so run exactly one instance.

## coturn

The server signs TURN credentials with the shared secret (the TURN REST API scheme), so coturn needs:

| Option | Why |
|---|---|
| `--use-auth-secret`, `--static-auth-secret=<secret>` | Accept the credentials the server hands out |
| `--external-ip=<public address>` | The address coturn advertises in relayed addresses |
| `--min-port`, `--max-port` | The relay ports; open the same range in the firewall |
| `--no-tcp`, `--no-tls`, `--no-tcp-relay` | The apps use TURN over UDP only. coturn 4.18 and later leave DTLS off by default and refuse `--no-dtls`; add it only on older versions |
| `--denied-peer-ip=<range>` for each private, loopback and link-local range | Keeps the relay from reaching into your own network |

The [Compose file](https://github.com/nickhartjes/specdeck/blob/main/deploy/compose/compose.yaml) and [Kubernetes manifest](https://github.com/nickhartjes/specdeck/blob/main/deploy/k8s/base/coturn.yaml) have the complete list.

## TURN URLs

TURN URLs must use `turn:` with `?transport=udp`. The app's WebRTC library has no TURN over TCP or TLS, so `turns:` URLs and TCP transports do not work, and viewers on networks that block UDP entirely cannot connect.
