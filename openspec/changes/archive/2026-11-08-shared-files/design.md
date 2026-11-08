# Design

## Context

See proposal.md for motivation and the delta specs for the behavior. The user settled five questions while exploring #55:

1. Host and viewers share. Viewers go through the #51 request channel, behind a host switch that is off at the start of each session.
2. Only viewable types (PNG, JPEG, GIF, WebP, Markdown, text, no SVG), checked by content. 10 MB a file, 50 MB a session.
3. Files last for the session, also for late joiners, in memory only.
4. *Put in the session folder* writes to `shared/` and never overwrites.
5. Each share is a timeline step.

What is there today:

- **Channels.** Each peer has two reliable, ordered data channels that the viewer opens (`peer.rs`, `app/ui/src/web/connection.ts`): `control` (postcard, `control.rs`) and `stream` (signed frames, `frames.rs`). The host maps an unknown label to the stream handler and ignores its messages. A frame is at most `MAX_FRAME_BYTES` (256 KB), and a timeline event's JSON at most 240 KB. `pump` keeps each viewer's stream queue under `HIGH_WATER` (1 MB) through `buffered_amount`.
- **Viewer requests.** `ViewerMessage::Send { id, request }` with `ViewerRequest::Suggest`. A `Send` is at most `MAX_REQUEST_BYTES` (about 4 KB). `Sharing` gates each request (switch, token bucket of 3 and 1 every 10 s, duplicate ids) and answers with `HostMessage::Reply`. New host messages go to newer viewers only through `send_newer(slot, version, …)`, and `pump` skips `FileText` events for protocol-3 viewers.
- **Versions.** `PROTOCOL_VERSION = 4` and `MIN_VIEWER_PROTOCOL_VERSION = 3`. Version 4 came with #56 after v1.6.0 and is not released yet.
- **Session process.** The window sends JSON `Command`s over a local connection whose frames may be 16 MB (`crates/local/src/frame.rs`). `AllowRequests { suggestions }` is in `adapter.rs`'s `NEW_COMMANDS` with `share-code`, held back from a 1.6.0 session. The snapshot's `suggestionsPossible` tells the window what a session supports.
- **Session folder.** `SessionFolder::contains` resolves symlinks (`limit-viewer-data`).
- **UI.** `FilesPanel` has *Documents* and *Code*. `DocumentTab` renders Markdown with *Formatted*/*Source*, and `CodeLines` renders numbered text. `Screenshot.tsx` shows images as `data:` URLs, which both CSPs allow (`img-src 'self' data:`). `SharingSettings` holds the host's switches. `tauri-plugin-dialog` is already a dependency.
- **Crates in the offline store:** `sha2` 0.11 (already used by `protocol`) and `base64` 0.23.

## Goals / Non-Goals

**Goals:**

- A file transfer path in both directions, which #61 and #62 reuse with their own request kinds.
- Large files never delay the terminal.
- What leaves the host is exactly what a person chose in a window. A viewer's request names a file id the host issued, never a path.
- Older viewers on a supported protocol keep watching unchanged.

**Non-Goals:**

- Peer-to-peer between viewers. Every file goes through the host, which also enforces the limits.
- Resuming a half-finished transfer. A dropped transfer restarts; files are at most 10 MB.
- Keeping files after the session, or encrypting them in the host's memory beyond what the process already has.
- Merging with the *Links* group or the concept's single *Add* dialog. #66 does that.

## Decisions

### 1. A third data channel, `files`, for content; metadata stays where it is

The store lives in `SessionView`, shared by the session process's commands and the sharing task, so the host can share before anyone joins and a timeline entry goes through the session's pump like the agent's.

New viewers open a third channel, `files`, next to `control` and `stream`. All file bytes, in both directions, travel on it. Everything else stays on the existing channels:

- **What exists** (id, name, size, type, hash, who, when) goes into the signed timeline as `Entry::FileShared`, plus a list in `HostMessage::Files`.
- **Asking to share or remove** is a `ViewerRequest` on `control`, through the existing gate.
- **Content** goes on `files`.

Each data channel is its own SCTP stream, so a 10 MB transfer on `files` doesn't hold up `stream`. The host sends file chunks only while the `files` queue is under a smaller mark (256 KB), so file bytes also leave room in the congestion window for terminal output.

The host accepts `files` only from a viewer admitted on `SHARED_FILES_PROTOCOL_VERSION` or later. Older viewers never open it. Old hosts map the unknown label to the stream handler and ignore it, but a newer viewer is refused by an older host anyway.

- *Alternative: chunks as a new frame kind on `stream`.* Simplest, but a file would delay terminal output behind it on the one ordered channel, against the spec's "sending a file shall not hold up the terminal". Rejected.
- *Alternative: file content in timeline events.* Capped at 240 KB, and sent to every viewer whether they open the file or not. Rejected.

### 2. Messages on `files`

Postcard, decoded strictly like `control` (`crates/protocol/src/files.rs`):

```rust
// viewer → host
enum FilesRequest {
    Fetch   { file: u32, from: u64 },             // send this file's bytes from offset `from`
    Cancel  { file: u32 },
    Upload  { upload: u32, offset: u64, bytes: Vec<u8> },  // a chunk of an accepted upload
}
// host → viewer
enum FilesReply {
    Chunk   { file: u32, offset: u64, bytes: Vec<u8> },
    Gone    { file: u32 },                         // removed, or never existed
    Ack     { upload: u32, received: u64 },        // flow control for uploads
    Finished { upload: u32, outcome: Outcome },    // what became of the upload
}
```

- A chunk carries at most `FILE_CHUNK_BYTES = 64 KB`. A message over 64 KB + 64 bytes is malformed and disconnects the viewer.
- Chunks are not signed. The viewer hashes the whole file with SHA-256 and compares it to `sha256` in the signed `FileShared` entry. A mismatch discards the file and fetches it again, once. If the second attempt fails too, the tab says the file couldn't be received, and the log says why. DTLS already protects the connection, so the hash only has to cover "the content is what the host announced". That keeps 160 signatures per 10 MB out of the host's hot path.
- A viewer fetches one file at a time. A new `Fetch` cancels the one before it.
- An upload is sent at most 512 KB ahead of the host's last `Ack`, so a slow host cannot be flooded. A chunk out of order or past the announced size is malformed and disconnects the viewer. Chunks for an upload id the host no longer has are ignored: they are still on their way when the host dropped the upload (switched off, timed out, refused).
- The pure state of a fetch (`Fetching`) and of an upload (`Uploading`) lives in `crates/protocol/src/files.rs`. The app's viewer drives them in `crates/core/src/viewer.rs`, and the web viewer through `FileFetch` and `FileUpload` in `crates/web`, so both check and send alike.

### 3. Sharing from a viewer: announce on `control`, send on `files`, publish when complete

```rust
ViewerRequest::ShareFile  { name: String, size: u64, sha256: [u8; 32] }   // appended
ViewerRequest::RemoveFile { file: u32 }                                   // appended
Outcome::{ Ready { upload: u32 }, TooLarge, Full, NotAllowedType, Gone }  // appended
HostMessage::FilesAllowed { files: bool }                                 // appended
```

`Sharing`'s gate handles `ShareFile` like a suggestion: the switch (`Off`), the token bucket (`RateLimited`), the size (`TooLarge`), and the session's and the viewer's room (`Full`). Room is reserved at once. If the request passes, the host replies `Ready { upload }`. The viewer streams chunks, and when `size` bytes have arrived, the host:

1. checks the hash;
2. checks the type by content (decision 5);
3. stores the file;
4. adds the timeline entry;
5. broadcasts the list.

It answers on `files` with `Finished { upload, outcome }`: `Received` when the file is shared, otherwise `NotAllowedType`, `Full`, `Off` (the switch went off meanwhile) or `Gone` (the hash did not match, or the upload stalled). A failed check frees the room.

- Only one upload per viewer is in flight. A second `ShareFile` while one is open is `RateLimited`.
- An upload with no chunk for 30 s is dropped and its room is freed.
- A viewer that disconnects loses its unfinished upload.
- `RemoveFile` is honoured only for the viewer's own file. A request for another person's file, or a file that is gone, is answered `Gone`, not treated as malformed, because the file may have been removed meanwhile. The switch does not gate it.
- A `ShareFile` is not kept for sending again after a reconnect, as suggestions are: its upload belongs to the connection. The viewer offers the file again itself, from the start. After `RateLimited` it offers it again 10 seconds later.
- Only suggestions are announced to the window as `request-sent`. The viewer's window follows its latest suggestion by that id, and files report back on their own.
- `name` is limited to 255 bytes, so `ShareFile` stays under `MAX_REQUEST_BYTES`.

The `Reply` and `Outcome` route keeps the viewer side identical for suggestions and files, and #61 and #62 add their kinds the same way.

### 4. The host's own files come from the window as bytes

The host's window reads the chosen or dropped `File` in the webview and sends `Command::ShareFile { name, data }`, with `data` base64. 10 MB becomes about 13.4 MB, under the 16 MB local frame. The session process never takes a path to read. Only bytes a person picked in a window can ever be shared, which makes *Only files a participant chose are sent* structural. It also works the same in the app and the web viewer, where there are no paths.

New commands, JSON-kinded, added to `NEW_COMMANDS` in `adapter.rs`. Those with an `id` are answered with a new `Event::Answer { id, value?, error? }`, which the app matches to the waiting call, as `browser-result` answers `browser` the other way:

| Command | Effect |
|---|---|
| `share-file { id, name, data }` | Shares as the host; answered with the file as listed. |
| `remove-shared-file { file }` | Removes any file. |
| `read-shared-file { id, file }` | Answered with the bytes (base64), so the host's window can show a viewer's file. |
| `put-shared-file { id, file }` | Decision 7. |
| `allow-files { files }` | The viewers' switch. |

`share-file` and `read-shared-file` carry an `id` as well, and refusals come back as the answer's `error`, in plain words. The viewers' switch is a command of its own rather than a field on `allow-requests`, so a window that sends only `suggestions` can never turn files off by leaving the field out. The snapshot gains `sharedFilesPossible`, `filesAllowed` and the list, and a `shared-files` host event carries both on every change. The window offers sharing only when `sharedFilesPossible` is there, as it does for suggestions.

The viewer side mirrors this. `crates/core/src/viewer.rs` and `app/src-tauri/src/viewer.rs` gain `share_file`, `remove_file`, `open_file` (fetch, verify, return bytes) and `save_file`, which writes the verified bytes to a path from the save dialog. The bridge passes bytes as `ArrayBuffer` through Tauri's raw IPC (`tauri::ipc::Response`), not as JSON arrays.

### 5. Types decided by content, in the host's store

`crates/core/src/shared_files.rs` holds the store and the rules:

- **Images:** recognised by magic bytes (PNG `89 50 4E 47`, JPEG `FF D8 FF`, GIF `GIF87a`/`GIF89a`, WebP `RIFF....WEBP`). The media type comes from the bytes, not the name, and the extension is corrected to match.
- **Markdown:** a name ending in `.md` or `.markdown`, with text content.
- **Text:** anything else whose content is valid UTF-8 with no NUL byte. Its name keeps its extension, so `.log`, `.json` and `.rs` stay recognisable for highlighting.
- **Everything else** is refused, SVG included, even though it is text: it would open as an image in some places, and a text view of SVG brings nothing over that risk.
- **Names** are cleaned (spec *A shared file's name is only a name*): last path component only, after both `/` and `\`; control characters dropped; leading dots dropped; cut to 255 bytes on a character boundary.
- **The store:** `Vec<SharedFile { id, name, kind, media_type, size, sha256, by: Sharer, at, bytes: Arc<[u8]> }>`. `Sharer` is the host or a viewer id. Ids count up and are never reused. Limits: 10 MB a file, 50 MB in all, 20 MB a viewer. Removing frees the room at once. Transfers already running hold their own `Arc` and finish, but no new `Fetch` is served.

The host's window applies the same checks before sending, so a person hears about a refusal at once. The session process checks again and decides.

### 6. Timeline entry and list, and older viewers

`Entry::FileShared { file: u32, name, kind, media_type, size, sha256 (hex), by: u32 /* participant id, 0 = host */ }`, kind `file-shared`, is added to the timeline when a file is stored. `HostMessage::Files { files: Vec<SharedFileInfo> }` lists the files that exist now. It goes to newer viewers on admission and on every change, through `send_newer(slot, SHARED_FILES_PROTOCOL_VERSION, …)`. The UI joins the two: a step whose file is not in the list is shown as removed. Steps are not part of a prompt: the timeline panel shows each between the prompts it came between, by time. A session without a timeline (a terminal session, or agents other than Claude Code) has no timeline to show them in, so there the *Shared* tab is the only place they appear. Labels come from `Participants`, which keeps who shared a file correct after they leave. The list holds the label at the time of sharing too, so a late joiner sees names of people who have already left.

`pump` skips `FileShared` for viewers below `SHARED_FILES_PROTOCOL_VERSION`, exactly as it skips `FileText`. The test that admits an older viewer and checks it only receives what it can read gets a shared file, a removal and a switch change.

### 7. *Put in the session folder*

`put-shared-file` writes `<folder>/shared/<name>`:

1. Create `shared` if it is missing.
2. Refuse when `SessionFolder::contains` says the resolved `shared` is outside the folder.
3. Open with `create_new`. If the name exists, try `name (2).ext`, `name (3).ext` and so on, up to 100. `create_new` also means a symlink planted at the target name is never followed.

The event answers with the relative path, and the window shows "In `shared/wireframe.png` now. The agent can read it there." Nothing is typed into the agent. The host tells the agent in their own words. The written file is a normal file in the folder from then on: if the agent reads or changes it, the existing timeline rules apply. No `.gitignore` is written.

### 8. Protocol version

All additions are appended variants, behind `SHARED_FILES_PROTOCOL_VERSION`:

- While protocol 4 is unreleased, the constant is 4 and `PROTOCOL_VERSION` stays 4. One unreleased protocol then carries #56 and #55. Viewers built from the branch between #56 and #55 are only development builds.
- If a release ships protocol 4 before this lands, `PROTOCOL_VERSION` and the constant become 5, and `MIN_VIEWER_PROTOCOL_VERSION` becomes 4. The task list checks this at the start of apply (task 1.1).
- #66 (links) is in the same situation. Whichever lands second keeps the same rule, and both use their own `*_PROTOCOL_VERSION` constant, so a bump is one line each.

### 9. UI

- **`FilesPanel`:** a *Shared* tab. Its body is `SharedTab`, a list of groups. This change adds `SharedWithEveryone`, and #66 and #61 add their groups as siblings. The footer has *Share files* (a hidden `<input type=file multiple>` plus drop on the panel), or the "host has not turned on sharing files" line.
- **`SharedFileTab`:** a new `TabKind` `shared`. It fetches on open, with progress for large files, then shows:
  - images through `Screenshot`-style `<img src="data:…">` with the host-decided media type;
  - Markdown through `DocumentTab`'s renderer with *Formatted*/*Source*, without "changes since last read", because a shared file doesn't change;
  - text through `CodeLines`, highlighted by extension when Prism knows it, and plain over 50,000 lines.

  The bar has who and when, *Save a copy*, *Remove*, and *Put in the session folder* (host). Rendering never uses `innerHTML`, the same rule as #56.
- **Timeline:** `TimelinePanel` renders `file-shared` as a step that opens the tab.
- **Settings:** `SharingSettings` gets the switch *Viewers can share files*, with: "Viewers' files reach you and everyone in the session. Claude sees one only when you put it in the session folder."
- **Saving:** the app uses the dialog plugin's save dialog with the shared name. The web viewer uses an `<a download>` on a `blob:` URL; downloads are not governed by `img-src`.

### 10. Diagnostics

Fixed events, without names or content:

- `host_file_shared { by, kind, bytes, outcome }`, where outcome is `shared`, `too_large`, `full`, `type`, `hash`, `off` or `limited`;
- `host_file_removed { by }`;
- `host_file_put { bytes, outcome }`;
- `host_file_sent { viewer, bytes, outcome }`;
- `viewer_file_fetched { bytes, outcome }`;
- `viewer_file_uploaded { bytes, outcome }`.

The marker test in `crates/core/tests/diagnostics_leak.rs` shares a file with a marker in its name and content.

## Risks / Trade-offs

- [A viewer shares something harmful: a misleading log, an image with hidden content, prompt-injection text] → Off by default. Files show who shared them. Only viewable types are allowed, rendered without script. Nothing reaches the agent unless the host puts it in the folder, and the docs say plainly that the host should read a file before doing that.
- [Memory: 50 MB of shared files in the session process, plus transfers in flight] → The caps are fixed, transfers share the stored `Arc`, and uploads reserve room up front. The 20-viewer limit and one transfer per viewer at a time bound the buffers.
- [Upload bandwidth for the host: 20 viewers opening one 10 MB file is 200 MB upstream] → Viewers fetch only what they open, one at a time, under `files` backpressure. The terminal keeps priority through the lower high-water mark.
- [The host sees a viewer's image before deciding anything] → That's the purpose. The image is decoded by the webview's image decoder, the same as screenshots.
- [Base64 in local JSON costs 33% and a copy] → It's local IPC, at most once per share or open. It's simpler than a binary local frame type, and within the 16 MB frame.
- [Parallel changes to the *Shared* tab (#66), the protocol and `TimelinePanel`] → Groups are separate components, variants are appended, and the version is a constant per feature. Rebase, then re-run `openspec validate` and the protocol tests.
- [`HostMessage::Files` grows with the number of files] → At most 50 MB at a minimum size of 1 byte would be many entries. The store caps the count at 200 files as well.

## Migration Plan

Nothing is stored. Ship the host app and the sharing service's web viewer in the same release, as for #56. A self-hosted service on an older release serves a page on an older protocol, whose viewers watch without the *Shared* tab. Sessions of 1.6.0 running when the app updates get none of the new commands (`NEW_COMMANDS`), and their snapshot has no `sharedFilesPossible`, so the window hides sharing. The session-process transcript for this release covers the new commands. Rolling back means releasing the previous version: viewers of this one are then told the host needs to update.

## Open Questions

- The exact copy of the refusals and the notes in the tab. It can be settled during implementation against the concept.
