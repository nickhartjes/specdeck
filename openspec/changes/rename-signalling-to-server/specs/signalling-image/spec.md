# Spec Delta

## REMOVED Requirements

### Requirement: Published on every release
**Reason**: The image is renamed to `ghcr.io/nickhartjes/specdeck-server`; the requirement moves to `server-image`.
**Migration**: Pull `ghcr.io/nickhartjes/specdeck-server`. Until 1.9.0 the old name is published alongside it (`server-image`, "Old image name during the transition").

### Requirement: Release tags
**Reason**: Moved to `server-image` with the image rename.
**Migration**: The same tags exist under `ghcr.io/nickhartjes/specdeck-server`.

### Requirement: Platforms
**Reason**: Moved to `server-image` with the image rename.
**Migration**: None; the new image has the same platforms.

### Requirement: Verifiable origin
**Reason**: Moved to `server-image` with the image rename.
**Migration**: Run `cosign verify` on `ghcr.io/nickhartjes/specdeck-server`, with the same identity and issuer.

### Requirement: Release only with its image
**Reason**: Moved to `server-image`, where it covers both names during the transition.
**Migration**: None.
