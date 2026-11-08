# Spec Delta

## Purpose

Lets the host, and viewers through the host, share a link with everyone in a session, such as a preview deploy, a ticket or a live whiteboard. Each participant opens it in their own browser, and SpecDeck only carries the address.

## ADDED Requirements

### Requirement: Host shares a link
The host SHALL be able to share a link with everyone in the session at any time while it runs, with an optional title of at most 120 characters. A link SHALL be at most 2048 bytes long. Every participant SHALL see the link as soon as it is shared.

#### Scenario: Host shares a staging link
- **WHEN** the host shares `https://staging.example.com/s/demo` with the title "Staging: the new pill"
- **THEN** every participant sees a link card with that title and address, shared by the host, just now

#### Scenario: No title
- **WHEN** the host shares a link without a title
- **THEN** the card is titled by the link's site, such as `staging.example.com`

#### Scenario: Too long
- **WHEN** the host pastes an address longer than the limit
- **THEN** they are told it is too long, and nothing is shared

### Requirement: Only web addresses others can reach
A link SHALL be accepted only when it starts with `https://`, or with `http://` for an address that is not on the sharer's own machine. A link to the sharer's own machine (`localhost`, a name ending in `.localhost`, `127.0.0.0/8`, `::1`, `0.0.0.0` or `::`) SHALL be refused for both schemes, with a hint that viewers can't reach it. A link with a user name or password in it SHALL be refused. The host SHALL check every link again before it is shared, whoever sent it.

#### Scenario: Plain http on the network
- **WHEN** the host shares `http://192.168.1.20:8080/`
- **THEN** it is shared

#### Scenario: localhost
- **WHEN** the host tries to share `http://localhost:5173/`
- **THEN** it is refused with a note that viewers can't reach the host's own machine, and that opening the host's app to viewers is not available yet

#### Scenario: Loopback over https
- **WHEN** someone tries to share `https://127.0.0.1:8443/`
- **THEN** it is refused as a link to their own machine

#### Scenario: Another scheme
- **WHEN** someone tries to share `javascript:alert(1)`, `file:///etc/hosts` or `ftp://example.com/`
- **THEN** it is refused with a note that only web addresses starting with `https://` can be shared

#### Scenario: Credentials in the address
- **WHEN** someone tries to share `https://user:secret@example.com/`
- **THEN** it is refused with a note that addresses with a password can't be shared

#### Scenario: Modified viewer app
- **WHEN** a viewer's app sends a link to `http://localhost:3000/` past its own check
- **THEN** the host does not accept it and disconnects that viewer, as for any malformed message, and nobody else sees the link

### Requirement: Link card
Each shared link SHALL be shown as a card with its title, its site, its address, who shared it, and when. The site SHALL be shown in its ASCII form, so a look-alike name can be told apart. Title and address SHALL be shown as text, never as markup.

#### Scenario: Shared by a viewer
- **WHEN** the host shares a link that Mo sent
- **THEN** the card says Mo shared it, at the time the host shared it

#### Scenario: Look-alike site
- **WHEN** a link to a site whose name uses non-Latin letters that look like Latin ones is shared
- **THEN** the card shows the site's name in its `xn--` form

#### Scenario: Markup in the title
- **WHEN** a link's title is `<img src=x onerror=alert(1)>`
- **THEN** the card shows that text as written

### Requirement: Opens in the participant's own browser
Opening a link SHALL open it in the participant's default browser. A link SHALL open only when the participant asks for it. The app and the web viewer SHALL NOT load the page, fetch its title or icon, or show a preview of it. The app SHALL open only links that were shared in the session. Every participant's app SHALL check a link against the same rules before showing it as openable.

#### Scenario: Link from a modified host
- **WHEN** a viewer receives a link entry for `file:///etc/passwd` from a host whose app was modified
- **THEN** the card says the link can't be opened, and there is no way to open it

#### Scenario: Open a link
- **WHEN** a participant chooses *Open in your browser* on a link card
- **THEN** the link opens in their default browser, with their own logins

#### Scenario: New link arrives
- **WHEN** a link is shared
- **THEN** nothing opens by itself, and no request is made to the link's site by SpecDeck

#### Scenario: Copy the link
- **WHEN** a participant chooses *Copy link*
- **THEN** the whole address is copied

### Requirement: For this session only
Whoever shares a link SHALL be able to mark it *for this session only*. The mark SHALL start on for a link with a non-empty `#` part, and off otherwise, and SHALL be fixed once the link is shared. The card of such a link SHALL show its address up to the `#` with the rest hidden, and say that the link holds a key. Copying it SHALL warn that it should be shared only with people in the session. Saved recaps and exports, when they come, SHALL leave it out.

#### Scenario: Excalidraw room
- **WHEN** the host pastes `https://excalidraw.com/#room=5c1e2a9b,Q3h9vX2m`
- **THEN** *For this session only* is on, and once shared the card shows `https://excalidraw.com/#…` and says the part after `#` is the room's key

#### Scenario: Mark turned off
- **WHEN** the host pastes `https://docs.example.com/guide#install` and turns the mark off before sharing
- **THEN** the card shows the whole address

#### Scenario: No # part
- **WHEN** the host pastes `https://example.com/ticket/42`
- **THEN** *For this session only* starts off, and the host can turn it on

#### Scenario: Open a link for this session only
- **WHEN** a participant opens a link marked for this session only
- **THEN** the whole address, with its `#` part, opens in their browser

### Requirement: Host removes a link
The host SHALL be able to remove any shared link. Before removing, the host SHALL be told that people who already saw it still have the address. After removal, every participant's card SHALL say that the host removed it, without the address, and participants who join later SHALL NOT receive the address.

#### Scenario: Wrong link pasted
- **WHEN** the host removes a link they shared by mistake
- **THEN** every participant's card says the host removed the link, and it can no longer be opened or copied

#### Scenario: Late joiner after removal
- **WHEN** a viewer joins after the host removed a link
- **THEN** the viewer sees that a link was removed, and does not receive its address

### Requirement: Links in sessions without a timeline
In a session without a timeline (a terminal, OpenCode or omp session), links SHALL be shared, shown, opened and removed as in a session with one. The timeline panel SHALL keep its note that the session has no timeline and list the session's links below it.

#### Scenario: Terminal session
- **WHEN** the host shares a link in a terminal session
- **THEN** every participant sees the link card in the timeline panel, below the note that a terminal session has no timeline

### Requirement: Links and viewers on the previous protocol
A viewer on the previous supported protocol SHALL NOT be sent links, removals or the links switch, and SHALL NOT be offered a way to share a link. The rest of the session SHALL work for them as before.

#### Scenario: Older viewer in a session with links
- **WHEN** the host shares a link while a viewer on the previous release watches
- **THEN** that viewer stays connected and sees no link card, and other viewers see the card
