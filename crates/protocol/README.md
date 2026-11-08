# specdeck-protocol

The wire protocol between a host and its viewers. Both sides use this crate; it does no I/O. Design background: decisions 7 and 8 in [`design.md`](../../openspec/changes/archive/2026-10-31-native-shared-session-mvp/design.md).

Current version: `PROTOCOL_VERSION = 6` ([`src/control.rs`](src/control.rs)). Hosts admit viewers from `MIN_VIEWER_PROTOCOL_VERSION = 5` up to their own version: one version back.

- Version 6 added the host's asks: a request to read something and a vote on a task's size ([`src/asks.rs`](src/asks.rs)). It added the `read-done` and `estimated` timeline entries, the `Answer` request, and the host's `ReadRequest` and `Vote`. Version 5 viewers are admitted and watch as before, suggestions and links included: the host never sends them those entries, `ReadRequest` or `Vote`, and lists them as on an older app, which can't answer.
- Version 5 added shared links: the `link` and `link-removed` timeline entries, the `ShareLink` request, and the host's `LinksAllowed` and `Links`. Which links are accepted is decided by `check_link` ([`src/links.rs`](src/links.rs)), used by the sharer, the host and every participant before opening one. Version 4 viewers are admitted and watch as before, suggestions included: the host never sends them a link entry, `LinksAllowed` or `Links`, and they can't share links.

- Version 4 added the whole text of code files: the `file-text` timeline entry and the host's setting for it (`Shared`). It also added shared files: the `files` channel, the `file-shared` entry, `FilesAllowed`, `Files`, and the `ShareFile` and `RemoveFile` requests with their outcomes. Version 3 viewers are admitted and watch as before, suggestions included: the host never sends them a `file-text` or `file-shared` event, `Shared`, `FilesAllowed` or `Files`, and does not accept a `files` channel from them. It skips such an event for them, which they see as a gap in the event sequence, as they already accept.
- Version 3 added requests from viewers to the host (`Send`) and the host's answers (`Allowed`, `Reply`, `Suggestions`). Version 2 viewers are admitted and watch read-only: the host never sends them a version 3 message, which they could not decode and would answer by reconnecting.
- Version 2 added Codex, Grok and pi to the agents; postcard encodes the agent by its index, so apps that speak version 1 cannot read a `Welcome` for them.

Not compatible with SpecDeck 0.x.

## Connecting

1. The host generates an [`InviteCode`](src/invite.rs): 12 random Crockford base32 characters (no I, L, O or U), shown as `7K2M-9QXH-4TPA`. When parsing, case, spaces and dashes are ignored, and O, I and L are read as 0, 1 and 1. A join link is `specdeck://join/<code>`.
2. Both sides derive two values from the code with HKDF-SHA256 (salt `specdeck invite v1`):
   - **room id** (info `room id`, 16 bytes, hex): the only thing the sharing service sees;
   - **auth key** (info `auth key`, 32 bytes): never sent anywhere.
3. The viewer's offer and the host's answer travel through the sharing service as [`SignedDescription`](src/signal_auth.rs)s: the SDP with an HMAC-SHA256 under the auth key over a version label, the kind (`offer` or `answer`), the connection id and the SDP, each length-prefixed. The receiver checks the tag for the expected kind and connection before using the SDP. The sharing service therefore cannot substitute DTLS fingerprints, or replay a description into another connection or direction.

## Channels

Each viewer has one WebRTC peer connection to the host with reliable, ordered data channels, all opened by the viewer:

| Channel | Content | Encoding |
|---|---|---|
| `control` | Handshake, participants, status, session end, viewers' requests | postcard ([`src/control.rs`](src/control.rs)) |
| `stream` | Terminal records, history, timeline events | Signed binary frames ([`src/frames.rs`](src/frames.rs)) |
| `files` (v4) | The bytes of shared files, both ways | postcard ([`src/files.rs`](src/files.rs)) |

`files` is a channel of its own so that a 10 MB file never holds up terminal output on `stream`.

## Control messages

### Viewer to host

