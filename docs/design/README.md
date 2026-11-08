# Design: the session window

Two clickable designs, both with the same features. Open them in a browser.

- **`workspace-mockup.html`**: the target layout in SpecDeck's current look. It follows the layout choices below: panels, tabs and layouts for each kind of person.
- **`frontend-design-concept.html`**: the same window designed with Anthropic's `frontend-design` skill, as "a stage-manager's prompt book":
  - paper and ink around a dark stage, the terminal;
  - plum for everything the host asks;
  - one highly legible type family;
  - the timeline as a cue rail.

  It departs from the layout choices in places: the layout follows the person, and it has no layout presets. Its panels can be resized, per person. It is ahead of the mockup with the braindump, files for the project, the Markdown source view, shared links, Claude's browser and the host's app.

The window takes the concept's look and the mockup's layout presets: the OpenSpec change `session-window-layout` (#63) decided it, and its specs describe the behaviour. The terminal stays dark in both appearances and keeps the platform's own monospace font; the panels stay where they are for every person (the concept moves Sanne's timeline to the left).

The details below describe `workspace-mockup.html`, except the braindump, files for the project, the Markdown source view, shared links, Claude's browser and the host's app, which are so far only in the concept. Both are clickable sketches; the mockup is one with made-up data, and it is not the app. Nothing in it is decided until a change is proposed for it (`/opsx:propose`), and the OpenSpec specs then describe the behaviour. This file lists what the mockup shows, which issue each part belongs to, and what is still open.

## Who uses the window

Switch between them at the top of the mockup.

| Who | Layout | Mainly wants |
|---|---|---|
| Host (Nick) | Developer | the terminal, the timeline, and the controls for viewers |
| Product owner (Sanne) | Overview: documents and the timeline in plain words, no terminal | the timeline and documents |
| Developer (Mo) | Developer: code, the terminal and the timeline in detail | everything, including the terminal |

The layout differs from today's app. Today the terminal is on the left, a side panel with *Timeline*, *Files* and *People* is on the right, and documents open over it. The mockup has three panels:

- **Left:** files.
- **Middle:** tabs for the terminal and the files and documents you open.
- **Right:** the timeline and prompts.
- **Above them:** a row for requests from the host.

The layouts Overview, Developer and Present set these panels for each kind of person. That layout is the base the features below build on.

## Layout choices from exploring the panels

These came out of exploring the panels idea (the session that made the first version of the mockup):

- **An IDE layout.** Panels open and close from the toolbar, with ×, or by keyboard, numbered from left to right: Cmd/Ctrl+1 for files, Cmd/Ctrl+2 for the terminal, Cmd/Ctrl+3 for the timeline. Cmd/Ctrl+0 stays Actual Size. The mockup uses Alt, because the browser takes Cmd+1, and so does the web viewer. The dividers can be dragged, or moved with the arrow keys. On a narrow window the panels slide in over the tabs.
- **Tabs like an editor's.** One click opens a preview tab, in italics, which the next click replaces. A double click keeps the tab open, and so does scrolling or selecting in it. The terminal tab can't be closed. Preview tabs are for everyone, product owners too.
- **The document reader becomes a tab** instead of the overlay it is today, one tab per document, with the plan's other documents linked at the top. *Read a document* (plan-documents) says reading must not interrupt the session, and a tab does that better.
- **People moves to the avatars** in the title bar, so Files and Timeline no longer share one strip of tabs.
- **A timeline step opens a tab** at the change, or switches to the terminal and highlights the line.
- **The host's terminal keeps its size** while another tab is shown. The agent's terminal must not resize because the host is reading a file, and it resizes once when a divider drag or a panel toggle ends.
- **Layouts are presets** (Overview, Developer, Present) that replace the View menu; the first-join question picks one. The window is remembered as the person left it, and choosing the preset again restores it. In Overview a viewer has no terminal tab; the host always has one.
- **Which files the tree shows:** option A first, the files the agent touched. A changed code file opens its diffs, and a file the agent only read opens the terminal where it was read. Sending the full text of changed code files moves to #56, together with the host's setting for it, so #63 needs no protocol change. Option B, the whole project, means the host exposes their working folder. It would be a change of its own: opt-in, respecting `.gitignore`, with a deny list for files like `.env`, and covered in the security docs. The mockup's *Touched files / Whole project* switch shows both.

## Parts and their issues

| Part of the mockup | Issue | Open questions |
|---|---|---|
| The window: three panels, tabs in the middle, a row for requests, layouts for each kind of person, resizable panels, a Files / Session / Timeline switch on narrow windows, and the file tree of touched files (option A) | #63 | Decided in `session-window-layout`: the concept's look, layout presets, preview tabs for everyone. |
| *Whole project* in the files panel, with a deny list (`.env`), and the host's *Any file in the project* | #64 (on #51) | Off by default; respects `.gitignore`; a security review first. |
| Files panel: *Documents*, *Code* and *Shared* tabs; a code file shows *Changes* or *Whole file, changes marked*, with the full text of changed files sent the way documents already are (a protocol change), and the host's *Code viewers can open*; every Markdown file shows *Formatted* or *Source* | #56 | Decided in `files-documents-and-code`: the whole file instead of the diff, one switch away, with a code file always opening on *Changes* and a document on *Formatted*; highlighting with Prism for a fixed set of languages (about 45 KB); *Code viewers can open* starts each session on *Only the changes*. |
| *Shared* tab, *Shared with everyone*: files to look at together, and the host's *Put in the session folder* | #55 | Decided in `shared-files`: the host and viewers share, viewers behind *Viewers can share files* (off at the start of each session); images (PNG, JPEG, GIF, WebP, no SVG), Markdown and text, checked by content, up to 10 MB a file, 50 MB a session and 20 MB per viewer; kept in memory for the whole session, late joiners included, and gone after it; *Put in the session folder* writes to `shared/` and never replaces a file; each share is a timeline step. Files travel on a `files` data channel of their own, fetched when opened. |
| *Shared* tab, *Braindump for Claude*: everyone adds notes and files; the host gives them to Claude with an editable prompt | #61 (on #51) | Decided in `braindump`: one group in the shared-files store, behind the same switch, renamed *Viewers can add files and notes*; files follow the shared-files limits, notes are text up to 32 KB named after their author (`sanne-note-1.md`); the author edits their own note until it is given, and removes their own items, the host any; gone with the session; *Give N to Claude* writes the items not given yet to `shared/braindump/` (never replacing a file) and types the host's edited prompt followed by their paths; a second round gives only what is new; a terminal session only copies and types nothing; the timeline shows only the giving. Viewers on protocol 4 keep watching without it. |
| *Shared* tab, *Files for the project*: a developer offers a file at a path; the host adds or declines it | #62 (on #51) | Never outside the session folder, never overwriting without the host confirming. |
| *Add files or a note*: one dialog for all three, with a file picker, dropping files, or a note | #55, #61, #62 | #55 starts it as *Share files* (a file picker, and dropping on the *Shared* tab); #61 makes it *Add files or a note*, with *Where it goes* (the braindump or everyone) and a note field; #62 and #66 add their choices to it. |
| *Prompts* tab: viewers suggest; the host sends (or edits first), keeps for later, or declines | #52 (on #51) | Sending, editing and declining come with #51 (`viewer-to-host-channel`): only the host and the viewer who suggested it see a suggestion. Still open: keeping for later, whether kept suggestions outlive the session, and marking a sent prompt as suggested in the timeline. |
| *Ask to read*: a document and a time limit; a banner with who is reading or ready and a countdown | #53 (on #51) | Decided in `read-and-estimate`: a viewer answers *I'm ready* or *I have a question* with an optional text of up to 280 characters that only the host sees; the host can ask to read a document, a changed code file, a shared file or *the changes in the last prompt* (fixed at the moment of asking), with no limit or 2, 5 or 10 minutes; a terminal session offers shared files only; a card in the timeline counts who was ready, without names or questions. Protocol 6, with 5 still accepted. |
| *Estimate*: hidden votes on cards (story points or T-shirt sizes), reveal, spread, vote again, keep the result in the timeline | #54 (on #51) | Decided in `read-and-estimate`: the host votes too, and no window gets a value before the reveal, the host's included; the host picks the cards for each vote, starting on the set used last, both with ? and ☕, which don't count in the spread; the task is free text that can be filled from the plan's open tasks or the last prompt; *Keep* proposes the most common card, the larger on a tie. Answers have their own rate limit (5, then 1 a second). |
| Sharing settings (the sliders button): which code viewers can open, suggestions, adding files and notes, saving a recap | #51, #56, #57, #64 | The button, *Code viewers can open* and the suggestions switch came with #56. Still open: the defaults of the rest. |
| *Joined late*: a "so far" card, and earlier prompts folded to their outcome | #58 | Decided in `catch-up`: the app writes the card and the outcomes from the timeline, with no protocol change. Only viewers who join late see them, never the host; the viewer works out from event times which prompts it missed, counting a prompt that ended within a minute of joining as seen. An outcome is a line of facts (the plan written or updated, files changed, tasks done, a step the host denied, ending on a failed step), else the first sentence of the reply. The card covers what happened before the join, does not change after it, can be closed, and holds *Jump to a prompt* and *Go to the latest prompt*. |
| Notifications: pop-ups inside the window, and in the concept a bell with a list and switches for each kind | #59 | Settled with `viewer-notifications`: a viewer gets OS notifications (browser notifications in the web viewer, opt-in) for a plan ready for review and a changed document, on by default, and for the agent finishing, off by default; at most one per turn. Read and vote requests joined *While you watch a session* with `read-and-estimate`, both on by default and outside the one per turn. Clicking one opens what its title names in the preview tab (#79, `notification-open`): the plan's first changed document, the named document, or Claude's plan-mode plan in a plan tab of its own; *finished* only brings the window to the front. Still open: the bell and its list. |
| Light and dark: both designs have a light and a dark palette; the app gets a *Light / Dark / System* choice | #65 | Both palettes came with #63, following the OS. The terminal is dark in both, so the agent's colours look the same to every viewer. |
| *Shared* tab, *Links* (concept only): a link card that opens in your own browser; an Excalidraw room to draw together, *for this session only* because its link holds the key; `localhost` refused | #66 | Decided in `shared-links`: every link with a `#` part starts *for this session only*; viewers' links wait for the host to share or decline; the host can remove a link, which hides its address from then on; links work in sessions without a timeline. The card sits in the timeline, and the share control above it, until the *Shared* tab (#55) takes it over. |
| Timeline (concept only): Claude's browser steps, with a screenshot that opens as a tab; *Viewers see Claude's browser* in the sharing settings | #67 | Which browser tools first; is the image always in the hook payload? |
| Bar and sharing settings (concept only): *Let viewers open localhost:5173*, a marker with *Stop*, *Open Nick's app* for viewers, each with their own page | #68 | The web viewer can't open a local port; only after a security review. |
| *End the session*: the recap with prompts and outcomes, plans and documents, decisions and changed files, and *Save recap* | #57 | Who may keep what; where it is saved. |

#51 is the channel from viewers to the host that #52, #53, #54, #61, #62 and #64 need (and #66 when viewers share links), with the host in control. Its change, `viewer-to-host-channel`, also brings the core of #52 (suggest, then send, edit or decline) as the channel's first user, in the app and the web viewer. Hosts on it admit viewers one protocol version older, read-only, so each feature on the channel can add its own message without locking older apps out. #60 is a bug (a rare crash in the WebRTC library on macOS) that is best fixed before the larger work.

## Order

1. **#63, the window layout:** the three panels, tabs in the middle and layouts per kind of person. It needs no protocol change, and #55, #56 and the request row depend on it.
2. **#51**, the viewer-to-host channel, then #52, #53, #54, #61 and #62 on it.
3. **#56, #55, #66, #67, #58, #59 and #57**, as each one is needed. #66 (links) is small and can come early.
4. **#65**, the theme choice, after #63, which brings both palettes.
5. **#64** and **#68**, browsing the whole project and opening the host's dev server, only after a security review.
