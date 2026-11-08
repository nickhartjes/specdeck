## MODIFIED Requirements

### Requirement: Notifications
The app SHALL send OS notifications for events that need attention while the window of the session concerned is not focused: to the host when a viewer joins or leaves, when the agent waits for the host's input (for agents whose events the app captures, and for agents and terminal programs that report a request for the host), and for the kinds of answer to the host's read requests and votes they turned on (see `host-requests`), and to a viewer when the session ends and for the kinds of viewer notification they turned on (see `viewer-notifications`). The user SHALL be able to turn them all off with one switch.

#### Scenario: Agent waits for the host
- **WHEN** Claude Code finishes a turn or asks for permission while the host's window is in the background
- **THEN** the host gets a notification, and clicking it brings the session window to the front

#### Scenario: Reported request
- **WHEN** omp reports that it needs permission while the host's window is in the background
- **THEN** the host gets a notification with omp's message

#### Scenario: Window focused
- **WHEN** a viewer joins while the host's window has focus
- **THEN** no notification is shown

#### Scenario: Notifications turned off
- **WHEN** the user turned notifications off in settings
- **THEN** the app shows no notifications

#### Scenario: Another window focused
- **WHEN** the agent in the `api` session waits for the host while the host works in the `shop` session's window
- **THEN** the host gets a notification naming `api`, and clicking it brings the `api` window to the front

#### Scenario: Viewer called in for a plan
- **WHEN** a viewer's window is in the background and Claude proposes a change in the session they watch
- **THEN** the viewer gets a notification that the plan is ready for review, and clicking it brings that session's window to the front with the plan's proposal open

#### Scenario: Viewer's window in front
- **WHEN** Claude changes a document while the viewer's window of that session has focus
- **THEN** the viewer gets no notification

#### Scenario: Main switch off for a viewer
- **WHEN** a viewer turned notifications off in settings, with plans still checked
- **THEN** a plan ready for review gives no notification

#### Scenario: Host called back for a question
- **WHEN** a viewer answers the host's read request with a question while the host works in another app
- **THEN** the host gets a notification that the viewer has a question, and clicking it brings the session window to the front
