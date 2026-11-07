# session-sharing Specification

## Purpose

Controls who can join a hosted session and how remote participants connect: invite codes and join links, admitting and removing viewers, and encrypted peer-to-peer connections that work across networks.

## Requirements

### Requirement: Invite code
When a session starts, the app SHALL generate an invite code for it and show it to the host with a way to copy it. The code SHALL be hard to guess and valid only for that session.

#### Scenario: Code generated
- **WHEN** the host starts a session
- **THEN** the app shows an invite code, for example `7K2M-9QXH-4TPA`, and a way to copy it

#### Scenario: Code for another session
- **WHEN** a viewer enters an invite code that belonged to a different, closed session
- **THEN** the join is refused

### Requirement: Join with invite code
A viewer SHALL join a session by entering its invite code. Case and dashes in the code SHALL NOT matter. An invalid or expired code SHALL be refused with a clear message.

#### Scenario: Valid code
- **WHEN** a viewer enters a valid invite code for an open session
- **THEN** the viewer joins, and the host sees the viewer's username in the participant list

#### Scenario: Code typed differently
- **WHEN** a viewer enters `7k2m9qxh4tpa` for the code `7K2M-9QXH-4TPA`
- **THEN** the viewer joins as with the original code

#### Scenario: Invalid code
- **WHEN** a viewer enters a code that does not match any open session
- **THEN** the app shows that the code is invalid or has expired

#### Scenario: Repeated wrong codes
- **WHEN** many invalid codes are tried from the same client in a short time
- **THEN** further attempts are temporarily refused, and the app says to wait

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

### Requirement: Invite code lifetime
The invite code SHALL expire when the host closes the session. The host SHALL be able to revoke it while the session is open and get a new one; revoking SHALL NOT disconnect viewers who already joined.

#### Scenario: Session closed
- **WHEN** the host closes the session
- **THEN** its invite code and join link no longer work

#### Scenario: Host revokes the code
- **WHEN** the host revokes the invite code during a session
- **THEN** the old code no longer works, a new code is shown, and connected viewers stay connected

### Requirement: Participant list
All participants SHALL see who is in the session: the host and each connected viewer by username, updated as people join and leave.

#### Scenario: Viewer leaves
- **WHEN** a viewer leaves the session
- **THEN** the viewer disappears from every participant's list

#### Scenario: Duplicate usernames
- **WHEN** two participants use the same username
- **THEN** both are listed and can be told apart

### Requirement: Remove a viewer
The host SHALL be able to remove any viewer. A removed viewer SHALL be disconnected immediately and SHALL NOT be able to rejoin with the invite code that was valid when they were removed.

#### Scenario: Host removes a viewer
- **WHEN** the host removes a viewer
- **THEN** that viewer is disconnected, is told they were removed, and cannot rejoin until the host shares a new code

### Requirement: Viewer limit
The host SHALL be able to set the session's viewer limit from 1 to 20, with 20 as the default. A join over the limit SHALL be refused; lowering the limit SHALL NOT disconnect viewers who already joined.

#### Scenario: Session full
- **WHEN** a viewer tries to join a session that has as many viewers as its limit
- **THEN** the join is refused with a message that the session is full

#### Scenario: Limit below current viewers
- **WHEN** the host sets the limit to 5 while 8 viewers are connected
- **THEN** all 8 stay connected and no new viewer can join until fewer than 5 are connected

### Requirement: Remote connectivity
Viewers SHALL be able to join from a different network than the host, including networks behind NAT or firewalls, without a VPN and without the host opening ports.

#### Scenario: Viewer on a different network
- **WHEN** host and viewer are on different home networks behind NAT
- **THEN** the viewer can join and receive the session

#### Scenario: Direct connection blocked
- **WHEN** a network blocks direct peer-to-peer connections but allows UDP to the team's relay
- **THEN** the connection goes through the team's relay and the session still works

#### Scenario: Network blocks UDP
- **WHEN** a viewer's network blocks UDP entirely, so neither a direct nor a relayed connection can be made
- **THEN** the app tells the viewer that their network does not allow the connection, instead of waiting indefinitely

### Requirement: End-to-end encryption
All session content (terminal output, timeline, diffs) SHALL be encrypted between host and viewer. The matchmaking service and the relay SHALL NOT be able to read session content or learn the invite code.

#### Scenario: Traffic through the relay
- **WHEN** a viewer's traffic passes through the team's relay
- **THEN** the relay only handles encrypted data and cannot read terminal output, prompts or diffs

#### Scenario: Matchmaking service tries to intercept
- **WHEN** the matchmaking service substitutes its own connection details during a join
- **THEN** the viewer's app rejects the connection and does not show any session content

### Requirement: Matchmaking service unavailable
If the matchmaking service is unreachable, the app SHALL tell the user that sharing is unavailable. The host SHALL still be able to run the session locally, and the app SHALL keep retrying until sharing works.

#### Scenario: Service down when hosting
- **WHEN** the host starts a session while the matchmaking service is unreachable
- **THEN** the session runs locally, the host is told sharing is unavailable, and the invite code appears once the service is reachable

#### Scenario: Service down when joining
- **WHEN** a viewer tries to join while the matchmaking service is unreachable
- **THEN** the app says the sharing service cannot be reached

### Requirement: Matching app versions
A viewer SHALL be admitted when their app speaks the host's session protocol or an older one the host still supports, and the host SHALL send that viewer only what its protocol can read. A viewer on an older supported protocol SHALL be able to watch but SHALL NOT be able to send requests to the host. Otherwise the viewer SHALL NOT be admitted and SHALL be told who needs to update: the viewer, when their app is older than the host supports, or the host, when the host's app is older.

#### Scenario: Viewer on an older app the host supports
- **WHEN** a viewer whose app speaks the protocol of the previous release joins a host on the current one
- **THEN** the viewer is admitted, watches the session as before, and is never sent a message their app cannot read

#### Scenario: Older viewer while the host allows suggestions
- **WHEN** the host allows prompt suggestions and a viewer on an older supported app is in the session
- **THEN** that viewer is not offered a way to suggest, and the host's view of the session is otherwise the same

#### Scenario: Viewer on an older app
- **WHEN** a viewer whose app speaks a session protocol older than any the host supports tries to join
- **THEN** the viewer is not admitted and is told to update their app to join the session

#### Scenario: Host on an older app
- **WHEN** a viewer whose app speaks a newer session protocol than the host's tries to join
- **THEN** the viewer is not admitted and is told that the host needs to update their app

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
