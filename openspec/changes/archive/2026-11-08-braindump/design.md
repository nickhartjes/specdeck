# Design

## Context

What there is today, after #55 and #66:

- **Shared files.** `crates/core/src/shared_files.rs` holds the store: `SharedFile { info: SharedFileInfo, sharer, bytes }`, with ids that count up and are never reused. It also holds the rules (types by content, name cleaning, 10/50/20 MB) and `put_in_folder`, which writes into `shared/` with `create_new`, numbered names and the `SessionFolder::contains` check. The store lives in `SessionView`.
- **Wire.** Bytes travel on the `files` data channel (`FilesRequest::Fetch`/`Upload`, `FilesReply::Chunk`/`Ack`/`Finished`). A viewer announces an upload with `ViewerRequest::ShareFile { name, size, sha256 }` through the request gate: switch, token bucket, size, room. It gets `Outcome::Ready { upload }`, and the host checks the hash and type when the last chunk arrives. `Entry::FileShared` carries the hash in the signed timeline. `HostMessage::Files` lists what exists. `RemoveFile` removes your own file.
- **Typing a prompt.** `Runtime::answer_suggestion` types `typed_prompt(text)` (control characters dropped except `\n` and `\t`), waits `SUBMIT_DELAY` and sends `\r`.
- **Versions.** `PROTOCOL_VERSION = 5`, `MIN_VIEWER_PROTOCOL_VERSION = 4`, with `SHARED_FILES_PROTOCOL_VERSION = 4` and `LINKS_PROTOCOL_VERSION = 5`. Release 1.6.0 shipped protocol 2, so 3 to 5 are all unreleased. `HostMessage`, `ViewerRequest` and `Outcome` are postcard: variants are only appended, and a struct inside them can't gain a field. Timeline entries are JSON, tagged by `kind`. An unknown kind is malformed for a viewer, so `pump` skips newer kinds for older viewers.
- **App and session process.** The window sends `Command`s. `adapter.rs` keeps commands new in this release from a session of the previous one (`NEW_COMMANDS`). Commands with an `id` are answered with `Event::Answer`.
- **UI.** `SharedFiles.tsx` has `SharedTab` (groups, with *Shared with everyone* today), `SharedFileTab`, and the share control in `FilesPanel`'s footer. `SharingSettings` has *Viewers can share files*. The web viewer uses the same components through `Bridge`.

## Goals / Non-Goals

**Goals:**

- One store, one transfer path, one set of checks for shared files and the braindump. A braindump item is a shared file with a mark.
- Giving writes through `put_in_folder` and types through the suggestion path, with no second copy of either.
- Small, append-only additions in the files shared with parallel changes (`control.rs`, `timeline.rs`, `adapter.rs`, `SharingSettings`, `TimelinePanel`).

**Non-Goals:**

