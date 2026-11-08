## ADDED Requirements

### Requirement: Links in the browser
The web viewer SHALL show link cards and let a viewer send a link to the host as the app does. Opening a link SHALL open it in a new browser tab that has no access back to the viewer's page and is not told which page it came from.

#### Scenario: Open a link from the browser
- **WHEN** a browser viewer opens a shared link
- **THEN** it opens in a new tab, and the page there cannot reach or navigate the SpecDeck tab, and gets no referrer

#### Scenario: Send a link from the browser
- **WHEN** viewers' links are allowed and a browser viewer sends a link
- **THEN** the host sees it waiting as from any other viewer, and the browser viewer sees what became of it
