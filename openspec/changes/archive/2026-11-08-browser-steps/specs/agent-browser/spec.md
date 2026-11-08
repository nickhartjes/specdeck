# Spec Delta

## MODIFIED Requirements

### Requirement: Browser steps in plain words
In the timeline's summary, browser tool calls SHALL be described by what Claude did in its browser, naming the address it opened, the key it pressed, and the element it clicked or typed into, and a run of browser calls in a row SHALL be one step. The element SHALL be named by its label from Claude's last read of the page, or by its kind when it has no label; when the session does not know the element, the step SHALL say that Claude clicked or typed on the page. The notice that Claude Code needs the host SHALL say what Claude wants to do in its browser, naming the element in the same way. The details view SHALL keep the tool's name and input.

#### Scenario: One page opened
- **WHEN** Claude opens `http://localhost:5173/login`
- **THEN** the summary shows "Opened localhost:5173/login in its browser"

#### Scenario: A run of browser calls
- **WHEN** Claude opens a page, types into its *Email* field, clicks its *Sign in* button and takes a screenshot, one after the other
- **THEN** the summary shows one step: "Used its browser: opened localhost:5173/login, typed into “Email”, clicked “Sign in”, took a screenshot"

#### Scenario: Element without a label
- **WHEN** Claude clicks a button that has no text, label or title
- **THEN** the step says "Clicked a button in its browser"

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

## ADDED Requirements

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
