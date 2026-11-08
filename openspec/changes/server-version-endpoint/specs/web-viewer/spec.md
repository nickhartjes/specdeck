# Spec Delta

## ADDED Requirements

### Requirement: The service states its version
A sharing service that serves the web viewer SHALL state, in its description of the web viewer (`/web-viewer.json`), the release version it was built from, next to the session protocol, so anyone can check which release a service runs with one request. The version SHALL be the release's version without a leading `v`. The service's health check SHALL keep its current answer.

#### Scenario: Checking which release runs
- **WHEN** an operator requests `https://specdeck.example.com/web-viewer.json` from a service built from release 1.7.0
- **THEN** the answer is `{"protocol":N,"version":"1.7.0"}`, where N is the session protocol release 1.7.0 speaks

#### Scenario: After an update
- **WHEN** a service is updated from one release to the next and the operator requests `/web-viewer.json` again
- **THEN** the answer gives the new release's version

#### Scenario: Health check unchanged
- **WHEN** a monitor requests `/health`
- **THEN** the answer is still `ok`

#### Scenario: Apps and services of different releases
- **WHEN** a host's app reads `/web-viewer.json` from a service of a newer or older release, with or without a version
- **THEN** the app finds the web viewer and its protocol as before
