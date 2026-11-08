# Proposal

## Why

People following a session often have something to look at together, such as a wireframe, acceptance criteria or a CI log, and today they can only pass it around outside SpecDeck. This is issue #55: the *Shared with everyone* group in the Files panel's *Shared* tab (`docs/design/frontend-design-concept.html`). #56 gave the Files panel its *Documents* and *Code* tabs. This change adds *Shared*, and with it a way for files to travel over the session's own connections in both directions. #61 (braindump), #62 (files for the project) and #64 (the whole project) need that later.

## What Changes

- **Share a file with everyone.** The host and the viewers can share files from the *Shared* tab, by choosing them or dropping them. Each file shows who shared it, when, and its size. Everyone can open it in a tab, or save a copy.
- **Viewers share only when the host allows it.** A new switch in the sharing settings, *Viewers can share files*, is off at the start of each session. The host can remove any shared file, and a viewer can remove their own.
- **Viewable types only, checked by content.** Images (PNG, JPEG, GIF, WebP; no SVG), Markdown and plain text (logs, JSON, code), up to 10 MB a file. The session holds at most 50 MB of shared files, and one viewer at most 20 MB of them. Markdown opens *Formatted* with *Source*, as documents do. Text and code open in the code view. Images open as images.
- **For the whole session.** Shared files stay available until they're removed or the session ends, also for people who join late or reconnect. The session process keeps them in memory and writes none to disk. When the session ends they are gone, except for copies people saved and files the host put in the session folder.
- **Put in the session folder.** The host can put any shared file into `shared/` in the session folder, so the agent can read it. That is the only way a shared file reaches the agent, and only the host can do it. An existing file is never overwritten: the copy gets a free name such as `wireframe (2).png`.
- **In the timeline.** Each share is a step: "Sanne shared heartbeat-wireframe.png", which opens the file. A removed file's step stays, marked removed.
- **Over the session's own connections.** Files travel on a new `files` data channel, in chunks, and only when a participant opens or saves one. They never pass through the sharing service. A file's name, size, type and hash come in the signed timeline, so a viewer can check every file it receives.
- **Nothing leaves the host unasked.** Only a file the host chose or dropped in their own window is ever sent. The session process does not read any other path to share it.
- **Protocol.** A new timeline entry, new host messages, a new request kind for viewers, and the `files` channel. They join protocol 4 while 4 is unreleased (#56 introduced it after 1.6.0). If a release ships 4 first, the protocol goes to 5 and admits 4. Viewers on an older supported protocol never get any of it, and watch as today.
- **The web viewer** gets the same *Shared* tab, sharing included, under the same rules.
- **Docs** say what sharing a file sends, who can see it, and that files a viewer shares reach the host's app but never the agent unless the host puts them in the session folder.

Out of scope: the braindump (#61), viewers offering files for the project (#62), the whole project (#64), links (#66), notifications for new files (#59), and keeping files after the session (#57's recap). The *Add files, a note or a link* dialog from the concept starts as *Share files* here, and those changes add their own choices to it.

## Capabilities

### New Capabilities

- `shared-files`: sharing files with everyone in a session. Covers who may share, the host's switch, types and limits, how long files stay, removing them, putting one in the session folder, transfer and verification, and older viewers.

### Modified Capabilities

- `viewer-requests`: *Host allows each kind of request* changes. Sharing files is a new kind, and unlike suggestions it is off when a session starts.
- `session-window`: a new requirement, *Shared tab*, covers the tab in the Files panel, the *Shared with everyone* group, opening a shared file in a tab, and room for the groups #61, #62 and #66 add.
- `session-timeline`: a new requirement, *Shared files in the timeline*, covers the step for each share.
- `web-viewer`: a new requirement, *Shared files in the browser*, covers sharing, opening and saving in the browser.
- `diagnostics`: a new requirement, *Shared files in diagnostics*, covers recording shares, transfers and refusals without file names or content.

## Impact

- **Protocol (`crates/protocol`):** a new `files` module with the `files` channel's messages (requests, chunks, uploads), strictly decoded with caps. `Entry::FileShared`, `HostMessage::Files` (the list), `HostMessage::FilesAllowed`, `ViewerRequest::ShareFile` and `ViewerRequest::RemoveFile`, and new `Outcome`s for files. A `SHARED_FILES_PROTOCOL_VERSION` constant. All additions are appended, as in earlier versions.
- **Host (`crates/core`):** a `shared_files` module (the store, type checks, limits, writing into `shared/`). `sharing.rs` accepts the `files` channel, serves and receives chunks with backpressure, and holds the new messages and entry back from older viewers. `peer.rs` gets a third channel.
- **Viewer (`crates/core/src/viewer.rs`, `crates/protocol/src/viewer_state.rs`, `crates/web`):** fetch, verify and upload files.
- **Session process and app (`crates/session`, `app/src-tauri`):** new commands to share, remove, read and put a file, `allow-files` for the switch, and `Event::Answer` for the commands that need one. `adapter.rs` adds the new commands to `NEW_COMMANDS`. The snapshot carries the list. Saving a copy uses the existing dialog plugin.
- **UI (`app/ui`):** the *Shared* tab in `FilesPanel`, a shared-file tab, a *Share files* dialog with dropping, and the switch in `SharingSettings`. The web viewer's connection opens the `files` channel.
- **Docs:** `docs/site` guide pages for hosts and viewers and the security page. `docs/design/README.md` records #55's decisions.
- **Not touched:** the sharing service (`crates/server`), including the web viewer's CSP. Images are shown as `data:` URLs, which it already allows.
- **Parallel work:** #66 (links) also adds to the *Shared* tab, the protocol and the timeline. #59 (notifications) may notify about shares later. This change keeps each group of the *Shared* tab in its own component, and appends its protocol variants at the end, so merging is a rebase. Whichever lands second re-runs `openspec validate` and the protocol tests.
