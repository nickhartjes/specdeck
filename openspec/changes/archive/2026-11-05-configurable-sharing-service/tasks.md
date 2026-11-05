# Tasks

## 1. Join links with a sharing service

- [x] 1.1 In `crates/protocol/src/invite.rs`, build join links with a percent-encoded `server` parameter and parse links into the code plus an optional service; a missing or unparsable parameter yields no service and never invalidates the code (design decision 4); verify with unit tests for both directions, links without a parameter, a trailing `/`, and garbage after `?`
- [x] 1.2 Add sharing-service normalization (bare host, host with port, `ws://`/`wss://` with or without path, refused inputs) where both the protocol and the app can use it (design decision 2); verify with unit tests covering each accepted form and each refused one

## 2. Choosing the service per session

- [x] 2.1 Add an optional `sharingService` to `Settings`; verify that an old `settings.json` without it loads unchanged and that a saved value round-trips (settings tests)
- [x] 2.2 Keep the built-in URL and the runtime environment override apart in `AppState`, and resolve the effective service when a session is hosted or joined, in the order environment, setting, built-in (design decision 1); verify with a unit test of the resolution order and by hosting with each source set (done; the check in the running app moved to `release-and-verification` 5.5)
- [x] 2.3 Host: build the copied join link from the session's service; viewer: keep the service the session was joined through for reconnects; verify a host's copied link carries its service and that `crates/core` reconnect tests still pass
- [x] 2.4 Report the stored setting, the effective service, whether the environment overrides it, and whether a session runs, in the profile; verify through the profile in a command test

## 3. Settings screen

- [x] 3.1 Add a command that normalizes and saves the sharing service (empty clears it), then checks it by opening a WebSocket with a 5 s timeout and returns the result (design decision 3); verify with tests against a local signalling service (reachable), a closed port (unreachable) and a refused input
- [x] 3.2 Add the Sharing service field to `SettingsScreen.tsx`: built-in service as placeholder, check result and "not encrypted" for `ws://` in the hint, disabled with an explanation during a session or when the environment overrides it; verify with `SettingsScreen.test.tsx` cases for each state and the a11y test
- [x] 3.3 Describe the setting in the user guide (`docs/site/src/content/docs/guide/settings.md`: what it is for, that it applies to the next session, that a team on its own service shares links rather than bare codes); verify against the running app (done; the check in the running app moved to `release-and-verification` 5.5)

## 4. Joining through another service

- [x] 4.1 Pass the link's service through join-link handling (opened links and links pasted into the Join field) and compare it with the viewer's effective service; verify with unit tests for same service, different service and no service
- [x] 4.2 Add the question on the start screen (host name, unchecked "Use this service from now on", hidden when the environment overrides; Cancel connects nowhere) and join through the link's service when accepted, saving it when the box is checked (design decision 5); verify with UI tests for join once, join and keep, cancel, and same service without a question
- [x] 4.3 Describe joining through another team's service in the user guide (`docs/site/src/content/docs/guide/join.md`); verify the wording against the running app (done; the check in the running app moved to `release-and-verification` 5.5)

## 5. Integration

- [x] 5.1 Moved to `release-and-verification` 5.6: With the local stack (`just local-up`) and `specdeck.hartj.es`: host on one, open the host's link on a viewer set to the other, join once, check the viewer's setting is unchanged, then join with "from now on" and check it changed; record the result in `docs/verification.md`
- [x] 5.2 Moved to `release-and-verification` 5.7: Open a link with `?server=` in the 1.0 release and record whether it joins, so the user guide's note for 1.0 viewers matches (design.md, Risks)
