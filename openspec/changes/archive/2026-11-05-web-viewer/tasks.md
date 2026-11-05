# Tasks

## 1. Every message fits in 256 KiB

- [x] 1.1 Measure the largest data-channel message the host sends (design, Risks): a history batch after a long session, and a timeline event with a large diff (a Write of a 1 MB file inside the session folder). Do it with a throwaway Chrome page that joins through `just local-dev`, or by logging message sizes in `sharing.rs` in a test, and compare with the `a=max-message-size` that Chrome offers in its SDP. Record the numbers in this task. If any message is larger than Chrome accepts, stop and update design.md with message splitting before going on.
  - 2026-11-04, measured with a throwaway test encoding frames with `SessionKey::encode`:
    - a history batch of 256 KiB of incompressible output: **262,492 bytes**;
    - the same batch of ordinary text output: 481 bytes;
    - a timeline event for a created file of 1.15 MB: **1,229,150 bytes**. The diff of a created file has no size cap (`added_file` in `crates/core/src/timeline.rs`).
  - The limit is 262,144 bytes (256 KiB). That is libdatachannel's own default (`DEFAULT_LOCAL_MAX_MESSAGE_SIZE` in `internals.hpp`), and Chrome uses the same value; it has not been checked against a live Chrome SDP yet.
  - Both measured cases are over it. libdatachannel's send refuses with "Message size exceeds limit". `sharing.rs` then disconnects the viewer, and the viewer reconnects and is sent the same event again. So this already affects desktop viewers today, not only browsers.
  - Stopped here, as the task says. The user chose to cap message sizes on the host (design decision 8).
- [x] 1.2 Add `MAX_FRAME_BYTES` and lower `READ_BYTES` to 192 KiB (design decision 8). Verify with a test in `frames.rs` that a 192 KiB incompressible history batch encodes under `MAX_FRAME_BYTES`, and that `cargo test -p specdeck-core --test sharing` still passes.
- [x] 1.3 Add `truncated` to `TimelineEvent` and `timeline::fit`, as design decision 8 describes. Verify with tests in `timeline.rs`:
  - a 1.15 MB created file is cut to whole lines under the budget, with `truncated` set and the hunk's line counts matching its lines;
  - documents, text entries, reports and tool calls each end under the budget;
  - an event that fits comes back unchanged, and serializes without `truncated`;
  - JSON with `"truncated": true` parses through `Entry` as before.
- [x] 1.4 Call `fit` in `session.rs`'s `emit_entry`. In `pump`, skip an event whose frame is still over the limit instead of failing. `specdeck-core` has no logger, so a comment explains the skip. Verify with a test in `crates/core/tests/sharing.rs`: a viewer stays connected through a 1.15 MB file change and receives it with `truncated` set.
  - The test passes. With `fit` turned off, the event is skipped by the guard and the test fails waiting for it, so the test does see the oversized event.
- [x] 1.5 Show the note in the UI: `truncated` in the TypeScript `TimelineEvent` type, and "Shown in part: too large to send in full" on cut entries, under the last hunk in `DiffView`. Verify with tests in `DiffView.test.tsx` and the timeline tests, then run `pnpm typecheck` and `pnpm test`.

## 2. The shared viewer core (`crates/protocol`)

- [x] 2.1 Move the pure viewer logic into `crates/protocol/src/viewer_state.rs` as `ViewerState` (design decision 1): room and key choice, `Hello`, signing the offer and checking the answer, `on_control`, `on_stream` and resend tracking. Move `handle_frame`'s tests from `crates/core/src/viewer.rs` with it, and verify they pass with `cargo test -p specdeck-protocol`.
- [x] 2.2 Make `crates/core/src/viewer.rs` use `ViewerState`, keeping only the connection loop and its public types. Verify `cargo test -p specdeck-core` passes, including `--test sharing` (join, late join, reconnect, new code, removal, viewer limit).
- [x] 2.3 Move `encode_records` from `app/src-tauri/src/terminal.rs` to `crates/protocol`, with its test, and use it from the app. Verify `cargo test --workspace` passes and `stream.test.ts` still passes with `pnpm test`.
- [x] 2.4 Build `crates/protocol` for `wasm32-unknown-unknown` (design decision 2):
  - `ruzstd` for history decompression on `wasm32` only;
  - `getrandom`'s browser backend.

  Verify:
  - `cargo build -p specdeck-protocol --target wasm32-unknown-unknown` succeeds;
  - a new test decodes a `zstd`-compressed history frame, including the 1.1.0 fixture in `frames.rs`, through the `ruzstd` path on the native target;
  - `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] 2.5 Add `InviteCode::web_link`, and teach `parse_join_link` the `https://…/join#CODE` form (design decision 6). Verify with tests in `invite.rs`:
  - round trips with and without a path prefix, and with `ws://` to `http://`;
  - a web link gives its service as `wss://…/signal`;
  - `https://example.com/7K2M-9QXH-4TPA`, without `/join` and `#`, is still refused;
  - a code in the query instead of the fragment is refused.

