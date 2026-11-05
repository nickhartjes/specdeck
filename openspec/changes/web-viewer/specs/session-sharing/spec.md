# Spec Delta

## MODIFIED Requirements

### Requirement: Join link
The host SHALL be able to copy a join link that contains the invite code. When the host's sharing service serves the web viewer, the link SHALL be a web address that opens in any browser, offers to open the session in the app or watch it in the browser, and is also accepted by the app. Otherwise the link SHALL open the app directly. Opening the link on a machine with the app installed SHALL be able to open the app and join the session.

#### Scenario: Viewer opens a join link
- **WHEN** a viewer with the app installed opens a join link from a chat message
- **THEN** the app opens and joins the session, asking for a username first if none is set

#### Scenario: Join link while in another session
- **WHEN** a viewer who is already viewing a session opens a join link for a different session
- **THEN** the app asks whether to leave the current session before joining

#### Scenario: Viewer without the app
- **WHEN** a viewer without the app opens a join link from a service that serves the web viewer
- **THEN** the browser shows the web viewer with the code filled in, and the viewer can watch after entering a name

#### Scenario: Viewer with the app opens the link in a browser
- **WHEN** a viewer with the app installed opens the web join link and chooses to open it in the app
- **THEN** the app opens and joins the session

#### Scenario: Web link pasted into the app
- **WHEN** a viewer pastes a web join link into the app's join field
- **THEN** the app joins the session on the service the link names, asking first when that is not the viewer's own service, as for other join links

#### Scenario: Service without the web viewer
- **WHEN** the host's sharing service does not serve the web viewer
- **THEN** the copied join link opens the app directly, as before
