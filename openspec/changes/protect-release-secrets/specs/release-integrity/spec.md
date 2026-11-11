# Spec Delta

## Purpose

Keeps what SpecDeck publishes trustworthy: release secrets reach only the code that needs them, the workflows' third-party code and tokens are constrained, and every download can be checked against the build that made it.

## ADDED Requirements

### Requirement: Update key only where updates are signed
The update signing key and its password SHALL be available only to the release job that signs the update files. That job SHALL NOT build the app or install the project's dependencies, and its only tool beyond the runner's own SHALL be the signing tool at the version the lockfile pins. No job that builds, bundles or tests SHALL be able to read the key.

#### Scenario: Build job
- **WHEN** the release workflow builds the installers on any platform
- **THEN** the update key and its password are not in the environment of any step of that job, and the job is not allowed to read the environment that holds them

#### Scenario: Signing job
- **WHEN** the signing job runs for a release tag
- **THEN** it downloads the update files from the draft release, signs each one, uploads the signatures, and runs no build script, package lifecycle script or cache restore

#### Scenario: Updates still verify
- **WHEN** an installed app checks a release whose update files were signed by the signing job
- **THEN** it accepts the update, as it accepts one signed during the build before this change

#### Scenario: Key missing
- **WHEN** a tag is released while the update key is not configured
- **THEN** the release is published without update files and without `latest.json`, with a warning in the run, as before

### Requirement: Actions pinned by commit
Every third-party action in every workflow SHALL be referenced by a full commit SHA, with its version in a comment on the same line. The dependency updater SHALL propose new pins when an action releases. A workflow change that adds an action by tag or branch SHALL fail a check.

#### Scenario: Moved tag
- **WHEN** an action's maintainer, or someone who took over their account, moves a version tag to other code
- **THEN** the workflows keep running the commit they pin

#### Scenario: New version
- **WHEN** an action publishes a new version
- **THEN** the weekly dependency update proposes the new commit SHA with the new version in the comment

#### Scenario: Action added by tag
- **WHEN** a pull request adds `uses: owner/action@v3` to a workflow
- **THEN** the check fails and names the line

### Requirement: Least privilege for workflow tokens
Every workflow SHALL declare the permissions of its token, and grant write permissions only to the jobs that need them. A job that does not push SHALL NOT leave the checkout's credential in the repository's git configuration.

#### Scenario: Workflow without a permissions block
- **WHEN** a workflow file has no top-level `permissions:`
- **THEN** the check fails and names the file

#### Scenario: Pull request run
- **WHEN** CI runs for a pull request
- **THEN** its token can read the repository's contents and nothing else, except where a job states a further permission and why

### Requirement: No long-lived token for dependency updates
The workflow that updates the Nix hash on dependency-update branches SHALL push with the workflow's own short-lived token. The repository SHALL hold no personal access token for it.

#### Scenario: Hash changed
- **WHEN** a dependency update changes the npm dependencies
- **THEN** the workflow commits the new hash to that branch and starts CI and the Nix build for the new commit, and their results show on the pull request

#### Scenario: Branch with a changed workflow
- **WHEN** someone with write access pushes a branch whose workflow prints its secrets
- **THEN** it can print no token that outlives the run or that may create release tags

### Requirement: Cask only for a final release
The Homebrew workflow SHALL write the cask only for a tag of the form `vMAJOR.MINOR.PATCH` whose release is published and is not a prerelease. The script that renders the cask SHALL refuse a version, a hash or a repository name that is not of the expected form, and write nothing.

#### Scenario: Crafted tag
- **WHEN** the workflow is started by hand with a tag that contains a quote, a space or any character outside `vMAJOR.MINOR.PATCH`
- **THEN** it fails before it downloads anything, and the tap is not changed

#### Scenario: Draft or prerelease
- **WHEN** the workflow is started for a tag whose release is a draft or a prerelease
- **THEN** it fails and the tap is not changed

#### Scenario: Wrong hash
- **WHEN** the render script is given a hash that is not 64 hexadecimal characters
- **THEN** it exits with an error and prints no cask

### Requirement: Checksums with every release
Every release SHALL include a `SHA256SUMS` file that lists the SHA-256 hash of every installer and update file in it, in the format `sha256sum --check` reads. The release SHALL NOT be published without it.

#### Scenario: Check a download
- **WHEN** a user downloads an installer and `SHA256SUMS` from a release and runs `sha256sum --check --ignore-missing SHA256SUMS`
- **THEN** the installer is reported as OK

#### Scenario: Checksums fail to build
- **WHEN** the checksums job fails for a tag
- **THEN** that tag's release stays a draft

### Requirement: Build provenance for installers
Every installer and update file in a release SHALL have a build provenance attestation, signed by GitHub for this repository's release workflow at the release tag and recorded in the public transparency log. The release SHALL NOT be published without it.

#### Scenario: Verify provenance
- **WHEN** a user runs `gh attestation verify <file> --repo nickhartjes/specdeck` on a downloaded installer
- **THEN** verification succeeds and names `.github/workflows/release.yml` at the release tag

#### Scenario: File from elsewhere
- **WHEN** the same command runs on a file that the release workflow did not produce
- **THEN** verification fails

### Requirement: Dependencies watched for advisories
A scheduled workflow SHALL check the Rust and npm dependencies against the public advisory databases at least once a week, and SHALL check whether upstream has released a newer `datachannel` than the vendored one. A finding SHALL be reported in one open issue, not only in a run log.

#### Scenario: New advisory
- **WHEN** an advisory is published for a crate or npm package in a lockfile
- **THEN** the next scheduled run opens an issue that names the package and the advisory, or adds to the open one

#### Scenario: Newer upstream datachannel
- **WHEN** crates.io has a newer `datachannel` than the version in `vendor/datachannel`
- **THEN** the issue says so, with the upstream version

#### Scenario: Nothing found
- **WHEN** a run finds no advisory and no newer upstream version
- **THEN** it opens no issue and closes the one it opened before

### Requirement: Release runs save no cache
The release workflow SHALL NOT save build caches. It MAY restore caches saved from the main branch.

#### Scenario: Tag build
- **WHEN** the release workflow finishes for a tag
- **THEN** the repository's cache list has no entry scoped to that tag