## 3. The WebAssembly package (`crates/web`)

- [x] 3.1 Create `crates/web` (`specdeck-web`, `cdylib`) with `wasm-bindgen`, exposing `WebViewer` over `ViewerState`:
  - control and stream updates as plain objects. Participants, status and timeline events have the same shape as in the app's `ViewEvent`; mapping updates to `ViewEvent`s and `EndReason`s is the web bridge's job (4.2);
  - records as a `Uint8Array` in the `encode_records` format;
  - `parseJoinLink` for both link forms.

  Pin `wasm-bindgen` exactly. Verify that `just web-wasm` produces `crates/web/pkg`, and that the `wasm-bindgen-test`s in Node (`just web-test`) check these: signing an offer, accepting the matching answer, and rejecting one signed with another code or for another connection.
  - Done: 5 tests pass in Node. The crate also builds natively, so it stays in `cargo test --workspace`. Its tests are `#![cfg(target_arch = "wasm32")]`.
- [x] 3.2 Add `just web-wasm` to build the package, checking that the `wasm-bindgen` CLI is the version the crate pins (no `wasm-pack`, design decision 2), and `just web-test`. Verify the recipe runs from a clean checkout after `just install`, and add the required tools to `mise.toml` and `flake.nix` if they are missing there.
  - Done:
    - `mise.toml` now has the `wasm32-unknown-unknown` target and `cargo:wasm-bindgen-cli` 0.2.130;
    - `.cargo/config.toml` sets the test runner;
    - `flake.nix` has no dev shell, only the app package, so it needs nothing.

    `just web-wasm` built `crates/web/pkg` (a 695 KB `.wasm`).

## 4. The browser connection and web bridge (`app/ui`)

- [x] 4.1 Write `src/web/connection.ts` (design decision 3): the WebSocket, TURN credentials, `RTCPeerConnection` with the `control` and `stream` channels, the 20-second join timeout, back-off, rejoin with the resume token, `Leave` and `pagehide`. Every protocol decision is a call into `WebViewer`. Verify with vitest tests using fake `WebSocket` and `RTCPeerConnection` objects:
  - an invalid code ends with `invalid-code`;
  - no connection within 20 seconds ends with `network-blocked`;
  - a dropped connection after admission sends `reconnecting`, then rejoins through the rejoin room;
  - an answer that fails the check is not applied.
- [x] 4.2 Write `src/web/bridge.ts` (design decisions 4 and 7):
  - a profile from `localStorage`, with every access in `try/catch`;
  - `joinSession` and `leaveSession` over the connection;
  - the `service-outdated` end reason, with its message added where the UI shows `EndReason`s;
  - non-viewer methods reject.

  Verify with vitest tests:
  - the name and view persist across bridge instances;
  - a throwing `localStorage` still gives a working profile;
  - `Refused(ViewerOutdated)` maps to `service-outdated`;
  - `HostOutdated` maps to `host-update`.
  - Done: 6 bridge tests. The refusal mapping is `refusalReason` in `connection.ts`, because the connection ends the session with it; it is tested in `connection.test.ts`, with the other refusals. The bridge also refuses a join through another service, and reads both link forms.
- [x] 4.3 Add the web entry: `web.html`, `src/web/main.tsx` and `WebApp` with the join screen. The join screen has:
  - a code and name form;
  - the code read from `location.hash`, then cleared with `history.replaceState`;
  - **Open in the SpecDeck app**;
  - the "This invite is for …" message for other services' links.

  `ViewerScreen` is reused. Add `pnpm build:web`, writing `dist-web`. Verify with tests in `WebApp.test.tsx`:
  - a hash code is filled in and removed from the address;
  - a link for another service does not call `joinSession`;
  - the page shows no host controls.

  Also check that `pnpm build` (the Tauri entry) does not include `src/web`.
