# Spec Delta

## MODIFIED Requirements

### Requirement: Join link
The host SHALL be able to copy a join link holding the invite code and the host's sharing service; copying only the code SHALL NOT include the service. When that service serves the web viewer, the link SHALL be a web address that opens in any browser, offers the app or the browser, and is accepted by the app; otherwise it SHALL open the app directly. Either way, a viewer with the app SHALL be able to join from it. A link without a service SHALL join through the viewer's own service.

#### Scenario: Viewer opens a join link
- **WHEN** a viewer with the app installed opens a join link from a chat message
- **THEN** the app opens and joins the session, asking for a username first if none is set

#### Scenario: Join link while in another session
- **WHEN** a viewer who is already viewing or hosting a session opens a join link for a different session
- **THEN** the app joins it in a new window, and the session already open keeps running in its own window

#### Scenario: Join link with a free window
- **WHEN** a viewer opens a join link while the focused window shows the start screen
- **THEN** the app joins the session in that window and opens no new one

#### Scenario: Link carries the host's service
- **WHEN** a host using `specdeck.example.com` copies the join link
- **THEN** the link contains the invite code and the service `specdeck.example.com`

#### Scenario: Link without a service
- **WHEN** a viewer opens a join link that has no sharing service, such as one copied from an earlier version
- **THEN** the app joins through the viewer's own sharing service

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

### Requirement: Sharing service setting
The user SHALL be able to set the sharing service in Settings, as a domain or a full URL. An empty setting SHALL mean the service built into the app. A changed setting SHALL apply to sessions started or joined afterwards, and SHALL NOT be changeable while the user hosts or views a session in any window.

#### Scenario: Own server entered as a domain
- **WHEN** the user enters `specdeck.example.com` as the sharing service and then hosts a session
- **THEN** the session is shared through the service at `specdeck.example.com`

#### Scenario: Setting cleared
- **WHEN** the user clears the sharing service setting
- **THEN** sessions started or joined afterwards use the built-in service

#### Scenario: During a session
- **WHEN** the user opens Settings while hosting or viewing a session
- **THEN** the sharing service cannot be changed, and Settings says it applies to the next session

#### Scenario: Session in another window
- **WHEN** one window hosts a session, another shows the start screen, and the user opens Settings
- **THEN** the sharing service cannot be changed until no window has a session
