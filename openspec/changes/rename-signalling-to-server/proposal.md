# Proposal

## Why

The service self-hosters run is called `specdeck-signalling`, but since 1.6.0 it does more than signalling: it also serves the web viewer and tells apps whether it has one. The docs and the app call it the sharing service, so the image, binary and deployment names are the odd ones out, and the name undersells what you are deploying. The image has been public for one release and the Helm chart does not exist yet, so renaming now costs little; after the Helm chart it would cost every self-hoster a migration.

## What Changes

- The image is published as `ghcr.io/nickhartjes/specdeck-server`, with the same tags, platforms and keyless signature as today.
- **Transition:** for the two minor release lines after the rename (1.7.x and 1.8.x), every release also publishes the same image, by the same digest and tags, as `ghcr.io/nickhartjes/specdeck-signalling`, signed there too. A Compose or Kubernetes setup that names the old image keeps updating until 1.9.0, which publishes only the new name. The docs say so.
- The Rust crate and binary become `specdeck-server` (`crates/server`), the Dockerfile `docker/server.Dockerfile`, and the container's entrypoint `specdeck-server`.
- Compose: the service is called `server`, and `compose.yaml` and the Caddyfile name the new image and service.
- **BREAKING (Kubernetes):** in `deploy/k8s`, the Deployment and Service become `specdeck-server` and the file `server.yaml`. An existing cluster has to point its Ingress at the new Service and delete the old Deployment and Service; the Kubernetes docs and the release notes give the steps.
- Docs, README, justfile, CI and the `cosign verify` example use the new names, and say "sharing service" where they describe what it does.
- Not renamed: the WebRTC signalling itself (the `/signal` path, the protocol's `signalling` module, `signalling_client.rs`), the app's `SPECDECK_SIGNALLING_URL` build setting and saved settings, and the service's `SPECDECK_*` environment variables. They name the signalling step, which still exists, and renaming them would break apps and setups for no gain.

## Capabilities

### New Capabilities
- `server-image`: the published container image of the sharing service under its new name, with tags, platforms, signature, and the transition period in which the old name is published alongside it.

### Modified Capabilities
- `signalling-image`: retired; its requirements move to `server-image`. (Its main spec is created when `publish-signalling-image` is archived, which happens before this change is applied.)

## Impact

- Code: `crates/signalling` → `crates/server` (package `specdeck-server`), workspace `Cargo.toml`, `Cargo.lock`, `justfile` (clippy, `local-up`, `local-dev`, port-forwards), `.dockerignore` comment.
- Build and release: `docker/server.Dockerfile`; `.github/workflows/ci.yml` image job; `.github/workflows/release.yml` image jobs (push, tag and sign under both names).
- Deploy: `deploy/compose/compose.yaml`, `Caddyfile`, `.env.example`; `deploy/k8s/base` (`server.yaml`, `kustomization.yaml` image entry), `deploy/k8s/local` patch target and local image tag, `deploy/k8s/README.md`.
- Docs: self-host pages (overview, Docker Compose, Kubernetes, configuration, connect apps), install, README, `crates/protocol/README.md` where it names the crate.
- GitHub: a new `specdeck-server` package on ghcr.io, which starts private and must be made public and linked to the repository after the first release.
- Self-hosters: Compose users on a pinned version change nothing until they upgrade; Kubernetes users follow the upgrade steps once.