- [x] 4.4 Add the web keyboard handling and its `ShortcutsDialog` set: ⌘F or Ctrl+F, Alt+1/2/3, and **?**. Make terminal links open in a new tab with `noopener`. Verify with tests that Alt+2 opens Files and that the dialog lists the web keys. Run `pnpm lint`, `pnpm typecheck` and `pnpm test`.
- [x] 4.5 From the user's test in `local-dev` (design decision 7):
  - LibreWolf ended with "Your network does not allow the connection", while Chromium worked;
  - the terminal text was very large in a wide browser window.

  Two changes:
  - **`webrtc-off`:** a browser without `RTCPeerConnection` is told at once that it blocks the connection;
  - **a 16 px ceiling** (was 48 px) for a viewer's scaled terminal font, in `terminal/fontScale.ts`.

  Verified:
  - tests in `connection.test.ts` and `fontScale.test.ts`;
  - Playwright Firefox with `media.peerconnection.enabled=false` showed the message 50 ms after selecting **Watch in this browser**;
  - the browser smoke test still passes in all three engines.

## 5. The service serves the page (`crates/signalling`)

- [x] 5.1 Add the `web-viewer` feature with `rust-embed` over `app/ui/dist-web`, and the routes and headers of design decision 5:
  - `/` and `/join`;
  - `/assets/*`, with `immutable`;
  - `/web-viewer.json`;
  - the CSP with the request's host, `no-referrer`, `nosniff` and COOP.

  Verify with tests in `crates/signalling/tests/` (with the feature, against a small fixture `dist-web`):
  - the headers are on `/` and `/join`;
  - `/web-viewer.json` gives the protocol version;
  - unknown paths give 404;
  - `/signal` and `/health` are unchanged.

  Also check that without the feature `/` gives 404, as today.
- [x] 5.2 Extend `docker/signalling.Dockerfile` with the `wasm`, `ui` and `build` stages. Verify that `docker build -f docker/signalling.Dockerfile .` succeeds, that the final image still runs as UID 10001 with one binary, and that `curl` against a container of it serves `/`, `/web-viewer.json` and `/health`.
  - 2026-11-04: the Dockerfile and `.dockerignore` are written.
    - Locally, the `wasm` stage builds.
    - The `ui` stage cannot run on this machine: its network blocks `registry.npmjs.org` (TLS reset, also with `curl`), while crates.io works. The developer uses a company registry instead.
    - The full build and the `curl` checks are therefore left to CI's new `image` job (5.3); tick this when it passes.
  - Later the same day: the npm steps mount an optional BuildKit secret `npmrc` at `/root/.npmrc`. It never lands in a layer, and without it the public registry is used, as in CI.
    - Built locally with `--secret id=npmrc,src=$HOME/.npmrc`.
    - The image runs as user 10001, with only `specdeck-signalling` in `/usr/local/bin`.
    - Against a container: `/health` 200; `/web-viewer.json` `{"protocol":2}`; `/` 200 `text/html` with the CSP.
    - The browser smoke test against the container passes in Chromium, Firefox and WebKit.
    - `just local-up` passes the secret when `~/.npmrc` exists. Its expansion was checked on macOS's bash 3.2.
    - The Compose guide documents the option.
  - `.dockerignore` fix: its patterns match from the root only, so `target/` and `node_modules/` did not cover the worktrees under `.claude/worktrees/`.
    - Building from the main checkout sent more than 15 GB of context (33 GB of worktrees on disk).
    - It now excludes `.claude/` and uses `**/target/`, `**/node_modules/` and `**/dist/`.
    - Measured from the main checkout with the new file: 4.4 MB.
- [x] 5.3 CI: build the wasm package and `dist-web`, run the `wasm-bindgen-test`, check the service with the feature, and build the Docker image. Verify on the PR that the new steps run and pass. Add `just check-web` for the web build.
  - Written:
    - `just check-web` runs `web-wasm`, `web-test`, clippy for `wasm32`, `build:web`, and clippy for the signalling crate with `web-viewer`;
    - `ci.yml` runs it on Linux only, because its recipes use bash and the result is the same on every platform;
    - a new `image` job builds the image and checks UID 10001, `/health`, `/web-viewer.json`, `/join` and the CSP header.

    `just check-web` passes locally; tick this when the PR's run passes. It is not part of `just check`, which also runs on Windows.
  - 2026-11-05, PR #27 (run 41983262413): the Ubuntu job's "Web viewer" step and the `image` job, which builds the image and serves the page, both passed. Every other check passed too: macOS, Windows, nix and the docs build.

