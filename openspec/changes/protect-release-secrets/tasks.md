# Tasks

## 1. Checks that guard the workflows

- [ ] 1.1 Add `scripts/check-workflows.sh`: fail when a `uses:` line in `.github/workflows/*.yml` that is not a local path has no 40-character hexadecimal reference, or when a workflow has no top-level `permissions:` (design decisions 3 and 4); add it to `just check`; verify it fails on the workflows as they are now and names each line
- [ ] 1.2 Pin every third-party action in all seven workflows to a commit SHA with the version in a comment (design decision 3). Take each SHA from the action's release page for the version in use now; verify with 1.1 and `actionlint`
- [ ] 1.3 Add `helpers:pinGitHubActionDigests` to `extends` in `renovate.json`; verify with `npx --yes renovate-config-validator renovate.json`
- [ ] 1.4 Add `permissions: contents: read` to `ci.yml` and `docs.yml`, move `actions: write` in `nix.yml` to the job with a comment saying why it is not narrower, and set `persist-credentials: false` on every checkout except the tap checkout in `homebrew.yml` and the one in `renovate-nix-hash.yml` (design decision 4); verify with 1.1, `actionlint`, and that CI, Docs and Nix pass on the pull request

## 2. The update key out of the build

- [ ] 2.1 In `release.yml`'s `package` job, remove the two `TAURI_SIGNING_*` lines from `env`, the `createUpdaterArtifacts` branch from the build step, and the key condition from the upload step, so the update files are always uploaded and the `.sig` patterns are gone from the matrix (design decision 1); verify with `actionlint` and by reading that no step of the job names the key
- [ ] 2.2 On macOS, make `SpecDeck-<arch>.app.tar.gz` with `tar` after the build (design decision 2). Before relying on it, download `SpecDeck-arm64.app.tar.gz` from the 1.8.0 release and compare `tar -tzf` of both; verify the top-level entry and the layout match, or switch to the fallback in design decision 2 and say so in design.md
- [ ] 2.3 Add the `sign-updates` job as in design decision 1: `environment: update-signing`, sparse checkout without credentials, the CLI at the lockfile's version installed with `npm install --ignore-scripts --no-save`, download, `tauri signer sign --app-version`, upload the `.sig` files, and the existing warning when the key is missing; verify with `actionlint` and 1.1
- [ ] 2.4 Check locally that the CLI runs after an install without lifecycle scripts: in an empty folder, `npm install --ignore-scripts --no-save @tauri-apps/cli@<lockfile version>`, generate a test key with `tauri signer generate`, sign a file, and verify the `.sig` decodes to a minisign signature whose trusted comment holds the file name and the version
- [ ] 2.5 Make `publish` need `sign-updates` and `checksums`, set `save-if: false` on the Rust cache in `release.yml` (design decision 9), and rewrite the workflow's header comment: which environment holds which secret, and that only `sign-updates` reads the update key; verify with `actionlint`
- [ ] 2.6 After the maintainer has done 8.2: ask before pushing a prerelease tag; then verify in the run and the release that (a) no `package` step has the key in its environment, (b) the four update files and four `.sig` files are in the release, (c) each signature verifies against the public key in `tauri.conf.json` (`minisign -V` after decoding, or a small script), (d) `latest.json` is absent for the prerelease as before or, if present, names the four files, and (e) `gh cache list` has no entry for the tag

## 3. Checksums and provenance

- [ ] 3.1 In `package`, write the SHA-256 of every uploaded file to `sums-<platform>.txt` and upload it as a workflow artifact (design decision 7); verify the file lists exactly the names the job uploads
- [ ] 3.2 Add the `checksums` job: compare the release's files with the recorded hashes, write and upload `SHA256SUMS`, and attest with `actions/attest-build-provenance` and `subject-checksums` (design decision 7); verify with `actionlint`, and that the pinned version of the action accepts `subject-checksums`
- [ ] 3.3 On the prerelease of 2.6, verify that `sha256sum --check --ignore-missing SHA256SUMS` passes for a downloaded installer, that `gh attestation verify <file> --repo nickhartjes/specdeck` succeeds and names `release.yml` at the tag, and that it fails for a file changed by one byte

## 4. Homebrew

- [ ] 4.1 In `homebrew.yml`, refuse a tag that is not `vMAJOR.MINOR.PATCH` as the first step, and a release that is a draft or a prerelease (design decision 6); verify with `actionlint`
- [ ] 4.2 In `scripts/render-cask.sh`, check the version, both hashes and the repository name before printing anything, and add `scripts/render-cask.test.sh` to `just check`: one good call whose output equals the current cask, and one failing call per bad argument, including a version with a quote; verify the test passes and fails when a check is removed
- [ ] 4.3 After merge, start `homebrew.yml` by hand with the tag `v0.0.0";x` and with the prerelease tag of 2.6; verify both runs fail before the tap is checked out, and that the tap has no new commit

