# Spec Delta

## ADDED Requirements

### Requirement: Choose the appearance in the browser
The web viewer SHALL offer the same appearance choice as the app, System, Light or Dark, with System as the default, which follows the browser's light or dark preference. A change SHALL apply at once, without reloading the page. The terminal SHALL stay dark in every appearance.

#### Scenario: Default
- **WHEN** a viewer opens the web viewer for the first time in a browser set to dark
- **THEN** the page is dark

#### Scenario: Choose Light
- **WHEN** a viewer watching a session chooses Light from the layout menu
- **THEN** the page switches to the light appearance at once, the session continues, and the terminal stays dark

#### Scenario: Host's choice does not carry over
- **WHEN** the host's app is dark and a browser viewer chose Light
- **THEN** the viewer's page is light

## MODIFIED Requirements

### Requirement: Name and settings stay in the browser
The web viewer SHALL ask for the viewer's name on their first join and SHALL keep it, their layout and their appearance only in that browser, with no account. When the browser cannot keep them, the page SHALL still work and ask again next time.

#### Scenario: Second visit
- **WHEN** a viewer who joined before opens the web viewer again in the same browser
- **THEN** their name, layout and appearance are already set

#### Scenario: Private window
- **WHEN** a viewer uses a private window that keeps nothing
- **THEN** they can join after entering a name, and are asked again on their next visit

#### Scenario: Appearance not kept
- **WHEN** the browser keeps nothing and a viewer chooses Dark
- **THEN** the page is dark until it is closed, and the next visit starts at System
