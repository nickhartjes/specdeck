# Proposal

## Why

Before or during a session, people collect input for the agent in different places. A product owner writes down what they need, a developer lists edge cases, someone has a screenshot. The host then retypes it all into one prompt. This is issue #61. The braindump gives that input one place in the session, visible to everyone. When it is complete, the host gives it all to Claude at once. It builds on shared files (#55), which already move files between the host and viewers, check them and keep them for the session, and on the viewer channel (#51).

## What Changes

- **A braindump in the *Shared* tab.** A group *Braindump for Claude*, above *Shared with everyone*. It lists each item with who added it, when, its size, and whether it was given to Claude. Everyone in the session sees it, late joiners included.
- **Add notes and files.** The *Share files* control becomes *Add files or a note*, with a choice of where it goes: *To the braindump for Claude* or *Share with everyone*. A file in the braindump follows the shared-files rules: images, Markdown and text, checked by content, at most 10 MB, and counted in the same 50 MB a session and 20 MB a viewer. A note is text of at most 32 KB. It is added as a Markdown file named after its author, such as `sanne-note-1.md`.
- **Who may add.** The host always can. Viewers can while the host's switch is on. The switch is the one that came with shared files, renamed from *Viewers can share files* to *Viewers can add files and notes*. It still starts off in every session, and now covers both.
- **Edit and remove.** The author of a note can edit it until it is given to Claude. The edited note replaces the old one. Files can't be edited: remove them and add them again. The author can remove their own item at any time, and the host can remove any item. Removing an item that was already given leaves its copy in the session folder.
- **Give to Claude.** In the host's window, *Give N to Claude* opens a dialog with a prompt the host can edit. Confirming copies the items not yet given into `shared/braindump/` in the session folder. It never replaces a file, as *Put in the session folder* doesn't. Then it types the prompt into the agent, as a sent suggestion is typed. The prompt ends with the exact paths written. Given items are marked *Given to Claude*. A second round only gives what was added since.
- **Terminal sessions.** People can add to the braindump as in an agent session. *Give* copies the files and types nothing, because every line typed into a shell runs. The host is told where the files are.
- **Timeline.** Each giving is a step, for example "Gave the braindump to Claude: 3 items from Sanne and Mo". The step opens the items. Additions are not drawn as steps, so ten notes don't become ten steps. They still exist as signed entries, so viewers can check each item's hash.
- **For the session only.** The braindump is held in the host's memory, like shared files, and is gone when the session ends. Only the copies given to Claude stay, in `shared/braindump/`.
- **Web viewer:** the same group, adding, editing, removing and opening.
- **Protocol.** New timeline entries for an item and for a giving, two new request kinds (add an item, replace a note) and a new host message with the list. Fetching, uploading and removing reuse the shared-files messages. Viewers on the previous supported protocol keep watching without the braindump.

Out of scope: a braindump that outlives the session; the host editing other people's notes; notes in *Shared with everyone*; *Files for the project* (#62) and its choice in the dialog; links in the dialog (#66 keeps its own control); and recaps (#57).

## Capabilities

### New Capabilities

- `braindump`: collecting notes and files for the agent from everyone in a session. It covers who may add, notes and their editing, limits, removing, giving to the agent with an editable prompt and only what is new, terminal sessions, the braindump's lifetime, and viewers on the previous protocol.

### Modified Capabilities

- `shared-files`: *Host decides whether viewers can share files* is modified. The switch is renamed *Viewers can add files and notes*, and it also decides whether viewers can add to the braindump.
- `session-timeline`: *The braindump in the timeline* is added. Giving the braindump is a step, and additions are not drawn as steps.
- `web-viewer`: *The braindump in the browser* is added.
- `security-documentation`: *The braindump described* is added. The docs say that braindump items reach the agent only when the host gives them, and where the copies are written.

## Impact

- **Protocol (`crates/protocol`):** a `BRAINDUMP_PROTOCOL_VERSION` constant. Entries `Entry::BraindumpItem` and `Entry::BraindumpGiven`. Requests `ViewerRequest::AddToBraindump` and `ViewerRequest::ReplaceNote`. Host message `HostMessage::Braindump`. Uploads, fetches and `RemoveFile` reuse the `files` channel and requests of #55.
- **Host (`crates/core`):** `shared_files.rs` holds the braindump items in the same store, with a group, a note mark and a given mark. `put_in_folder` takes a subfolder, so giving writes to `shared/braindump/` through the same code. `session.rs` gives the braindump: it copies the items, then types the prompt through the path that sends a suggestion. `sharing.rs` holds the new entries and messages back from older viewers.
- **Session process and app (`crates/session`, `app/src-tauri`):** commands `add-to-braindump`, `replace-note` and `give-braindump`. `adapter.rs` adds them to `NEW_COMMANDS`. The snapshot and the `shared-files` event carry the braindump.
- **UI (`app/ui`):** a braindump group in `SharedFiles`, the *Add files or a note* dialog, editing a note in its tab, the *Give to Claude* dialog, a timeline step in `TimelinePanel`, and the renamed switch in `SharingSettings`. The web viewer (`crates/web`) gets the same.
- **Docs:** the host and follow guides, and the security pages `data.md` and `overview.mdx`.
- **Parallel work:** `read-and-estimate` (#53, #54) also adds requests, host messages and possibly a protocol version. `catch-up` (#58) and `browser-steps` (#77) touch the timeline components. This change only appends variants, constants, commands and a switch label, so whichever merges second resolves those lines.
