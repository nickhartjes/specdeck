# Proposal

## Why

People in a session often need to look at the same page: a preview deploy, staging, a ticket, or a live whiteboard. Today they have to paste the address into a chat outside SpecDeck, and people who join late never see it. This is issue #66. It is small, needs no new connection type, and is the first use of the viewer channel (#51) besides suggestions. It also makes drawing together possible: the host shares an Excalidraw room and everyone draws in their own browser, while Excalidraw encrypts the drawing end to end.

## What Changes

- **Share a link.** The host shares a link, with an optional title, from the timeline panel. It appears as a link card in everyone's timeline, with the title, the address, who shared it and when. Late joiners get every link, because the card is a timeline entry.
- **Viewers share through the host.** A viewer's link is a request on the viewer channel (#51), like a suggestion: only the host and that viewer see it until the host shares or declines it. The host has a switch for viewers' links, on when a session starts, as for suggestions. The same rate limit applies.
- **Opens in the default browser, never in the app.** A click on *Open in your browser* opens the link in the participant's default browser, so they see it with their own logins. The app and the web viewer never load the page, fetch its title or show a preview. The app opens only links that are in the session; the window passes the link's id, never an address.
- **Which addresses.** Only `https://`, and `http://` for addresses that are not on the sharer's own machine. Loopback addresses (`localhost`, `*.localhost`, `127.0.0.0/8`, `::1`, `0.0.0.0`) are refused for both schemes, with a hint that opening the host's app to viewers comes with #68. Addresses with a user name or password (`https://user:pass@…`) are refused too. The host checks every link again, whoever sent it.
- **For this session only.** A link can be marked *for this session only*. The mark starts on for every link with a `#` part, such as an Excalidraw room (`#room=<id>,<key>`), and the sharer can turn it off before sharing. For now the mark means: the card shows the address masked after the `#`, and *Copy link* warns that it holds a key. When recaps and exports (#57) come, they leave such links out. The mark can't be changed once the link is shared.
- **The host can remove a link.** The card then says the host removed it, without the address, and late joiners never get the address. People who already saw it still have it, and the window says so before the host removes it.
- **Sessions without a timeline.** In terminal, OpenCode and omp sessions, the timeline panel keeps its note that there is no timeline and lists the shared links below it.
- **Web viewer:** the same cards and controls. A link opens in a new browser tab with no access back to the viewer page.
- **Protocol change.** A new timeline entry for a link and one for its removal, a new request kind, and new host messages for the switch and for a viewer's own links. The session protocol goes from 4 to 5. Hosts keep admitting viewers on protocol 4. Those viewers never get the new entries or messages, so they don't see links and can't share them, and otherwise watch as today. Viewers on protocol 3 are no longer admitted. **BREAKING** for viewers two releases behind.

Out of scope: the *Shared* tab and its combined *Add files, a note or a link* dialog (#55), which can take over the share control later. Also out of scope: recaps and exports (#57), notifications for new links (#59), opening the host's dev server (#68), and editing a viewer's link before sharing it.

## Capabilities

### New Capabilities

- `shared-links`: sharing a link with everyone in a session. It covers which addresses are accepted, the link card, opening it in the default browser only, *for this session only*, removal by the host, links in sessions without a timeline, and viewers on the previous protocol.

### Modified Capabilities

All of these only gain requirements, so that changes made in parallel to the same specs merge cleanly:

- `viewer-requests`: *Share a link* (a viewer's link is a request the host shares or declines) and *Host allows viewers' links* (the switch).
- `session-timeline`: *Links in the timeline*: links are timeline entries that late joiners receive, and removing one hides its address from then on.
- `web-viewer`: *Links in the browser*: the same cards and requests, and a link opens in a new tab with no access back to the viewer page.

## Impact

- **Protocol (`crates/protocol`):** `PROTOCOL_VERSION` 5, `MIN_VIEWER_PROTOCOL_VERSION` 4, and `LINKS_PROTOCOL_VERSION = 5`. Entries `Entry::Link` and `Entry::LinkRemoved`. A request `ViewerRequest::ShareLink`. Host messages `LinksAllowed` and `Links`. One function checks a link, built on the `url` crate (already in `Cargo.lock`, and it builds for wasm).
- **Host (`crates/core`):** the session keeps its links and redacts a removed link's stored entry. `sharing.rs` holds the new entries and messages back from protocol-4 viewers.
- **Session process and app (`crates/session`, `app/src-tauri`):** commands `share-link`, `answer-link`, `remove-link` and `allow-links`. `adapter.rs` adds the new commands to `NEW_COMMANDS`. A Tauri command `open_shared_link(id)` uses the existing `tauri-plugin-opener`, from Rust only.
- **UI (`app/ui`):** a link card in `TimelinePanel`, a bar above the timeline with the share dialog, the host's waiting links and a viewer's own links, and the links switch in `SharingSettings`. The same in the web viewer, which checks links with the browser's own URL parser to keep the `url` crate out of its WebAssembly.
- **Docs:** the host and follow guides say what a shared link carries, and that SpecDeck never loads it.
- **Parallel work:** #55 (shared files) will probably also raise the protocol to 5. If both ship in one release, they share version 5 and its constants. Whichever change merges second resolves the version lines in `control.rs`, `NEW_COMMANDS` in `adapter.rs`, and `SharingSettings`. This change only adds lines there.