## 5. Renovate without the token

- [ ] 5.1 In `renovate-nix-hash.yml`, check out with `persist-credentials: false`, push with the workflow token passed to `git push` in that step only, always start `ci.yml` and `nix.yml` for the new commit, and remove `HAS_TOKEN`, the secret and the warning (design decision 5); rewrite the header comment; verify with `actionlint` and 1.1
- [ ] 5.2 Remove the token from `README.md`'s CI section (what it was for, and that it expires)
- [ ] 5.3 On the next Renovate pull request that changes the npm dependencies, verify the hash commit is pushed, CI and Nix run for it, and their checks show on the pull request and satisfy any required checks; if they do not, stop and raise the fallback in design decision 5 with the maintainer

## 6. Relay manifests and tool versions

- [ ] 6.1 Set the Kubernetes relay to the coturn version in `deploy/compose/compose.yaml`, drop arguments that version refuses, and set `--user-quota=10` with a comment that explains it (design decision 8); verify with `docker run` of that image and the manifest's arguments that coturn stays up for 10 seconds, and with `just local-test` where Colima is available
- [ ] 6.2 Add `scripts/check-relay-manifests.sh` to `just check` (same coturn tag in both files; per-credential quota below the total in each) and extend `ci.yml`'s `relay` job to start coturn with the Kubernetes arguments too; verify the script fails on the manifests as they are now and passes after 6.1
- [ ] 6.3 Make the Dockerfile's pnpm version a build argument whose default equals `mise.toml`, pass it in `ci.yml` and `release.yml`, and fail `just check` when the two differ (design decision 9); verify `docker build -f docker/server.Dockerfile .` still builds
- [ ] 6.4 Update `docs/site/src/content/docs/self-host/configuration.md` (the `--user-quota` row: at most half of the total, no more than 40) and `kubernetes.md` (raise the port range, the Service's ports and both quotas together); verify with `just docs-build`

## 7. Dependency audit

- [ ] 7.1 Add `.github/workflows/audit.yml` (weekly and by hand; `contents: read`, `issues: write`): `cargo audit`, `pnpm audit --prod` in the root, `docs/site` and `docs/video`, and the vendored `datachannel` version against crates.io; one issue "Dependency audit" that is created, updated or closed (design decision 9); add an empty `.cargo/audit.toml` with a comment on how to ignore an advisory with a reason and a date; verify with `actionlint` and 1.1
- [ ] 7.2 After merge, run it by hand; verify a clean run opens no issue, and that with a temporary known-vulnerable crate version on a branch it opens the issue and a second run updates the same one; record today's findings, if any, in the issue
- [ ] 7.3 Add "Following upstream" to `vendor/datachannel/PATCHED.md`: the audit issue is the signal, and the steps to update (design decision 9)

## 8. Repository settings, by the maintainer

These cannot be done or read from the repository. Each is a manual step; note the outcome in the pull request.

- [ ] 8.1 Check the tag ruleset: only the admin can create, move or delete `v*` tags, with no bypass for others. Check the `release` environment allows `v*` tags and main only, as `release.yml`'s header says
- [ ] 8.2 Create the `update-signing` environment, allowed for `v*` tags only; add `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to it; delete both from `release`. Do this before the tag of 2.6
- [ ] 8.3 After 5.3 passes: delete the `RENOVATE_NIX_HASH_TOKEN` secret and revoke the token in the account's settings
- [ ] 8.4 List who has write access to this repository and to `nickhartjes/homebrew-tap`, and remove what is not needed; check the default workflow token permission is read-only, and turn on "Require actions to be pinned to a full-length commit SHA"

## 9. Documentation

Not a feature of the app, so no README overview line and no video (proposal.md, Impact). The pages that describe releases change:

- [ ] 9.1 `docs/site/src/content/docs/security/known-limits.md`: remove "No checksums or build provenance" and "The release workflow trusts tagged actions", and point the remaining "Installers are not signed by Apple or Microsoft" item to checking a download
- [ ] 9.2 `docs/site/src/content/docs/security/your-computer.md`, "How releases are signed": add rows for checksums and build provenance with the two commands, and replace the sentence that says there are none
- [ ] 9.3 `docs/site/src/content/docs/install.md`: add "Check your download" with the two commands and what a good result looks like, linked from the first-launch section
- [ ] 9.4 `README.md`: the Releases section (the signing job, `update-signing`, `SHA256SUMS` and provenance) and the CI section (pinned actions, the audit workflow)
- [ ] 9.5 Verify with `just docs-build`, and read the three pages against a real release once 2.6 and 3.3 have passed: every command on them works as written

## 10. After the first final release

- [ ] 10.1 Ask `release-and-verification` 5.1 to 5.3 to run against this release: an installed app on macOS, Linux and Windows accepts an update signed by `sign-updates`. Record here only that they passed; the tasks stay in that change
