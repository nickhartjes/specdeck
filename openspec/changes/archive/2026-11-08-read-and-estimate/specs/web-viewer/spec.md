## ADDED Requirements

### Requirement: Read requests and estimates in the browser
The web viewer SHALL show read requests and votes, let a viewer answer and vote, open what the host asked to read, including the changes in the last prompt, and show their timeline entries, as the app does.

#### Scenario: Vote from the browser
- **WHEN** the host starts a vote and a browser viewer votes M
- **THEN** the host sees the browser viewer as voted, and after the reveal everyone sees their M

#### Scenario: Question from the browser
- **WHEN** a browser viewer answers a read request with a question
- **THEN** the host sees the question as from any other viewer
