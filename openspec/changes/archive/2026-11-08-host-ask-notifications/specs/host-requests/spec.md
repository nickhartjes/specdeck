## ADDED Requirements

### Requirement: The host hears about a question
When a viewer answers an open read request with a question, the host SHALL be notified, naming the viewer, with the question's text, or what was asked to read when there is no text. A viewer SHALL give at most one such notification per read request. Questions SHALL NOT be grouped.

#### Scenario: Question with a text
- **WHEN** the host's window is in the background and Anna answers the request to read `design.md` with the question "Why not reuse the bucket?"
- **THEN** the host is notified that Anna has a question, with "Why not reuse the bucket?" as its text

#### Scenario: Question without a text
- **WHEN** Anna answers the request to read `design.md` with a question and no text
- **THEN** the host is notified that Anna has a question about `design.md`

#### Scenario: Two viewers
- **WHEN** Anna and then Mo answer with a question within a second
- **THEN** the host gets two notifications, one for each

#### Scenario: Question edited or asked again
- **WHEN** Anna, who has a question, changes its text, or takes it back and asks again in the same request
- **THEN** the host gets no further notification about Anna's question

### Requirement: The host hears when everyone has read
The host SHALL be notified when, in an open read request, no viewer who can answer is still reading, saying how many are ready and how many have a question. Only viewers present who can answer SHALL count. It SHALL notify again each time this becomes true after it was not.

#### Scenario: Last viewer is ready
- **WHEN** four viewers are ready, one has a question, and the last one still reading marks themselves ready
- **THEN** the host is notified that everyone has read, with 5 ready and 1 with a question

#### Scenario: The last reader leaves
- **WHEN** everyone else has answered and the only viewer still reading leaves the session
- **THEN** the host is notified that everyone has read

#### Scenario: A viewer joins afterwards
- **WHEN** everyone has read, then a viewer joins and later marks themselves ready
- **THEN** the host is notified a second time

#### Scenario: Older app and nobody present
- **WHEN** the only viewers present are on an older app, or no viewer is present
- **THEN** the host is not notified that everyone has read

### Requirement: The host hears when the time is up
When a read request closes because its time limit passed while a viewer who can answer was still reading, the host SHALL be notified of the outcome: how many were ready, had a question and were still reading. When nobody was still reading, the time running out SHALL NOT notify.

#### Scenario: Someone still reading
- **WHEN** the 5 minutes of a request to read `design.md` pass while 3 viewers are ready, 1 has a question and 1 is reading
- **THEN** the host is notified that the time is up, with 3 ready, 1 with a question and 1 still reading

#### Scenario: Everyone had already read
- **WHEN** the host was notified that everyone has read and the time limit then passes with nobody reading
- **THEN** the host gets no notification that the time is up

#### Scenario: Host goes on
- **WHEN** the host goes on before the time is up
- **THEN** the host gets no notification about the request closing

### Requirement: The host hears when everyone has voted
The host SHALL be notified when every viewer present who can vote has voted in the current round, before the reveal, with the number of votes and whether the host's own vote is missing. The host's own vote SHALL NOT be needed. It SHALL notify again each time this becomes true after it was not, including in a new round. No vote's value SHALL be in it.

#### Scenario: Last viewer votes
- **WHEN** three viewers watch, the host has not voted, and the third viewer votes
- **THEN** the host is notified that everyone has voted, with 3 votes, and that their own vote is missing

#### Scenario: Vote again
- **WHEN** the host starts a new round and every viewer votes again
- **THEN** the host is notified again that everyone has voted

#### Scenario: Votes revealed
- **WHEN** the host revealed the votes and a viewer then joins
- **THEN** the host gets no notification about votes

#### Scenario: Only the host voted
- **WHEN** no viewer who can vote is present and the host votes
- **THEN** the host is not notified that everyone has voted

### Requirement: Switches for the host's ask notifications
The host's notification settings SHALL have a switch for each of: a viewer has a question; everyone has read or the time is up; everyone has voted. All three SHALL be on by default, also for settings from a release without them. They SHALL apply only while the main notification switch is on, and SHALL be shown disabled while it is off.

#### Scenario: Defaults after an upgrade
- **WHEN** a host whose settings come from a release without these switches updates
- **THEN** all three are on and their other settings are kept

#### Scenario: Questions turned off
- **WHEN** the host turned off *A viewer has a question* and a viewer asks a question
- **THEN** the host gets no notification about it, and is still notified when everyone has read

#### Scenario: Main switch off
- **WHEN** the host turned off *Show notifications*
- **THEN** the three switches are shown disabled with their values, and none of these events notifies

### Requirement: Opening the host's ask notifications
A click on one of these notifications SHALL bring the session's window to the front. A click on a question's notification SHALL also show the question texts in the read request, while that request is open.

#### Scenario: Click on a question
- **WHEN** the host clicks the notification that Anna has a question while the request is still open
- **THEN** the session's window comes to the front with the request's question texts shown

#### Scenario: Request closed since
- **WHEN** the host clicks a question's notification after the request closed
- **THEN** the session's window comes to the front and nothing else opens
