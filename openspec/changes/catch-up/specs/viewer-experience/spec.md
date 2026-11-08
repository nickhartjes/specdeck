## ADDED Requirements

### Requirement: Earlier prompts folded for late joiners
A viewer SHALL see each prompt that finished before they joined folded to the prompt and its outcome, with a control naming how many steps it hides that unfolds it. A prompt finished less than a minute before joining SHALL count as seen. The latest prompt at the time of joining SHALL NOT fold, even once newer prompts start, nor SHALL prompts finishing after the viewer joined. The host SHALL never see prompts folded. Reconnecting SHALL NOT count as joining.

#### Scenario: Viewer joins after an hour
- **WHEN** a viewer joins a session in which six prompts finished and a seventh is running
- **THEN** the first six prompts each show the prompt, its outcome and *Show the N steps*, and the seventh shows its steps

#### Scenario: Unfold one prompt
- **WHEN** the viewer selects *Show the 23 steps* under the third prompt
- **THEN** the third prompt shows its steps in the viewer's chosen detail, and the other folded prompts stay folded

#### Scenario: New prompt after joining
- **WHEN** a viewer joins while the session is idle after six prompts, and the host then sends a seventh
- **THEN** the sixth prompt stays unfolded, and the first five stay folded

#### Scenario: Viewer joins at the start
- **WHEN** a viewer joins before the first prompt, and the session then has five prompts
- **THEN** no prompt is folded

#### Scenario: Prompt finished just before joining
- **WHEN** the latest finished prompt ended 30 seconds before the viewer joined, and a new prompt has started since
- **THEN** that prompt is shown with its steps, not folded

#### Scenario: Host
- **WHEN** the host's window opens again on a session with forty prompts
- **THEN** no prompt is folded

#### Scenario: Reconnect
- **WHEN** a viewer who joined at the start loses the connection for two minutes and comes back
- **THEN** no prompt is folded

#### Scenario: Search lands on a folded prompt
- **WHEN** a late joiner's search steps to a match inside a folded prompt
- **THEN** that prompt unfolds and the match is shown

### Requirement: Outcome of a folded prompt
A folded prompt's outcome SHALL be one line of facts from the timeline: the plan it wrote or updated by name, how many files it changed, the tasks done where it changed a plan's count, that the host denied a step, and that it ended on a failed step. Without any of these, the outcome SHALL be the first sentence of the agent's reply. The outcome SHALL NOT claim anything the timeline does not show, such as that tests pass.

#### Scenario: Plan written
- **WHEN** a folded prompt wrote the proposal, design, specs and tasks of `viewer-heartbeat`
- **THEN** its outcome reads like "Wrote the plan: viewer-heartbeat"

#### Scenario: Implementation step
- **WHEN** a folded prompt changed four source files and ticked three tasks of a plan with six
- **THEN** its outcome reads like "4 files changed · 3 of 6 tasks done"

#### Scenario: Question answered without changes
- **WHEN** a folded prompt only read files, and the agent replied "The heartbeat runs every 5 seconds. It is set in `config.ts`."
- **THEN** its outcome is "The heartbeat runs every 5 seconds."

#### Scenario: Host denied a step
- **WHEN** the host denied a command in a folded prompt that changed one file
- **THEN** its outcome says that one file changed and that the host denied a step

### Requirement: So far overview
A late joiner SHALL see, above the timeline, an overview of what happened before they joined: how long the session had run and how many prompts it had, the plans with their tasks done, the documents written, the files and links still shared, and how many code files changed. Each list SHALL show at most five entries and how many more there are. The overview SHALL NOT change after the viewer joined, and SHALL stay closed for that session once the viewer closes it.

#### Scenario: Overview content
- **WHEN** a viewer joins 40 minutes into a session with six prompts, one plan with 3 of 6 tasks done, two documents, a shared screenshot, a shared link and twelve changed code files
- **THEN** the overview says the session ran 40 minutes with six prompts, lists the plan with "3 of 6 tasks" and *Read the plan*, both documents, the screenshot and the link, and "12 code files changed"

#### Scenario: Opening from the overview
- **WHEN** the viewer selects a document, a shared file or a link in the overview
- **THEN** it opens as it would from the files panel or the timeline

#### Scenario: Long lists
- **WHEN** nine documents were written before the viewer joined
- **THEN** the overview lists five of them and "and 4 more"

#### Scenario: Removed before joining
- **WHEN** the host removed a shared file and a link before the viewer joined
- **THEN** the overview mentions neither

#### Scenario: Frozen at join
- **WHEN** the agent ticks another task after the viewer joined
- **THEN** the overview still shows the tasks done when the viewer joined, and the timeline shows the new tick

#### Scenario: Closed
- **WHEN** the viewer closes the overview
- **THEN** it does not come back in that session, and the folded prompts stay folded

#### Scenario: No overview
- **WHEN** a viewer joins and no prompt is folded
- **THEN** no overview is shown

### Requirement: Jump from the overview
The overview SHALL end with the prompt list, *Jump to a prompt*, and *Go to the latest prompt*. Jumping to a folded prompt SHALL scroll to it and leave it folded. The prompt list above the timeline SHALL stay for everyone.

#### Scenario: Jump to a folded prompt
- **WHEN** a late joiner picks the second prompt in the overview's *Jump to a prompt*
- **THEN** the timeline and terminal scroll to the second prompt, it stays folded, and the viewer is in free-scroll

#### Scenario: Go to the latest prompt
- **WHEN** the late joiner selects *Go to the latest prompt*
- **THEN** the timeline scrolls to the latest prompt and follows new entries again

### Requirement: Catching up in long sessions
Folded prompts SHALL cost no more to show than their prompt and outcome, and working out the overview and outcomes SHALL NOT be repeated for each new event, so that joining a long session keeps the window responsive.

#### Scenario: Joining a session with 400 prompts
- **WHEN** a viewer joins a session with 400 finished prompts of six file changes each
- **THEN** the timeline shows 399 folded prompts and the latest with its steps, with fewer page elements than the same timeline unfolded, and a new event re-renders only the prompt it belongs to
