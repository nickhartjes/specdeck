# Design

## Context

The app picks its signalling URL once at start (`app/src-tauri/src/lib.rs`, `setup`): the `SPECDECK_SIGNALLING_URL` environment variable, else the same name baked in at build time with `option_env!`, else `ws://127.0.0.1:18081/signal` for `just local-dev`. It is stored in `AppState` and copied into `HostConfig` and `ViewerConfig` when a session starts. TURN servers are not configured in the app: the signalling service hands them out with short-lived credentials, so choosing a signalling service also chooses its relay.

Settings live in `settings.json` (`settings.rs`, `#[serde(default)]`, so new optional fields read old files). Join links are `specdeck://join/<code>`, parsed by `InviteCode::from_join_link` in `crates/protocol/src/invite.rs`, which rejects anything after the code other than a trailing `/`.

## Goals / Non-Goals

**Goals:**
- One place that decides which service a session uses, at the moment the session starts.
- A link from a host on any service works for a viewer on any service, after one question.

**Non-Goals:**
- Several saved services or per-team profiles.
- Configuring TURN in the app; it keeps coming from the signalling service.
- Changing the service of a running session.
- A server list or discovery.

## Decisions

### 1. Resolve the service per session

`AppState` keeps the build-time default and the runtime environment override separately, instead of one resolved URL:

```
effective = env SPECDECK_SIGNALLING_URL (read at start)
          ?? settings.sharing_service
          ?? built-in (option_env! at build time, else the local-dev URL)
```

Host start and viewer join call one function for the effective URL. A viewer join from a link with an accepted foreign service passes that URL instead. Each runtime keeps the URL it started with, so reconnects (viewer) and code rotation (host) stay on it, whatever the setting does meanwhile.

*Alternative*: keep resolving at app start and require a restart after changing the setting. Rejected: the URL is already only used when a session starts, and a restart prompt adds friction for nothing.

### 2. Normalize input, store the full URL

The Settings field accepts `specdeck.example.com`, `specdeck.example.com:8443`, `wss://specdeck.example.com`, or `wss://specdeck.example.com/signal`. A bare host becomes `wss://<host>/signal`; a `ws://` or `wss://` URL without a path gets `/signal`. Other schemes, or input that is not a host, are refused with a message. The normalized URL is stored and is what services are compared by (a link's service equals the viewer's when the normalized URLs are equal). An empty field stores nothing.

`ws://` is accepted (local testing) and marked "not encrypted". Session content is end-to-end encrypted either way; without TLS the signalling metadata (room id, network addresses) travels in the clear.

### 3. Reachability check by opening the WebSocket

On save, the app opens a WebSocket to the normalized URL with a 5 s timeout and closes it. Success means "can be reached"; anything else shows the error in plain words. This uses the existing `tokio-tungstenite` client, so no HTTP client is added, and it tests what sessions actually need (a proxy that blocks WebSocket upgrades fails the check, which a `GET /health` would not).

### 4. Join link format

```
specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal
```

- The parameter carries the host's effective, normalized URL, percent-encoded, and is always present on links a host copies, including for the built-in service. A viewer who set their own service then still gets the question for a link from a host on the default one, instead of "invalid or expired code".
- Parsing returns the code and an optional service. A missing or unparsable `server` parameter means "no service in the link"; it never makes the code invalid.
- "Copy code" stays the bare code.
- The tray's and menu's "Copy Invite Code" stay the bare code.

*Alternative*: include the service only when it differs from the built-in one. Rejected: a viewer with a custom service would get no question for a default host's link and simply fail to join.

### 5. The question

When a link (opened, or pasted into the Join field) carries a service different from the viewer's effective one, the start screen asks before connecting:

```
  Join through specdeck.example.com?
  This session uses a different sharing service than yours.
  [ ] Use this service from now on
                                   [Cancel]  [Join]
```

The dialog names the host part only. Joining with the box checked saves the link's service as the setting. Cancel connects nowhere. If the environment variable overrides the service, the link's service is still used for that session when accepted, but the checkbox is hidden (the setting would have no effect).

What a foreign service learns, which is why the app asks instead of silently following links: the viewer's IP address, the room id, and the ICE candidates (including local network addresses), plus the relay sees the viewer's traffic volume. It cannot read session content or the invite code, cannot learn the username (sent inside the encrypted channel), and cannot impersonate the host (offers and answers are authenticated with the key derived from the code, decision 8 of the MVP design).

### 6. Settings while a session runs

The Settings field is disabled while hosting or viewing, with the hint "Applies to the next session you start or join". The profile already reports `hosting`; it gains `viewing` (or a single `in_session`), and the sharing service fields: the stored setting, the effective URL, and whether the environment overrides it. When it does, the field is disabled and says so.

## Risks / Trade-offs

- [A 1.0 app cannot read a link with `?server=`: `from_join_link` refuses anything after the code, so the viewer sees "this link does not work"] → Make `from_join_link` ignore a query string in the 1.0 release if it has not shipped yet; otherwise the user guide tells 1.0 viewers to paste the code, and the problem ends when they update.
- [A malicious link sends a viewer to a hostile service] → The question names the service before connecting; content and identity stay protected (decision 5).
- [Host and viewer use different services and share only the bare code] → The join fails as "invalid or expired code", indistinguishable from a typo. The user guide says to share the link when a team runs its own service.
- [A typo in the setting breaks sharing] → The check on save shows it at once; clearing the field restores the built-in service.

## Migration Plan

No data migration: an absent `sharingService` in `settings.json` means the built-in service, so existing installs behave as before. Rolling back to an earlier 1.x ignores the unknown field.
