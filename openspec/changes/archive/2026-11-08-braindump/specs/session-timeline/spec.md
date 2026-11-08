# Spec Delta

## ADDED Requirements

### Requirement: The braindump in the timeline
Each time the host gives the braindump, the timeline SHALL show one step at that moment, saying how many items were given and from whom, such as "Gave the braindump to Claude: 3 items from Sanne and Mo". Selecting the step SHALL list the items it gave, each opening the item while it exists. Adding, editing or removing an item SHALL NOT be shown as a step. The step SHALL be part of the session's history.

#### Scenario: Step for a giving
- **WHEN** the host gives a note from Sanne and two files from Mo
- **THEN** everyone's timeline shows "Gave the braindump to Claude: 3 items from Sanne and Mo", and selecting it lists the three items

#### Scenario: Additions are not steps
- **WHEN** viewers add five notes to the braindump
- **THEN** the timeline shows no step for them, and the *Shared* tab lists them

#### Scenario: Item removed after giving
- **WHEN** the host removes an item that a giving step lists
- **THEN** the step stays, and lists that item as removed

#### Scenario: Session without a timeline
- **WHEN** the host gives the braindump in a terminal session
- **THEN** no step is shown, because there is no timeline to show it in, and the items show as given in the *Shared* tab
