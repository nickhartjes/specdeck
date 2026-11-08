## ADDED Requirements

### Requirement: Shared files in diagnostics
The host's log SHALL record each shared file with who shared it (the host, or a viewer's number), its type, its size, and whether it was shared or refused and why. It SHALL also record each file removed, each file put in the session folder, and each transfer to or from a viewer with its size and outcome. Viewers' logs SHALL record their own transfers in the same way. No log SHALL record a file's name, path or content, with Verbose logging on or off.

#### Scenario: Shared file recorded without its name or content
- **WHEN** a viewer shares a file whose name and content contain a known marker, with Verbose logging on, and another viewer opens it
- **THEN** the host's log records the share and the transfer with their sizes, and the marker does not appear in any log or exported report

#### Scenario: Refused file
- **WHEN** a participant shares a file that is too large
- **THEN** the log records the refusal, its size and that it was too large
