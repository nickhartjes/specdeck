# Spec Delta

## ADDED Requirements

### Requirement: Windows show only SpecDeck
A session window and the settings window SHALL show only the app's own page. They SHALL refuse to load any other page, whatever asks for it: a link, a script or a redirect. The session in the window SHALL go on as before.

#### Scenario: Something tries to load another page
- **WHEN** content in a session window tries to load `https://example.com/` in the window itself
- **THEN** the window keeps showing the session, and no request is made to `example.com` by the window

#### Scenario: Settings window
- **WHEN** something tries to load another page in the settings window
- **THEN** the settings window keeps showing the settings

### Requirement: App functions are granted per window
Only session windows and the settings window SHALL be able to use the app's functions. A window of Claude's browser SHALL NOT be able to use any of them, whatever page it shows. Every function the app offers its windows SHALL be granted by name, so a function that was not granted fails for every window.

#### Scenario: Browser window showing the app's page
- **WHEN** a window of Claude's browser shows the app's own page and that page calls an app function, such as reading the settings or starting a session
- **THEN** the call fails, and nothing in SpecDeck changes

#### Scenario: Session window
- **WHEN** a session window calls an app function
- **THEN** it works as before

#### Scenario: A function without a grant
- **WHEN** a function is added to the app without being granted to its windows
- **THEN** the app's checks fail before it is released

### Requirement: Names in notifications are plain text
A notification SHALL show a participant's name as plain text. Markup in a name SHALL NOT be rendered as formatting or as a link on any platform.

#### Scenario: Name with markup on Linux
- **WHEN** a viewer named `<b>nick</b>` joins while the host's window is in the background on Linux
- **THEN** the notification shows the characters `<b>nick</b>`, not a bold name
