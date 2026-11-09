# agent-browser Specification

## Purpose

Gives a Claude Code session a browser of SpecDeck's own, so Claude can open, read, use and screenshot web pages, such as the app it is building, without the host installing or setting up a browser for it.

## Requirements

### Requirement: A browser for Claude Code
On macOS, a Claude Code session started while *Give Claude a browser* is on SHALL offer Claude Code a browser as tools of an MCP server named `specdeck`. Sessions of other agents, terminal sessions, and sessions on Windows and Linux SHALL NOT get it.

#### Scenario: Claude Code on macOS
- **WHEN** the host starts a Claude Code session on macOS with the setting on
- **THEN** Claude Code lists the `specdeck` browser tools and can use them

#### Scenario: Another agent
- **WHEN** the host starts an OpenCode session or a terminal session
- **THEN** the session offers no browser

#### Scenario: Windows
- **WHEN** the host starts a Claude Code session on Windows
- **THEN** the session offers no browser, and the session works as before

### Requirement: Browser tools
The browser SHALL offer these tools: open an address, read the page, click an element, type into an element, press a key, take a screenshot, go back, and read the console. Each tool SHALL answer with its result or with an error that says in plain words what went wrong.

#### Scenario: Open a page
- **WHEN** Claude opens `http://localhost:5173/login`
- **THEN** the tool answers once the page has loaded, with its address and title

#### Scenario: Page does not load
- **WHEN** Claude opens an address where nothing answers
- **THEN** the tool answers with an error saying the page could not be loaded, and why

#### Scenario: Console
- **WHEN** a page logs an error to its console and Claude reads the console
- **THEN** the answer lists that error with its level, in the order the page logged it

### Requirement: Reading the page
Reading the page SHALL give its text in reading order, with every element Claude can act on (links, buttons, fields) numbered. Click and type SHALL take such a number. A number from before the page changed SHALL be refused with an error asking Claude to read the page again.

#### Scenario: Fill in a form
- **WHEN** Claude reads a login page, types an address into field 2 and clicks button 4
- **THEN** the address is in that field and the button is clicked, as if a person did it

#### Scenario: Page changed
- **WHEN** Claude clicks number 7 after the page navigated elsewhere
- **THEN** the click is refused, and the error asks Claude to read the page again

### Requirement: Screenshots
A screenshot SHALL show the page as it is at that moment, as a JPEG image of fixed width, no larger than 150 KB. Taking one SHALL work while the browser is never on screen.

#### Scenario: Screenshot after a change
- **WHEN** Claude clicks a button that shows an error message and then takes a screenshot
- **THEN** the image shows the error message

#### Scenario: Long page
- **WHEN** Claude takes a screenshot of a page longer than the browser's window
- **THEN** the image shows the visible part of the page, at the same fixed width

### Requirement: Never on screen
The browser SHALL never show a window, take focus, or appear in the Window menu or the window list. It SHALL be created on the session's first browser tool call, one per session, and closed when the session ends.

#### Scenario: Claude browses while the host types elsewhere
- **WHEN** Claude opens and clicks through pages while the host works in another app
- **THEN** no window appears and the host's focus does not move

#### Scenario: Two sessions
- **WHEN** two Claude Code sessions each open a page
- **THEN** each has its own browser, and neither sees the other's page

### Requirement: A clean, separate browser
Each session's browser SHALL start without cookies, storage, history or logins, and SHALL NOT share them with the host's own browsers, with SpecDeck's windows or with another session. Nothing it stored SHALL remain after the session ends.

#### Scenario: Logged in elsewhere
- **WHEN** the host is logged in to a site in their own browser and Claude opens that site
- **THEN** Claude's browser is not logged in

#### Scenario: Next session
- **WHEN** Claude logs in to the app it builds, the session ends, and a new session opens the app
- **THEN** the new session's browser is not logged in

### Requirement: Pages cannot reach SpecDeck
A page opened in the browser SHALL NOT be able to use any of SpecDeck's functions, read its settings or sessions, or send input to the agent. Only the session's own Claude Code SHALL be able to use the session's browser tools.

#### Scenario: Hostile page
- **WHEN** Claude opens a page whose script tries to call SpecDeck's app functions
- **THEN** the calls fail, and nothing in SpecDeck changes

