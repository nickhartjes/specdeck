## ADDED Requirements

### Requirement: Shared files in the browser
A viewer in the web viewer SHALL see, open and save shared files as a viewer in the app does, and SHALL be able to share files under the same rules and limits, by choosing or dropping them. Saving a copy SHALL download the file under its shared name.

#### Scenario: Open a shared image in the browser
- **WHEN** the host shares `wireframe.png` and a viewer in the browser opens it
- **THEN** the image shows in a tab, as in the app

#### Scenario: Share from the browser
- **WHEN** the host allows viewers to share files and a viewer in the browser drops a log on the *Shared* tab
- **THEN** everyone sees it as shared by that viewer
