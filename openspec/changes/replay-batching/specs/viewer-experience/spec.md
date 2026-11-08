# Spec Delta

## ADDED Requirements

### Requirement: Replaying the history on joining
Timeline events that reach a viewer's window together SHALL be applied together, so that the session view renders once per batch instead of once per event or per message. Events SHALL be applied in the order they arrived, none SHALL be lost, and the view SHALL be at most one frame behind while the window is visible. Notifications SHALL see each event as it arrives, as without batching.

#### Scenario: Joining a session with 400 prompts
- **WHEN** a viewer joins a session with 400 finished prompts and 8,400 timeline events that arrive in messages of 20 events
- **THEN** the window shows the whole history, folded for a late joiner as before, after far fewer renders than messages

#### Scenario: Order is kept
- **WHEN** timeline events, a status change and the reply to the viewer's request arrive within the same frame
- **THEN** the timeline holds every event in arrival order, and the request shown with the status and the reply are as they would be if each had been applied on its own

#### Scenario: A live event during the session
- **WHEN** one timeline event arrives while the viewer watches
- **THEN** it is shown by the next frame

#### Scenario: History still does not notify
- **WHEN** a viewer joins a session in which Claude proposed a change an hour ago, and the history arrives in batches
- **THEN** the viewer gets no notification about it, and gets notified about turns that start after the join as before

#### Scenario: Window in the background
- **WHEN** events arrive while the viewer's window is hidden
- **THEN** notifications arrive as the events do, and the timeline has caught up when the window is shown again
