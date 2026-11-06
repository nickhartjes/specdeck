# Spec Delta

## Purpose

The published container image of the sharing service (signalling, TURN credentials and the web viewer), so teams can self-host by pulling an image whose origin they can verify instead of building it from source.

## ADDED Requirements

### Requirement: Published on every release
Every release SHALL publish the sharing service's image, built from the release's tagged commit with the web viewer included, to `ghcr.io/nickhartjes/specdeck-server`. Anyone SHALL be able to pull it without signing in.

#### Scenario: Anonymous pull
- **WHEN** someone who is not signed in to ghcr.io runs `docker pull ghcr.io/nickhartjes/specdeck-server:<version>` for a released version
- **THEN** the image downloads

#### Scenario: Same service as built from source
- **WHEN** the published image for a version runs
- **THEN** it answers `/health`, serves the web viewer at `/join`, and runs as the non-root user 10001, as an image built from that version's `docker/server.Dockerfile` does

### Requirement: Release tags
Every release SHALL tag the image with its exact version, without the `v`. A stable release SHALL also move the `<major>.<minor>`, `<major>` and `latest` tags to it. A prerelease SHALL get only its exact tag.

#### Scenario: Stable release
- **WHEN** `v1.7.0` is released
- **THEN** `1.7.0`, `1.7`, `1` and `latest` all name that release's image

#### Scenario: Prerelease
- **WHEN** `v1.8.0-rc.1` is released
- **THEN** `1.8.0-rc.1` names its image, and `1.8`, `1` and `latest` still name the previous stable release

### Requirement: Platforms
The image SHALL run on `linux/amd64` and `linux/arm64`, under one tag, so the container runtime picks the matching platform.

#### Scenario: ARM server
- **WHEN** an arm64 server pulls a released tag
- **THEN** it gets the arm64 image and the service starts

### Requirement: Verifiable origin
Every published image SHALL be signed with a keyless Sigstore signature whose identity is this repository's release workflow at the release tag, recorded in the public transparency log. No long-lived signing key SHALL exist.

#### Scenario: Verify a release
- **WHEN** a self-hoster runs `cosign verify` on a released tag of `ghcr.io/nickhartjes/specdeck-server`, requiring the issuer `https://token.actions.githubusercontent.com` and the identity of `.github/workflows/release.yml` in `nickhartjes/specdeck` at that tag
- **THEN** verification succeeds

#### Scenario: Image from elsewhere
- **WHEN** the same command runs on an image that was not pushed by the release workflow, for example one built and pushed by hand
- **THEN** verification fails

### Requirement: Release only with its image
A GitHub release SHALL NOT be published until its image is pushed and signed, under every name that release publishes.

#### Scenario: Image build fails
- **WHEN** the image fails to build, push or sign for a tag
- **THEN** that tag's release stays a draft

#### Scenario: Old name fails during the transition
- **WHEN** a 1.7.x or 1.8.x release pushes and signs `specdeck-server` but fails to push or sign `specdeck-signalling`
- **THEN** that tag's release stays a draft

### Requirement: Old image name during the transition
Every release before 1.9.0 that publishes `ghcr.io/nickhartjes/specdeck-server`, prereleases included, SHALL also publish its image to `ghcr.io/nickhartjes/specdeck-signalling`, as the same image by digest and with the same tags, signed there in the same way. From 1.9.0 on, releases SHALL publish only `ghcr.io/nickhartjes/specdeck-server`; tags already published under the old name SHALL stay pullable.

#### Scenario: Compose setup on the old name
- **WHEN** a self-hoster whose Compose file names `ghcr.io/nickhartjes/specdeck-signalling:1` pulls after 1.7.0 is released
- **THEN** they get the 1.7.0 image, with the same digest as `ghcr.io/nickhartjes/specdeck-server:1.7.0`

#### Scenario: Verify under the old name
- **WHEN** a self-hoster runs the documented `cosign verify` on `ghcr.io/nickhartjes/specdeck-signalling:1.7.0`
- **THEN** verification succeeds

#### Scenario: After the transition
- **WHEN** 1.9.0 is released
- **THEN** `ghcr.io/nickhartjes/specdeck-server` gets its tags, `ghcr.io/nickhartjes/specdeck-signalling` gets none, and `ghcr.io/nickhartjes/specdeck-signalling:1.8.0` still pulls

### Requirement: Deployment files name the new image
The Compose and Kubernetes files in the repository SHALL run `ghcr.io/nickhartjes/specdeck-server`, and the Kubernetes Deployment and Service SHALL be named `specdeck-server`. The self-host docs SHALL describe how to move an existing setup from the old names.

#### Scenario: Fresh Compose setup
- **WHEN** a self-hoster follows the Docker Compose guide for a release from 1.7.0 on
- **THEN** the service runs from `ghcr.io/nickhartjes/specdeck-server` and answers on its domain

#### Scenario: Upgrading a Kubernetes setup
- **WHEN** a self-hoster with the 1.6 `deploy/k8s` base applies the new base and follows the upgrade steps in the Kubernetes guide
- **THEN** the Ingress reaches the `specdeck-server` Service, apps and the web viewer connect, and no `specdeck-signalling` Deployment or Service is left running
