# Proposal

## Why

Everyone in a session needs the SpecDeck app today, and some people can't install apps at all: company laptops that block unsigned or unapproved software, or allow no installs. They can only watch, since hosting needs an agent installed too. A browser runs everything a viewer needs (WebRTC, WebSockets, the terminal and the UI), so they can watch from a web page.

## What Changes

- **A web viewer**: a page that joins a session with an invite code and shows it as the app's viewer does: terminal, timeline, files, people, status bar, follow and scroll, search, copy, and the Overview and Developer views. Watching only; hosting stays in the desktop app.
- **Served by the sharing service itself.** The page is built into the signalling service, so the public service (`specdeck.hartj.es`) serves the main site and every self-hosted service serves its own copy. Nothing else needs to be deployed.
- **Each page joins only sessions on its own service.** A page never connects to another service, and the browser enforces that.
- **One join link for everyone.** The host's **Copy join link** gives `https://<service>/join#CODE` when the service has the web viewer. In a browser it offers **Open in the SpecDeck app** and **Watch in this browser**. The app's join field accepts it too. The code sits after `#`, so it is never sent to the server. Services without the web viewer (older self-hosted ones, local development) keep the `specdeck://` link.
- **No account, nothing stored on the server**: the viewer's name and view settings stay in the browser.
- **The protocol code is shared**: the frame checks, signatures, history decoding and the viewer's ordering and resend logic move into a part of `crates/protocol` that compiles to WebAssembly. The desktop viewer uses the same code.
- **Clear messages for browser cases**: a network that blocks the connection, and a page older than the host's app ("the sharing service needs to be updated", not "update your app").
- **Every message fits in 256 KiB**, the limit of both the app's WebRTC library and browsers. Measuring this showed a bug the app already has: a created file of 1.15 MB gives a 1.2 MB timeline message, the host fails to send it, drops the viewer, and the viewer reconnects into the same message. The host now cuts a timeline entry that is too large to send whole and marks it as shown in part, and sends terminal history in smaller batches.
- **Docs**: the install page and README offer "Can't install? Watch in your browser", and the self-host docs say the service now also serves the page.

Out of scope, each its own item in `todo.txt` or the hardening plan:

- hosting in a browser;
- TURN over TLS on port 443, for networks that block UDP;
- a published container image and a Helm chart;
- browser notifications.

## Capabilities

### New Capabilities

- `web-viewer`: watching a session in a browser. How the page is served, that it joins only its own service's sessions, which viewer features it has, where it keeps the viewer's name and settings, and how it reports browser-specific failures.

### Modified Capabilities

- `session-sharing`: the requirement "Join link" changes. The host's link opens in a browser, offers the app or the web viewer, and is accepted by the app. A service without the web viewer keeps today's `specdeck://` link.
- `session-timeline`: a new requirement, "Entries too large to send whole". An entry is cut and marked as shown in part, and viewers stay connected.

## Impact

- **`crates/protocol`**:
  - builds for `wasm32-unknown-unknown`, with `getrandom`'s browser backend;
  - history frames are decoded with a pure-Rust zstd decoder (`ruzstd`) in the WebAssembly build;
  - a new sans-IO viewer state (ordering, resends, Welcome handling) is taken out of `crates/core/src/viewer.rs`;
  - `InviteCode` gets the web link.
- **New `crates/web`**: `wasm-bindgen` bindings to the protocol code, built with the `wasm-bindgen` CLI.
- **`crates/core/src/viewer.rs`**: uses the shared viewer state; its behavior is unchanged.
- **Message size**:
  - `crates/protocol` gets a frame size limit and a function that fits a timeline event to it, with an optional `truncated` flag that older viewers ignore;
  - `crates/core/src/session.rs` applies it to every entry;
  - `sharing.rs` reads smaller history batches;
  - `app/ui` shows the "shown in part" note.
- **`app/ui`**:
  - a new `web.ts` bridge, built on the browser's `WebSocket` and `RTCPeerConnection`;
  - a web build entry that shows only the join and viewer screens;
  - browser-safe shortcuts;
  - the join page with its app/browser choice.
- **`crates/signalling`**:
  - serves the built page, embedded in the binary, at `/` and `/join`, with a strict Content-Security-Policy;
  - a small `/web-viewer.json` endpoint that says the page is there and which protocol it speaks.
- **`app/src-tauri`**: the host checks `/web-viewer.json` to choose the link; the join field reads the https link.
- **Builds**:
  - `docker/signalling.Dockerfile` gains Node and WebAssembly build stages;
  - `justfile` and CI build and test the web viewer;
  - the Compose and Kubernetes files stay the same.
- **Docs**: `install.md`, `guide/join.md`, `self-host/overview.md`, `README.md`.
