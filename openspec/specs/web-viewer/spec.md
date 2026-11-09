# web-viewer Specification

## Purpose
Lets people who cannot install the SpecDeck app watch a shared session from a web browser, served by the same sharing service the host uses, with the same protections as the app.

## Requirements

### Requirement: Watch in a browser
A sharing service SHALL serve a web page where a viewer joins a session with its invite code, without installing anything. The page SHALL only watch: it SHALL offer no way to host a session or to send input to one.

#### Scenario: Join with a code
- **WHEN** someone opens the sharing service's address in a browser and enters a valid invite code and a name
- **THEN** they join the session as a viewer, and the host sees their name in the participant list

#### Scenario: No hosting
- **WHEN** someone opens the web viewer
- **THEN** the page offers joining a session and nothing else

### Requirement: Same session view as the app
The web viewer SHALL show a session as the app's viewer does: the files, tabs and timeline panels, the people in the title bar and the status bar, following and free scrolling, search, copy, jumping to prompts, and the Overview, Developer and Present presets. Each viewer's navigation SHALL stay local to their page. The app's type SHALL come from the sharing service the page was loaded from.

#### Scenario: Late join
- **WHEN** a viewer joins in a browser after the session has been running for a while
- **THEN** they see the terminal output and timeline from the start of the session, as in the app

#### Scenario: Developer view
- **WHEN** a browser viewer chooses the Developer preset
- **THEN** the terminal tab is shown between the files and the timeline, with every step listed, as in the app

### Requirement: Served by the session's own service
The web viewer SHALL be part of the sharing service, so every service, the public one or a self-hosted one, serves its own copy with no separate deployment. The page SHALL load nothing from any other address.

#### Scenario: Self-hosted service
- **WHEN** a team runs its own sharing service with the self-hosting setup
- **THEN** its address serves the web viewer, and the page's code and data come only from that address

#### Scenario: No third parties
- **WHEN** a browser viewer loads the page and watches a session
- **THEN** the browser makes no request to any address other than the sharing service, the team's relay and the host's connection

### Requirement: The service states its version
A sharing service that serves the web viewer SHALL state, in its description of the web viewer (`/web-viewer.json`), the release version it was built from, next to the session protocol, so anyone can check which release a service runs with one request. The version SHALL be the release's version without a leading `v`. The service's health check SHALL keep its current answer.

#### Scenario: Checking which release runs
- **WHEN** an operator requests `https://specdeck.example.com/web-viewer.json` from a service built from release 1.7.0
- **THEN** the answer is `{"protocol":N,"version":"1.7.0"}`, where N is the session protocol release 1.7.0 speaks

#### Scenario: After an update
- **WHEN** a service is updated from one release to the next and the operator requests `/web-viewer.json` again
- **THEN** the answer gives the new release's version

#### Scenario: Health check unchanged
- **WHEN** a monitor requests `/health`
- **THEN** the answer is still `ok`

#### Scenario: Apps and services of different releases
- **WHEN** a host's app reads `/web-viewer.json` from a service of a newer or older release, with or without a version
- **THEN** the app finds the web viewer and its protocol as before

### Requirement: The join page shows the service's release
The web viewer's join page SHALL show the release of the sharing service that serves it, as the service states it, as small secondary text below the join form. When the service states no release, the page SHALL show no release text, and joining SHALL NOT wait for the version.

#### Scenario: Service states its release
- **WHEN** a browser opens the join page of a service that runs release 1.7.0
- **THEN** the page shows "SpecDeck 1.7.0" below the join form

#### Scenario: Service without a version
- **WHEN** a browser opens the join page of a service that states no release, or the request for it fails
- **THEN** the page shows no release text, and the form works as before

#### Scenario: Joining right away
- **WHEN** a viewer submits the form before the version has arrived
- **THEN** they join without waiting for it

### Requirement: Only the page's own service
The web viewer SHALL join only sessions on the sharing service that served it, and SHALL refuse a join link that names another service, pointing the viewer to that service's own page instead.

#### Scenario: Link for another service
- **WHEN** someone pastes into the main site a join link whose session is on a self-hosted service
- **THEN** the page does not connect to that service, and offers to open the link on that service's own page

### Requirement: Code stays out of requests
The web viewer SHALL NOT send the invite code to the sharing service or any other server, including in the page's address, request headers or logs.

#### Scenario: Join link opened in a browser
- **WHEN** a viewer opens `https://specdeck.example.com/join#7K2M-9QXH-4TPA`
- **THEN** the request for the page does not contain the code, and the code is filled in on the page

### Requirement: Same protection as the app
A browser viewer SHALL verify the host and every piece of session data as the app does. It SHALL show nothing from a connection whose details were not made with the session's code, nor data that fails verification.

#### Scenario: Matchmaking service substitutes a connection
- **WHEN** the sharing service substitutes its own connection details while a browser viewer joins
- **THEN** the page rejects the connection and shows no session content

#### Scenario: Tampered data
- **WHEN** a browser viewer receives output that was altered after the host sent it
- **THEN** the page discards it and requests it again

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

### Requirement: Browser-specific failures
The web viewer SHALL tell the viewer why a join failed, using the same messages as the app. When the page's session protocol is older than any the host's app supports, it SHALL say that the sharing service needs to be updated, and SHALL NOT tell the viewer to update an app.

#### Scenario: Network blocks the connection
- **WHEN** a browser viewer's network allows the page to load but blocks the connection to the host
- **THEN** the page says that their network does not allow the connection, instead of waiting indefinitely

#### Scenario: Browser with WebRTC turned off
- **WHEN** a viewer joins in a browser whose settings turn WebRTC off
- **THEN** the page says at once that the browser blocks the connection, and suggests allowing WebRTC, another browser or the app, without waiting for a timeout

