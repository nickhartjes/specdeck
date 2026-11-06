# Tasks

## 1. Crate, binary and Dockerfile

- [x] 1.1 `git mv crates/signalling crates/server`, rename the package to `specdeck-server` (description: the sharing service), update the workspace `members`, the crate's own tests and doc comments (`specdeck_signalling` → `specdeck_server`, "signalling service" → "sharing service" where it means the whole service), and `Cargo.lock` (design decision 2); verify `cargo test -p specdeck-server --features web-viewer` passes and `cargo clippy -p specdeck-server --all-targets --features web-viewer -- -D warnings` is clean
- [x] 1.2 `git mv docker/signalling.Dockerfile docker/server.Dockerfile`, build and copy `specdeck-server`, set the entrypoint to `specdeck-server`, and update its header comment and the `.dockerignore` comment; verify `docker build -f docker/server.Dockerfile -t specdeck-server:test .` succeeds and the container runs as user 10001, answers `/health` and serves `/join`
- [x] 1.3 Update the `justfile` (clippy recipe, `local-up` building `specdeck-server:local`, restart, rollout and port-forwards on `deployment/specdeck-server` and `svc/specdeck-server`), and make `local-up` delete the old `specdeck-signalling` Deployment and Service with `--ignore-not-found` (decision 9); verify `just check` passes, and `just local-up` on Colima leaves only `specdeck-server` and `specdeck-turn` in the namespace and the web viewer opens on the port-forward
- [ ] 1.4 Update CI's image job in `.github/workflows/ci.yml` to build `docker/server.Dockerfile` as `specdeck-server:ci`; verify with `actionlint` and with the job passing on the pull request

## 2. Release workflow

- [x] 2.1 In `.github/workflows/release.yml`, push the per-platform digests to `specdeck-server`, give `docker/metadata-action` in `image-merge` both image names from one env value with the old name commented "remove in 1.9.0" (decisions 3, 5), create every tag of both names with one `imagetools create` from the `specdeck-server` digests, and check both names resolve to the same digest; update the header comment; verify with `actionlint`, and verify from metadata-action's output (run locally or in a dry run) that `v1.7.0` yields the four tags for each name and `v1.7.0-rc.1` only the exact tag for each
- [x] 2.2 Sign `<name>@<digest>` for every published name in `image-merge`, so the job fails when either push or signature fails and `publish` stays blocked (decision 4); verify with `actionlint` that `publish` still needs `image-merge` and that the sign step loops over the same env value as the tags

## 3. Deploy files

- [x] 3.1 Compose: rename the service to `server`, use `ghcr.io/nickhartjes/specdeck-server:${SPECDECK_VERSION…}` and `docker/server.Dockerfile`, point the Caddyfile at `server:8080`, and update comments and `.env.example` (decision 7); verify `docker compose config` resolves the new image and that `docker compose up -d --remove-orphans` from a setup running the old `signalling` service leaves only `server`, `caddy` and `turn`, with `/health` answering through Caddy
- [x] 3.2 Kubernetes: `git mv deploy/k8s/base/signalling.yaml deploy/k8s/base/server.yaml`, rename the Deployment, Service, container and labels to `specdeck-server`, and update the `images` entry in `base/kustomization.yaml` and the patch target and local image in `local/kustomization.yaml` (decision 6); verify `kubectl kustomize deploy/k8s/base` and `deploy/k8s/local` render with no `specdeck-signalling` left and the local overlay still uses `imagePullPolicy: Never`

## 4. Docs

- [x] 4.1 Update the self-host docs (`overview.md` incl. the `cosign verify` example on `specdeck-server`, `docker-compose.md` incl. `--remove-orphans` in the update steps, `kubernetes.md`, `configuration.md`, `connect-apps.md`) and `deploy/k8s/README.md` to the new names; call it the sharing service where they describe it; add the transition note (old image name published until 1.9.0) to the overview; verify the docs site builds and every command in the changed pages matches the deploy files
- [x] 4.2 Add an "Upgrading from 1.6" section to the Kubernetes guide (apply the new base, switch the Ingress backend to `specdeck-server`, wait for the rollout, `kubectl delete deployment,service specdeck-signalling`) and run it on Colima's k3s against a namespace deployed from the 1.6.0 base with an Ingress; verify the web viewer and an app connect through the Ingress after each step and no old object is left
- [ ] 4.3 Update `README.md`, `install.md`, `crates/protocol/README.md` and other docs that name the crate, image or Dockerfile; add the 1.9.0 removal of the old name to `todo.txt` and to the release checklist in `release-and-verification`; verify `git grep -n -i 'specdeck-signalling\|signalling\.Dockerfile\|crates/signalling'` outside `openspec/changes/archive` finds only the old-name line in `release.yml`, the transition and upgrade notes, and this change

## 5. Release

- [ ] 5.1 Release `v1.7.0-rc.1`; make the new `specdeck-server` package public and linked to `nickhartjes/specdeck`; verify signed out of ghcr.io that `1.7.0-rc.1` pulls under both names with one digest and that the documented `cosign verify` passes on both names
- [ ] 5.2 Release `v1.7.0` with release notes leading with the new image name and the Kubernetes upgrade steps; verify signed out that `1.7.0`, `1.7`, `1` and `latest` resolve to one digest under both names

## Workflow follow-up

- Archive `publish-signalling-image` before this change is archived, so the `signalling-image` spec it retires exists.
- Archive this change after 5.2.
- In the 1.9.0 release: remove the old image name from `release.yml` and the transition note from the docs.
