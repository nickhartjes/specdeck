# Tasks

## 1. Release workflow

- [x] 1.1 Add to `.github/workflows/release.yml` an `image` matrix job (`needs: draft`; amd64 on `ubuntu-24.04`, arm64 on `ubuntu-24.04-arm`; permissions `contents: read`, `packages: write`) that logs in to ghcr.io with `github.token`, builds `docker/signalling.Dockerfile` with `docker/build-push-action` pushing by digest, and uploads the digest as an artifact (design decisions 1, 2, 5); verify with `actionlint`
- [x] 1.2 Add an `image-merge` job (`needs: image`; permissions `contents: read`, `packages: write`, `id-token: write`) that downloads the digests, takes tags and OCI labels from `docker/metadata-action` (semver `{{version}}`, `{{major}}.{{minor}}`, `{{major}}`, `latest`), creates the list with `docker buildx imagetools create`, and signs the list digest with keyless cosign (decisions 3, 4); verify with `actionlint`, and verify with metadata-action run locally or from its docs that `v1.7.0-rc.1` yields only `1.7.0-rc.1` while `v1.6.0` yields all four tags
- [x] 1.3 Make `publish` need `[package, image-merge]` and list the image, its tags and the needed permissions in the workflow's header comment (decision 1); verify with `actionlint` that the job graph is draft → package and image → image-merge → publish
- [x] 1.4 Run the image jobs end to end without a release, from a temporary `workflow_dispatch` copy on a branch that pushes to a throwaway tag such as `0.0.0-test` (or to a test package name); verify `docker buildx imagetools inspect` shows amd64 and arm64, the arm64 image starts and answers `/health` on an arm64 machine or runner, and `cosign verify` with the identity of that workflow succeeds; then delete the test tag or package version and the temporary workflow

## 2. Deploy files

- [ ] 2.1 In `deploy/compose/compose.yaml` set `image: ghcr.io/nickhartjes/specdeck-signalling:${SPECDECK_VERSION:?Set SPECDECK_VERSION in .env}`, keeping `build:`; add `SPECDECK_VERSION` to `.env.example` with a comment; update the header comment (decision 6); verify `docker compose config` resolves the image with the variable set and fails with the message without it
- [x] 2.2 In `deploy/k8s/base` change the signalling image to `ghcr.io/nickhartjes/specdeck-signalling` with an `images:` entry (`newTag: "1"`) in `kustomization.yaml`, and update `deploy/k8s/README.md` (pinning in an overlay); verify `kubectl kustomize deploy/k8s/base` shows the ghcr image with tag `1`, and `kubectl kustomize deploy/k8s/local` still shows `specdeck-signalling:local` with `imagePullPolicy: Never`

## 3. Docs and first release

- [x] 3.1 Update `docs/site/src/content/docs/self-host/`: `docker-compose.md` (pull a pinned version by default, build from source with `--build` as the alternative, update by changing `SPECDECK_VERSION` and `docker compose pull`), `kubernetes.md` (the published image, pin a version or digest in an overlay, building and pushing your own as the alternative), `overview.md` (the web viewer's code paragraph: verifying the published image with `cosign verify` from design decision 4, or building it yourself); verify the docs site builds and each command in the changed pages matches the deploy files
- [ ] 3.2 After the first release with an image: make the `specdeck-signalling` package public and linked to `nickhartjes/specdeck` in its package settings if it is not; verify, signed out of ghcr.io, `docker pull ghcr.io/nickhartjes/specdeck-signalling:<version>` and the `cosign verify` command from the docs succeed, and that `latest`, `<major>.<minor>` and `<major>` point at the same digest
- [x] 3.3 Remove the `container publishing` item from `todo.txt`; verify the Helm chart item still says it needs the published image

## Workflow follow-up

- Archive the change after the first release has verified 3.2.
