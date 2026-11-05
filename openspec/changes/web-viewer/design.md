# Design

## Context

See proposal.md for why, and the `web-viewer` and `session-sharing` deltas for the behavior.

What is there today:

- **The viewer** (`crates/core/src/viewer.rs`, ~530 lines) mixes two things:
  - the connection loop: signalling, TURN credentials, the peer connection, timeouts and back-off, and the rejoin room with the resume token;
  - pure logic: `handle_control` (Welcome, Refused, Participants, Status, SessionEnded), `handle_frame` (verify, order, resend) and `request_resend`.
- **The peer** (`peer.rs`) uses `datachannel` (libdatachannel). The viewer offers and opens two data channels, `control` and `stream`. Remote candidates wait until local gathering is complete, so a relayed connection does not get stuck.
- **`crates/protocol`** depends on `ed25519-dalek`, `hkdf`, `hmac`, `sha2`, `postcard`, `serde_json`, `getrandom` and `zstd`. `zstd` is a C library, and is only needed to decompress history frames on the viewer's side.
- **The Tauri glue** (`app/src-tauri/src/viewer.rs`) turns `ViewerEvent` into the UI's `ViewEvent` and `EndReason`. Terminal records go to the window as a binary stream (`terminal.rs` `encode_records`, decoded by `src/terminal/stream.ts`).
- **The UI** reaches the Rust side only through `Bridge` (`app/ui/src/api/bridge.ts`), which has two implementations, `tauri.ts` and `demo.ts`. `main.tsx` picks one.
  - The host's join link is built in the UI: `joinLink(code, service)` in `model/code.ts`, used by `HostScreen`.
  - Links have the form `specdeck://join/CODE?server=<wss url>` (`InviteCode::join_link`, `parse_join_link`).
- **The signalling service** (`crates/signalling`) is an axum router with `/signal` and `/health`.
  - It is built by `docker/signalling.Dockerfile`, a single Rust stage, and runs behind Caddy (Compose) or an ingress (Kubernetes) that passes every path through.
  - Local development reaches it at `http://127.0.0.1:18081` through `just local-dev`'s port-forward.
- **The host sends history in batches**: up to 256 KiB of records per message (`READ_BYTES` in `sharing.rs`). Timeline events, which can carry large diffs, go as single messages. The diff of a created file has no size cap. Documents and helper reports are capped at 200 KiB, and shell diffs at 2,000 lines.

## Goals / Non-Goals

**Goals:**

- One implementation of everything that decides what a viewer trusts and shows (verification, ordering, resends), shared by the app and the page.
- A page whose code and data come only from its own service, enforced by the browser.
- The desktop app and the service work as before when the web viewer is not built.

**Non-Goals:**

- A web-only UI. The page reuses `app/ui`'s viewer screens.
- Offline use or installing the page as an app (PWA).
- Supporting browsers without WebRTC data channels and WebAssembly. The page supports the current Chrome, Edge, Firefox and Safari.

## Decisions

### 1. A sans-IO viewer core in `crates/protocol`

The pure half of `viewer.rs` moves to a new `crates/protocol/src/viewer_state.rs` as `ViewerState`:

- `room_and_key()`: the code's room and HMAC key, or the rejoin room and resume token after the first Welcome;
- `hello(protocol_version, username)`;
- `sign_offer` and `verify_answer`;
- `on_control(bytes)`, returning a viewer event or an outcome (final, retry);
- `on_stream(bytes)`, returning records or a timeline event, plus an optional `Resend`.

`crates/core/src/viewer.rs` keeps only the connection loop and calls `ViewerState`. Its existing tests move with the logic, and its event types stay the same. `encode_records` moves from `app/src-tauri/src/terminal.rs` to `crates/protocol`, so the app and the page produce the same terminal stream for `stream.ts`.

- **Alternative: port everything to TypeScript.** That would make two implementations of the security-relevant parts, which could drift, and the parked `harden-session-protocol` change would need SPAKE2 in TypeScript too.

### 2. `crates/web`: the core as WebAssembly

