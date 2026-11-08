# Spec Delta

## ADDED Requirements

### Requirement: The braindump in the browser
A viewer in the web viewer SHALL see, open and save braindump items as a viewer in the app does. Under the same rules and limits, they SHALL be able to add files by choosing or dropping them, write notes, edit their own notes and remove their own items.

#### Scenario: Note from the browser
- **WHEN** the host allows adding files and notes and a viewer in the browser writes a note to the braindump
- **THEN** everyone sees the note in the braindump as added by that viewer

#### Scenario: Given items in the browser
- **WHEN** the host gives the braindump while a viewer watches in the browser
- **THEN** the viewer sees the items marked *Given to Claude*, as in the app
