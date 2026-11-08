# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. The user settled four questions while exploring #66:

- The host approves viewers' links, as with suggestions.
- *For this session only* starts on for any link with a `#` part.
- Links also work in sessions without a timeline.
- The host can remove a link. The mark is fixed once a link is shared.

What is there today:

- **Timeline on the wire.** `HostSession::emit_entry` (`crates/core/src/session.rs`) stamps an `Entry`, fits it, and pushes it to `timeline: Arc<RwLock<Vec<TimelineEvent>>>`. `pump` in `sharing.rs` sends each event to each viewer as its own signed `Frame::Event`, and late joiners read the same vector from the start. An unknown entry `kind` makes the frame `Malformed` for a viewer, so `pump` skips `FileText` for viewers below `WHOLE_FILES_PROTOCOL_VERSION`. Viewers accept the gap.
- **The event channel runs in every session**, terminal sessions included (`TimelineUnavailable` is itself an entry). Links can therefore travel the same way in a session without a timeline.
- **Requests.** `Sharing` owns `Requests`: the switches, a token bucket per viewer, and the suggestions (decision 4 of `viewer-to-host-channel`). `ViewerMessage::Send { id, request }` carries a `ViewerRequest`, decoded strictly, so an invalid suggestion disconnects the viewer. Replies come from the gate, and the host's later decisions arrive in `HostMessage::Suggestions { mine }`.
- **Versions.** `PROTOCOL_VERSION = 4` and `MIN_VIEWER_PROTOCOL_VERSION = 3`. `HostMessage` is postcard, so a new variant must never reach an older viewer.
- **Session process and app.** The window sends `Command`s (`crates/session/src/messages.rs`). `adapter.rs` holds commands new in this release back from a session of the previous one (`NEW_COMMANDS`). The window offers a control only when the snapshot has the matching field, such as `suggestionsPossible` or `wholeFilesPossible`.
- **Opening pages.** `tauri-plugin-opener` is already a dependency, used only from Rust (`open_install_page`). The webview has no opener permission.
- **UI.** `TimelinePanel`, `SuggestionsInbox` (the host's waiting suggestions), `SuggestPanel` (a viewer's own) and `SharingSettings` (the toolbar's sliders button). The web viewer uses the same components through `Bridge`.

## Goals / Non-Goals

**Goals:**

- One function decides whether a link may be shared. It is used by the sharer's UI, by the host on every link, and by every participant before opening one.
- No part of SpecDeck ever requests the linked page.
- Viewers on protocol 4 keep watching, unchanged.
- Small additions in the files shared with the parallel changes (`control.rs`, `adapter.rs`, `SharingSettings`).

**Non-Goals:**

- Resolving host names to find out whether they point to the sharer's machine.
- Editing a viewer's link before sharing it, or changing the mark afterwards.
- Notifications for new links (#59), and recaps or exports (#57). The mark is stored now, so #57 only has to read it.

## Decisions

### 1. One check for a link, in the protocol crate

`crates/protocol/src/links.rs`:

```rust
pub const MAX_LINK_BYTES: usize = 2048;
pub const MAX_LINK_TITLE_CHARS: usize = 120;
pub fn check_link(input: &str) -> Result<Url, LinkRefusal>
pub fn valid_title(title: &str) -> bool          // 1..=120 chars, no control characters
pub fn has_fragment(url: &Url) -> bool           // non-empty `#` part: the mark's default
enum LinkRefusal { TooLong, NotWeb, OwnMachine, Credentials, Malformed, Title }
```

`check_link` returns the normalized address as a `String`. `HostSession::share_link` also refuses a title that fails `valid_title` (`LinkRefusal::Title`). `check_link` parses with the `url` crate, which is already in `Cargo.lock` through other dependencies and builds for wasm. The crate also normalizes the forms that hide loopback: `http://2130706433/`, `http://0x7f.1/` and `[::ffff:127.0.0.1]` all parse to loopback addresses. Then:

- **Scheme.** Only `https` and `http`.
- **Credentials.** No user name and no password.
- **Own machine.** The host is not `localhost`, does not end in `.localhost`, and is not an IP address that is loopback or unspecified. IPv4-mapped IPv6 addresses count as the IPv4 address in them.
- **Length.** The normalized form is at most 2048 bytes.

The normalized form (`Url::as_str`) is what is sent, stored, shown and opened. The site on the card is `url.host_str()`, which the crate already gives in its ASCII (punycode) form.

`LinkRefusal::OwnMachine` is the same for `https` and `http`. The message differs per side: the host gets the hint about #68, and a viewer is told that the address is on their own machine.

- **Alternative: resolve the name and refuse loopback results.** This catches names like `lvh.me`. But the host would make a DNS request for every link, which leaks which links are shared, and a name can resolve differently for each participant. Such a link is only useless, not dangerous, so it is rejected.
- **Alternative: refuse private addresses (`10/8`, `192.168/16`) over `http`.** Viewers on the same network can reach them, and the issue allows `http` for exactly these. Rejected.

### 2. Links are timeline entries; a removal redacts the stored entry

```rust
Entry::Link { id: u32, url: Option<String>, title: Option<String>,
              by: String, session_only: bool }          // kind "link"
Entry::LinkRemoved { id: u32 }                          // kind "link-removed"
```

`id` is the host's, counting from 1 within the session. `by` is the participant's label when the link was shared. `url: None` means the link was removed.

`HostSession` gains `share_link(url, title, by, session_only) -> u32` and `remove_link(id)`. Both run through the session loop, like `emit_entry`, so a link is stamped with the current `output_seq` and lands in order with the agent's entries. `remove_link` does two things under the timeline's write lock. First it sets `url` to `None` in the stored `Link` event, so late joiners and viewers whose cursor has not reached it never get the address. Then it emits `LinkRemoved`, so participants who already have the card update it. The `Link`'s title stays, because the title is not the secret. A removed link stays in the list as removed.

`fit` leaves `Link` alone: the limits keep it far below the event budget.

- **Alternative: keep links outside the timeline, in a `HostMessage::Links` list for everyone.** That would need its own catch-up path and its own ordering. The issue asks for the card in the timeline at the moment it was shared. Rejected.
- **Alternative: append only `LinkRemoved` and leave the stored entry as it is.** Late joiners would then still receive the address. Rejected.

### 3. Protocol 5, and the host holds links back from protocol 4

`PROTOCOL_VERSION = 5`, `MIN_VIEWER_PROTOCOL_VERSION = 4`, and a new `LINKS_PROTOCOL_VERSION = 5`. `REQUESTS_PROTOCOL_VERSION` and `WHOLE_FILES_PROTOCOL_VERSION` stay as they are. In `pump`, `Link` and `LinkRemoved` are skipped for viewers below `LINKS_PROTOCOL_VERSION`, next to the existing `FileText` check. The new host messages are sent only to those viewers too.

If #55 also needs protocol 5 in the same release, both changes use version 5. Each keeps its own `*_PROTOCOL_VERSION` constant, and whichever merges second resolves the version lines.

### 4. Viewers' links are a new request kind on the existing gate

```rust
ViewerRequest::ShareLink { url: String, title: Option<String>, session_only: bool }  // appended
HostMessage::LinksAllowed { links: bool }               // appended; on admission and on change
HostMessage::Links { mine: Vec<SentLink> }              // appended; to one viewer
struct SentLink { id: u32, url: String, title: Option<String>, session_only: bool, state: SentLinkState }
enum SentLinkState { Waiting, Shared, Declined }
```

The names avoid `ViewerLink` and `LinkState`, which `crates/core` already uses for a viewer's connection. `decode_viewer` checks `ShareLink` with `check_link` and `valid_title`, and treats a failure as malformed (`DecodeError::InvalidLink`), as for an invalid suggestion. It accepts only the normalized form, so the host shares exactly what it checked. The UI checks the same rules first, so an honest client never trips them. `MAX_REQUEST_BYTES` (4160) already holds a 2048-byte link and a 120-character title.

In `Sharing`, `Requests` gains `links_on` (true at start, in every session including terminal ones, because a link is never typed into the PTY) and `links: Vec<HostLink>`. The gate runs as for suggestions: known id, then switch, then bucket (the same bucket per viewer), then store as `Waiting`. Answering goes through the same path as suggestions. `Links` carries a viewer's newest 50 links, so it always fits in one frame. Older ones that are still waiting are rare, and the host still has them.

`Allowed { suggestions }` stays as it is: changing its fields would break protocol-4 viewers. `LinksAllowed` is its own message.

### 5. Host actions are new commands

`crates/session/src/messages.rs` gains:

- `ShareLink { url, title, session_only }`: the host's own link.
- `AnswerLink { viewer_id, id, answer: Share | Decline }`.
- `RemoveLink { id }`.
- `AllowLinks { links }`.

The session process checks every `ShareLink` with `check_link`, also when it comes from the host's own window. `AnswerLink::Share` shares the viewer's stored link, never a text from the window, so the host shares exactly what they saw. The snapshot gains `linksPossible`, `linksAllowed` and the waiting viewer links. A new `HostEvent::Links` carries the waiting list when it changes.

`adapter.rs` adds `share-link`, `answer-link`, `remove-link` and `allow-links` to `NEW_COMMANDS`. A session of the previous release has no `linksPossible`, so the window offers none of these controls for it.

### 6. Opening: Rust opens only a known link, after checking it again

The app's Rust side passes timeline events to the window without keeping them. It now also records the links as they pass, in a `LinkBook` (`app/src-tauri/src/links.rs`): one per host session, in `SessionClient`'s state (`client.rs`), and one per viewer connection (`viewer.rs`). A `LinkRemoved` takes the address out, and a host snapshot starts the book over. A new Tauri command, `open_shared_link(id)`, looks the link up for the window's session (`open_link` already names the handler for join links). It runs `check_link` on the stored address and opens it with `app.opener().open_url`. The window never passes an address, and the webview gets no opener permission. A link that fails the check (only possible from a modified host) is shown as *can't be opened* and has no button. *Copy link* copies `url` from the entry, with the existing clipboard path.

The web viewer opens a link with `window.open(url, '_blank', 'noopener,noreferrer')`, so the new tab gets no `window.opener` and no referrer. Before that it checks the link with `checkLink` in `app/ui/src/model/links.ts`, which uses the browser's own `URL` parser. That parser follows the same WHATWG standard as the `url` crate, and the tests run the same cases against both.

- **Alternative: the same check in wasm.** It worked, but the `url` crate and its IDNA tables made the web viewer's WebAssembly 264 KB larger (+36%, +88 KB gzipped). Without it the growth is 25 KB. The host still checks every link, so a page that skips its own check can only lose its own connection. Rejected.

- **Alternative: a Tauri `open_url` command that takes the address.** Any script in the webview could then open any address. Rejected.

### 7. UI

- **The bar above the timeline.** `LinksBar` sits at the top of the timeline panel's body, also in sessions without a timeline. It holds *Share a link* for the host (*Send a link to Nick* for a viewer), the host's waiting viewer links with *Share* and *Decline* and their whole address, and a viewer's own links with their state and the reply to the latest one. Waiting links are not in `SuggestionsInbox`, and a viewer's links not in `SuggestPanel`: the Prompts tab doesn't exist in terminal sessions, where links do. A new waiting link is announced with a toast, as a suggestion is.
- **Share control.** *Share a link* opens a small dialog with *Address*, *Title (optional)* and *For this session only*. The mark follows `has_fragment` until the person changes it. The error text for each `LinkRefusal` is shown under the field. For a viewer, the same dialog sends the link as a request. #55's combined *Add files, a note or a link* dialog can take this over later.
- **Card.** `LinkCard`, rendered by `TimelinePanel` for `link` entries: in the turn that was running when the link was shared, or above the turns for links shared before the first prompt. It shows the title (or the site), the site, the address (masked after `#` when the link is for this session only), who shared it and when, *Open in your browser* and *Copy link*. The host also gets *Remove…* with a confirmation that says people who already saw the link keep it. All text goes into React text nodes.
- **No timeline.** `TimelinePanel` keeps its note and renders the bar and every link below it. The other entries of such sessions are already empty.
- **Styles.** In `app/ui/src/links.css`, imported after `styles.css` as `highlight.css` is, so `styles.css` is untouched.
- **Switch.** *Viewers can share links* in `SharingSettings`, below the suggestions switch.

## Risks / Trade-offs

- [A link name that resolves to the sharer's machine (`lvh.me`) gets through.] → The link is only unreachable, not harmful. The docs say SpecDeck checks the address, not where it points.
- [A shared link can be a phishing page.] → Only the host can put a link in front of everyone. The card shows the site in ASCII form, and nothing opens without a click.
- [After a removal, people who saw the link still have it.] → The confirmation and the docs say so. Removal hides the address from late joiners and from future recaps.
- [Protocol-4 viewers miss links.] → The rule is the same as for every protocol change (one version older watches read-only). They see everything else.
- [Merge conflicts with #55 over the protocol version and `NEW_COMMANDS`.] → Only additions here. Decision 3 says how the versions combine.
- [Two sharing tests used `MIN_VIEWER_PROTOCOL_VERSION` as "an older viewer that can't read whole files".] → With 4 as the minimum, every admitted viewer reads whole files. `an_older_viewer_watches_without_receiving_newer_messages` now checks the link messages instead, and the whole-files test no longer has an older viewer. The host's `FileText` check stays, though no admitted viewer is below it any more.
- [Masking after `#` misses keys in the query (`?key=`).] → The mark can be set by hand. The card masks only the `#` part, because that part never reaches the site's server and is where tools like Excalidraw put their key.

## Migration Plan

Ships with the next release, and nothing is stored on disk. A session process of the previous release keeps working without links (decision 5). Rollback means releasing without the change: protocol-5 viewers are then refused by a protocol-4 host and told the host needs to update, as with any protocol change.
