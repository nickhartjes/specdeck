# Spec Delta

## ADDED Requirements

### Requirement: Replaying the history in the browser
A browser viewer SHALL apply timeline events that arrive together as one batch, in arrival order, as the app's viewer does, so that joining a long session in a browser does not render once per event.

#### Scenario: Late join of a long session in a browser
- **WHEN** a viewer joins in a browser a session with 400 finished prompts, and its events arrive one per message
- **THEN** they see the whole history in order, as in the app, after far fewer renders than messages

#### Scenario: Background tab
- **WHEN** events arrive while the browser tab is in the background
- **THEN** the timeline has caught up when the viewer returns to the tab
