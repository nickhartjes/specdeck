# Proposal

## Why

The security review of 2026-11-10 found that the release pipeline is the easiest way to reach every SpecDeck user at once, and that its most valuable secret is the least protected:

- **The update signing key is visible to the whole build.** `release.yml` sets `TAURI_SIGNING_PRIVATE_KEY` and its password as job-level environment of the `package` job. Every step sees them: `jdx/mise-action@v5` and `Swatinem/rust-cache@v2` (pinned by tag), `pnpm install`, and every build script, proc macro and Vite plugin of every dependency. The app trusts one public key (`tauri.conf.json`), so a stolen key is permanent: whoever holds it can sign updates that every installed app accepts.
- **Actions are pinned by tag.** A tag can be moved to other code. This is on the known-limits list as "The release workflow trusts tagged actions".
- **`RENOVATE_NIX_HASH_TOKEN` is a repository secret.** It is a personal access token with Contents write, readable by any workflow on any pushed branch, and it stays in `.git/config` while `cachix/install-nix-action` runs. With it, someone with write access can create a `v*` tag as the admin, which the `release` environment trusts. It also expires and has to be renewed.
- **`homebrew.yml` accepts any tag when started by hand** and `scripts/render-cask.sh` writes the version into Ruby without checking it. A release under a crafted tag name would put code into the cask that runs on `brew install`.
- **Downloads cannot be checked.** Releases have no checksums and no build provenance (known limit "No checksums or build provenance").
- **Smaller faults:** `release.yml` saves a Rust cache per tag that no run can restore; the Dockerfile and `mise.toml` pin different pnpm versions; the Kubernetes relay has `--user-quota=40` above `--total-quota=20` and a different coturn version than Compose; nothing scans dependencies for advisories on a schedule, and nothing says how the vendored `datachannel` follows upstream.

All of it can be fixed without paid certificates or services.

## What Changes

- **The update key is used by one job that builds nothing.** The `package` job builds the installers and the update files without the key. A new `sign-updates` job downloads the update files from the draft release and signs them with `tauri signer sign`. The key moves to an environment of its own, `update-signing`, so the build jobs cannot read it at all.
- **Actions are pinned by commit SHA** in every workflow, with the version in a comment, and Renovate keeps the pins current.
- **Least privilege for the workflows' tokens.** `ci.yml` and `docs.yml` get a `permissions:` block, `nix.yml` grants `actions: write` only where it saves the cache, and checkouts in jobs that do not push set `persist-credentials: false`.
- **The Renovate token is removed.** `renovate-nix-hash.yml` pushes with the workflow's own token and starts CI and Nix for the new commit itself, which it already does when the secret is missing. No long-lived token is left to steal or renew.
- **The Homebrew workflow checks what it is given.** It refuses a tag that is not `vMAJOR.MINOR.PATCH`, and a release that is a draft or a prerelease. `scripts/render-cask.sh` refuses a version, hash or repository name that is not of the expected form.
- **Releases can be verified.** Each release gets a `SHA256SUMS` file and a GitHub build provenance attestation for every installer and update file. The documentation says how to check both.
- **Dependencies are scanned weekly** for known advisories (`cargo audit`, `pnpm audit`), and for a newer upstream `datachannel` than the vendored one. A finding opens or updates one issue.
- **Small fixes:** `release.yml` stops saving a Rust cache; one pnpm version, taken from `mise.toml`; the Kubernetes relay's per-credential quota is below its total quota, it runs the same coturn version as Compose, and a check keeps both true.

Not in this change:
- Signing with an Apple Developer ID or a Windows certificate (paid; the steps for it in `release.yml` stay as they are).
- A software bill of materials, and reproducible builds.
- Rotating the update key. Nothing suggests it leaked; design.md says what a rotation would take.
- The checks of `release-and-verification` (its tasks 1.3 and 5.1 to 5.3 verify the release workflow and the updater on real machines). This change adds no task of that kind twice; its own verification is one prerelease tag, and it asks `release-and-verification` 5.1 to 5.3 to run after it, because they prove that updates signed by the new job are accepted.

## Capabilities

### New Capabilities

- `release-integrity`: how release secrets are kept from code that does not need them, how the workflows' third-party actions and tokens are constrained, what a release publishes so a download can be verified, and how dependencies are watched for advisories.

### Modified Capabilities

- `sharing-service-limits`: the Relay quotas requirement says the per-credential quota is lower than the total quota in every deployment the repository ships.
- `security-documentation`: The app on your computer also covers how to verify a download; the scenario for signing names checksums and provenance.

`app-updates` (Verified updates) and `signalling-image` do not change: updates are signed with the same key and verified the same way, and the image is signed as before.

## Impact

- **Workflows:** `.github/workflows/release.yml` (new `sign-updates` and `checksums` jobs, no key in `package`, no cache save), `homebrew.yml`, `renovate-nix-hash.yml`, `ci.yml`, `docs.yml`, `nix.yml`, and a new `audit.yml`.
- **Scripts and config:** `scripts/render-cask.sh`, a new `scripts/check-relay-manifests.sh`, `renovate.json`, `docker/server.Dockerfile`, `deploy/k8s/base/coturn.yaml`, `justfile`.
- **Repository settings, by the maintainer (not in the repository):** a new `update-signing` environment holding the two update key secrets; removing them from `release`; deleting `RENOVATE_NIX_HASH_TOKEN` and revoking the token; checking the tag ruleset, the environments' rules and who has write access. tasks.md lists them.
- **Documentation:**
  - `docs/site/src/content/docs/security/known-limits.md`: remove "No checksums or build provenance" and "The release workflow trusts tagged actions"; rewrite the Releases section's workarounds.
  - `docs/site/src/content/docs/security/your-computer.md`, "How releases are signed": the table gets checksums and provenance, with the commands to verify.
  - `docs/site/src/content/docs/install.md`: a short "Check your download" section.
  - `docs/site/src/content/docs/self-host/kubernetes.md` and `configuration.md`: the relay's quotas and port count.
  - `README.md`: the Releases and CI sections (the signing job, the removed token, the audit workflow). The overview list gets no new line: this is not a feature of the app, and the security line there already links to the pages above.
  - No video: nothing changes in the app, and verifying a download is two commands on one page.
- **Users:** none for people who install or update. Updates are signed with the same key, so installed apps accept them as before.
- **Self-hosters on Kubernetes:** with the manifests as shipped (20 relay ports), one credential gets at most 10 allocations instead of all 20, and the relay runs the coturn version Compose runs (design decision 8). Whoever widened the relay range sets the quotas as `configuration.md` says.
