# Spec Delta

## MODIFIED Requirements

### Requirement: Join link
The host SHALL be able to copy a join link holding the invite code and the host's sharing service; copying only the code SHALL NOT include the service. When that service serves the web viewer, the link SHALL be a web address that opens in any browser, offers the app or the browser, and is accepted by the app; otherwise it SHALL open the app directly. Either way, a viewer with the app SHALL be able to join from it. A link without a service SHALL join through the viewer's own service. A link opened from outside the app SHALL NOT join by itself: the app SHALL first ask the viewer whether to join, showing the invite code and the name they join with.

#### Scenario: Viewer opens a join link
- **WHEN** a viewer with the app installed opens a join link from a chat message
- **THEN** the app opens and asks whether to join that session, asking for a username first if none is set, and joins when the viewer agrees

#### Scenario: Viewer declines a join link
- **WHEN** a web page opens a join link in the app and the viewer chooses not to join
- **THEN** the app connects to nothing, and shows the start screen

#### Scenario: Join link while in another session
- **WHEN** a viewer who is already viewing or hosting a session opens a join link for a different session
- **THEN** the app asks in a new window and joins there when the viewer agrees, and the session already open keeps running in its own window

#### Scenario: Join link with a free window
- **WHEN** a viewer opens a join link while the focused window shows the start screen
- **THEN** the app asks in that window, joins the session there when the viewer agrees, and opens no new one

#### Scenario: Link carries the host's service
- **WHEN** a host using `specdeck.example.com` copies the join link
- **THEN** the link contains the invite code and the service `specdeck.example.com`

#### Scenario: Link without a service
- **WHEN** a viewer opens a join link that has no sharing service, such as one copied from an earlier version
- **THEN** the app joins through the viewer's own sharing service, once the viewer agrees

#### Scenario: Viewer without the app
- **WHEN** a viewer without the app opens a join link from a service that serves the web viewer
- **THEN** the browser shows the web viewer with the code filled in, and the viewer can watch after entering a name

#### Scenario: Viewer with the app opens the link in a browser
- **WHEN** a viewer with the app installed opens the web join link and chooses to open it in the app
- **THEN** the app opens and joins the session when the viewer agrees

#### Scenario: Web link pasted into the app
- **WHEN** a viewer pastes a web join link into the app's join field
- **THEN** the app joins the session on the service the link names, asking first when that is not the viewer's own service, as for other join links, and not asking otherwise

#### Scenario: Link to another service asks once
- **WHEN** a viewer opens a join link that names a sharing service other than their own
- **THEN** the app asks one question, which names that service, the invite code and the viewer's name

#### Scenario: Service without the web viewer
- **WHEN** the host's sharing service does not serve the web viewer
- **THEN** the copied join link opens the app directly, as before

### Requirement: Participant list
All participants SHALL see who is in the session: the host and each connected viewer by username, updated as people join and leave. A username SHALL be shown without invisible and text-direction characters, and a name with nothing left after that SHALL be refused.

#### Scenario: Viewer leaves
- **WHEN** a viewer leaves the session
- **THEN** the viewer disappears from every participant's list

#### Scenario: Duplicate usernames
- **WHEN** two participants use the same username
- **THEN** both are listed and can be told apart

#### Scenario: Name with a hidden character
- **WHEN** a viewer joins as `nick` followed by a zero-width space while another `nick` is in the session
- **THEN** the two are listed as `nick` and `nick (2)`

#### Scenario: Name that reverses its text
- **WHEN** a viewer joins with a name that holds a right-to-left override character
- **THEN** every list, timeline step and notification shows the name without that character

#### Scenario: Only invisible characters
- **WHEN** a viewer's app sends a name made only of zero-width characters
- **THEN** the host refuses the viewer
