# Proposal

## Why

Self-hosting the sharing service today means building the signalling image from source: the Docker Compose guide builds it on the server (a few minutes, and it needs crates.io, the npm registry and Docker Hub), and the Kubernetes guide asks you to build and push it to your own registry first. A published image makes self-hosting a pull, and the Helm chart on the todo list needs one to point at. Signing it lets a self-hoster check that the image came from this repository's release workflow, which matters because the web viewer's code in that image is what decrypts sessions in the browser.

## What Changes

- The release workflow builds the signalling image (`docker/signalling.Dockerfile`, web viewer included) for `linux/amd64` and `linux/arm64` on every release tag and pushes it to `ghcr.io/nickhartjes/specdeck-signalling`, as a public package linked to the repository.
- Tags: the exact version (`1.6.0`) on every release; stable releases also move `1.6`, `1` and `latest`. Prereleases (`v1.6.0-rc.1`) get only their exact tag.
- The image is signed with keyless cosign (Sigstore, GitHub's OIDC identity of the release workflow); no signing key is stored anywhere. The signature is in the public Rekor log.
- The GitHub release is published only after its image is pushed and signed, so a release never names an image that does not exist.
- Self-host docs and deploy files use the published image: Docker Compose pulls a pinned version (building from source stays available with `--build`), the Kubernetes base points at ghcr.io, and the docs show how to verify the signature with `cosign verify`.

## Capabilities

### New Capabilities

- `signalling-image`: the published container image of the sharing service: where it is, which tags exist for which releases, which platforms it runs on, and how its origin can be verified.

### Modified Capabilities

None.

## Impact

- `.github/workflows/release.yml`: a new image job (per-platform builds on native runners, a merge into one multi-platform image, cosign signing) with `packages: write` and `id-token: write`; `publish` waits for it.
- `deploy/compose/compose.yaml` and `.env.example`, `deploy/k8s/base` (image reference), `deploy/k8s/README.md`.
- Docs: `docs/site/src/content/docs/self-host/{overview,docker-compose,kubernetes}.md`.
- One manual step on the first release: make the ghcr.io package public and link it to the repository if GitHub did not.
- No change to the app, the service or the Dockerfile's output; CI's image job stays as it is.
