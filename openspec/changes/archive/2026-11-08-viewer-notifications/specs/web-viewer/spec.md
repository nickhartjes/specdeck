# Spec Delta

## ADDED Requirements

### Requirement: Notifications in the browser
The web viewer SHALL offer the same kinds of viewer notification as the app, under the same rules (see `viewer-notifications`), as the browser's notifications. They SHALL be off until the viewer turns them on in the layout menu, which SHALL ask the browser's permission at that moment. They SHALL be shown only while the page is hidden or the browser window is not focused. Clicking one SHALL bring the page to the front.

#### Scenario: Turn them on
- **WHEN** a browser viewer turns on notifications in the layout menu
- **THEN** the browser asks for permission, and once it is given, plans and documents are on and the agent finished is off

#### Scenario: Off by default
- **WHEN** a viewer opens the web viewer for the first time and Claude proposes a change while the tab is hidden
- **THEN** no notification is shown, and the browser never asked for permission

#### Scenario: Tab hidden
- **WHEN** a browser viewer turned notifications on, switched to another tab, and Claude proposes a change
- **THEN** the browser shows a notification that the plan is ready for review, and clicking it shows the session's tab

#### Scenario: Page in front
- **WHEN** a browser viewer turned notifications on and Claude changes a document while the page is in front
- **THEN** no notification is shown

#### Scenario: Permission denied
- **WHEN** the browser refuses permission, or cannot show notifications
- **THEN** the layout menu says that this browser does not allow notifications, and the switches stay off

## MODIFIED Requirements

### Requirement: Name and settings stay in the browser
The web viewer SHALL ask for the viewer's name on their first join and SHALL keep it, their layout, their appearance and their notification choices only in that browser, with no account. When the browser cannot keep them, the page SHALL still work and ask again next time.

#### Scenario: Second visit
- **WHEN** a viewer who joined before opens the web viewer again in the same browser
- **THEN** their name, layout and appearance are already set

#### Scenario: Private window
- **WHEN** a viewer uses a private window that keeps nothing
- **THEN** they can join after entering a name, and are asked again on their next visit

#### Scenario: Appearance not kept
- **WHEN** the browser keeps nothing and a viewer chooses Dark
- **THEN** the page is dark until it is closed, and the next visit starts at System

#### Scenario: Notification choices kept
- **WHEN** a browser viewer turned notifications on and turned on the agent finished, and later opens the web viewer again in the same browser, which still allows notifications
- **THEN** notifications are on with the agent finished on, without being asked again
