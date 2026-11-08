## ADDED Requirements

### Requirement: A click opens what the notification is about
Clicking a viewer's notification SHALL bring the session's window to the front and open, in a preview tab, the thing its title names: for a plan of an OpenSpec change, the first of the plan's documents that the turn wrote, in the plan's order; for documents, the document the title names; for Claude's plan-mode plan, that plan in a plan tab. A tab that is already open SHALL be selected instead.

#### Scenario: New plan
- **WHEN** a viewer clicks the notification that the plan `add-login` is ready for review
- **THEN** the session's window comes to the front with `proposal.md` of `add-login` open in the preview tab

#### Scenario: Revised plan
- **WHEN** a viewer clicks the notification that the plan `add-login` was updated, after Claude changed only its design
- **THEN** `design.md` of `add-login` opens

#### Scenario: Several documents
- **WHEN** a viewer clicks the notification that `README.md` and 2 other documents changed
- **THEN** `README.md` opens

#### Scenario: Plan-mode plan
- **WHEN** a viewer clicks the notification that Claude has a plan for them to review
- **THEN** a plan tab opens with Claude's plan, formatted, marked as waiting for the host

#### Scenario: Document already open
- **WHEN** a viewer kept `README.md` open in a tab, switched to the terminal tab, and clicks a notification that `README.md` changed
- **THEN** the `README.md` tab is selected, and no other tab opens or is replaced

### Requirement: The most important thing opens
When a notification covers several things, a click SHALL open only the one its title names: a plan before documents. A notification whose title is that the agent finished SHALL only bring the window to the front.

#### Scenario: Plan and a README
- **WHEN** a viewer clicks the notification that the plan `add-login` is ready for review, whose body says `README.md` changed too
- **THEN** `proposal.md` of `add-login` opens, and `README.md` does not

#### Scenario: Two plans
- **WHEN** a viewer clicks a notification that the plan `add-login` is ready for review, whose body says the plan `fix-search` changed too
- **THEN** a document of `add-login` opens

#### Scenario: Agent finished
- **WHEN** a viewer clicks the notification that Claude finished
- **THEN** the session's window comes to the front, and its tabs stay as they were

### Requirement: A click on an older notification
A click on a notification SHALL open what it was about also after later turns and after the session ended, at the latest version the viewer received. A document deleted since SHALL open at its last version. When the viewer left the session, or joined again in the same window, a click SHALL open nothing.

#### Scenario: Document changed again
- **WHEN** Claude changes `README.md` in two turns, and the viewer clicks the first turn's notification
- **THEN** `README.md` opens at its latest version, with what changed since the viewer last read it marked

#### Scenario: Document deleted since
- **WHEN** Claude changes `notes.md`, deletes it in the next turn, and the viewer then clicks the notification about `notes.md`
- **THEN** `notes.md` opens with the last text the viewer received

#### Scenario: Session ended
- **WHEN** the host ends the session, and the viewer then clicks an earlier notification that `README.md` changed
- **THEN** `README.md` opens in the ended session's window

#### Scenario: Viewer left
- **WHEN** the viewer left the session and closed its window, and then clicks an earlier notification about it
- **THEN** no document opens, and the app comes to the front as it does today
