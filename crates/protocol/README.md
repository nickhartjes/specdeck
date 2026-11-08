# specdeck-protocol

The wire protocol between a host and its viewers. Both sides use this crate; it does no I/O. Design background: decisions 7 and 8 in [`design.md`](../../openspec/changes/archive/2026-10-31-native-shared-session-mvp/design.md).

Current version: `PROTOCOL_VERSION = 4` ([`src/control.rs`](src/control.rs)). Hosts admit viewers from `MIN_VIEWER_PROTOCOL_VERSION = 3` up to their own version: one version back.

- Version 4 added the whole text of code files: the `file-text` timeline entry and the host's setting for it (`Shared`). Version 3 viewers are admitted and watch as before, suggestions included: the host never sends them a `file-text` event or `Shared`. It skips such an event for them, which they see as a gap in the event sequence, as they already accept.
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

Each viewer has one WebRTC peer connection to the host with two reliable, ordered data channels:

| Channel | Content | Encoding |
|---|---|---|
| `control` | Handshake, participants, status, session end, viewers' requests | postcard ([`src/control.rs`](src/control.rs)) |
| `stream` | Terminal records, history, timeline events | Signed binary frames ([`src/frames.rs`](src/frames.rs)) |

## Control messages

### Viewer to host

Viewer messages come from untrusted peers. `decode_viewer` rejects messages over 1 KB (`Send` up to 4 KB plus 64 bytes), unknown variants, truncated messages and trailing bytes, usernames that are empty, longer than 64 characters or contain control characters, and suggestions that are blank, longer than 4096 bytes or contain control characters other than line breaks and tabs. A viewer that sends anything malformed is disconnected. There is deliberately **no message that carries terminal input**: a suggestion reaches the agent only when the host sends it from their own window.

| Message | Fields | Meaning |
|---|---|---|
| `Hello` | `protocol_version`, `username`, `last_output_seq?`, `last_event_seq?`, `resume_token?` | First message on a connection. When reconnecting, the `last_*` fields say where the viewer left off and `resume_token` lets it take its old place |
| `Resend` | `from_output_seq` | Send records again from this sequence, after a gap or a failed verification |
| `Leave` | | The viewer is leaving |
| `Send` (v3) | `id`, `request` | A request to the host. `request` is `Suggest { text }`. `id` is the viewer's own and unique among its requests; a repeated id is answered again without taking the request twice, so a viewer resends unanswered requests after reconnecting |

### Host to viewer

| Message | Fields | Meaning |
|---|---|---|
| `Welcome` | `protocol_version`, `viewer_id`, `session_public_key`, `cols`, `rows`, `resume_token`, `rejoin_room`, `agent`, `timeline_supported` | Viewer admitted. `session_public_key` verifies every stream frame; `resume_token` and `rejoin_room` are for reconnecting (see below); `timeline_supported` is false for agents whose events are not captured (all but Claude Code) |
| `Refused` | `reason` | Not admitted: `ViewerOutdated`, `HostOutdated`, `SessionFull`, `Removed` or `SessionEnded`. `RefusalReason::message()` is the text to show. The host closes the connection afterwards |
| `Participants` | `participants[]` (`id`, `username`, `role`, `label`) | Everyone in the session, on every change. `label` adds a suffix when usernames collide |
| `Status` | `status` | `Starting`, `Working`, `Waiting`, `Exited` or `Unknown` (agents whose events are not captured) |
| `SessionEnded` | | The host closed the session |
| `Allowed` (v3) | `suggestions` | What viewers may send; on admission and on every change |
| `Reply` (v3) | `id`, `outcome` | To the sender only: `Received`, `Off` (the host has turned that kind off) or `RateLimited` (3 at once, then 1 every 10 seconds per viewer, not reset by reconnecting) |
| `Suggestions` (v3) | `mine[]` (`id`, `text`, `state`) | To one viewer: their own suggestions in this session, `Waiting`, `Sent { edited }` or `Declined`; on admission and on every change |
| `Shared` (v4) | `whole_files` | Whether the host shares whole code files; on admission and on every change |

Messages marked v3 or v4 are sent only to viewers admitted with that version or later.

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
  | <-- Shared (v4 viewers) ------------------  |
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
| `timeline-unavailable` | Structured events cannot be captured; the terminal mirror still works |
| `timeline-available` | Events arrive again |

Entries of one prompt share `promptId`.
