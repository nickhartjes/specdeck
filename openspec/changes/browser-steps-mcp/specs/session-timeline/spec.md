# Spec Delta

## MODIFIED Requirements

### Requirement: Capture tool calls
Each tool Claude Code uses SHALL appear as a timeline entry showing the tool, its input, and whether it succeeded, failed, or did not run. The input SHALL NOT carry file contents: the new text of a written file, the old and new text of an edit, and a notebook cell's new source are left out, since the diff shows the change. Nor SHALL it carry text Claude types into a sensitive or unknown field of its browser (`agent-browser`, *Sensitive fields stay hidden*), or text typed through another browser that is not known to go into an ordinary field (`agent-browser`, *Text typed in other browsers*). File paths in the input SHALL be shown as other paths in the timeline are: relative inside the session folder, starting with `~` elsewhere in the home folder, otherwise unchanged. Shell commands SHALL be shown as typed.

#### Scenario: Claude runs a command
- **WHEN** Claude Code runs a shell command
- **THEN** a tool entry shows the command and whether it succeeded

#### Scenario: Tool call fails or is denied
- **WHEN** a tool call fails, or the host denies it
- **THEN** the tool entry shows that it failed or did not run

#### Scenario: Claude writes a file
- **WHEN** Claude Code writes `src/server.ts` in the session folder
- **THEN** the tool entry names `src/server.ts` and does not carry the file's text, and the file change shows what changed

#### Scenario: Claude edits a file outside the session folder
- **WHEN** Claude Code edits a file in the host's home folder but outside the session folder
- **THEN** the tool entry names the file by a path starting with `~`, and carries neither the old nor the new text

#### Scenario: Claude types a password in its browser
- **WHEN** Claude types into the password field of a page in its browser
- **THEN** the tool entry names the field and says the text is hidden, and does not carry the text

#### Scenario: Claude types through Playwright MCP
- **WHEN** Claude types into any field through Playwright MCP
- **THEN** the tool entry says the text is hidden, and does not carry the text

### Requirement: Older viewers keep working
The answers, plan decisions, permission decisions, helper reports, screenshots and the elements of browser steps, those of other browsers included, SHALL be sent in a form that viewers on an earlier app version of the same protocol version ignore, so that those viewers still show the timeline as before.

#### Scenario: Viewer on an earlier version
- **WHEN** a viewer on an app version without this change joins a session in which Claude asked a question
- **THEN** their timeline shows the question's tool entry as before, without the answer, and no error

#### Scenario: Screenshot for an earlier version
- **WHEN** a viewer on an app version without screenshots watches Claude take a screenshot
- **THEN** their timeline shows the screenshot's tool call as before, without the image, and the viewer stays connected

#### Scenario: Browser click for an earlier version
- **WHEN** a viewer on an app version without element names watches Claude click the *Sign in* button
- **THEN** their timeline shows "Clicked on the page in its browser", and the viewer stays connected

#### Scenario: Password for an earlier version
- **WHEN** a viewer on an app version without element names watches Claude type a password in its browser
- **THEN** their timeline does not carry the password

#### Scenario: Other browser for an earlier version
- **WHEN** a viewer on an app version without steps for other browsers watches Claude fill a form through Playwright MCP
- **THEN** their timeline shows the tool call as before, without the filled-in values, and the viewer stays connected
