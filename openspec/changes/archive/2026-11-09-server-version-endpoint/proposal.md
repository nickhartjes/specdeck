# Proposal

## Why

Nothing outside the server's host says which release a sharing service is running. `/health` answers `ok`, `/web-viewer.json` gives only the session protocol, and no response header carries a version. To check that an update landed, an operator has to log in and inspect the container: the image's version label or digest. With auto-updating setups (a floating `1` tag that the host pulls by itself) that check is needed after every release, and it should take one request from anywhere.

## What Changes

- `/web-viewer.json` also reports the release the service was built from, next to the protocol, for example `{"protocol":6,"version":"1.7.0"}`. The version is the release's version without the `v`, the same one the tag, the image tags and the app use.
- `/health` is unchanged and still answers `ok`, so health checks that compare the body keep working.
- The self-hosting guides' update steps end with checking the version with `curl https://<service>/web-viewer.json`.
- Apps already read `/web-viewer.json` for the protocol and ignore other fields, so older apps keep working with a newer service, and newer apps with an older service that has no `version`.

Out of scope: showing the service's version in the app or the web viewer, and a version on services built without the web viewer, which serve no `/web-viewer.json` at all. The published image always includes the web viewer.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `web-viewer`: the service's description of its web viewer (`/web-viewer.json`) also states the service's release version.

## Impact

- `crates/server/src/web.rs` (the `/web-viewer.json` response) and its test in `crates/server/tests/web.rs`.
- Self-hosting docs: `docs/site/src/content/docs/self-host/docker-compose.md` and `kubernetes.md`.
- Public information: anyone can read the version of a service, as they can already read its protocol. The project is open source, so a version gives an attacker little beyond what the protocol already narrows down, and it helps with debugging a self-hosted service.
- No change to the protocol, the apps, or the image.
