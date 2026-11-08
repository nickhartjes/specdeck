# Spec Delta

## MODIFIED Requirements

### Requirement: Host decides whether viewers can share files
The host SHALL have a switch, *Viewers can add files and notes*, that is off when a session starts. It SHALL decide both whether viewers can share files and whether they can add to the braindump. While it is off, a viewer SHALL NOT be offered a way to share files or add to the braindump, and a file or note a viewer sends SHALL NOT be shared or added. Turning it off SHALL NOT remove files or items already there. Viewers SHALL see the change as soon as the host makes it.

#### Scenario: Off at the start
- **WHEN** the host starts a session and a viewer opens the *Shared* tab
- **THEN** the viewer can open shared files and braindump items, but is told that the host has not turned on adding files and notes

#### Scenario: Host turns it on
- **WHEN** the host turns on *Viewers can add files and notes*
- **THEN** viewers can share files and add to the braindump without rejoining

#### Scenario: Host turns it off during an upload
- **WHEN** the host turns the switch off while a viewer's file is still on its way
- **THEN** that file is not shared, and the viewer is told that the host has turned adding files and notes off