Viewer messages come from untrusted peers. `decode_viewer` rejects messages over 1 KB (`Send` up to 4 KB plus 64 bytes), unknown variants, truncated messages and trailing bytes, usernames that are empty, longer than 64 characters or contain control characters, and suggestions that are blank, longer than 4096 bytes or contain control characters other than line breaks and tabs. A viewer that sends anything malformed is disconnected. There is deliberately **no message that carries terminal input**: a suggestion reaches the agent only when the host sends it from their own window.

| Message | Fields | Meaning |
|---|---|---|
| `Hello` | `protocol_version`, `username`, `last_output_seq?`, `last_event_seq?`, `resume_token?` | First message on a connection. When reconnecting, the `last_*` fields say where the viewer left off and `resume_token` lets it take its old place |
| `Resend` | `from_output_seq` | Send records again from this sequence, after a gap or a failed verification |
| `Leave` | | The viewer is leaving |
| `Send` (v3) | `id`, `request` | A request to the host. `request` is `Suggest { text }`, (v4) `ShareFile { name, size, sha256 }` (name 1 to 255 bytes) or `RemoveFile { file }`, (v5) `ShareLink { url, title?, session_only }`, where `url` must be in the normalized form `check_link` returns and `title` 1 to 120 characters without control characters, or (v6) `Answer(Read { ask, answer } | Vote { ask, round, card? })`. A read answer is `Reading`, `Ready` or `Question { text? }`, the text at most 280 characters without control characters but line breaks; a card is `Value(index)` into the vote's set (below 6 for any set), `Unsure` or `Break`, and no card takes the vote back. `id` is the viewer's own and unique among its requests; a repeated id is answered again without taking the request twice, so a viewer resends unanswered requests after reconnecting. A `ShareFile` is not resent: its upload belongs to the connection |

### Host to viewer

