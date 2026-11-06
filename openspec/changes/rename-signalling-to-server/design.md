# Design

## Context

See proposal.md for why. Today:

- One crate, `crates/signalling` (package and binary `specdeck-signalling`), built by `docker/signalling.Dockerfile` with the `web-viewer` feature.
- `release.yml` builds the image once per platform (`image`, pushed by digest), then `image-merge` creates the multi-platform tags with `docker buildx imagetools create`, and `cosign sign`s the digest keylessly. `publish` waits for `image-merge` (publish-signalling-image design decisions 1 to 5).
- The package `specdeck-signalling` is public and linked to the repository through the image's OCI `source` label; 1.6.0 is the only release that published it.
- `deploy/k8s/base/signalling.yaml` holds the Deployment and Service `specdeck-signalling` (label `app: specdeck-signalling`); the cluster's Ingress lives in the self-hoster's overlay and names that Service. `deploy/k8s/local` patches the Deployment by name and uses the image `specdeck-signalling:local`; the `justfile` restarts and port-forwards it by name.
- Compose runs the service as `signalling`, and Caddy proxies to `signalling:8080`. The service keeps no state.

## Goals / Non-Goals

**Goals:**
- One name, `specdeck-server`, for everything a self-hoster deploys or builds.
- Nobody on the old image name is cut off without a release-notes warning and two minor release lines to move.

**Non-Goals:**
- Renaming the signalling step itself, its protocol types, the `/signal` path, `SPECDECK_SIGNALLING_URL`, or the service's environment variables (proposal, "Not renamed").
- The Helm chart; it starts with the new names in its own change.
- A redirect or alias for the old Kubernetes resource names; Kubernetes has no such thing for a Deployment.

## Decisions

1. **The name is `specdeck-server`.** It is the one thing a self-hoster runs besides the TURN relay, and the word fits whatever else it may serve later. Considered: `specdeck-sharing` (matches "sharing service" in the docs, but reads as a verb on an image), `specdeck-service` (too generic next to `specdeck-turn`), keeping `specdeck-signalling` (the problem this change exists for). The docs keep calling it the sharing service when they explain what it does.

2. **Move the crate with `git mv`, rename the package, keep the module layout.** `crates/signalling` becomes `crates/server`, package `specdeck-server`; Rust paths change from `specdeck_signalling` to `specdeck_server` in its own tests and nowhere else (no other crate depends on it). History follows the move. The Dockerfile becomes `docker/server.Dockerfile`; there is no copy at the old path, because only our own docs and CI name it.

3. **Build once, publish the same digest under both names.** The per-platform `image` job pushes by digest to `specdeck-server` only. In `image-merge`, `docker/metadata-action` gets both image names, so its tag list holds every tag for both; `imagetools create` writes them all from the `specdeck-server` digests. ghcr.io mounts the blobs across the two repositories of the same owner, so nothing is uploaded twice and both names have one identical manifest digest. Considered: building twice (two digests for one release, slower), or `crane copy` after the fact (an extra tool and a window where the names differ).

4. **Sign under each name.** A cosign signature is stored next to the image in its own repository, so a signature on `specdeck-server@<digest>` does not make `cosign verify` pass on `specdeck-signalling`. `image-merge` signs `<name>@<digest>` for each published name; the identity is the same workflow, so the documented `cosign verify` command works on either name. `publish` keeps waiting for `image-merge`, which fails if any push or signature fails, so the "release only with its image" requirement holds for both names without new wiring.

5. **The old name is one line, with its end in the comment.** The list of image names in `release.yml` is a single env value; the old name sits in it with a comment saying it goes in 1.9.0, and `todo.txt` and the release checklist carry the same reminder. Removing it is a one-line change in the 1.9.0 release; the published old tags stay on ghcr.io, and the package is not deleted.

6. **Kubernetes resources are renamed outright (user's choice).** The base gets `server.yaml` with Deployment and Service `specdeck-server` and label `app: specdeck-server`; the kustomize `images` entry and the local patch target follow. A label selector cannot change on an existing Deployment, so a rename means a new Deployment anyway. The Kubernetes guide gets an "Upgrading from 1.6" section: apply the new base, point the Ingress backend at `specdeck-server`, wait for the rollout, then `kubectl delete deployment,service specdeck-signalling`. Until the old objects are deleted both run side by side, which is harmless: the service keeps no state and the Ingress decides which one gets traffic.

7. **Compose service `server`; upgrades remove the old container.** `compose.yaml` and the Caddyfile name `server`; the upgrade steps in the Docker Compose guide become `docker compose pull` and `docker compose up -d --remove-orphans`, which stops the old `signalling` container. A setup that keeps its own copy of `compose.yaml` with the old image name keeps working until 1.9.0 (decision 3).

8. **Retire `signalling-image` and add `server-image`.** The image's name is in every requirement, and the capability is named after the image, so its requirements move to a capability with the new name and the old one is retired (`retire_capabilities: true`). `publish-signalling-image` is archived first, so the retired spec exists when this change is archived.

9. **Local development follows the new names.** `just local-up` builds `specdeck-server:local` and restarts `deployment/specdeck-server`; because the local namespace already has the old Deployment and Service, `local-up` deletes them if present (`--ignore-not-found`), so developers need no manual step.

## Risks / Trade-offs

- [The new package `specdeck-server` starts private on ghcr.io, so the first release with it would publish an image nobody can pull] → A task after the first release makes it public, as was done for `specdeck-signalling`, and the anonymous-pull check from publish-signalling-image task 3.2 runs on both names. The rename ships in a release candidate first, so this is caught before a stable tag.
- [A Kubernetes self-hoster upgrades without reading the notes and their Ingress still names `specdeck-signalling`] → The old Deployment and Service keep running (kustomize does not prune them), so traffic keeps flowing to the old pod until they delete it; the release notes and the Kubernetes guide lead with the upgrade steps.
- [Self-hosters on the old name miss the 1.9.0 cut-off] → They are not broken, only no longer updated: their pinned or `:1` tag stays at the last 1.8.x. The 1.7 and 1.8 release notes and the self-host overview say when the old name stops.
- [Something outside this repository names `docker/signalling.Dockerfile` or the `specdeck-signalling` crate] → Only our own docs, CI and justfile do; `git grep` after the rename must find the old names only in the places the proposal keeps on purpose and in archived changes.

## Migration Plan

1. Archive `publish-signalling-image` (PR #43 must be merged first).
2. Implement the rename in one pull request; CI's image job builds `docker/server.Dockerfile` and checks the running container as today.
3. Release `v1.7.0-rc.1`: check that both names have the prerelease tag with one digest, both verify with cosign, and make `specdeck-server` public and linked to the repository.
4. Release `v1.7.0` with release notes leading with the new name and the Kubernetes upgrade steps.
5. In the 1.9.0 release, remove the old name from `release.yml` and the transition note from the docs.

Rollback: reverting the pull request restores the old names; the `specdeck-signalling` package was never stopped, so self-hosters are unaffected.
