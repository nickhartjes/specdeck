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
In the timeline's summary, browser tool calls SHALL be described by what Claude did in its browser, naming the address it opened and the key it pressed, and a run of browser calls in a row SHALL be one step. The notice that Claude Code needs the host SHALL say what Claude wants to do in its browser. The details view SHALL keep the tool's name and input.

#### Scenario: One page opened
- **WHEN** Claude opens `http://localhost:5173/login`
- **THEN** the summary shows "Opened localhost:5173/login in its browser"

#### Scenario: A run of browser calls
- **WHEN** Claude opens a page, types into a field, clicks a button and takes a screenshot, one after the other
- **THEN** the summary shows one step: "Used its browser: opened localhost:5173/login, typed on the page, clicked on the page, took a screenshot"

#### Scenario: Permission asked
- **WHEN** Claude Code asks the host's permission to open `http://localhost:5173/`
- **THEN** the notice says "Claude wants to open localhost:5173/ in its browser"
