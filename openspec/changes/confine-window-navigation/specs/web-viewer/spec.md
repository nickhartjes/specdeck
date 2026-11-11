# Spec Delta

## ADDED Requirements

### Requirement: Links in session content in the browser
In the web viewer, a link in a document, a plan, one of the agent's answers or a shared Markdown file SHALL open in a new browser tab that has no access back to the viewer's page and is not told which page it came from. Only web addresses SHALL open. The tab with the session SHALL stay on the session.

#### Scenario: Open a link in a document
- **WHEN** a browser viewer selects a link in a document
- **THEN** it opens in a new tab, the page there cannot reach or navigate the SpecDeck tab and gets no referrer, and the SpecDeck tab keeps showing the session

#### Scenario: Not a web address
- **WHEN** a browser viewer selects a link whose address is not `http` or `https`
- **THEN** nothing opens
