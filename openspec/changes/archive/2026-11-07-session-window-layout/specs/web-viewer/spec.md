# Spec Delta

## MODIFIED Requirements

### Requirement: Same session view as the app
The web viewer SHALL show a session as the app's viewer does: the files, tabs and timeline panels, the people in the title bar and the status bar, following and free scrolling, search, copy, jumping to prompts, and the Overview, Developer and Present presets. Each viewer's navigation SHALL stay local to their page. The app's type SHALL come from the sharing service the page was loaded from.

#### Scenario: Late join
- **WHEN** a viewer joins in a browser after the session has been running for a while
- **THEN** they see the terminal output and timeline from the start of the session, as in the app

#### Scenario: Developer view
- **WHEN** a browser viewer chooses the Developer preset
- **THEN** the terminal tab is shown between the files and the timeline, with every step listed, as in the app

### Requirement: Name and settings stay in the browser
The web viewer SHALL ask for the viewer's name on their first join and SHALL keep it and their layout only in that browser, with no account. When the browser cannot keep them, the page SHALL still work and ask again next time.

#### Scenario: Second visit
- **WHEN** a viewer who joined before opens the web viewer again in the same browser
- **THEN** their name and layout are already set

#### Scenario: Private window
- **WHEN** a viewer uses a private window that keeps nothing
- **THEN** they can join after entering a name, and are asked again on their next visit