#### Scenario: Page one release older than the host's app
- **WHEN** the web viewer its sharing service serves speaks the protocol of the release before the host's app
- **THEN** the viewer is admitted and watches the session, without the requests the newer protocol adds

#### Scenario: Page older than the host's app
- **WHEN** the host's app no longer supports the session protocol of the web viewer its sharing service serves
- **THEN** the viewer is not admitted and is told the sharing service's web viewer needs to be updated, and that the SpecDeck app can be used instead

#### Scenario: Host's app older than the page
- **WHEN** the host's app speaks an older session protocol than the web viewer
- **THEN** the viewer is not admitted and is told that the host needs to update their app

### Requirement: Session ends in the browser
When the session ends, the viewer is removed, or the viewer leaves, the web viewer SHALL close the connection and say why, as the app does, and SHALL offer to join another session.

#### Scenario: Host closes the session
- **WHEN** the host closes the session while a browser viewer watches
- **THEN** the page shows that the session has ended, and offers to join another one

#### Scenario: Tab closed
- **WHEN** a browser viewer closes the tab
- **THEN** the host's participant list no longer shows them

### Requirement: Requests from the browser
A viewer in the web viewer SHALL be able to send the same requests to the host as a viewer in the app, under the same rules, and SHALL see what became of them in the same way.

#### Scenario: Suggest from the browser
- **WHEN** the host allows prompt suggestions and a viewer in the browser suggests a prompt
- **THEN** the host sees it as from any other viewer, and the browser viewer sees what became of it

### Requirement: Links in the browser
The web viewer SHALL show link cards and let a viewer send a link to the host as the app does. Opening a link SHALL open it in a new browser tab that has no access back to the viewer's page and is not told which page it came from.

#### Scenario: Open a link from the browser
- **WHEN** a browser viewer opens a shared link
- **THEN** it opens in a new tab, and the page there cannot reach or navigate the SpecDeck tab, and gets no referrer

#### Scenario: Send a link from the browser
- **WHEN** viewers' links are allowed and a browser viewer sends a link
- **THEN** the host sees it waiting as from any other viewer, and the browser viewer sees what became of it

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

### Requirement: Shared files in the browser
A viewer in the web viewer SHALL see, open and save shared files as a viewer in the app does, and SHALL be able to share files under the same rules and limits, by choosing or dropping them. Saving a copy SHALL download the file under its shared name.

#### Scenario: Open a shared image in the browser
- **WHEN** the host shares `wireframe.png` and a viewer in the browser opens it
- **THEN** the image shows in a tab, as in the app

#### Scenario: Share from the browser
- **WHEN** the host allows viewers to share files and a viewer in the browser drops a log on the *Shared* tab
- **THEN** everyone sees it as shared by that viewer

### Requirement: Read requests and estimates in the browser
The web viewer SHALL show read requests and votes, let a viewer answer and vote, open what the host asked to read, including the changes in the last prompt, and show their timeline entries, as the app does.

#### Scenario: Vote from the browser
- **WHEN** the host starts a vote and a browser viewer votes M
- **THEN** the host sees the browser viewer as voted, and after the reveal everyone sees their M

#### Scenario: Question from the browser
- **WHEN** a browser viewer answers a read request with a question
- **THEN** the host sees the question as from any other viewer

### Requirement: Catching up in the browser
A browser viewer who joins late SHALL see earlier prompts folded and the overview of what happened before they joined, as in the app. Reloading the page SHALL count as joining again.

#### Scenario: Late join in a browser
- **WHEN** a viewer joins in a browser after five prompts finished
- **THEN** they see the overview and the five prompts folded, as in the app

#### Scenario: Page reloaded
- **WHEN** a browser viewer who joined at the start reloads the page after three more prompts finished
- **THEN** those three prompts are folded and the overview covers them

### Requirement: The braindump in the browser
A viewer in the web viewer SHALL see, open and save braindump items as a viewer in the app does. Under the same rules and limits, they SHALL be able to add files by choosing or dropping them, write notes, edit their own notes and remove their own items.

#### Scenario: Note from the browser
- **WHEN** the host allows adding files and notes and a viewer in the browser writes a note to the braindump
- **THEN** everyone sees the note in the braindump as added by that viewer

#### Scenario: Given items in the browser
- **WHEN** the host gives the braindump while a viewer watches in the browser
- **THEN** the viewer sees the items marked *Given to Claude*, as in the app

### Requirement: Replaying the history in the browser
A browser viewer SHALL apply timeline events that arrive together as one batch, in arrival order, as the app's viewer does, so that joining a long session in a browser does not render once per event.

#### Scenario: Late join of a long session in a browser
- **WHEN** a viewer joins in a browser a session with 400 finished prompts, and its events arrive one per message
- **THEN** they see the whole history in order, as in the app, after far fewer renders than messages

#### Scenario: Background tab
- **WHEN** events arrive while the browser tab is in the background
- **THEN** the timeline has caught up when the viewer returns to the tab

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

### Requirement: Kept and sent suggestions in the browser
The web viewer SHALL show a viewer's suggestion as kept for later when the host keeps it, and SHALL show prompts sent from a suggestion as suggested by their author, as the app does.

#### Scenario: Kept from the browser
- **WHEN** a browser viewer suggests a prompt and the host keeps it for later
- **THEN** the browser viewer sees it as kept for later by the host

#### Scenario: Mark in the browser
- **WHEN** the host sends a suggestion from a viewer in the app
- **THEN** a browser viewer's timeline shows the prompt as suggested by that viewer

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
