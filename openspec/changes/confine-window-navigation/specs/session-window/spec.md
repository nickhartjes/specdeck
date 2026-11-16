# Spec Delta

## ADDED Requirements

### Requirement: Links in session content
A link in a document, a plan, one of the agent's answers or a shared Markdown file SHALL open in the participant's default browser, and only when the participant selects it. The app SHALL open only web addresses that pass the check for shared links (see `shared-links`). A link that does not pass SHALL NOT open, and the app SHALL say so. The participant SHALL be able to see a link's address before opening it.

#### Scenario: Link in a document
- **WHEN** a participant selects a link to `https://example.com/guide` in a document
- **THEN** the page opens in their default browser, and the session window keeps showing the document

#### Scenario: Link planted in a shared file
- **WHEN** a viewer shares a Markdown file with a link, and the host selects that link
- **THEN** the page opens in the host's browser, and the page receives nothing of the session

#### Scenario: Not a web address
- **WHEN** a participant selects a link to `file:///etc/passwd`, or to `http://localhost:3000/`
- **THEN** nothing opens, and the app says the link can't be opened

#### Scenario: See the address first
- **WHEN** a participant points at a link whose text reads "the docs"
- **THEN** the link's address is shown
