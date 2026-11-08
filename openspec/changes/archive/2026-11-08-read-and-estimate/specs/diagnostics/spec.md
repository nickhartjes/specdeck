## ADDED Requirements

### Requirement: Read requests and estimates in diagnostics
The host's log SHALL record each read request and vote opened and closed, with its id, the kind of thing to read or the card set, the time limit, and the counts of answers or votes, and each answer refused for the rate limit. No log SHALL record what was read by name or path, a task, a question's text or a vote's value, with Verbose logging on or off.

#### Scenario: Nothing of the content logged
- **WHEN** a read request names a file, a vote's task and a question's text contain a known marker, and the session runs with Verbose logging on
- **THEN** the host's and viewers' logs record the requests with their counts, and the marker does not appear in any log or exported report
