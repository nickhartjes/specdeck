# Design: the session window

Two clickable designs, both with the same features. Open them in a browser.

- **`workspace-mockup.html`**: the target layout in SpecDeck's current look. It follows the layout choices below: panels, tabs and layouts for each kind of person.
- **`frontend-design-concept.html`**: the same window designed with Anthropic's `frontend-design` skill, as "a stage-manager's prompt book":
  - paper and ink around a dark stage, the terminal;
  - plum for everything the host asks;
  - one highly legible type family;
  - the timeline as a cue rail.

  It departs from the layout choices in places: the layout follows the person, and it has no layout presets and no resizable panels. It is ahead of the mockup with the braindump, files for the project and the Markdown source view.

Which look, and which of the concept's ideas, to take is still open.

The details below describe `workspace-mockup.html`, except the braindump, files for the project and the Markdown source view, which are so far only in the concept. Both are clickable sketches; the mockup is one with made-up data, and it is not the app. Nothing in it is decided until a change is proposed for it (`/opsx:propose`), and the OpenSpec specs then describe the behaviour. This file lists what the mockup shows, which issue each part belongs to, and what is still open.

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

- **An IDE layout.** Panels open and close from the toolbar, with ×, or by keyboard: Cmd/Ctrl+1 for files, Cmd/Ctrl+2 for the timeline, Cmd/Ctrl+0 for the terminal. The mockup uses Alt, because the browser takes Cmd+1. The dividers can be dragged, or moved with the arrow keys. On a narrow window the panels slide in over the tabs.
- **Tabs like an editor's.** One click opens a preview tab, in italics, which the next click replaces. A double click keeps the tab open. The terminal tab can't be closed.
- **The document reader becomes a tab** instead of the overlay it is today. *Read a document* (plan-documents) says reading must not interrupt the session, and a tab does that better.
- **People moves to the avatars** in the title bar, so Files and Timeline no longer share one strip of tabs.
- **A timeline step opens a tab** at the change, or switches to the terminal and highlights the line.
- **The host's terminal keeps its size** while another tab is shown. The agent's terminal must not resize because the host is reading a file.
- **Which files the tree shows:** option A first, the files the agent touched, with the full text of changed files sent the way documents already are. That's a small protocol change, and viewers see only what the session showed. Option B, the whole project, means the host exposes their working folder. It would be a change of its own: opt-in, respecting `.gitignore`, with a deny list for files like `.env`, and covered in the security docs. The mockup's *Touched files / Whole project* switch shows both.

## Parts and their issues

| Part of the mockup | Issue | Open questions |
|---|---|---|
| Files panel: *Documents*, *Code* and *Shared* tabs; a code file shows *Changes* or *Whole file, changes marked*; every Markdown file shows *Formatted* or *Source* | #56 | Whole-file view next to the diff or instead of it? Syntax highlighting, given the installer-size limit? |
| *Shared* tab, *Shared with everyone*: files to look at together, and the host's *Put in the session folder* | #55 | Size and type limits; do files stay for late joiners? |
| *Shared* tab, *Braindump for Claude*: everyone adds notes and files; the host gives them to Claude with an editable prompt | #61 (on #51) | Limits; does the braindump outlive the session? Can people remove their own items? |
| *Shared* tab, *Files for the project*: a developer offers a file at a path; the host adds or declines it | #62 (on #51) | Never outside the session folder, never overwriting without the host confirming. |
| *Add files or a note*: one dialog for all three, with a file picker, dropping files, or a note | #55, #61, #62 | |
| *Prompts* tab: viewers suggest; the host sends (or edits first), keeps for later, or declines | #52 (on #51) | Who sees pending suggestions? Do kept suggestions outlive the session? |
| *Ask to read*: a document and a time limit; a banner with who is reading or ready and a countdown | #53 (on #51) | "Not ready, I have a question"? Shown in the timeline afterwards? |
| *Estimate*: hidden votes on cards, reveal, spread, vote again, keep the result in the timeline | #54 (on #51) | Does the host vote? Which card sets? Starting the vote was reported broken once and not reproduced. |
| Sharing settings (the sliders button): which code viewers can open, suggestions, sharing files, saving a recap | #51, #56, #57 | The defaults. |
| *Joined late*: a "so far" card, and earlier prompts folded to their outcome | #58 | Who writes the summary: the app from the timeline, or the agent? |
| Notifications (shown inside the window in the mockup) | #59 | Which kinds are on by default? |
| *End the session*: the recap with prompts and outcomes, plans and documents, decisions and changed files, and *Save recap* | #57 | Who may keep what; where it is saved. |

#51 is the channel from viewers to the host that #52, #53 and #54 need, with the host in control.

## Order

1. **The window layout:** the three panels, tabs in the middle and layouts per kind of person. It needs no protocol change, and #55, #56 and the banner row depend on it.
2. **#51**, the viewer-to-host channel, then #52, #53, #54, #61 and #62 on it.
3. **#56, #55, #58, #59 and #57**, as each one is needed.
