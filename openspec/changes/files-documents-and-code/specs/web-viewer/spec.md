## MODIFIED Requirements

### Requirement: Same session view as the app
The web viewer SHALL show a session as the app's viewer does: the files, tabs and timeline panels, the people in the title bar and the status bar, following and free scrolling, search, copy, jumping to prompts, and the Overview, Developer and Present presets. Each viewer's navigation SHALL stay local to their page. The app's type SHALL come from the sharing service the page was loaded from.

#### Scenario: Late join
- **WHEN** a viewer joins in a browser after the session has been running for a while
- **THEN** they see the terminal output and timeline from the start of the session, as in the app

#### Scenario: Developer view
- **WHEN** a browser viewer chooses the Developer preset
- **THEN** the terminal tab is shown between the files and the timeline, with every step listed, as in the app

#### Scenario: Whole file and source in the browser
- **WHEN** the host shares whole code files and a browser viewer opens a changed code file and a document
- **THEN** they can switch the code file to *Whole file, changes marked* and the document to *Source*, highlighted as in the app
