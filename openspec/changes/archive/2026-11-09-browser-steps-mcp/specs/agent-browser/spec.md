# Spec Delta

## ADDED Requirements

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