A new crate, `crates/web` (`specdeck-web`, `cdylib`), wraps `ViewerState` with `wasm-bindgen`:

- it exposes a `WebViewer` class with the methods of decision 1;
- events cross as plain objects (`serde-wasm-bindgen`), in the same shape as the app's `ViewEvent`, so the UI code does not change;
- terminal records cross as an `ArrayBuffer` in the `encode_records` format.

It is built by `just web-wasm`: `cargo build --target wasm32-unknown-unknown`, then `wasm-bindgen --target web` into `crates/web/pkg`. The `wasm-bindgen` crate is pinned exactly and the CLI comes from `mise.toml` at the same version; the recipe refuses to run when they differ. `wasm-pack` is not used: it only wraps these two steps and downloads its own CLI. `.cargo/config.toml` sets `wasm-bindgen-test-runner` as the runner, so `just web-test` runs the WebAssembly tests in Node.

To build `crates/protocol` for `wasm32-unknown-unknown`:

- **zstd**: on `wasm32` only, history frames are decompressed with `ruzstd`, a pure-Rust decoder, and the native build keeps `zstd`. A test decodes a frame compressed by `zstd` with the `ruzstd` path, on the native target, so the two cannot drift.
- **randomness**: `getrandom` uses its browser (`wasm_js`) backend on `wasm32`, configured as the pinned version's docs require. The viewer itself draws no random numbers today, but the crate depends on `getrandom` (key generation for hosts), and the parked SPAKE2 change will need it on the viewer's side.

### 3. The connection loop in TypeScript, on the browser's WebRTC

`app/ui/src/web/connection.ts` ports `Viewer::run` and `attempt`:

- the WebSocket to the service, and `TurnCredentials` before `Join`;
- an `RTCPeerConnection` with the service's TURN servers as `RTCIceServer` entries;
- the viewer creates the ordered, reliable `control` and `stream` channels, as `peer.rs` does;
- the same 20-second connect timeout, back-off from 250 ms to 5 s, and rejoin with the resume token.

Each step's decision (what to sign, whether an answer is valid, what a control message means) is a call into `WebViewer`. Remote candidates are added as they arrive; the browser queues them itself, so the native "wait for gathering" workaround is not copied. Leaving sends `Leave` on `control`, and `pagehide` closes the connection, so the host sees the viewer go.

- **Alternative: WebRTC in WebAssembly.** Not possible: a page has no UDP sockets, only `RTCPeerConnection`.

### 4. The web bridge and a viewer-only entry

- **A second Vite entry, `app/ui/web.html` with `src/web/main.tsx`,** renders a `WebApp`. It has a join screen (code and name, plus **Open in the SpecDeck app**) and reuses `ViewerScreen` unchanged.
- **`src/web/bridge.ts` implements `Bridge`:**
  - `joinSession`, `leaveSession`, `readJoinLink`, `copyText`, `setUsername` and `setView` are real;
  - `getProfile` returns a profile built from browser storage, with notifications off and no update support;
  - host, folder, agent and update methods reject, because the web UI never shows them.
- **`pnpm build:web`** writes `app/ui/dist-web`. The Tauri build and its entry are untouched and do not include the web code.
- **Storage**: the name and view sit in `localStorage` under `specdeck.username` and `specdeck.view`, with every access in `try/catch` (spec: private windows).
- **Shortcuts**: the native menu does not exist, so `WebApp` handles keys itself:
  - find stays **⌘F / Ctrl+F**;
  - panel tabs move to **Alt+1/2/3**, because browsers keep Ctrl+1–3 for their own tabs;
  - the shortcuts list opens with **?**;
  - zoom is left to the browser.

  `ShortcutsDialog` shows the web set when it runs in the page.

### 5. The service serves the page

- **Embedded in the binary.** `crates/signalling` gets a cargo feature, `web-viewer`. With it, `rust-embed` builds `app/ui/dist-web` into the binary. Without it, the service behaves exactly as today, so `cargo test --workspace` needs no Node build.
- **Routes:**
  - `/` and `/join` serve `web.html`, with `Cache-Control: no-cache`;
  - `/assets/*` serves hashed files, with `immutable`;
  - `/web-viewer.json` serves `{"protocol": <PROTOCOL_VERSION>}`;
  - anything else is 404. `/signal` and `/health` are unchanged.
