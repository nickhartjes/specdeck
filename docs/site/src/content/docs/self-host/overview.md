---
title: How the sharing service works
description: What the signalling service and the TURN relay do, and what they can see.
---

SpecDeck sends a session straight from the host to each viewer. The **sharing service** only helps them find each other. It is two programs:

- The **signalling service** (`specdeck-signalling`) matches viewers to hosts by invite code and passes on the messages that set up the connection. It also hands out short-lived credentials for the relay. Apps reach it over a WebSocket, `wss://<domain>/signal`. It also serves the [web viewer](/specdeck/guide/join/#watch-in-a-browser) at `https://<domain>/`, built into the same program, so people who cannot install the app can watch.
- The **TURN relay** ([coturn](https://github.com/coturn/coturn)) forwards the session's traffic when host and viewer cannot reach each other directly, for example behind strict company firewalls. It uses UDP only.

```
   host app                                      viewer app
      |                                              |
      |-- wss://specdeck.example.com/signal ---------|   who is who (invite code),
      |          signalling service                  |   connection setup
      |                                              |
      |<============ direct, encrypted =============>|   the session itself
      |                                              |
      |<==== or through turn.example.com:3478 ======>|   when direct does not work
                    TURN relay (UDP)
```

## What the service can see

Sessions are end-to-end encrypted between host and viewers, and every frame is signed by the host. Neither the signalling service nor the relay can read or change a session. The signalling service never sees invite codes either: apps send it a room id derived from the code, and sign the connection setup with a key that never leaves the apps, so the service cannot put itself in between.

They do see:

- the network addresses of hosts and viewers;
- that a session exists, and how many people join it.

That is why SpecDeck asks before it joins through a service other than your own (see [Through another sharing service](/specdeck/guide/join/#through-another-sharing-service)).

## The web viewer's code

The web viewer is different from the app in one way: a browser loads its code from the sharing service on every visit, and that code is what decrypts the session. Whoever runs the service could change the code and read the sessions of everyone who watches in a browser from then on. The app is installed once, and checks every update against SpecDeck's signature.

So watching in a browser means trusting the people who run the sharing service, as well as the host:

- **On your own service**, that is your team. The page comes only from your server: its security policy lets the browser load and connect to nothing else, and each page joins only sessions on its own service.
- **The published image** is signed by SpecDeck's release workflow. Check that the image you run was built by that workflow from a release tag, and not changed since:

  ```sh
  cosign verify ghcr.io/nickhartjes/specdeck-signalling:1.6.0 \
    --certificate-identity-regexp '^https://github\.com/nickhartjes/specdeck/\.github/workflows/release\.yml@refs/tags/v' \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

  [cosign](https://docs.sigstore.dev/cosign/system_config/installation/) checks the signature against Sigstore's public log; no key from us is involved.
- **Building the image yourself** from the repository, as both guides describe, lets you read the exact code your viewers run.
- **Viewers who use the app** do not depend on the page at all.

The page needs nothing extra to deploy: it is built into the signalling image.

## Why run your own

SpecDeck releases come with a built-in sharing service. Run your own when your team wants to decide where that service runs and who operates it, or when your network only allows certain servers.

Rooms live in memory: run **one** signalling service. Restarting it ends the matching of new viewers until it is back; sessions that are already connected keep running.

## What you need

- A server with a public IPv4 address on its own network interface, such as a small VPS. A few hundred MB of memory is plenty.
- Two DNS names pointing at it, for example `specdeck.example.com` (signalling) and `turn.example.com` (TURN).
- These ports reachable from the internet:

| Port | For |
|---|---|
| 80/TCP, 443/TCP | The signalling service through a TLS proxy (80 only for the certificate) |
| 3478/UDP | TURN |
| A UDP range, for example 49160-49259 | TURN relay ports; each relayed viewer uses about two |

Then choose how to deploy:

- [Docker Compose](/specdeck/self-host/docker-compose/) on one server: the quickest way.
- [Kubernetes](/specdeck/self-host/kubernetes/) with the manifests in the repository.

And [connect the apps](/specdeck/self-host/connect-apps/) to it.
