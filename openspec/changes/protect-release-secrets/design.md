# Design

## Context

See proposal.md - Why. What the workflows do today, read on 2026-11-10:

- **`release.yml`** runs on `v*` tags: `draft` → `package` (four platforms) and `image` → `image-merge` → `publish` → `homebrew`.
  - `package` has `environment: release` and sets `TAURI_SIGNING_PRIVATE_KEY` and its password as job-level `env`. When the key is set, it adds `--config '{"bundle":{"createUpdaterArtifacts":true}}'`, and `tauri build` writes the update files and signs them. Tauri fails the build when that option is on and the key is missing, so today the key has to be in the build step.
  - The update files are: on macOS `macos/*.app.tar.gz`, which exists only with `createUpdaterArtifacts`; on Windows and Linux the installer itself (`.exe`, `.AppImage`). Each gets a `.sig`.
  - `publish` downloads the `*.sig` files from the draft, writes `latest.json` with `scripts/latest-json.mjs`, and publishes.
  - `Swatinem/rust-cache@v2` has `cache-on-failure: true` and no `save-if`, so each tag saves a cache scoped to that tag, which no later run can restore. `ci.yml` has `save-if: main`.
- **Actions** are pinned by major tag everywhere, except `sigstore/cosign-installer@v4.1.3`. `renovate.json` extends `config:recommended` and groups the `github-actions` manager's updates into one pull request.
- **`renovate-nix-hash.yml`** checks out with `secrets.RENOVATE_NIX_HASH_TOKEN || github.token`, runs `cachix/install-nix-action@v31`, and pushes. It already has a path for a missing secret: push with the workflow token, then `gh workflow run ci.yml` and `nix.yml` on the branch.
- **`homebrew.yml`** takes `inputs.tag` from `workflow_call` or `workflow_dispatch`, with no check, and passes `${TAG#v}` to `scripts/render-cask.sh`, which writes it into `version "$version"`.
- **Permissions:** `ci.yml` and `docs.yml` have no top-level `permissions:`; `nix.yml` has `contents: read` and `actions: write` for every run, pull requests included.
- **Tool versions:** `@tauri-apps/cli` 2.12.4 is in the root `package.json` and `pnpm-lock.yaml`. `mise.toml` has pnpm 12.18.0; `docker/server.Dockerfile` installs 12.16.1.
- **Relay:** `deploy/k8s/base/coturn.yaml` runs `coturn/coturn:4.7` with 20 relay ports, `--user-quota=40` and `--total-quota=20`. `deploy/compose/compose.yaml` runs `coturn/coturn:4.18` with 100 ports, 40 and 100. `ci.yml`'s `relay` job starts only the Compose relay; its comment says 4.18 refuses `--no-dtls`, which the Kubernetes manifest passes.

`tauri signer sign --help` (CLI 2.12.4, run locally) confirms: it takes a file, reads the key from `--private-key` or `TAURI_SIGNING_PRIVATE_KEY`, the password from `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, writes `<file>.sig`, and has `--app-version`, which `tauri build` sets by itself and which has to be passed when signing by hand.

Not verified, and so tasks of their own:
- that a `.app.tar.gz` made with `tar` is accepted by the updater like the one `tauri build` makes (decision 2);
- that the CLI runs after `npm install --ignore-scripts` (decision 1);
- that workflow runs started with `gh workflow run` satisfy the pull request's required checks, if any are set (decision 5);
- the repository's settings: rulesets, environments, collaborators.

## Goals / Non-Goals

**Goals:**
- A compromised action, dependency or build script in the build cannot read the update key.
- A moved tag of an action changes nothing until a pull request is merged.
- No secret that outlives a run is readable from a branch someone can push.
- A user can check a download against the release workflow that made it, without paying for anything.
- Releases keep working exactly as before for users: same file names, same key, same `latest.json`.

**Non-Goals:**
- Apple or Windows code signing, an SBOM, reproducible builds.
- Protecting against a compromised GitHub, runner image or Tauri CLI: the signing job has to trust the tool that signs.
- Rotating the update key.

## Decisions

### 1. A `sign-updates` job with its own environment

```
draft → package (×4) ─┬→ sign-updates ─┬→ publish → homebrew
        image → image-merge ───────────┘
                      └→ checksums ────┘
