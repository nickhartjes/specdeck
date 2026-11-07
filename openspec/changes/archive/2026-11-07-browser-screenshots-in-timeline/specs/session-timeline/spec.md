# Spec Delta

## ADDED Requirements

### Requirement: Capture screenshots
When the result of an MCP tool holds an image, such as a screenshot from SpecDeck's browser, Playwright MCP or Claude in Chrome, the timeline SHALL keep the first image with that tool call. For SpecDeck's own browser, it SHALL also keep the address of the page shown. Images in the results of Claude Code's own tools SHALL NOT be kept. Participants who join later SHALL get the screenshots too.

#### Scenario: Screenshot from SpecDeck's browser
- **WHEN** Claude takes a screenshot of `http://localhost:5173/login` in its browser
- **THEN** the screenshot's tool call in the timeline carries the image and the address `localhost:5173/login`

#### Scenario: Screenshot from another MCP server
- **WHEN** Claude takes a screenshot with Playwright MCP
- **THEN** that tool call carries the image, without an address

#### Scenario: Claude reads an image file
- **WHEN** Claude Code reads `~/Desktop/photo.png` with its own Read tool
- **THEN** the tool entry names the file and carries no image

#### Scenario: Late joiner
- **WHEN** a viewer joins after Claude took three screenshots
- **THEN** the viewer's timeline has all three

### Requirement: Screenshots small enough to send
An image too large for the timeline entry to be sent whole SHALL be made smaller, by scaling it down and saving it as JPEG, before it is kept. An image that is still too large, or that cannot be read, SHALL be left out, and the step SHALL say that the screenshot was too large to show.

#### Scenario: Large screenshot
- **WHEN** an MCP server returns a 480 KB screenshot
- **THEN** the timeline keeps a smaller JPEG of it, and every viewer stays connected

#### Scenario: Screenshot from SpecDeck's browser
- **WHEN** SpecDeck's browser returns a screenshot of 150 KB or less
- **THEN** the timeline keeps it as it is

#### Scenario: Image that cannot be read
- **WHEN** an MCP tool returns an image in a format that cannot be read
- **THEN** the step says the screenshot could not be shown, and the tool call is shown as usual

### Requirement: Show screenshots in the timeline
Settings SHALL offer *Show screenshots in the timeline*, on by default, saying that viewers see them too. While it is off, sessions SHALL keep no images: everyone, the host included, sees that a screenshot was taken, but not the image. A change SHALL apply to sessions started afterwards.

#### Scenario: Turned off
- **WHEN** the host turns the setting off, starts a session, and Claude takes a screenshot
- **THEN** every participant sees that Claude took a screenshot, with a note that screenshots aren't shown in this session, and no image is sent

#### Scenario: Running session
- **WHEN** the host turns the setting off while a session is running
- **THEN** that session keeps showing screenshots, and sessions started afterwards don't

## MODIFIED Requirements

### Requirement: Older viewers keep working
The answers, plan decisions, permission decisions, helper reports and screenshots SHALL be sent in a form that viewers on an earlier app version of the same protocol version ignore, so that those viewers still show the timeline as before.

#### Scenario: Viewer on an earlier version
- **WHEN** a viewer on an app version without this change joins a session in which Claude asked a question
- **THEN** their timeline shows the question's tool entry as before, without the answer, and no error

#### Scenario: Screenshot for an earlier version
- **WHEN** a viewer on an app version without screenshots watches Claude take a screenshot
- **THEN** their timeline shows the screenshot's tool call as before, without the image, and the viewer stays connected

### Requirement: Entries too large to send whole
The host SHALL send every timeline entry in a form small enough for every viewer's connection to accept. An entry too large for that, such as the diff of a very large created file, SHALL be cut to fit and marked, and participants SHALL see a note that it is shown in part. Sending a large entry SHALL NOT disconnect any viewer.

#### Scenario: Very large file created
- **WHEN** Claude Code creates a 1 MB file inside the session folder while viewers watch
- **THEN** the timeline shows the beginning of the file's content as added lines, with a note that the change is too large to show in full, and every viewer stays connected

#### Scenario: Ordinary changes
- **WHEN** Claude Code edits a file with a diff of a few hundred lines
- **THEN** the timeline shows the whole diff, with no note

#### Scenario: Viewer on an older app
- **WHEN** a viewer whose app does not know the note receives an entry that was cut
- **THEN** the viewer stays connected and sees the part that was sent

#### Scenario: Screenshot with a long error
- **WHEN** a tool result carries both a screenshot and an error message that together are too large to send
- **THEN** the screenshot is left out first, with the note that it is shown in part, and the error is shown