## 6. The host's link (`app/src-tauri`, `app/ui`)

- [x] 6.1 When hosting starts, check `<web base>/web-viewer.json` from Rust, waiting at most 3 seconds, and carry `webViewer` in the host state. `joinLink` in `model/code.ts` then gives the web link or the `specdeck://` one (design decision 6). Verify with tests:
  - a Rust test against a local service with the feature, and one without it;
  - `code.test.ts` for both link forms;
  - a `HostScreen` test that **Copy join link** copies the web link when `webViewer` is set.
- [x] 6.2 Make the app's join field and `read_link` accept the web link, including the ask-first dialog for another service. Verify with tests in `app/src-tauri/src/lib.rs`, next to `a_join_link_names_its_service_only_when_it_differs`.

## 7. Docs

- [x] 7.1 Update the docs:
  - `install.md`: "Can't install? Watch in your browser", the supported browsers, and that hosting needs the app;
  - `guide/join.md`: joining in a browser, the web link and its two buttons, and the web shortcuts;
  - `self-host/overview.md` and `docker-compose.md`: the service serves the page, the image build needs nothing extra, and who serves the page's code;
  - `README.md`: a line about watching in a browser.

  Verify that `just docs-build` succeeds.

## 8. Check it end to end

- [x] 8.1 Moved to `release-and-verification` 5.15: In `just local-dev`, open `http://127.0.0.1:18081/` in Chrome, Firefox and Safari, and join a Claude Code session hosted by the app. In each browser, check:
  - the terminal and the timeline;
  - a late join after some output;
  - Overview and Developer views;
  - search and copy;
  - a reconnect after restarting the signalling pod;
  - the session ending;
  - the host's participant list after closing the tab.

  Repeat once in relay-only mode (direct connections blocked). Record the results in the PR.
  - 2026-11-04, automated, without the app or coturn:
    - **Setup:**
      - the service, built with `web-viewer`, on 127.0.0.1;
      - a host started from `specdeck-core` with a fake Claude Code (a throwaway example, removed again), bound to loopback;
      - Playwright's Chromium, Firefox and WebKit, allowed to use loopback candidates, because this machine's sandbox blocks LAN UDP.
    - **Passed in all three:**
      - joining a session that was already running: late join, with history from its first line;
      - the terminal and the timeline (prompt and answer);
      - the Developer view;
      - the People tab listing host and viewer;
      - **Leave** removing the viewer from the host's list at once.
    - **Found and fixed:** bytes from WebAssembly arrive as plain arrays of numbers, which broke the terminal stream ("First argument to DataView constructor must be an ArrayBuffer"). `connection.ts` now copies them into a `Uint8Array`, with a test.
    - **Earlier, with no session:** all three load the page from the service, run the WebAssembly core, and show "That code does not work".
    - **Still to do here, by hand with the app:** Overview, search and copy, reconnect after a signalling restart, the session ending, closing the tab, and relay-only mode.
    - **Note:** `just local-up` builds the signalling image, which now needs `registry.npmjs.org`, and that registry is blocked on the developer's network (see 5.2).
- [x] 8.2 In DevTools' network panel during 8.1, verify that the page makes no request outside its own service and the relay, and that no request contains the invite code.
  - Checked with Playwright, recording every request and WebSocket, in Chromium, Firefox and WebKit, both while joining a running session and with a code that has no session. In each:
    - 0 requests contain the code;
    - 0 go to another host;
    - the code is removed from the address bar after the page reads it;
    - there are no security policy errors.
- [x] 8.3 Run `just check` and verify it passes.
  - 2026-11-04: passes. All Rust tests, clippy and fmt pass; the UI has 225 tests, typecheck and lint pass. `just check-web` passes as well.

## Workflow follow-up

- After merge, archive with `/opsx:archive web-viewer`, and update the security docs (hardening plan, change 5) with the web viewer's trust model.