| Message | Fields | Meaning |
|---|---|---|
| `Welcome` | `protocol_version`, `viewer_id`, `session_public_key`, `cols`, `rows`, `resume_token`, `rejoin_room`, `agent`, `timeline_supported` | Viewer admitted. `session_public_key` verifies every stream frame; `resume_token` and `rejoin_room` are for reconnecting (see below); `timeline_supported` is false for agents whose events are not captured (all but Claude Code) |
| `Refused` | `reason` | Not admitted: `ViewerOutdated`, `HostOutdated`, `SessionFull`, `Removed` or `SessionEnded`. `RefusalReason::message()` is the text to show. The host closes the connection afterwards |
| `Participants` | `participants[]` (`id`, `username`, `role`, `label`) | Everyone in the session, on every change. `label` adds a suffix when usernames collide |
| `Status` | `status` | `Starting`, `Working`, `Waiting`, `Exited` or `Unknown` (agents whose events are not captured) |
| `SessionEnded` | | The host closed the session |
| `Allowed` (v3) | `suggestions` | What viewers may send; on admission and on every change |
| `Reply` (v3) | `id`, `outcome` | To the sender only: `Received`, `Off` (the host has turned that kind off) or `RateLimited` (3 at once, then 1 every 10 seconds per viewer, not reset by reconnecting). For files (v4) also `Ready { upload }` (send the bytes as that upload on `files`), `TooLarge`, `Full`, `NotAllowedType` and `Gone`. Answers (v6) get `Received`, `RateLimited` (a limit of their own: 5 at once, then 1 every second) or `Gone` (the ask closed, the round is over, the votes are revealed, or the card is not of the vote's set) |
| `Suggestions` (v3) | `mine[]` (`id`, `text`, `state`) | To one viewer: their own suggestions in this session, `Waiting`, `Sent { edited }` or `Declined`; on admission and on every change |
| `Shared` (v4) | `whole_files` | Whether the host shares whole code files; on admission and on every change |
| `FilesAllowed` (v4) | `files` | Whether viewers may share files; on admission and on every change |
| `Files` (v4) | `files[]` (`file`, `name`, `kind`, `media_type`, `size`, `sha256`, `by`, `by_label`, `at_ms`) | The files shared now, oldest first; on admission and on every change |
| `LinksAllowed` (v5) | `links` | Whether viewers may send links; on admission and on every change |
| `Links` (v5) | `mine[]` (`id`, `url`, `title?`, `session_only`, `state`) | To one viewer: their newest 50 links in this session, `Waiting`, `Shared` or `Declined`; on admission and on every change |
| `ReadRequest` (v6) | `open?` (`id`, `target`, `label`, `limit_s`, `left_ms?`, `people[]`, `mine`, `questions[]`) | To one viewer: the open read request, `None` once closed; on admission and on every change. `target` is `Document { path }`, `Code { path }`, `Shared { file }` or `Changes { from_seq, to_seq }` (the timeline events after the host's last prompt up to the moment of asking). `left_ms` is the time left when sent, so clocks never have to agree. `people` pairs each participant id with `Reading`, `Ready`, `Question` or `OlderApp`; `mine` is the viewer's own answer, with their own question's text. `questions` is always empty for viewers: only the host sees question texts |
| `Vote` (v6) | `open?` (`id`, `round`, `task`, `cards`, `voted[]`, `mine?`, `revealed?`, `older[]`) | To one viewer: the open vote, `None` once closed; on admission and on every change. `cards` is `StoryPoints` (1, 2, 3, 5, 8, 13) or `TShirt` (XS to XL), both with ? and ☕. Before the reveal it holds no vote's value but the viewer's own (`mine`); `revealed` pairs every participant id with their card afterwards |

Messages marked v3, v4, v5 or v6 are sent only to viewers admitted with that version or later.

### Handshake

```
viewer                                      host
  | -- Hello {protocol_version, username} -->  |
  |                                            |  check_version(), viewer limit
  | <-- Refused {reason} ---------------------  |  (then the host closes the connection)
  |  or                                        |
  | <-- Welcome {..., session_public_key} ----  |
  | <-- Participants, Status -----------------  |
  | <-- Allowed, Suggestions (v3 viewers) ----  |
  | <-- Shared, FilesAllowed, Files (v4) -----  |
  | <-- LinksAllowed, Links (v5 viewers) -----  |
  | <-- ReadRequest, Vote (v6 viewers) -------  |
  | <== history, then live records and ======  |
  |     timeline events on `stream`             |
```

`check_version(host_min, host, viewer)` accepts viewer versions from `host_min` to `host`; otherwise the refusal says which side has to update. `Welcome.protocol_version` is the version agreed with that viewer, the lower of the two. `Welcome`, including the signing key, is trusted only because the connection itself was authenticated with the invite code.

## Stream frames

Big-endian layout:

| Field | Size | Notes |
|---|---|---|
| `kind` | 1 byte | `1` output, `2` resize, `3` history, `4` timeline event |
| `seq` | 8 bytes | See below |
| payload | rest | Output: terminal bytes. Resize: `cols` and `rows`, 2 bytes each. History: zstd-compressed postcard list of records. Event: UTF-8 JSON of a `TimelineEvent` |
| signature | 64 bytes | Ed25519 over every byte before it |

`FrameVerifier::decode` verifies the signature before parsing anything and returns `FrameError::BadSignature` when it does not match; the viewer discards the frame and sends `Resend`. Because the signature covers `kind` and `seq`, a frame cannot be moved or changed into another kind. A history batch decompresses to at most 64 MB and must contain consecutive records starting at its `seq`.

### Sequence numbers

- **Records** (output and resize) share one sequence: it starts at 0 and increases by one per record, with no gaps. A resize is a record, so a replay reproduces every size the host had.
- **History** frames carry the sequence of their first record.
- **Event** frames have their own sequence, starting at 0. Each event also has `outputSeq`: the newest record when it happened (-1 before any output), which links timeline entries to terminal positions.

### Late join, reconnect and catch-up

These are the same operation: the host starts sending records from a sequence number.

- **Late join**: from record 0, as history batches until the viewer is close to live, then live records; and every timeline event from 0.
- **Reconnect**: the viewer's `Hello` carries `last_output_seq` and `last_event_seq`; the host continues from the next ones.
- **Slow viewer**: the host stops sending live records to a viewer whose send buffer is full, and continues from where it is with history batches once the buffer drains.

A viewer applies a record only if its sequence is exactly one higher than the last one applied, and otherwise asks for the missing range with `Resend`.

### Rejoin room

A viewer that was admitted reconnects through the session's **rejoin room** (from `Welcome`), not the invite code: the host keeps that room open for the whole session, so a viewer that was offline while the host made a new code can still return. Its offer and the host's answer are authenticated with the viewer's resume token instead of the code's auth key; the host tries the tokens of the viewers it admitted. A removed viewer's token is forgotten, so it cannot return this way. When the host closed the session the rejoin room is gone, and the viewer is told the session ended.

## Shared files

Messages on `files` ([`src/files.rs`](src/files.rs)), postcard, at most 64 KB of content plus 64 bytes each:

| Direction | Message | Meaning |
|---|---|---|
| viewer to host | `Fetch { file, from }` | Send the file's bytes from offset `from`; replaces a fetch still running |
| viewer to host | `Cancel { file }` | Stop sending it |
| viewer to host | `Upload { upload, offset, bytes }` | A chunk of an upload the host accepted with `Ready`, in order, at most 512 KB beyond the last `Ack` |
| host to viewer | `Chunk { file, offset, bytes }` | A chunk of a fetched file, in order |
| host to viewer | `Gone { file }` | Removed, or never existed |
| host to viewer | `Ack { upload, received }` | Bytes of the upload the host has |

Chunks are not signed: the signed `file-shared` event announces `size` and `sha256`, and the receiver checks the whole file against them, discarding it and fetching again when it does not match. The host sends a viewer only files that were shared, by the id it issued; a viewer cannot name a path. An upload becomes a shared file only after the host checked its hash and its type from its content (PNG, JPEG, GIF, WebP, Markdown or UTF-8 text), and within the limits: 10 MB a file, 50 MB a session, 20 MB a viewer. Chunks of an upload the host no longer has (refused, timed out, switched off) are ignored; a chunk out of order or past the announced size disconnects the viewer.

## Timeline events

`TimelineEvent` ([`src/timeline.rs`](src/timeline.rs)) has `outputSeq`, `at` (milliseconds since the epoch) and an entry tagged by `kind`:

| `kind` | Meaning |
|---|---|
| `session-started`, `session-ended` | Agent session lifecycle |
| `prompt` | A prompt the host submitted |
| `message` | Assistant text as it is shown (live; several per prompt) |
| `response` | The final response of a prompt |
| `tool-call` | A tool started |
| `tool-result` | A tool finished: `succeeded`, `failed` (with `error`) or `not-run` |
| `file-change` | A file created or edited, with diff hunks |
| `file-read` | A file the agent read |
| `document` | The whole text of a Markdown file the agent created or changed, after its `file-change`; `text` is null when too large |
| `file-text` (v4) | The whole text of a code file the agent created or changed, after its `file-change`, only while the host shares whole code files; `text` is null when too large. Never sent to version 3 viewers |
| `file-shared` (v4) | Someone shared a file: `file`, `name`, `fileKind` (`image`, `markdown` or `text`), `mediaType`, `size`, `sha256` (hex), `by` (participant id, 0 for the host) and `byLabel`. Its bytes come on `files` and are checked against `size` and `sha256`. Never sent to version 3 viewers |
| `link` (v5) | A link shared with everyone: `id` (from 1 within the session), `url`, `title?`, `by` (the label of whoever shared it) and `sessionOnly`. `url` is null once the host removed it: the host then also sends the stored entry without it to anyone who receives it later. Never sent to version 4 viewers |
| `link-removed` (v5) | The host removed the link `id`. Never sent to version 4 viewers |
| `read-done` (v6) | A read request closed: `id`, `target`, `label`, how many were `ready` and had `questions` out of the `of` who could answer, and `timeUp`. No names, no question texts. Never sent to version 5 viewers |
| `estimated` (v6) | The host kept an estimate: `id`, `task`, `cards`, `kept`, the `low` and `high` value cards voted (null when every vote was ? or ☕) and the number of `votes`. No names or single votes. Never sent to version 5 viewers |
| `timeline-unavailable` | Structured events cannot be captured; the terminal mirror still works |
| `timeline-available` | Events arrive again |

Entries of one prompt share `promptId`.
