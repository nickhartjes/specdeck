# Spec Delta

## MODIFIED Requirements

### Requirement: The app on your computer
The documentation SHALL state:
- what the app stores on the user's computer, and with which file permissions;
- what logs and diagnostic exports hold, and that the app sends no telemetry;
- what notifications can show;
- how updates are signed and verified, and what an update check sends;
- how releases and the server image are signed, including what is not signed yet;
- how to check a downloaded installer against the release's checksums and its build provenance.

#### Scenario: Reader checks signing
- **WHEN** a reader looks up whether installers are code-signed
- **THEN** the documentation says which are and which are not today, and how to verify the server image, updates, and a downloaded installer by its checksum and its build provenance

#### Scenario: Reader verifies a download
- **WHEN** a reader follows the documented commands on an installer downloaded from a release
- **THEN** the checksum matches and the provenance check names this repository's release workflow at that release's tag