- **Headers on every page response:**
  - `Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' wss://<host> ws://<host>; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`. `<host>` is the request's `Host`; explicit `ws(s)` sources cover browsers where `'self'` does not match WebSockets.
  - `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `Cross-Origin-Opener-Policy: same-origin`.
- **The page's own service is fixed.** The page derives its signalling URL from its own address, `new URL('signal', location)` with `ws`/`wss` for `http`/`https`, and has no setting to change it. The CSP's `connect-src` makes any other WebSocket fail. WebRTC is not covered by CSP, so the page only uses the ICE servers its own service returns.
- **The Docker build** gets three stages:
  1. `wasm`: Rust with the `wasm32` target and the `wasm-bindgen` CLI, building `crates/web/pkg`;
  2. `ui`: Node with pnpm, running `build:web` with that package;
  3. `build`: `cargo build --release --locked -p specdeck-signalling --features web-viewer`.

  The final image is unchanged: one binary, non-root, read-only.

- **Alternative: plain files in the image, or a separate web server.** That would make a version mismatch between the page and the service possible, and give self-hosters one more thing to run and configure.

### 6. One join link

- **`InviteCode::web_link(service)`** maps `wss://host[:port]/prefix/signal` to `https://host[:port]/prefix/join#CODE`, and `ws://` to `http://`.
- **`parse_join_link`** also reads that form and gives the service back as `wss://…/prefix/signal`. It only accepts paths ending in `/join`, with the code in the fragment.
- **Which link the host copies:**
  - When hosting starts, the app fetches `<web base>/web-viewer.json`, waiting at most 3 seconds, from Rust (`services.rs`), so the window's CSP stays as it is.
  - When it answers, the host state carries `webViewer: true`, and **Copy join link** gives the web link. Otherwise it gives the `specdeck://` link as today.
  - `joinLink` in `model/code.ts` takes that flag.
- **On the page:**
  - the code comes from `location.hash`, and is then removed with `history.replaceState`, so it does not stay in the address bar or the browser history;
  - **Open in the SpecDeck app** is a plain `specdeck://join/CODE?server=<this service>` link, with no detection;
  - a pasted link naming another service, as `https://other/join#…` or `specdeck://…?server=…`, is not joined. The page shows "This invite is for other.example.com" with a link to that service's page.

### 7. Messages that differ in the browser

`src/web/connection.ts` maps the outcomes to the existing `EndReason`s, with two additions:

- **`Refused(ViewerOutdated)` becomes a new `EndReason`, `service-outdated`:** "This sharing service's web viewer is older than the host's SpecDeck. Ask whoever runs the service to update it, or watch in the SpecDeck app."
- **A browser without `RTCPeerConnection` ends at once with `webrtc-off`:** "This browser blocks the connection". Browsers with WebRTC turned off in their settings, such as LibreWolf's `media.peerconnection.enabled`, leave it out. Found in testing: LibreWolf failed with `network-blocked` after 20 seconds, which pointed at the network instead of the browser.
- **`HostOutdated`** keeps "The host needs to update".
- **The connect timeout** keeps the existing `network-blocked` message.

A related change, also found in testing: a viewer's terminal scales its text so the host's columns fill the pane, up to a ceiling. The ceiling was 48 px; in a maximized browser, a narrow host terminal became huge. It is now 16 px, a little above the usual 13 px (`app/ui/src/terminal/fontScale.ts`). This applies to the desktop viewer as well.

### 8. Every message fits in 256 KiB

Task 1.1 measured two messages over libdatachannel's and Chrome's 262,144-byte limit:

- a 256 KiB history batch of incompressible output: 262,492 bytes;
- the event for a created 1.15 MB file: 1,229,150 bytes.

