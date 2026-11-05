# web-viewer Specification

## Purpose
Lets people who cannot install the SpecDeck app watch a shared session from a web browser, served by the same sharing service the host uses, with the same protections as the app.

## Requirements

### Requirement: Watch in a browser
A sharing service SHALL serve a web page where a viewer joins a session with its invite code, without installing anything. The page SHALL only watch: it SHALL offer no way to host a session or to send input to one.

#### Scenario: Join with a code
- **WHEN** someone opens the sharing service's address in a browser and enters a valid invite code and a name
- **THEN** they join the session as a viewer, and the host sees their name in the participant list

#### Scenario: No hosting
- **WHEN** someone opens the web viewer
- **THEN** the page offers joining a session and nothing else

### Requirement: Same session view as the app
The web viewer SHALL show a session as the app's viewer does: the terminal, the timeline, files, people and the status bar, following and free scrolling, search, copy, jumping to prompts, and the Overview and Developer views. Each viewer's navigation SHALL stay local to their page.

#### Scenario: Late join
- **WHEN** a viewer joins in a browser after the session has been running for a while
- **THEN** they see the terminal output and timeline from the start of the session, as in the app

#### Scenario: Developer view
- **WHEN** a browser viewer switches to the Developer view
- **THEN** the terminal is shown next to the timeline with every step listed, as in the app

### Requirement: Served by the session's own service
The web viewer SHALL be part of the sharing service, so every service, the public one or a self-hosted one, serves its own copy with no separate deployment. The page SHALL load nothing from any other address.

#### Scenario: Self-hosted service
- **WHEN** a team runs its own sharing service with the self-hosting setup
- **THEN** its address serves the web viewer, and the page's code and data come only from that address

#### Scenario: No third parties
- **WHEN** a browser viewer loads the page and watches a session
- **THEN** the browser makes no request to any address other than the sharing service, the team's relay and the host's connection

### Requirement: Only the page's own service
The web viewer SHALL join only sessions on the sharing service that served it, and SHALL refuse a join link that names another service, pointing the viewer to that service's own page instead.

#### Scenario: Link for another service
- **WHEN** someone pastes into the main site a join link whose session is on a self-hosted service
- **THEN** the page does not connect to that service, and offers to open the link on that service's own page

### Requirement: Code stays out of requests
The web viewer SHALL NOT send the invite code to the sharing service or any other server, including in the page's address, request headers or logs.

#### Scenario: Join link opened in a browser
- **WHEN** a viewer opens `https://specdeck.example.com/join#7K2M-9QXH-4TPA`
- **THEN** the request for the page does not contain the code, and the code is filled in on the page

### Requirement: Same protection as the app
A browser viewer SHALL verify the host and every piece of session data as the app does. It SHALL show nothing from a connection whose details were not made with the session's code, nor data that fails verification.

#### Scenario: Matchmaking service substitutes a connection
- **WHEN** the sharing service substitutes its own connection details while a browser viewer joins
- **THEN** the page rejects the connection and shows no session content

#### Scenario: Tampered data
- **WHEN** a browser viewer receives output that was altered after the host sent it
- **THEN** the page discards it and requests it again

### Requirement: Name and settings stay in the browser
The web viewer SHALL ask for the viewer's name on their first join and SHALL keep it and their view settings only in that browser, with no account. When the browser cannot keep them, the page SHALL still work and ask again next time.

#### Scenario: Second visit
- **WHEN** a viewer who joined before opens the web viewer again in the same browser
- **THEN** their name and view are already set

#### Scenario: Private window
- **WHEN** a viewer uses a private window that keeps nothing
- **THEN** they can join after entering a name, and are asked again on their next visit

### Requirement: Browser-specific failures
The web viewer SHALL tell the viewer why a join failed, using the same messages as the app. When the page's session protocol is older than the host's app, it SHALL say that the sharing service needs to be updated, and SHALL NOT tell the viewer to update an app.

#### Scenario: Network blocks the connection
- **WHEN** a browser viewer's network allows the page to load but blocks the connection to the host
- **THEN** the page says that their network does not allow the connection, instead of waiting indefinitely

#### Scenario: Browser with WebRTC turned off
- **WHEN** a viewer joins in a browser whose settings turn WebRTC off
- **THEN** the page says at once that the browser blocks the connection, and suggests allowing WebRTC, another browser or the app, without waiting for a timeout

#### Scenario: Page older than the host's app
- **WHEN** the host's app speaks a newer session protocol than the web viewer its sharing service serves
- **THEN** the viewer is not admitted and is told the sharing service's web viewer needs to be updated, and that the SpecDeck app can be used instead

#### Scenario: Host's app older than the page
- **WHEN** the host's app speaks an older session protocol than the web viewer
- **THEN** the viewer is not admitted and is told that the host needs to update their app

### Requirement: Session ends in the browser
When the session ends, the viewer is removed, or the viewer leaves, the web viewer SHALL close the connection and say why, as the app does, and SHALL offer to join another session.

#### Scenario: Host closes the session
- **WHEN** the host closes the session while a browser viewer watches
- **THEN** the page shows that the session has ended, and offers to join another one

#### Scenario: Tab closed
- **WHEN** a browser viewer closes the tab
- **THEN** the host's participant list no longer shows them
