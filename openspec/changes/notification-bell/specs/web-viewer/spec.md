# Spec Delta

## MODIFIED Requirements

### Requirement: Notifications in the browser
The web viewer SHALL offer the same kinds of viewer notification as the app, under the same rules (see `viewer-notifications`), as the browser's notifications. They SHALL be off until the viewer turns them on in the bell, which SHALL ask the browser's permission at that moment. They SHALL be shown only while the page is hidden or the browser window is not focused. Clicking one SHALL bring the page to the front and open what it is about, as in the app.

#### Scenario: Turn them on
- **WHEN** a browser viewer turns on notifications in the bell
- **THEN** the browser asks for permission, and once it is given, plans and documents are on and the agent finished is off

#### Scenario: Off by default
- **WHEN** a viewer opens the web viewer for the first time and Claude proposes a change while the tab is hidden
- **THEN** no notification is shown, and the browser never asked for permission

#### Scenario: Tab hidden
- **WHEN** a browser viewer turned notifications on, switched to another tab, and Claude proposes a change
- **THEN** the browser shows a notification that the plan is ready for review, and clicking it shows the session's tab with the plan's proposal open

#### Scenario: Page in front
- **WHEN** a browser viewer turned notifications on and Claude changes a document while the page is in front
- **THEN** no notification is shown

#### Scenario: Permission denied
- **WHEN** the browser refuses permission, or cannot show notifications
- **THEN** the bell says that this browser does not allow notifications, and the switches stay off

#### Scenario: Page closed
- **WHEN** a browser viewer closed the page, and then clicks a notification it showed
- **THEN** nothing opens

## ADDED Requirements

### Requirement: The bell in the browser
The web viewer SHALL have the same bell and list as the app. Since it has no Settings, the bell SHALL hold the main switch for the browser's notifications above the kinds, and the layout menu SHALL NOT offer notifications. The list SHALL be kept until the page is closed or reloaded.

#### Scenario: List without notifications
- **WHEN** a browser viewer never turned notifications on, and Claude proposes `add-login`
- **THEN** the bell shows 1, and its list has the entry that the plan `add-login` is ready for review

#### Scenario: Switches in the bell
- **WHEN** a browser viewer opens the bell
- **THEN** it shows the main switch for this browser's notifications and, once it is on, the kinds

#### Scenario: Layout menu
- **WHEN** a browser viewer opens the layout menu
- **THEN** it has no notification switches

#### Scenario: Reload
- **WHEN** a browser viewer reloads the page
- **THEN** the list is empty
