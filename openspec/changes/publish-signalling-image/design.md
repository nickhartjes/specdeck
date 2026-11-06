# Design

## Context

See proposal.md - Why. `docker/signalling.Dockerfile` builds the service in four stages (WebAssembly package, web page, Rust service, `debian:trixie-slim` runtime as user 10001). CI's `image` job builds it for amd64 and checks `/health`, `/web-viewer.json` and `/join`, but pushes nothing. `release.yml` runs on `v*` tags: `draft` checks the tag against the app version and creates a draft release, `package` builds the installers per platform, `publish` adds `latest.json` and publishes the draft, `homebrew` follows for stable tags. The workflow's top-level permission is `contents: write`. The repository is public, so ghcr.io storage and transfer for a public package are free, and GitHub's arm64 runners are free too.

## Goals / Non-Goals

**Goals:**
- One signed, multi-platform image per release, pulled anonymously from ghcr.io.
- Self-hosters can verify the image came from the release workflow without trusting anything but Sigstore and GitHub.
- Deploy files and docs default to the published image, with building from source still documented.

**Non-Goals:**
- Images for commits on `main` or pull requests (CI keeps building without pushing).
- SBOMs, SLSA provenance attestations and vulnerability scanning: possible later additions on the same digest.
- Publishing to Docker Hub or any other registry.
- The Helm chart (its own change, using this image).

## Decisions

### 1. A job in `release.yml`, not a separate workflow

The image belongs to the release: it uses the tag check in `draft`, and `publish` gets `needs: [package, image]` so the release is published only with its image (spec: Release only with its image). A separate workflow on `push: tags` could not hold the release back. The image jobs set their own permissions (`contents: read`, `packages: write`, `id-token: write`), so the `contents: write` of the installer jobs does not reach them and the OIDC token is only available where cosign needs it.

### 2. Native builds per platform, merged by digest

Two matrix entries, `linux/amd64` on `ubuntu-24.04` and `linux/arm64` on `ubuntu-24.04-arm` (the same Ubuntu on both; actionlint does not know an arm 26.04 label yet), each run `docker/build-push-action` with `push-by-digest=true` and upload the digest as an artifact. An `image-merge` job then creates the multi-platform list with `docker buildx imagetools create` and the tags. Building arm64 under QEMU on an amd64 runner was rejected: the Rust and WebAssembly stages would take hours emulated. Cross-compiling in the Dockerfile was rejected because it would make the Dockerfile differ from what CI and self-builders run.

### 3. Tags from `docker/metadata-action`

`type=semver,pattern={{version}}`, `{{major}}.{{minor}}` and `{{major}}`. For a prerelease, metadata-action outputs the full version for the last two as well (its README: pre-releases "only extend `{{version}}`"), so `v1.7.0-rc.1` gets only `1.7.0-rc.1`. Its README does not say the same for `latest`, so `flavor: latest=false` and `latest` is added as a raw tag only when the tag has no `-`, the same check the Homebrew job uses. It also sets the OCI labels: `org.opencontainers.image.source` links the package to the repository, plus `revision` and `version`.

### 4. Keyless cosign on the list digest

`sigstore/cosign-installer` (pinned to an exact version: unlike the Docker actions it has no floating major tag, which the task 1.4 test run caught), then `cosign sign --yes ghcr.io/nickhartjes/specdeck-signalling@<digest>` with the merged list's digest, never a tag, so the signature covers exactly what was pushed. The certificate's identity is `https://github.com/nickhartjes/specdeck/.github/workflows/release.yml@refs/tags/<tag>`, issuer `https://token.actions.githubusercontent.com`. A key pair stored as a secret was rejected: it can leak, needs rotating, and verifying it means trusting a key we publish ourselves.

The docs give the check as:

```sh
cosign verify ghcr.io/nickhartjes/specdeck-signalling:1.6.0 \
  --certificate-identity-regexp '^https://github\.com/nickhartjes/specdeck/\.github/workflows/release\.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

### 5. Pushed with `GITHUB_TOKEN`

`docker/login-action` with `${{ github.token }}` and `packages: write`. The first push creates the package under the `nickhartjes` account; it is linked to the repository through the source label (decision 3). In the task 1.4 test run, a package pushed from this public repository came out public right away; task 3.2 still checks an anonymous pull after the first release.

### 6. Deploy files pin a version

- Compose: `image: ghcr.io/nickhartjes/specdeck-signalling:${SPECDECK_VERSION:?Set SPECDECK_VERSION in .env}`, keeping the `build:` section, so `docker compose up -d` pulls and `docker compose up -d --build` still builds from source (under the same name). `.env.example` gets `SPECDECK_VERSION`. Updating becomes: change the version, `docker compose pull`, `up -d`.
- Kubernetes base: the image becomes `ghcr.io/nickhartjes/specdeck-signalling` with a Kustomize `images:` entry setting the tag, so an overlay changes one line. The base uses the major tag `1`, and the docs tell production to pin an exact version or digest. The local overlay keeps replacing it with `specdeck-signalling:local` and `imagePullPolicy: Never`.
- `latest` is never used by the deploy files.

## Risks / Trade-offs

- [An image build failure holds back the app release] → Intended (spec). The draft stays, and re-running the failed jobs finishes the release.
- [arm64 runner label changes or runners are busy] → The label is in one matrix entry; a missing arm64 image fails the merge job rather than shipping amd64 only.
- [The package starts private and anonymous pulls fail] → Task 3.2 checks an anonymous pull after the first release.
- [The `1` tag in the Kubernetes base moves under a running deployment] → Pods only pull it on restart with `IfNotPresent`; the docs say to pin. Compose always pins.
- [Signatures trust GitHub and Sigstore] → That is the point of keyless signing; the identity check ties it to this repository's release workflow at a tag, which only maintainers can push.

## Migration Plan

1. Merge; the next release tag publishes the first image.
2. Make the package public if needed, then run the anonymous pull and `cosign verify` from the docs.
3. Self-hosters on Compose add `SPECDECK_VERSION` to `.env`; until they do, `docker compose up` stops with the `:?` message, which names the variable.
4. Rollback: remove the image jobs and the `needs`; published images stay and can be deleted from the package settings.