libdatachannel refuses to send them, and `sharing.rs` drops the viewer, who reconnects into the same message. The fix keeps the frame format, so it needs no protocol bump and does not collide with `harden-session-protocol`.

- **The limit.** `crates/protocol::frames` gets `MAX_FRAME_BYTES = 256 * 1024`. It is enforced by the guard in `pump` (below), not by an assertion in `encode`, so an oversized frame is skipped rather than crashing a debug build.
- **Terminal history.** `READ_BYTES` in `sharing.rs` goes from 256 KiB to 192 KiB.
  - zstd's worst case for incompressible input adds well under 1%, so a batch stays under the limit.
  - Single records are at most `MAX_BATCH_BYTES` (64 KiB) plus one pty read.
  - A test encodes a 192 KiB incompressible batch and checks it is under the limit.
- **Timeline events.** `TimelineEvent` gets `truncated: bool`, with `#[serde(default, skip_serializing_if = "not")]`, so ordinary events are unchanged on the wire.
  - A new function, `fit(event, budget)`, in `crates/protocol::timeline`, returns the event unchanged when its JSON fits, and otherwise cuts it and sets `truncated`:
    - `FileChange`: keeps whole hunk lines from the start, as many as fit, and fixes the last hunk's line counts;
    - `Document`: drops `text`, which is the existing "too large to show whole" form;
    - text entries (`Prompt`, `Message`, `Response`) and a helper's `Report`: cut on a character boundary;
    - `ToolCall`: cuts each string in `input`, and empties `input` if that is not enough.
  - The budget is 240 KiB of JSON, leaving room for the frame header and signature.
  - `fit` runs once, in `session.rs`'s `emit_entry`. The host's own timeline, the history late joiners get and the live stream are then the same.
- **Older viewers.** The JSON parser ignores fields it does not know (no `deny_unknown_fields`, and the variants are internally tagged), so they parse a cut event and show the part that was sent. A test parses an event with `truncated` through the existing `Entry` type to check this.
- **The UI.** The UI shows "Shown in part: too large to send in full" on any entry with `truncated` set. `DiffView` puts it under the last hunk.
- **A guard in `pump`.** If a frame is over the limit anyway, the host logs it and skips that event instead of dropping the viewer. Viewers accept event sequence gaps (`seq >= next_event`).

- **Alternative: split large messages into parts.** That needs a new frame kind and a protocol bump, and every viewer would have to buffer up to megabytes for one entry nobody reads in full.

## Risks / Trade-offs

- **[A very large change is only shown in part]** → The note says so. The terminal still shows whatever the agent printed, and the file is in the host's folder.
- **[Whoever serves the page can change what it does]** → That is the service's operator, whom the decisions in this change make the only party. The security docs (hardening plan, change 5) state it. Self-hosters build the image from source. A published, signed image is the later `container publishing` item.
- **[The CSP's `connect-src` uses the request's `Host` header]** → It only names the address the page was loaded from. A forged `Host` makes a page that connects to that same forged address, which the browser would have reached anyway.
- **[libdatachannel and browsers disagree on something in SCTP or DTLS]** → libdatachannel is tested against browsers upstream. An end-to-end check in Chrome, Firefox and Safari is a task, with TURN-only (relay) mode too.
- **[A company network blocks UDP]** → The page says the network blocks the connection (spec). TURN over TLS is a separate change in the hardening plan.
- **[The Docker build gets slower and needs Node]** → It only happens on image builds. CI caches the stages.
- **[`ruzstd` and `zstd` diverge]** → The cross-decoding test (decision 2) runs in CI.

## Migration Plan

- **Services without the feature keep working.** The host app falls back to `specdeck://` links when `/web-viewer.json` is missing, so updating the app before the service is safe.
- **Update the service, then the apps.** A self-hosted service gets the page by rebuilding its image. Hosts on an older app keep copying `specdeck://` links until they update.
- **Rollback.** An image without the feature removes the page. Web links already shared then show the service's 404, and the app link still works.

## Open Questions

- The exact versions of `ruzstd` and `rust-embed` are chosen when the crates are added, with Renovate keeping them current.
