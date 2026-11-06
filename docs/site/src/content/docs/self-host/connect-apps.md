---
title: Connect the apps
description: Point SpecDeck at your own sharing service.
---

Once your sharing service runs, everyone on the team points SpecDeck at it. Apps use, in this order:

1. `SPECDECK_SIGNALLING_URL` in the environment when SpecDeck starts;
2. the **Sharing service** in Settings;
3. the service built into SpecDeck.

## In Settings

Each person opens **Settings** and enters the server's domain under **Sharing service**, for example `specdeck.example.com`. SpecDeck makes it `wss://specdeck.example.com/signal`, checks that it answers, and uses it from the next session on. See [Settings](/specdeck/guide/settings/#sharing-service).

## Through a link

A host who already uses your service can send a join link. Viewers whose SpecDeck uses another service are asked **Join through specdeck.example.com?** and can tick **Use this service from now on**, which saves it in their Settings. See [Through another sharing service](/specdeck/guide/join/#through-another-sharing-service).

Until everyone has switched, share links rather than bare codes: a code on its own always goes through the viewer's own service.

## With an environment variable

Start SpecDeck with `SPECDECK_SIGNALLING_URL` set in its environment, for example from a managed launcher or script. On Linux:

```sh
SPECDECK_SIGNALLING_URL=wss://specdeck.example.com/signal specdeck-app
```

It takes the full URL. While it is set, the setting in Settings cannot be changed.

## Built in

Builds from source take the built-in service from `SPECDECK_SIGNALLING_URL` at build time. The Nix package sets it to the public service, `wss://specdeck.hartj.es/signal`; to build in your own:

```nix
specdeck.override { signallingUrl = "wss://specdeck.example.com/signal"; }
```

Releases built in your own fork of the repository use the `SPECDECK_SIGNALLING_URL` repository secret, as described at the top of `.github/workflows/release.yml`.

## Check that it works

- `curl https://specdeck.example.com/health` prints `ok`.
- **Settings** shows the service as reachable.
- Host a session and join it from another network, such as a phone's hotspot.

To check the relay on its own, run the relay test from a checkout of the repository. It connects two peers through TURN only, with direct connections turned off:

```sh
SPECDECK_SIGNALLING_URL=wss://specdeck.example.com/signal \
  cargo test -p specdeck-core --test turn_relay -- --ignored --nocapture
```
