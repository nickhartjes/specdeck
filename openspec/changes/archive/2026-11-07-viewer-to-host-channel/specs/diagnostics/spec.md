## ADDED Requirements

### Requirement: Viewer requests in diagnostics
The host's log SHALL record each request a viewer sends, with the viewer's number, the kind of request, its size and whether it was accepted, refused because the kind is off, or over the rate limit, and SHALL record what the host did with it. It SHALL NOT record the text of a request, with Verbose logging on or off.

#### Scenario: Suggestion recorded without its text
- **WHEN** a viewer suggests a prompt containing a known marker, with Verbose logging on, and the host sends it
- **THEN** the log records that viewer's suggestion, its size and that it was sent, and the marker does not appear in the log or the exported report

#### Scenario: Over the limit
- **WHEN** a viewer sends requests faster than the rate limit allows
- **THEN** the log records the requests over the limit with that viewer's number