- Keeping the braindump across sessions, or a host-side edit of other people's notes.
- Moving an item between *Shared with everyone* and the braindump.
- Notifications for new items (#59). The bell can pick up the list later.

## Decisions

### 1. A braindump item is a shared file in the same store, listed separately

`SharedFile` gains `group: Group { Everyone, Braindump }`, `note: bool` and `given: Option<u32>` (the number of the giving round). Ids, limits, room, removal, `Fetch` and `put_in_folder` all work as they do now. The 50/20 MB room is shared, which is what the spec asks for.

On the wire, `SharedFileInfo` can't gain a field (postcard), so:

- `HostMessage::Files` keeps listing only `Everyone` files, unchanged for protocol-4 viewers.
- A new `HostMessage::Braindump { items: Vec<BraindumpItemInfo> }`, appended, lists the braindump. `struct BraindumpItemInfo { info: SharedFileInfo, note: bool, given: Option<u32> }`. It goes to viewers on `BRAINDUMP_PROTOCOL_VERSION` or later through `send_newer`, on admission and on every change.
- The host answers a `Fetch` for a braindump id with `Gone` when it comes from an older viewer, so the files channel never gives an older viewer an item it can't list.

- *Alternative: a separate braindump store.* It would duplicate room accounting, removal and transfers, against "don't build a second path". Rejected.
- *Alternative: put braindump items in `Files` too.* Protocol-4 viewers would show them as shared with everyone. Rejected.

### 2. Requests: three appended variants on the existing gate

```rust
ViewerRequest::AddToBraindump { name: String, size: u64, sha256: [u8; 32] }  // a file
ViewerRequest::AddNote        { size: u64, sha256: [u8; 32] }                // a note
ViewerRequest::ReplaceNote    { file: u32, size: u64, sha256: [u8; 32] }     // edit your note
Outcome::AlreadyGiven                                                         // appended
```

Each goes through the gate as `ShareFile` does: the files switch (`Off`), the bucket (`RateLimited`), the size (`TooLarge`; 10 MB for a file, 32 KB for a note) and the room (`Full`). It then gets `Ready { upload }` and uploads on `files`. When the upload completes, the host checks the hash and the type, as for any shared file.

- **A note** must check as Markdown text: valid UTF-8, no NUL, not empty. The host names it, not the viewer: `<label>-note-<n>.md`. `<label>` is the author's label put through `clean_name`, lowercased, with spaces as `-`, and `note` alone when nothing is left. `<n>` counts that author's notes in the session. The window shows the same name after the host lists it.
- **`ReplaceNote`** is accepted only for the sender's own note that is not given. Another person's or a removed note is answered `Gone`, and a given note `AlreadyGiven`. When the upload completes, the new version gets a new id, keeps the old name and author, and the old one is removed in the same step. Viewers who had the old one open see the tab switch to the new version, because the list maps `replaces`. Room is checked for the new size minus the old.
- **Removal** reuses `RemoveFile { file }`. It already allows only your own file, and isn't gated by the switch.
- `decode_viewer` checks sizes strictly. `AddToBraindump`'s name stays under 255 bytes, so all three fit in `MAX_REQUEST_BYTES`.

- *Alternative: `AddToBraindump { note: bool, … }` with a name that the host ignores for notes.* Two meanings in one message. Rejected for a separate `AddNote`.

### 3. Timeline entries

```rust
Entry::BraindumpItem  { file, name, kind, media_type, size, sha256, by, note: bool, replaces: Option<u32> }  // kind "braindump-item"
Entry::BraindumpGiven { round: u32, files: Vec<u32>, paths: Vec<String>, from: Vec<String> }             // kind "braindump-given"
```

- `BraindumpItem` is added when an item is stored. It exists for the signed hash the viewer checks against, and for late joiners. `TimelinePanel` does not draw it.
- `BraindumpGiven` is added after a successful giving. `from` holds the labels of the authors at that moment, in order of first item, so the step reads "3 items from Sanne and Mo" after people leave. The step lists `files`, and an id no longer in the list shows as removed.
- `pump` skips both kinds for viewers below `BRAINDUMP_PROTOCOL_VERSION`, next to the `FileText`, `FileShared` and `Link` checks.
- The prompt is not in the entry: the agent's own timeline records it as a prompt, like any other.

### 4. Giving: copy, then type

`HostSession::give_braindump(prompt: Option<&str>) -> Result<Given, GiveError>` in `crates/core/src/session.rs`:

1. Takes the items with `group == Braindump && given.is_none()`, oldest first. If there are none, it returns `Nothing`.
2. Writes each with `put_in_folder(folder, root, "braindump", name, bytes)`. `put_in_folder` gains a subfolder argument (`""` for today's callers). It checks `shared` and then `shared/braindump` with `SessionFolder::contains` before and after creating each, and keeps `create_new` with numbered names.
3. If any write fails, it removes the files written in this call (each was created new by this call), marks nothing, and returns the error in plain words.
4. Otherwise it sets `given = Some(round)` on each item, adds `BraindumpGiven`, and broadcasts the list.

`Runtime::give_braindump(prompt)` in `crates/session` calls it. In an agent session it then types:

```
<prompt>

shared/braindump/what-sanne-needs.md
shared/braindump/edge-cases (2).txt
```

It goes through the same `typed_prompt`, `SUBMIT_DELAY`, `\r` sequence as `answer_suggestion`, which is factored into a private `submit_prompt(text)` that both use. In a terminal session (no prompt possible), `prompt` must be `None` and nothing is typed. The answer returns the paths either way, so the window can say where the files are.

The items are marked before typing. If typing fails because the agent's process is gone, the files are in the folder, and that is true. The host gets the error and can point the agent to the paths.

- *Alternative: type first, then write.* The agent would read a folder that isn't there yet. Rejected.
- *Alternative: let the host edit the list of paths.* A typo would point the agent to a missing file. The paths are fixed text under the editable prompt.

### 5. Commands and snapshot

New commands, JSON-kinded, in `NEW_COMMANDS`:

| Command | Effect |
|---|---|
| `add-to-braindump { id, name, data }` | The host adds a file (base64, as `share-file`). |
| `add-note { id, text }` | The host adds a note. |
| `replace-note { id, file, text }` | The host edits their own note. |
| `give-braindump { id, prompt? }` | Decision 4. Answered with the paths. |

`remove-shared-file` and `read-shared-file` already work on any id. The snapshot gains `braindump` (the items) and `braindumpPromptPossible`, which is false in terminal sessions. The `shared-files` host event carries the braindump list next to the files. `allow-files` stays the one switch, as the user decided. Only its label changes.

The host's own items take the same path as viewers' items: the same checks, the same entry and the same store. Only the transfer differs, as for `share-file`.

### 6. Protocol version

`BRAINDUMP_PROTOCOL_VERSION` gets its own constant. Protocol 5 is still unreleased, so following #55's decision 8, it is 5 and `PROTOCOL_VERSION` stays 5. Viewers on protocol 4 are the older viewers of the spec. A development build from `main` between #66 and this change would read `HostMessage::Braindump` as malformed. That is accepted, as it was for #55 and #66.

If a release ships protocol 5 before this merges, or `read-and-estimate` raises the version first, the constant becomes the current unreleased version, and `MIN_VIEWER_PROTOCOL_VERSION` follows the one-older rule. Task 1.1 checks this at the start of apply. Each change keeps its own constant, so merging is one line each.

`read-and-estimate` merged first and raised the version to 6, with the minimum at 5. On merging, `BRAINDUMP_PROTOCOL_VERSION` became 6, beside `ASKS_PROTOCOL_VERSION`; viewers on protocol 5 are the older viewers. The braindump's messages and requests come after the asks' on the wire.

### 7. UI

- **`SharedTab`:** a `Braindump` group above `SharedWithEveryone`, with the concept's help line ("Notes and files from everyone. <host> gives them to Claude together."). Each row shows the name, who and when, the size, and *Given to Claude* when given. The host gets a header button *Give N to Claude* when N > 0.
- **Adding:** the footer control becomes *Add files or a note*. It opens `AddDialog`, with *Where it goes* (*To the braindump for Claude* / *Share with everyone*), a file picker and drop area, and, for the braindump, a note field with a live 32 KB count. Dropping files on the *Shared* tab opens the dialog with them chosen. Without the switch, a viewer sees the "host has not turned on adding files and notes" line instead.
- **Note tab:** `SharedFileTab` shows *Edit* for the author's own note that isn't given. It swaps the formatted view for a textarea with *Save* and *Cancel*. *Remove* follows the existing rule. A given note says *Given to Claude* in the bar.
- **`GiveDialog`:** an editable prompt with a default text, which is not empty. Below it, fixed text: "These go into `shared/braindump/` and their paths are added to the end: …" with the item names, and "a taken name gets a number". In a terminal session there's no prompt field, and the button says *Copy to the session folder*. After giving, a toast names the folder.
- **`TimelinePanel`:** draws `braindump-given` as a step that expands to its items. It ignores `braindump-item`.
- **`SharingSettings`:** *Viewers can add files and notes*, with the hint "To the braindump or shared with everyone. Claude sees nothing until you give the braindump or put a file in the session folder."
- **Web viewer:** the same components. `Bridge` gains `addToBraindump`, `addNote` and `replaceNote`, on `FileUpload` in `crates/web`, as `shareFile` is.

Default prompt: "Read the files below: notes and files from the people in this session. Turn them into a short list of what they need, and ask me about anything that is unclear or contradicts the plan."

### 8. Viewer side

`crates/core/src/viewer.rs` and `app/src-tauri/src/viewer.rs` generalize `share_file` into one upload routine that takes the request to announce. That routine runs `ShareFile`, `AddToBraindump`, `AddNote` and `ReplaceNote` through the same `Uploading` state in `crates/protocol/src/files.rs`, so nothing about chunking or acks is duplicated. Notes are encoded as UTF-8 bytes in the viewer's app before the upload. `open_file` and `save_file` work on braindump ids unchanged, checking against the hash from `BraindumpItem`.

### 9. Diagnostics

`host_file_shared` gains `group`: `everyone`, `braindump` or `note` (a note or a new version of one). A new fixed event `host_braindump_given { items, bytes, outcome }` records outcomes `given`, `outside`, `io` and `nothing`, without names or content.

## Risks / Trade-offs

- [A note's text from a viewer becomes part of a prompt the agent reads.] → It's only written into `shared/braindump/` when the host gives the braindump, and the host sees every item in the *Shared* tab first. The prompt the host types names the files; it doesn't paste their text into the terminal. The security docs say so.
- [The prompt lists paths, but the agent may not read them all.] → That's up to the agent. The default prompt asks it to read them, and the timeline shows its reads.
- [A failed write in the middle leaves the folder as it was, but a crash between writing and marking could leave copies of unmarked items.] → On the next giving they get numbered names. That's rare and harmless.
- [Parallel changes to `control.rs`, `timeline.rs`, `adapter.rs`, `SharingSettings` and `TimelinePanel`.] → Only appended variants, constants, commands, a label and one entry kind in the panel.
- [Renaming the switch changes what people learned in #55.] → Same switch, same default. Only the label and hint change, and the docs follow.
- [`put_in_folder` gains an argument.] → Today's two callers pass `""`, and its existing tests stay as they are, plus a test for the subfolder.

## Migration Plan

Ships with the next release. Nothing is stored on disk except what the host gives. A session process of the previous release doesn't get the new commands (`NEW_COMMANDS`), and its snapshot has no `braindump`, so the window shows no braindump group. Rollback means releasing without the change. Protocol-5 viewers then see no braindump from a host without it. If the version was raised for this change, they are refused as for any protocol change.
