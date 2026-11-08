# security-documentation Specification

## Purpose
Defines what SpecDeck's published security and privacy documentation must tell a developer and their security officer, so that they can decide whether SpecDeck may be used at work, and so that the documentation never claims more than the app and the sharing service do.

## Requirements

### Requirement: A security section in the docs
The documentation site SHALL have its own *Security and privacy* section, listed in the sidebar. It SHALL cover your data, the sharing service, Claude's browser, what is on your computer, and the known limits, and it SHALL start with an overview that links to the rest. It SHALL be written in plain words, without marketing.

#### Scenario: Security officer looks for it
- **WHEN** someone opens the documentation site
- **THEN** the sidebar shows the Security and privacy section, and its overview links to each of its pages

### Requirement: Claims match the code
Every statement in the security documentation SHALL hold for the latest release. A protection that is planned but not built SHALL be described as planned, without a date. A protection SHALL NOT be presented as a separate layer when it adds nothing beyond another one.

#### Scenario: Planned protection
- **WHEN** the documentation mentions the PAKE for join codes
- **THEN** it says the PAKE is planned and not built, and gives no date

#### Scenario: Frame signatures
- **WHEN** the documentation describes how session content is protected
- **THEN** it names DTLS as the protection, and does not present the host's frame signatures as an extra layer against the service

#### Scenario: Existing pages agree
- **WHEN** another page, such as the self-host overview, says what the sharing service can see or do
- **THEN** it says the same as the security section, or links to it

### Requirement: Where session data lives and travels
The documentation SHALL state that session content travels between host and each viewer over encrypted WebRTC (DTLS) connections, and never through the sharing service. This covers terminal, timeline, documents, shared files and links. It SHALL state what the host's computer keeps while a session runs and when that is deleted, and that nothing of a session is stored anywhere else.

#### Scenario: Reader asks whether the service stores sessions
- **WHEN** a reader looks up where session content is stored
- **THEN** the documentation says it is held only on the host's computer, names the temporary files kept while the session runs, says they are deleted when it ends, and says the sharing service and the relay store none of it

### Requirement: What viewers can see
The documentation SHALL state as its rule that everything in the terminal is shared, and that the timeline adds nothing from outside the session folder. It SHALL NOT say that files outside the session folder stay private. It SHALL also state:
- that late joiners get the whole history;
- that whole code files are sent only under *Code viewers can open*;
- that a shared file reaches the agent only when the host puts it in the session folder.

#### Scenario: File outside the session folder
- **WHEN** a reader looks up whether a file outside the session folder can reach viewers
- **THEN** the documentation says the timeline shows it only by path, and that what the agent or a command prints about it in the terminal is shared

#### Scenario: Late joiner
- **WHEN** a reader looks up what someone who joins late sees
- **THEN** the documentation says they get the whole terminal and timeline from the start of the session

### Requirement: Join codes and their weakness
The documentation SHALL explain how join codes protect a session, their size and lifetime, and the join limits. It SHALL also state that the room id and the connection-setup key are derived from the code, so whoever runs the sharing service could try codes offline. It SHALL say what that would let them do, how a team can limit the risk, and that a fix is planned.

#### Scenario: Reader evaluates the public service
- **WHEN** a security officer reads about join codes
- **THEN** they learn that the service operator could in principle guess a code offline and then intercept that session, and that self-hosting or short sessions with new codes reduce the exposure

### Requirement: The sharing service described
The documentation SHALL state the following, linking to the self-host pages for setup instead of repeating them:
- who runs the built-in service, and on what terms;
- what the service and the relay see and log;
- who gets relay credentials;
- the quotas, and the addresses the relay never reaches;
- what apps show when the service is full;
- that web viewers trust the operator for the page's code;
- why self-hosting gives full control.

#### Scenario: Reader considers self-hosting
- **WHEN** a reader wants to know what they gain by running their own service
- **THEN** the documentation says what moves under their control and links to the self-host overview

#### Scenario: Operator named
- **WHEN** a reader looks up who runs the built-in service
- **THEN** the documentation names the project's maintainer, and says it runs best effort, with no uptime or support promise

### Requirement: Claude's browser described
The documentation SHALL describe the following for Claude's browser:
- the per-session tool endpoint is reachable only from the host's own computer and only with the session's token;
- the token is not written to disk;
- each browser is hidden and private, its pages cannot reach the app, and it keeps nothing after the session;
- Claude Code asks before using it under its own permission mode;
- screenshots enter the shared timeline unless the host turns that off.

#### Scenario: Reader asks if a page can reach SpecDeck
- **WHEN** a reader looks up what a web page opened in Claude's browser can do
- **THEN** the documentation says it cannot call SpecDeck's functions, read its settings or sessions, or type into the agent

#### Scenario: Screenshots
- **WHEN** a reader looks up whether screenshots reach viewers
- **THEN** the documentation says they are shared in the timeline and names the setting that stops keeping them

### Requirement: The app on your computer
The documentation SHALL state:
- what the app stores on the user's computer, and with which file permissions;
- what logs and diagnostic exports hold, and that the app sends no telemetry;
- what notifications can show;
- how updates are signed and verified, and what an update check sends;
- how releases and the server image are signed, including what is not signed yet.

#### Scenario: Reader checks signing
- **WHEN** a reader looks up whether installers are code-signed
- **THEN** the documentation says which are and which are not today, and how to verify the server image and updates

### Requirement: Known limits listed
The documentation SHALL have one list of the known security limits. Each item SHALL say what it means for the reader and give a workaround where one exists. An item SHALL leave the list only when a release fixes it.

#### Scenario: Unfixed weakness
- **WHEN** a weakness is known and not fixed in the latest release
- **THEN** it is on the list, in plain words

#### Scenario: Fixed in a release
- **WHEN** a release fixes a listed limit
- **THEN** the change that fixes it removes or rewrites the item

### Requirement: Reporting a vulnerability
The repository SHALL have a `SECURITY.md` at its root. It SHALL link to the security documentation and say how to report a vulnerability privately: through GitHub's private vulnerability reporting, or by email to `security@hartj.es`. It SHALL ask reporters not to open public issues for vulnerabilities.

#### Scenario: Researcher finds a bug
- **WHEN** someone finds a vulnerability and opens `SECURITY.md`
- **THEN** they find both private ways to report it and a link to the security documentation

### Requirement: Diagrams in both themes
The data flow, and how Claude's browser connects, SHALL each be shown as a diagram. Each diagram SHALL stay readable in the docs site's light and dark themes, and on GitHub in both, and SHALL carry a text alternative.

#### Scenario: Dark theme
- **WHEN** a reader switches the docs site to dark
- **THEN** the diagrams' lines and labels follow the theme and stay readable

#### Scenario: README on GitHub in dark mode
- **WHEN** a reader views the README on GitHub with a dark theme
- **THEN** the data-flow diagram shows its dark variant

### Requirement: Found from where people start
The README, the hosting, joining and browser guides, and the self-host overview SHALL link to the security documentation where they discuss what is shared or what the service sees.

#### Scenario: Host reads the hosting guide
- **WHEN** a host reads *What viewers see* in the hosting guide
- **THEN** a link leads to the security documentation's page on your data
