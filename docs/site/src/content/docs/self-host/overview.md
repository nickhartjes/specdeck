---
title: How the sharing service works
description: What the sharing service and the TURN relay do, and what they can see.
---

SpecDeck sends a session straight from the host to each viewer. The **sharing service** only helps them find each other. It is two programs:

- The **server** (`specdeck-server`) matches viewers to hosts by invite code and passes on the messages that set up the connection. It also hands out short-lived credentials for the relay, only to apps that host a session or have joined one, and limits how much any one client can hold (see [Limits](/specdeck/self-host/configuration/#limits)). Apps reach it over a WebSocket, `wss://<domain>/signal`. It also serves the [web viewer](/specdeck/guide/join/#watch-in-a-browser) at `https://<domain>/`, built into the same program, so people who cannot install the app can watch.
- The **TURN relay** ([coturn](https://github.com/coturn/coturn)) forwards the session's traffic when host and viewer cannot reach each other directly, for example behind strict company firewalls. It uses UDP only.

```
   host app                                      viewer app
      |                                              |
      |-- wss://specdeck.example.com/signal ---------|   who is who (invite code),
      |          specdeck-server                     |   connection setup
      |                                              |
      |<============ direct, encrypted =============>|   the session itself
      |                                              |
      |<==== or through turn.example.com:3478 ======>|   when direct does not work
                    TURN relay (UDP)
```

## What the service can see

Sessions are end-to-end encrypted between host and viewers, so neither the server nor the relay can read them. Apps send the server a room id made from the invite code, not the code itself. Whoever runs the service could still try to guess a code from its room id offline, which is why it matters who runs it; see [The sharing service](/specdeck/security/sharing-service/) in *Security and privacy*.

They do see:

- the network addresses of hosts and viewers;
- the messages that set up each connection, which list the addresses each app can be reached at, including local network addresses from the app;
- that a session exists, and how many people join it.

The server logs when rooms open and close, viewers joining, leaving or being turned away, and its limits, without room ids, codes or addresses (see [Logs](/specdeck/self-host/configuration/#logs)). The relay logs each connection it relays, with both ends' addresses.

That is why SpecDeck asks before it joins through a service other than your own (see [Through another sharing service](/specdeck/guide/join/#through-another-sharing-service)).

## The web viewer's code

The web viewer is different from the app in one way: a browser loads its code from the sharing service on every visit, and that code is what decrypts the session. Whoever runs the service could change the code and read the sessions of everyone who watches in a browser from then on. The app is installed once, and checks every update against SpecDeck's signature.

So watching in a browser means trusting the people who run the sharing service, as well as the host:

- **On your own service**, that is your team. The page comes only from your server: its security policy lets the browser load and connect to nothing else, and each page joins only sessions on its own service.
- **The published image** is signed by SpecDeck's release workflow. Check that the image you run was built by that workflow from a release tag, and not changed since:

  ```sh
  cosign verify ghcr.io/nickhartjes/specdeck-server:1.7.0 \
    --certificate-identity-regexp '^https://github\.com/nickhartjes/specdeck/\.github/workflows/release\.yml@refs/tags/v' \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

  [cosign](https://docs.sigstore.dev/cosign/system_config/installation/) checks the signature against Sigstore's public log; no key from us is involved.
- **Building the image yourself** from the repository, as both guides describe, lets you read the exact code your viewers run.
- **Viewers who use the app** do not depend on the page at all.

The page needs nothing extra to deploy: it is built into the server image.

## The image's name

The image is `ghcr.io/nickhartjes/specdeck-server`. Up to 1.6 it was called `ghcr.io/nickhartjes/specdeck-signalling`; until 1.9.0, every release is also published under that name, as the same signed image, so a setup that still uses it keeps getting updates. From 1.9.0 on, only `specdeck-server` gets new releases. To move, change the image name in your Compose file or overlay; the [Docker Compose](/specdeck/self-host/docker-compose/#update) and [Kubernetes](/specdeck/self-host/kubernetes/#upgrading-from-16) guides have the steps.

## Why run your own

SpecDeck releases come with a built-in sharing service. Run your own when your team wants to decide where that service runs and who operates it, or when your network only allows certain servers.

Rooms live in memory: run **one** server. Restarting it ends the matching of new viewers until it is back; sessions that are already connected keep running.

## What you need

- A server with a public IPv4 address on its own network interface, such as a small VPS. A few hundred MB of memory is plenty.
- Two DNS names pointing at it, for example `specdeck.example.com` (the server) and `turn.example.com` (TURN).
- These ports reachable from the internet:

| Port | For |
|---|---|
| 80/TCP, 443/TCP | The server through a TLS proxy (80 only for the certificate) |
| 3478/UDP | TURN |
| A UDP range, for example 49160-49259 | TURN relay ports; each relayed viewer uses about two |

Then choose how to deploy:

- [Docker Compose](/specdeck/self-host/docker-compose/) on one server: the quickest way.
- [Kubernetes](/specdeck/self-host/kubernetes/) with the manifests in the repository.

And [connect the apps](/specdeck/self-host/connect-apps/) to it.
