---
title: Docker Compose
description: Run the sharing service and the TURN relay on one server.
---

The repository's [`deploy/compose`](https://github.com/nickhartjes/specdeck/tree/main/deploy/compose) runs the whole sharing service on one Linux server with Docker:

| Service | What it does |
|---|---|
| `server` | The SpecDeck server, from the signed release image (or built from the repository) |
| `caddy` | Gets a TLS certificate for the server's domain and passes connections through |
| `coturn` | The TURN relay, on the host's network |

Check [what you need](/specdeck/self-host/overview/#what-you-need) first: a server with a public IPv4 address, two DNS names and open ports.

## 1. DNS

Create two records pointing at the server's public address, for example:

| Name | Type | Value |
|---|---|---|
| `specdeck.example.com` | A | `203.0.113.10` |
| `turn.example.com` | A | `203.0.113.10` |

:::caution[Cloudflare and other proxies]
TURN is UDP, which web proxies do not carry: set the TURN record to **DNS only**. The simplest setup does the same for the server's record. If you do proxy the server's domain, the rate limits see the proxy's addresses instead of the apps' unless Caddy is told to trust the proxy (`trusted_proxies`) and `SPECDECK_TRUSTED_PROXIES` is raised to `2`.
:::

## 2. Firewall

Open these ports to the internet:

- `80/tcp` and `443/tcp` for Caddy;
- `3478/udp` for TURN;
- the relay range, `49160-49259/udp` by default (`TURN_MIN_PORT` to `TURN_MAX_PORT`).

## 3. Configure

Install Docker with the Compose plugin, then get the repository at the release you want to run:

```sh
git clone --branch v1.0.0 https://github.com/nickhartjes/specdeck.git
cd specdeck/deploy/compose
cp .env.example .env
```

Fill in `.env`:

| Setting | Value |
|---|---|
| `SPECDECK_VERSION` | The release to run, such as `1.0.0`: the same version you checked out |
| `SPECDECK_DOMAIN` | The server's domain, such as `specdeck.example.com` |
| `TURN_DOMAIN` | The TURN domain, such as `turn.example.com` |
| `TURN_EXTERNAL_IP` | The server's public IPv4 address |
| `TURN_SECRET` | A random secret shared by the server and coturn: `openssl rand -hex 32` |
| `TURN_MIN_PORT`, `TURN_MAX_PORT` | The relay range you opened in the firewall |

Keep `.env` private: anyone with `TURN_SECRET` can use your relay.

## 4. Start

```sh
docker compose up -d
```

This pulls the server image for `SPECDECK_VERSION` from `ghcr.io/nickhartjes/specdeck-server`, with the [web viewer](/specdeck/guide/join/#watch-in-a-browser) built in. Every release publishes it for amd64 and arm64 servers, signed by the release workflow; to check the signature before you start, see [the web viewer's code](/specdeck/self-host/overview/#the-web-viewers-code). Caddy gets its certificate as soon as the DNS records resolve to the server.

### Or build it from source

```sh
docker compose up -d --build
```

This builds the image from the checked-out release instead, under the same name, which takes a few minutes. The build downloads from crates.io, the npm registry (`registry.npmjs.org`) and Docker Hub, so the server needs to reach them. If your network only allows a company npm registry, build the image once with your npm config as a build secret, which is not stored in the image: `docker build --secret id=npmrc,src=$HOME/.npmrc -f docker/server.Dockerfile -t ghcr.io/nickhartjes/specdeck-server:<SPECDECK_VERSION> ../..`, then start without `--build`.

Once it runs, `https://<SPECDECK_DOMAIN>/` is the web viewer, and hosts on this service copy join links that open there.

Check that the service answers:

```sh
curl https://specdeck.example.com/health   # prints: ok
docker compose logs -f                      # follow the logs
```

Then [connect the apps](/specdeck/self-host/connect-apps/).

## Update

```sh
git fetch --tags && git checkout v1.1.0    # the new release
# set SPECDECK_VERSION=1.1.0 in .env
docker compose pull
docker compose up -d --remove-orphans
```

If you build from source, run `docker compose up -d --build --remove-orphans` after the checkout instead of pulling.

`--remove-orphans` matters once, when you update from 1.6 or earlier: the service was called `signalling` then, and is `server` now, so it stops the old container.

Restarting the server does not end sessions that are already connected; new viewers can join again once it is back.

## Notes

- coturn uses the host's network (`network_mode: host`), so it advertises the server's real address and needs no published port per relay port. It refuses to relay to private and loopback ranges, so the relay cannot be used to reach the server's own services or its local network.
- The server needs its public address on its own interface. Behind 1:1 NAT, coturn needs extra settings (`--external-ip=<public>/<private>` and allowing its own private address as a peer) that this setup does not include.
- Each relayed viewer uses about two relay ports; 100 ports cover about 50 relayed viewers at once. Most connections are direct and use none.
