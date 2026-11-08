# viewer-requests Specification

## Purpose

Lets viewers send things to the host, such as prompt suggestions, while the host decides what is allowed and what happens with it, and nothing a viewer sends reaches the agent without the host acting on it.

## Requirements

### Requirement: Host allows each kind of request
The host SHALL be able to turn each kind of viewer request on or off for the session at any time. Prompt suggestions SHALL be on when a session starts, except where they cannot be used, such as in a terminal session. Sharing files SHALL be off when a session starts, in every session. A request of a kind that is off SHALL NOT reach the host's window, and the viewer SHALL be told it is off.

#### Scenario: On when the session starts
- **WHEN** the host starts an agent session
- **THEN** prompt suggestions are on, and every viewer whose app can send requests can suggest a prompt

#### Scenario: Sharing files off when the session starts
- **WHEN** the host starts any session
- **THEN** viewers cannot share files until the host turns sharing files on

#### Scenario: Host turns suggestions off
- **WHEN** the host turns prompt suggestions off
- **THEN** viewers have no way to suggest a prompt until the host turns them on again

#### Scenario: Host turns suggestions off while a viewer writes one
- **WHEN** a viewer sends a suggestion just after the host turned suggestions off
- **THEN** the host does not see it, and the viewer is told that the host has turned suggestions off

#### Scenario: Terminal session
- **WHEN** the host shares a terminal session instead of an agent
- **THEN** the host is not offered prompt suggestions, because every line typed into a shell runs

#### Scenario: Suggestions already waiting
- **WHEN** the host turns suggestions off while some are waiting
- **THEN** the waiting suggestions stay, and the host can still send or decline them

### Requirement: Viewers see what is allowed
Viewers SHALL see which kinds of request the host allows, and SHALL see a change as soon as the host makes it. A viewer SHALL NOT be offered a control for a kind that is off.

#### Scenario: Suggestions turned on during the session
- **WHEN** the host turns prompt suggestions on while a viewer watches
- **THEN** the viewer can suggest a prompt without rejoining

#### Scenario: Suggestions off
- **WHEN** prompt suggestions are off and a viewer opens the place where they would suggest one
- **THEN** it says that the host has turned suggestions off

### Requirement: Rate limit per viewer
The host SHALL accept at most 3 requests from a viewer at once and then 1 every 10 seconds. A request over the limit SHALL NOT reach the host's window, the viewer SHALL be told to wait, and the viewer SHALL stay in the session. Reconnecting SHALL NOT reset the limit.

#### Scenario: Viewer sends too fast
- **WHEN** a viewer sends four suggestions within a few seconds
- **THEN** the host sees the first three, and the viewer is told to wait before sending the fourth again

#### Scenario: Reconnecting does not reset the limit
- **WHEN** a viewer who reached the limit reconnects and sends again at once
- **THEN** the request is still over the limit

### Requirement: Nothing reaches the agent without the host
Nothing a viewer sends SHALL be typed into the session, or reach the agent in any other way, unless the host acts on it in their own window.

#### Scenario: Suggestion waits for the host
- **WHEN** a viewer suggests a prompt and the host does nothing with it
- **THEN** the agent and the terminal receive nothing from it

#### Scenario: Host away
- **WHEN** viewers suggest prompts while the host's window is closed and the session keeps running
- **THEN** the suggestions wait for the host, and nothing is typed into the session

### Requirement: The viewer learns what became of a request
A viewer SHALL be told, for each request they sent, whether it reached the host, and later what the host did with it. Other viewers SHALL NOT be told. A viewer who reconnects SHALL see the current state of each of their requests in the session.

#### Scenario: Suggestion received
- **WHEN** a viewer's suggestion reaches the host
- **THEN** the viewer sees it as waiting for the host

#### Scenario: Reconnected viewer
- **WHEN** the host sends a viewer's suggestion while that viewer is reconnecting
- **THEN** after reconnecting the viewer sees that the suggestion was sent

### Requirement: Suggest a prompt
A viewer SHALL be able to suggest a prompt for the agent when the host allows suggestions. A suggestion SHALL be text of at most 4096 bytes in UTF-8 with no control characters other than line breaks and tabs, and SHALL NOT be empty. A viewer SHALL see the suggestions they made in the session and what became of each.

