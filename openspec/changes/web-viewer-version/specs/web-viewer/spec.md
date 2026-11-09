# Spec Delta

## ADDED Requirements

### Requirement: The join page shows the service's release
The web viewer's join page SHALL show the release of the sharing service that serves it, as the service states it, as small secondary text below the join form. When the service states no release, the page SHALL show no release text, and joining SHALL NOT wait for the version.

#### Scenario: Service states its release
- **WHEN** a browser opens the join page of a service that runs release 1.7.0
- **THEN** the page shows "SpecDeck 1.7.0" below the join form

#### Scenario: Service without a version
- **WHEN** a browser opens the join page of a service that states no release, or the request for it fails
- **THEN** the page shows no release text, and the form works as before

#### Scenario: Joining right away
- **WHEN** a viewer submits the form before the version has arrived
- **THEN** they join without waiting for it