#### Scenario: Another program on the computer
- **WHEN** another local program sends a request to the session's browser tools without the session's token
- **THEN** the request is refused

### Requirement: The host approves browser use
SpecDeck SHALL NOT approve the browser tools itself, in the settings it gives Claude Code or in its answers to permission requests. Their use SHALL follow the host's own Claude Code permission rules and mode, as for any MCP tool.

#### Scenario: First page
- **WHEN** Claude wants to open a page for the first time in a session where the host has not allowed the browser tools, in the default permission mode
- **THEN** the host is asked for permission in the terminal, and the page opens only if the host allows it

#### Scenario: Auto mode
- **WHEN** the host runs Claude Code in auto mode
- **THEN** Claude Code decides about the browser tools as it does for any other tool, without SpecDeck taking part

### Requirement: Browser without the app
While no SpecDeck app is running, each browser tool SHALL answer with an error saying the browser is not available until SpecDeck is open again. The session SHALL keep running. Once the app is back, the next tool call SHALL start a new, clean browser.

#### Scenario: Restart to update
- **WHEN** the host restarts SpecDeck to update while Claude has a page open, and Claude then takes a screenshot
- **THEN** the screenshot fails while the app is gone; after the restart, Claude can open the page again in a new browser

### Requirement: Give Claude a browser
Settings SHALL offer *Give Claude a browser*, on by default. A change SHALL apply to sessions started afterwards; running sessions keep what they started with.

#### Scenario: Turned off
- **WHEN** the host turns the setting off and starts a Claude Code session
- **THEN** that session offers no browser, and sessions already running keep theirs

### Requirement: Browser steps in plain words
In the timeline's summary, browser tool calls SHALL be described by what Claude did in its browser, naming the address it opened, the key it pressed, and the element it clicked or typed into, and a run of browser calls in a row SHALL be one step. The element SHALL be named as Claude's last read of the page describes it. The notice that Claude Code needs the host SHALL say what Claude wants to do there, naming the element the same way. The details view SHALL keep the tool's name and input.

#### Scenario: One page opened
- **WHEN** Claude opens `http://localhost:5173/login`
- **THEN** the summary shows "Opened localhost:5173/login in its browser"

#### Scenario: A run of browser calls
- **WHEN** Claude opens a page, types into its *Email* field, clicks its *Sign in* button and takes a screenshot, one after the other
- **THEN** the summary shows one step: "Used its browser: opened localhost:5173/login, typed into “Email”, clicked “Sign in”, took a screenshot"

#### Scenario: Element without a label
- **WHEN** Claude clicks a button that has no text, label or title
- **THEN** the step names the button by its kind: "Clicked a button in its browser"

#### Scenario: Element not known
- **WHEN** Claude clicks a number the session has not seen in a read of the page
- **THEN** the step says "Clicked on the page in its browser"

#### Scenario: While it runs
- **WHEN** Claude is clicking the *Sign in* button
- **THEN** the step reads that Claude is clicking “Sign in” in its browser

#### Scenario: Permission asked
- **WHEN** Claude Code asks the host's permission to open `http://localhost:5173/`
- **THEN** the notice says "Claude wants to open localhost:5173/ in its browser"

#### Scenario: Permission to click
- **WHEN** Claude Code asks the host's permission to click the *Sign in* button
- **THEN** the notice says "Claude wants to click “Sign in” in its browser"

### Requirement: Sensitive fields stay hidden
A sensitive field is a password field, or a field the page marks as a password, one-time code, card number or card security code. Reading the page SHALL say whether such a field is filled, never its value. Text Claude types into a sensitive field, or into a field the session does not know, SHALL NOT be sent to anyone in the timeline, and its step SHALL say that Claude filled in the field.

#### Scenario: Password typed
- **WHEN** Claude types `secret` into the *Password* field of a login page it read
- **THEN** the step says "Filled in “Password”", and no participant's timeline, the details view included, contains `secret`

#### Scenario: One-time code
- **WHEN** Claude reads a page with a field marked as a one-time code that holds `123456`
- **THEN** the page's text says the field is filled, without `123456`

#### Scenario: Ordinary field
- **WHEN** Claude types `anna@example.com` into the *Email* field
- **THEN** the step says "Typed into “Email”", and the details view shows `anna@example.com`