#### Scenario: Viewer suggests a prompt
- **WHEN** suggestions are on and a viewer writes a prompt and suggests it
- **THEN** the host sees it waiting, with the viewer's name, and the viewer sees it under their suggestions

#### Scenario: Too long
- **WHEN** a viewer writes a suggestion longer than the limit
- **THEN** the viewer is told it is too long before sending, and nothing is sent

#### Scenario: Control characters from a modified viewer
- **WHEN** a viewer's app sends a suggestion containing a terminal escape sequence or a carriage return
- **THEN** the host does not accept it and disconnects that viewer, as for any malformed message

### Requirement: Only the host and the suggester see a suggestion
A suggestion SHALL be shown only to the host and to the viewer who made it, while it waits and after the host acted on it.

#### Scenario: Another viewer
- **WHEN** one viewer suggests a prompt
- **THEN** the other viewers do not see it

### Requirement: Host sends, edits or declines a suggestion
The host SHALL see the suggestions waiting, oldest first, with who made each. The host SHALL be able to send a suggestion as written, edit it and then send it, or decline it. Sending SHALL type the text into the agent's prompt, keeping its line breaks, and submit it, as if the host typed it.

#### Scenario: Host sends a suggestion
- **WHEN** the host sends a waiting suggestion
- **THEN** the agent receives it as a prompt, the suggestion is no longer waiting, and the viewer who made it sees that it was sent

#### Scenario: Host edits first
- **WHEN** the host changes the text of a suggestion and then sends it
- **THEN** the agent receives the host's text, and the viewer sees that their suggestion was sent after the host edited it

#### Scenario: Host declines
- **WHEN** the host declines a suggestion
- **THEN** nothing is sent to the agent, and the viewer sees that it was declined

#### Scenario: Several lines
- **WHEN** the host sends a suggestion of three lines
- **THEN** the agent receives one prompt of three lines, not three prompts

#### Scenario: New suggestion while the host is in the session
- **WHEN** a viewer suggests a prompt while the host's window is open
- **THEN** the host is told a suggestion is waiting, without the terminal losing focus

### Requirement: Suggestions last for the session
Waiting suggestions and what became of each SHALL be kept while the session runs, also when the host's window closes and opens again, and SHALL be gone when the session ends.

#### Scenario: Host reopens the window
- **WHEN** the host closes the session's window while suggestions wait and opens it again
- **THEN** the same suggestions are waiting

#### Scenario: Removed viewer's suggestions
- **WHEN** the host removes a viewer who has suggestions waiting
- **THEN** those suggestions stay waiting for the host to send or decline

### Requirement: Share a link
A viewer SHALL be able to send the host a link, with an optional title and the *for this session only* mark, when the host allows viewers' links. The link SHALL follow the rules of `shared-links`, and the viewer SHALL be told before sending when it breaks them. Only the host and that viewer SHALL see the link while it waits. The host SHALL share it as sent or decline it, and the viewer SHALL see which.

#### Scenario: Viewer sends a link
- **WHEN** viewers' links are allowed and Mo sends `https://staging.example.com/s/demo`
- **THEN** the host sees it waiting, with Mo's name, its title and its whole address, and other viewers do not see it

#### Scenario: Host shares it
- **WHEN** the host shares Mo's waiting link
- **THEN** every participant sees its link card, shared by Mo, and Mo sees that the host shared it

#### Scenario: Host declines it
- **WHEN** the host declines Mo's waiting link
- **THEN** nobody else ever sees it, and Mo sees that it was declined

#### Scenario: localhost from a viewer
- **WHEN** a viewer types `http://localhost:3000/` to send
- **THEN** they are told before sending that the address is on their own machine, so nobody else can reach it, and nothing is sent

#### Scenario: Waiting links last for the session
- **WHEN** the host closes and reopens the session's window while a viewer's link waits
- **THEN** the same link is waiting

### Requirement: Host allows viewers' links
Viewers' links SHALL be a kind of request with its own switch, on when a session starts, and SHALL count towards the viewer's rate limit like any other request. Turning it off SHALL leave waiting links waiting for the host.

#### Scenario: Host turns viewers' links off
- **WHEN** the host turns viewers' links off
- **THEN** viewers are not offered a way to send a link, and the host can still share their own

#### Scenario: Terminal session
- **WHEN** the host shares a terminal session
- **THEN** viewers' links are on, since a link is never typed into the session
