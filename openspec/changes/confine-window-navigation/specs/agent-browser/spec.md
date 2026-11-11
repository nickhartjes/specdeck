# Spec Delta

## MODIFIED Requirements

### Requirement: Pages cannot reach SpecDeck
A page opened in the browser SHALL NOT be able to use any of SpecDeck's functions, read its settings or sessions, or send input to the agent. Only the session's own Claude Code SHALL be able to use the session's browser tools. The browser SHALL NOT load SpecDeck's own page.

#### Scenario: Hostile page
- **WHEN** Claude opens a page whose script tries to call SpecDeck's app functions
- **THEN** the calls fail, and nothing in SpecDeck changes

#### Scenario: Another program on the computer
- **WHEN** another local program sends a request to the session's browser tools without the session's token
- **THEN** the request is refused

#### Scenario: Page sends the browser to SpecDeck's own page
- **WHEN** a page in the browser redirects to the address of SpecDeck's own page
- **THEN** the browser does not load it, and stays on the page it had

## ADDED Requirements

### Requirement: Only web pages in the browser
The browser SHALL load only `http` and `https` addresses, and its empty start page. This SHALL hold however the browser is sent somewhere: by a browser tool, a redirect, a link, a script, or going back. An address of any other kind SHALL be refused, and the browser SHALL stay on the page it had.

#### Scenario: Redirect to a file
- **WHEN** a page in the browser redirects to `file:///etc/passwd`
- **THEN** the browser does not load it

#### Scenario: Redirect between web pages
- **WHEN** `http://localhost:5173/` redirects to `http://localhost:5173/login`
- **THEN** the browser loads the login page as before

#### Scenario: Tool call with another kind of address
- **WHEN** Claude asks the browser to open `tauri://localhost/`
- **THEN** the tool call fails with a message that only web addresses open