```

- The two update key secrets move from the `release` environment to a new environment, `update-signing`, allowed for `v*` tags only. `package` keeps `environment: release` (sharing service URL, Apple and Windows certificates) and loses the two `env` lines. A job reads only its own environment's secrets, so no build step can reach the key, whatever it runs.
- `sign-updates` runs on `ubuntu-latest` with `environment: update-signing` and `permissions: contents: write`. Its steps:
  1. Exit with the existing warning when the key is not set.
  2. Sparse checkout of `package.json` and `pnpm-lock.yaml` only, `persist-credentials: false`.
  3. Read the `@tauri-apps/cli` version from the lockfile and install exactly that version into an empty folder with the runner's own npm: `npm install --ignore-scripts --no-save @tauri-apps/cli@<version>`. No mise, no pnpm workspace install, no cache.
  4. Download the four update files (the names in `scripts/latest-json.mjs`) from the `update-<platform>` workflow artifacts the `package` jobs uploaded, not from the draft, and the `sums-*` artifacts (decision 7).
  5. Fail unless each file's hash equals the one its `package` job recorded. The signed files are verified first: only jobs of this run can write its artifacts, while anyone with write access can replace a draft asset, and a file swapped in before signing would get a valid signature from the real key.
  6. For each file, `tauri signer sign --app-version <version> <file>`, with the key and password in the environment of this step only.
  7. `gh release upload --clobber` the four files and their `.sig` files, so the draft holds exactly what was signed.
- The signing tool is the one piece of third-party code the job cannot avoid. Installing it by exact version without lifecycle scripts keeps it to that.
- `package` uploads the update files always, signatures never. `publish` is unchanged: it finds the `.sig` files, or warns and publishes without `latest.json`.
- Accepted limit: `sign-updates` signs what this run's `package` jobs built. A compromised build step can change its own outputs (and `package` has `contents: write`), and they are then signed; a compromised build makes a malicious app either way. What this decision guarantees is that the key cannot be copied out, and that nothing from outside the run gets signed.

Alternatives:
- **Key only on the `Build the installers` step:** not enough. That step runs every build script and proc macro, which is most of the risk.
- **Sign inside `package` in a later step:** the key would still be an environment secret that job can read, and earlier steps can leave code behind for later ones (a changed `PATH`, a patched binary in `node_modules`).
- **Sign with `minisign` or `rsign2` directly:** Tauri wraps the minisign format in base64 and sets its own trusted comment. Reproducing that by hand is fragile, and a mistake only shows as updates that fail.

### 2. The macOS update file is made in the build, without the key

Without `createUpdaterArtifacts`, Tauri writes no `macos/*.app.tar.gz`. The `package` job makes it after the build, on macOS only: `tar -czf SpecDeck.app.tar.gz -C target/release/bundle/macos SpecDeck.app`. The other platforms' update files are the installers the job already uploads.

The archive must have the layout the updater expects: one `SpecDeck.app` folder at the top. Task 2.2 compares it with the `.app.tar.gz` of the 1.8.0 release (`tar -tzf` on both) before anything else is built on it. If the layouts differ in a way `tar` cannot match, the fallback is to keep `createUpdaterArtifacts` on macOS with a throwaway key generated in the job (`tauri signer generate`), discard its signature, and sign again in `sign-updates`.

### 3. Actions pinned by SHA, kept current by Renovate

- Every `uses:` of a third-party action, in all seven workflows, becomes `owner/action@<40-character SHA> # vX.Y.Z`. The local `./.github/workflows/homebrew.yml` reference stays.
- `renovate.json` adds `helpers:pinGitHubActionDigests` to `extends`. Renovate then updates the SHA and the comment together, inside the existing "GitHub Actions" group and the Monday schedule. The group's rule matches `matchDatasources: ["github-tags"]`; the first Renovate pull request for the workflows' actions after the merge shows whether digest updates still land in that group; if they do not, the datasource condition goes. No task covers this: it needs Renovate on GitHub, and a separate pull request is the only harm.
- A check in `just check` (a few lines of shell: every `uses:` line that is not a local path has a 40-character hexadecimal reference) fails a workflow change that adds a tag. The same script fails a workflow without a top-level `permissions:` (decision 4).

Alternative: the repository setting "Require actions to be pinned to a full-length commit SHA" does the same on GitHub's side. It is listed as a manual task as well; the script is what a pull request sees first.

### 4. Token permissions

- `ci.yml` and `docs.yml` get top-level `permissions: contents: read`. `docs.yml`'s deploy job keeps its own block.
- `nix.yml` keeps `contents: read` at the top and moves `actions: write` to the job, with the cache-delete step guarded as today to pushes on main. GitHub cannot grant a permission per step, so the job has it on pull requests too; forks get a read-only token anyway. The gain is small, and the alternative, a second job for the save, costs a second Nix install. Decision: keep one job, state the reason in a comment.
- `persist-credentials: false` on every checkout except the two that push: the tap checkout in `homebrew.yml` (deploy key) and `renovate-nix-hash.yml` (decision 5).

### 5. No personal access token for the Nix hash

Options for `RENOVATE_NIX_HASH_TOKEN`:

| Option | Stops a pushed branch reading it | Upkeep |
|---|---|---|
| Environment restricted to `renovate/**` | No: anyone who can push can name a branch `renovate/x` | Token renewal stays |
| Environment with a required reviewer | Yes | An approval for every Renovate branch push, and token renewal |
| GitHub App token | No: the app's private key is a secret like the token | An app to create and keep |
| **Remove it; push with the workflow token** | Yes: nothing long-lived exists | None |

Decision: remove it. The workflow already has this path: a push with the workflow token starts no workflows, so it runs `gh workflow run ci.yml` and `nix.yml` on the branch. Those runs are for the branch's new head commit, so their checks show on the pull request.

- The checkout sets `persist-credentials: false`, so `cachix/install-nix-action` and the Nix build run with no credential in `.git/config`. The push step passes the token itself: `git push "https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}" "HEAD:${GITHUB_REF_NAME}"`.
- The warning about the missing secret and the `HAS_TOKEN` switch go.
- Cost: the two runs are started by `workflow_dispatch`, not by the push. If branch protection requires checks by name, they must be the job names both events produce; task 5.3 checks this on one Renovate pull request, and names the fallback (the environment with a required reviewer) if it fails.

### 6. The Homebrew workflow validates its input

- First step, before any checkout or download: `[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]`, else fail. `TAG` is already passed through `env`, not interpolated into the script.
- Then `gh release view "$TAG" --json isDraft,isPrerelease`: fail unless both are false.
- `scripts/render-cask.sh` checks its own arguments, because it can be run by hand: version `MAJOR.MINOR.PATCH`, both hashes 64 lowercase hexadecimal characters, repository `owner/name` of `[A-Za-z0-9._-]`. On a mismatch it prints which argument to stderr and exits non-zero before printing anything.
- A shell test next to the script covers a good call and each bad argument, run from `just check`.

### 7. Checksums and provenance

- Each `package` job writes the SHA-256 of every file it uploads into `sums-<platform>.txt` and uploads that as a workflow artifact, outside the release.
- A `checksums` job (`needs: [package, sign-updates]`, `permissions: contents: write, id-token: write, attestations: write`, no environment):
  1. downloads the `sums-*` artifacts and the release's installers and update files;
  2. fails unless every file's hash equals the one its build job recorded, so a draft asset replaced between the jobs is caught (the update files were already checked before signing, decision 1; this covers the other installers);
  3. writes `SHA256SUMS` (installers and update files; not the `.sig` files, `latest.json` or a `SHA256SUMS` from an earlier attempt, so a re-run passes);
  4. runs `actions/attest-build-provenance` with `subject-checksums: SHA256SUMS`, then uploads `SHA256SUMS`, so a draft never has checksums without provenance.
- `publish` gets `needs: [package, image-merge, sign-updates, checksums]`, so a release without checksums or provenance stays a draft.
- The attestation is made by a job that did not build the files. What it states is true and useful: these hashes were produced by `release.yml` at this tag in this repository. Step 2 ties them to the build jobs.
- Verifying, as the docs will say: `sha256sum --check --ignore-missing SHA256SUMS`, and `gh attestation verify <file> --repo nickhartjes/specdeck`.

Alternative: attest inside each `package` job. That gives the build job an OIDC token, which build scripts could use to mint attestations of their own. Rejected for the same reason as decision 1.

### 8. The Kubernetes relay

- **Version:** both manifests run the coturn version Compose runs now. The Kubernetes arguments drop `--no-dtls` if that version refuses it, as the `relay` job's comment says. `ci.yml`'s `relay` job also starts coturn with the Kubernetes manifest's arguments (read from the YAML, with the two `$(…)` variables filled in) and checks that it stays up.
- **Quotas:** the shipped manifest has 20 relay ports and `--total-quota=20`, which the docs describe as room for about 10 relayed viewers. `--user-quota` becomes 10: one credential gets at most half the relay, and a host can still relay as many viewers as that relay has room for. `configuration.md` and `kubernetes.md` say to raise the port range, the Service's port list, `--total-quota` and `--user-quota` together, with `--user-quota` at most half of the total and no more than 40.
- **Check:** `scripts/check-relay-manifests.sh`, run from `just check`: both files name the same `coturn/coturn` tag, and in each the per-credential quota is lower than the total quota (the Compose default included).

Alternative: 60 relay ports in the Kubernetes base, keeping `--user-quota=40`. It changes every self-hoster's load balancer for a default most will resize anyway.

### 9. Small ones

- **Rust cache in `release.yml`:** `save-if: false`. The job still restores main's cache.
- **pnpm:** the Dockerfile takes the version as a build argument, and `ci.yml`, `release.yml` and the docs' build command pass the one from `mise.toml`. `just check` fails when the Dockerfile's default differs from `mise.toml`.
- **Audit:** a new `audit.yml`, weekly and by hand, `permissions: contents: read, issues: write`. It runs `cargo audit` (installed with `cargo install --locked` at a pinned version, from the runner's Rust), `pnpm audit --prod` for the root, `docs/site` and `docs/video`, and compares `vendor/datachannel`'s version with the newest on crates.io. It writes one issue titled "Dependency audit" with the findings, updates it on the next run, and closes it when a run is clean. Dependabot's security updates stay as they are; this covers what they miss: crates with an advisory and no fixed version, yanked crates, and the vendored copy.
- **`vendor/datachannel/PATCHED.md`** gets a section "Following upstream": the audit issue is the signal; to update, vendor the new version, re-apply the one patch, and run `datachannel_close.rs`.

## Risks / Trade-offs

- **Updates signed by hand are refused by installed apps** (wrong archive layout, missing version in the signature) → Verified on a prerelease tag before a final release: task 2.6 signs, then checks each signature with the public key from `tauri.conf.json`. Prereleases are never offered, so a broken one harms nobody. `release-and-verification` 5.1 to 5.3 then confirm on real machines.
- **The first release after this change has no update files, because a secret was moved wrongly** → the run warns, and installed apps stay on their version until the next release; nothing breaks. Task 2.6 catches it first.
- **Renovate's digest pins make the weekly Actions pull request noisier** → they stay in the one group.
- **Dispatched CI runs do not count as required checks** → see decision 5; checked in task 5.3.
- **The audit issue becomes noise** (advisories with no fix) → `cargo audit` reads an ignore list, `.cargo/audit.toml`, where each entry has a reason and a date.
- **A lower `--user-quota` on Kubernetes refuses allocations for a host with more than 10 relayed viewers** → the 20 ports ran out there already; the docs say how to resize.

## Migration Plan

1. Merge the workflow and script changes with the secrets still in `release`: `sign-updates` names `update-signing`, which does not exist yet, so do step 2 before the next tag.
2. Maintainer, in the repository's settings: create `update-signing` (tags `v*` only), add the two update key secrets, delete them from `release`; delete `RENOVATE_NIX_HASH_TOKEN` and revoke the token.
3. Push a prerelease tag (`vX.Y.Z-rc.1`) and run the checks of task 2.6 and 7.2.
4. Release normally.

Rollback: revert the workflow commit and put the two secrets back in `release`. The key itself never changes, so installed apps are unaffected either way.

**If the key ever has to be rotated:** installed apps trust only the key they were built with. A rotation needs one release signed with the old key that carries the new public key, and every later release signed with the new one; apps that skip that release must be reinstalled. That is why keeping the key out of the build matters more than anything else here.

## Open Questions

- Should the app also set the updater's `requireSignedVersion`, so a tampered `latest.json` cannot pair a new version number with an older signed file? `--app-version` in decision 1 makes the signatures ready for it. It changes the app, so it belongs in a change to `app-updates`.
- Does a tag ruleset exist that lets only the admin create `v*` tags, as `release.yml`'s header says? It cannot be read from the repository (task 8.1).
