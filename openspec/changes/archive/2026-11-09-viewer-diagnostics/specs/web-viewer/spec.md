## ADDED Requirements

### Requirement: Diagnostic log in the browser
The web viewer SHALL keep a diagnostic log of its own connection from the moment the page opens, with the same event names and details as the app's viewer, and the reason a session ended for the page. It SHALL keep the latest 2,000 entries in that browser tab, also across a reload, and forget them when the tab closes. It SHALL record state changes only, never traffic, and SHALL NOT slow or stop a session when it cannot be kept.

#### Scenario: Failed attempts survive a reload
- **WHEN** a browser viewer's joins keep failing and they reload the page and try again
- **THEN** the log holds the attempts from before the reload and the new ones, each with how it ended

#### Scenario: Relayed connection recorded
- **WHEN** a browser viewer connects through the TURN relay
- **THEN** the log records that the connection is relayed, by candidate type only

#### Scenario: Oldest entries go first
- **WHEN** the log holds 2,000 entries and another is recorded
- **THEN** the oldest entry is dropped

#### Scenario: Browser keeps nothing
- **WHEN** a browser viewer uses a private window whose storage fails
- **THEN** the log is kept in memory until the page closes, and the session works as usual

#### Scenario: Tab closed
- **WHEN** a browser viewer closes the tab and opens the web viewer again
- **THEN** the log starts empty

### Requirement: Export diagnostics in the browser
The web viewer SHALL offer Export diagnostics in the layout menu while watching, and on the join page and the screen shown when a session ends, so a viewer who never got in can export. It SHALL say what the file contains and what it does not, then download one readable file in the app's export format. The page SHALL NOT upload it.

#### Scenario: Export after a failed join
- **WHEN** a browser viewer's join fails because their network blocks the connection, and they choose Export diagnostics on the join page
- **THEN** the browser downloads one file with the page's state and the log of the attempts, readable in a text editor

#### Scenario: Export while watching
- **WHEN** a browser viewer chooses Export diagnostics from the layout menu
- **THEN** the browser downloads the file and the session continues

#### Scenario: Export fails
- **WHEN** the browser does not allow the download
- **THEN** the page says the export failed and the session is not affected

### Requirement: The browser export describes the browser
The first line of the browser's export SHALL state that it comes from the web viewer, the release the page was built from, the browser's name and major version, the operating system's family, the sharing service as default or custom, and the session's state. It SHALL NOT contain the browser's user-agent string, screen size, language or anything that the app's export may not hold.

#### Scenario: First line of a browser export
- **WHEN** a viewer on a 1.8.0 web viewer in Firefox 132 on macOS exports diagnostics
- **THEN** the first line says it is from the web viewer of release 1.8.0, in Firefox 132 on macOS, and holds no user-agent string

#### Scenario: Self-hosted service
- **WHEN** a viewer exports diagnostics from the web viewer of a team's own sharing service
- **THEN** the file says the sharing service is custom and does not contain its address

### Requirement: No content in browser diagnostics
The web viewer's log and export SHALL follow the same rules on content and secrets as the app's: no terminal output, timeline text, names, invite codes, room ids, resume tokens, TURN credentials, session descriptions, ICE candidates, IP addresses, file names or paths.

#### Scenario: Markers do not leak from the browser
- **WHEN** a browser viewer joins with an invite code, name, TURN credential, terminal output, timeline, shared file name and suggestion that each contain a known marker, and exports diagnostics
- **THEN** none of the markers and no IP address appears in the exported file
