# Spec Delta

## Purpose

The published container image of the sharing service (signalling with the web viewer built in), so teams can self-host by pulling an image whose origin they can verify instead of building it from source.

## ADDED Requirements

### Requirement: Published on every release
Every release SHALL publish the signalling image, built from the release's tagged commit with the web viewer included, to `ghcr.io/nickhartjes/specdeck-signalling`. Anyone SHALL be able to pull it without signing in.

#### Scenario: Anonymous pull
- **WHEN** someone who is not signed in to ghcr.io runs `docker pull ghcr.io/nickhartjes/specdeck-signalling:<version>` for a released version
- **THEN** the image downloads

#### Scenario: Same service as built from source
- **WHEN** the published image for a version runs
- **THEN** it answers `/health`, serves the web viewer at `/join`, and runs as the non-root user 10001, as an image built from that version's `docker/signalling.Dockerfile` does

### Requirement: Release tags
Every release SHALL tag the image with its exact version, without the `v`. A stable release SHALL also move the `<major>.<minor>`, `<major>` and `latest` tags to it. A prerelease SHALL get only its exact tag.

#### Scenario: Stable release
- **WHEN** `v1.6.0` is released
- **THEN** `1.6.0`, `1.6`, `1` and `latest` all name that release's image

#### Scenario: Prerelease
- **WHEN** `v1.7.0-rc.1` is released
- **THEN** `1.7.0-rc.1` names its image, and `1.7`, `1` and `latest` still name the previous stable release

### Requirement: Platforms
The image SHALL run on `linux/amd64` and `linux/arm64`, under one tag, so the container runtime picks the matching platform.

#### Scenario: ARM server
- **WHEN** an arm64 server pulls a released tag
- **THEN** it gets the arm64 image and the service starts

### Requirement: Verifiable origin
Every published image SHALL be signed with a keyless Sigstore signature whose identity is this repository's release workflow at the release tag, recorded in the public transparency log. No long-lived signing key SHALL exist.

#### Scenario: Verify a release
- **WHEN** a self-hoster runs `cosign verify` on a released tag, requiring the issuer `https://token.actions.githubusercontent.com` and the identity of `.github/workflows/release.yml` in `nickhartjes/specdeck` at that tag
- **THEN** verification succeeds

#### Scenario: Image from elsewhere
- **WHEN** the same command runs on an image that was not pushed by the release workflow, for example one built and pushed by hand
- **THEN** verification fails

### Requirement: Release only with its image
A GitHub release SHALL NOT be published until its image is pushed and signed.

#### Scenario: Image build fails
- **WHEN** the image fails to build, push or sign for a tag
- **THEN** that tag's release stays a draft
