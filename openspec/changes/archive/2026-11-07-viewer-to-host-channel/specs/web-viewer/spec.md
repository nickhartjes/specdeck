## MODIFIED Requirements

### Requirement: Browser-specific failures
The web viewer SHALL tell the viewer why a join failed, using the same messages as the app. When the page's session protocol is older than any the host's app supports, it SHALL say that the sharing service needs to be updated, and SHALL NOT tell the viewer to update an app.

#### Scenario: Network blocks the connection
- **WHEN** a browser viewer's network allows the page to load but blocks the connection to the host
- **THEN** the page says that their network does not allow the connection, instead of waiting indefinitely

#### Scenario: Browser with WebRTC turned off
- **WHEN** a viewer joins in a browser whose settings turn WebRTC off
- **THEN** the page says at once that the browser blocks the connection, and suggests allowing WebRTC, another browser or the app, without waiting for a timeout

#### Scenario: Page one release older than the host's app
- **WHEN** the web viewer its sharing service serves speaks the protocol of the release before the host's app
- **THEN** the viewer is admitted and watches the session, without the requests the newer protocol adds

#### Scenario: Page older than the host's app
- **WHEN** the host's app no longer supports the session protocol of the web viewer its sharing service serves
- **THEN** the viewer is not admitted and is told the sharing service's web viewer needs to be updated, and that the SpecDeck app can be used instead

#### Scenario: Host's app older than the page
- **WHEN** the host's app speaks an older session protocol than the web viewer
- **THEN** the viewer is not admitted and is told that the host needs to update their app

## ADDED Requirements

### Requirement: Requests from the browser
A viewer in the web viewer SHALL be able to send the same requests to the host as a viewer in the app, under the same rules, and SHALL see what became of them in the same way.

#### Scenario: Suggest from the browser
- **WHEN** the host allows prompt suggestions and a viewer in the browser suggests a prompt
- **THEN** the host sees it as from any other viewer, and the browser viewer sees what became of it
