# Spec Delta

## ADDED Requirements

### Requirement: Sharing service setting
The user SHALL be able to set the sharing service in Settings, as a domain or a full URL. An empty setting SHALL mean the service built into the app. A changed setting SHALL apply to sessions started or joined afterwards, and SHALL NOT be changeable while the user hosts or views a session.

#### Scenario: Own server entered as a domain
- **WHEN** the user enters `specdeck.example.com` as the sharing service and then hosts a session
- **THEN** the session is shared through the service at `specdeck.example.com`

#### Scenario: Setting cleared
- **WHEN** the user clears the sharing service setting
- **THEN** sessions started or joined afterwards use the built-in service

#### Scenario: During a session
- **WHEN** the user opens Settings while hosting or viewing a session
- **THEN** the sharing service cannot be changed, and Settings says it applies to the next session

### Requirement: Sharing service check
When the user saves a sharing service, the app SHALL check whether it can be reached and show the result. The setting SHALL be saved whether or not the check succeeds. A service reached without encryption SHALL be marked as not encrypted.

#### Scenario: Service reachable
- **WHEN** the user saves a sharing service that answers
- **THEN** Settings shows that the service can be reached

#### Scenario: Service unreachable
- **WHEN** the user saves a sharing service that does not answer
- **THEN** Settings shows that it cannot be reached, and the setting is saved

#### Scenario: Unencrypted service
- **WHEN** the user enters a `ws://` address
- **THEN** Settings marks the service as not encrypted

### Requirement: Sharing service override
An environment variable naming the sharing service, set when the app starts, SHALL take precedence over the setting and the built-in service.

#### Scenario: Environment variable set
- **WHEN** the app is started with the environment variable set and the user has a sharing service setting
- **THEN** sessions use the service from the environment variable

### Requirement: Join link through another sharing service
When a viewer opens a join link whose sharing service differs from the one the viewer would use, the app SHALL ask before joining through it, naming the service. Joining SHALL use that service for this session only, unless the viewer chooses to use it from now on.

#### Scenario: Viewer accepts once
- **WHEN** a viewer opens a join link for `specdeck.example.com` while using another service, and joins without choosing to keep it
- **THEN** the viewer joins through `specdeck.example.com`, and their sharing service setting is unchanged

#### Scenario: Viewer keeps the service
- **WHEN** the viewer joins and chooses to use the service from now on
- **THEN** the viewer joins through it, and it becomes their sharing service setting

#### Scenario: Viewer declines
- **WHEN** the viewer cancels the question
- **THEN** the app does not connect to that service and does not join

#### Scenario: Same service
- **WHEN** a viewer opens a join link whose sharing service is the one they use
- **THEN** the app joins without asking

#### Scenario: Reconnect
- **WHEN** a viewer who joined through another service loses the connection and reconnects
- **THEN** the reconnect uses the service they joined through

## MODIFIED Requirements

### Requirement: Join link
The host SHALL be able to copy a join link that contains the invite code and the sharing service the host uses. Opening the link on a machine with the app installed SHALL open the app and join the session. A link without a sharing service SHALL join through the viewer's own service. Copying only the invite code SHALL NOT include the service.

#### Scenario: Viewer opens a join link
- **WHEN** a viewer with the app installed opens a join link from a chat message
- **THEN** the app opens and joins the session, asking for a username first if none is set

#### Scenario: Join link while in another session
- **WHEN** a viewer who is already viewing a session opens a join link for a different session
- **THEN** the app asks whether to leave the current session before joining

#### Scenario: Link carries the host's service
- **WHEN** a host using `specdeck.example.com` copies the join link
- **THEN** the link contains the invite code and the service `specdeck.example.com`

#### Scenario: Link without a service
- **WHEN** a viewer opens a join link that has no sharing service, such as one copied from an earlier version
- **THEN** the app joins through the viewer's own sharing service