#### Scenario: Field not known
- **WHEN** Claude types into a number the session has not seen in a read of the page
- **THEN** the typed text is left out of the timeline, and the step says Claude filled in a field

### Requirement: Steps in other browsers in plain words
Browser calls of Playwright MCP and Claude in Chrome SHALL be described in the timeline's summary and in permission notices as SpecDeck's own browser calls are, each action of a batch on its own. An element clicked or typed into SHALL be named by its label or kind from Claude's last read of the page in that browser; without one, by Claude's own description of it in the call; otherwise as "on the page".

#### Scenario: Playwright run
- **WHEN** Claude, through Playwright MCP, opens `http://127.0.0.1:5179/`, takes a snapshot of the page, and clicks its *Sign in* button
- **THEN** the summary shows one step: "Used its browser: opened 127.0.0.1:5179/, read the page, clicked “Sign in”"

#### Scenario: Claude in Chrome batch
- **WHEN** Claude, after reading the page with Claude in Chrome, logs in with one batch that clicks the *Email* field, types into it, and clicks *Sign in*
- **THEN** the step lists each action: "clicked “Email”, typed into “Email”, clicked “Sign in”"

#### Scenario: Element from a read
- **WHEN** Claude in Chrome's read of the page lists a text field under the label *Email*, and Claude clicks it
- **THEN** the step says "Clicked “Email” in its browser"

#### Scenario: No read, Claude's description
- **WHEN** Claude clicks through Playwright MCP without having taken a snapshot, describing the element as "Sign in button"
- **THEN** the step says "Clicked Sign in button in its browser"

#### Scenario: Claude in Chrome without a read
- **WHEN** Claude clicks through Claude in Chrome a spot on the page, summarised as "Submits the sign-in form", without an element from a read
- **THEN** the step says "Submits the sign-in form in its browser"

#### Scenario: Call not recognised
- **WHEN** Claude uses a Claude in Chrome tool the session has no words for, such as `gif_creator`
- **THEN** the step reads "used gif_creator" in its browser step, and the details view shows the tool's name and input

#### Scenario: Permission to click
- **WHEN** Claude Code asks the host's permission to click, through Playwright MCP, the *Sign in* button from Claude's last snapshot
- **THEN** the notice says "Claude wants to click “Sign in” in its browser"

### Requirement: Text typed in other browsers
Text Claude types or fills in through Playwright MCP or Claude in Chrome SHALL be sent in the timeline only when Claude's last read of the page in that browser says the field is not a password field. Any other such text SHALL NOT be sent to anyone in the timeline, and its step SHALL say that Claude filled in the field.

#### Scenario: Password through Playwright
- **WHEN** Claude types `hunter2-test` into the *Password* field through Playwright MCP
- **THEN** the step says "Filled in “Password”", and no participant's timeline, the details view included, contains `hunter2-test`

#### Scenario: Ordinary field through Playwright
- **WHEN** Claude types `test@example.com` into the *Email* field through Playwright MCP
- **THEN** the step says "Filled in “Email”", and no participant's timeline contains `test@example.com`

#### Scenario: Ordinary field through Claude in Chrome
- **WHEN** Claude in Chrome's last read lists the *Email* field as `type="email"`, and Claude clicks it and types `test@example.com`
- **THEN** the step says "Typed into “Email”", and the details view shows `test@example.com`

#### Scenario: Password through Claude in Chrome
- **WHEN** Claude in Chrome's last read lists the *Password* field as `type="password"`, and Claude clicks it and types `hunter2-test`, alone or in a batch
- **THEN** the step says "Filled in “Password”", and no participant's timeline contains `hunter2-test`

#### Scenario: Field not known
- **WHEN** Claude types through Claude in Chrome without having clicked a field from a read of the page
- **THEN** the typed text is left out of the timeline, and the step says Claude filled in a field

#### Scenario: Form filled through Playwright
- **WHEN** Claude fills the *Card number* and *Notes* fields in one `browser_fill_form` call
- **THEN** the step says "Filled in “Card number” and “Notes”", the details view says the text is hidden, and neither value is in the timeline

#### Scenario: Choice in a form
- **WHEN** Claude ticks a checkbox in a `browser_fill_form` call, or sets a select from Claude in Chrome's read with `form_input`
- **THEN** the chosen value is in the details view, since it is not typed text
