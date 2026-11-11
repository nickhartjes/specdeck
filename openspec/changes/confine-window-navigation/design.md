# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. What is there today:

- **Windows.** `open_session_window` and `open_settings_window` (`app/src-tauri/src/lib.rs`) build windows on the app's own page with no `on_navigation` handler. The browser window (`app/src-tauri/src/browser/webkit.rs`, `open`) starts on `about:blank`, also without one. `navigate()` there checks the scheme of the tool's address only.
- **How the session reaches a window.** `client.rs` sends terminal data and host events through Tauri channels. Tauri 2.12.5 delivers a channel message with `webview.eval("window.__TAURI_INTERNALS__.runCallback(…)")` (`tauri/src/ipc/channel.rs`), into whatever document the webview shows. Large messages go through the `plugin:__TAURI_CHANNEL__|fetch` command, which is exempt from the remote-origin check (`tauri/src/webview/mod.rs`, around line 2080). The channels are dropped only when the window closes.
- **Who may call commands.** `build.rs` is `tauri_build::build()`. Without an app manifest, Tauri checks the ACL only for plugin commands and for remote origins, so all 81 `#[tauri::command]`s work from any window on the app's origin, whatever its label. `capabilities/default.json` (`main`, `window-*`) and `capabilities/settings.json` (`settings`) grant plugin permissions only. Two tests already guard the capabilities: `no_capability_covers_browser_windows` (`browser/mod.rs`) and `no_capability_lets_a_window_open_addresses` (`links.rs`).
- **Links.** `Markdown.tsx` renders react-markdown's default `<a href>`; its `urlTransform` already drops `javascript:` and `data:`. Shared links open by id through `open_shared_link`, which reads the address from a Rust-side `LinkBook` and runs `check_link` (`crates/protocol/src/links.rs`). The webview has no opener permission. The web viewer opens shared links with `window.open(url, '_blank', 'noopener,noreferrer')` (`web/bridge.ts`).
- **Join links.** `open_link` (`lib.rs`) stores the link for a window; `takeJoinLink` in `App.tsx` joins at once, except that a link naming another service opens `JoinThroughDialog`.
- **Names.** `decode_viewer` (`crates/protocol/src/control.rs`) refuses control characters only. `program_status.rs` has `invisible()`, which lists the zero-width and text-direction characters, used for agents' reports. `host_observer` (`lib.rs`) puts names into notification titles and bodies as they are.

## Goals / Non-Goals

**Goals:**

- No page but the app's own ever runs in a window that receives a session.
- A window of Claude's browser can call no app command, on any page, and this no longer rests on a comment.
- One place decides whether a content link opens, and it is in Rust.
- No protocol change: hosts and viewers of 1.7 and 1.8 keep working with the new release.

**Non-Goals:**

- Different grants for host and viewer windows. A session window changes role (start screen, host, viewer) without changing its label, so capabilities cannot tell them apart. Refusing host commands in Rust when the window's session is a viewer is a possible follow-up; it needs a look at every command and is not needed to close the findings above.
- Tauri's isolation pattern.
- Previews of linked pages, or a list of trusted sites.
- Changing what `check_link` accepts.

## Decisions

### 1. An `on_navigation` handler on every window the app builds

Two handlers, set where the windows are built:

- **Session and settings windows:** allow a URL only when its scheme, host and port equal those of the app's own page. That is `tauri://localhost` on macOS and Linux, `http://tauri.localhost` on Windows, and the `devUrl` in a debug build. Take the origin from the window's first URL (the handler sees it as the first navigation) or from `app.config()`, not from a hard-coded list. Everything else returns `false`.
- **Browser windows:** allow `http`, `https` and `about:blank`; refuse everything else, including `tauri:`, `file:`, `data:`, `blob:` and `javascript:`.

The handler only refuses. It does not hand the refused address to the system browser: links open through decision 2, so there is one path to the browser and it always carries the check.

*Why not only fix the anchors (decision 2)?* The anchors are the known way in. A handler also covers what is not known yet: a renderer added later, a `window.location` set by an injected script, a meta refresh in content.

*Alternative: detach the session's channels when the window navigates.* Rejected: it leaves the window on a foreign page with the session gone, which is the known limit again.

`window.open` needs nothing: wry denies new windows when no handler is set, and none is.

### 2. Content links open through a new command, checked in Rust

- `Markdown.tsx` gives react-markdown an `a` component. It renders a link with `href` kept (for the address on hover, for copying, and for screen readers), `title` set to the address, and an `onClick` that prevents the default and calls `bridge.openContentLink(href)`.
- The app's bridge calls a new command `open_content_link(url)`. It runs `check_link(url)` and opens the normalized address with `app.opener().open_url`, as `open_shared_link` does. On refusal it returns "This link can't be opened.", which the UI shows next to the link.
- The web viewer's bridge runs `checkLink` from `model/links.ts` and calls `window.open(url, '_blank', 'noopener,noreferrer')`, the same call as for shared links.
- A link without `http` or `https`, a relative link and a bare `#fragment` open nothing. Relative links between documents are not a feature today and are not added here.

*Why the address and not an id, as for shared links?* Shared links are few and pass through Rust as timeline entries, so a `LinkBook` can hold them. Content links sit inside Markdown that Rust never parses. Passing the address means a window that was already taken over could open any web page in the user's browser. That is far less than it can do otherwise (it holds every app command), and `check_link` keeps it to `http` and `https` without credentials.

*Why the same check as shared links, although it refuses `localhost`?* A document often links to `http://localhost:5173`. For the host that link would work; for a viewer it points at their own computer, which is why shared links refuse it. One rule for everyone is simpler to explain and to test than a rule per role, and the address stays visible to copy. If hosts miss it, allowing the host's own machine for the host only is a small later change.

### 3. An app manifest, and capabilities that grant every command by name

- `build.rs` becomes `tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(&[…])))` with all command names. tauri-build then generates `allow-<command>` and `deny-<command>` permissions (command names with dashes, for example `allow-get-profile`), and Tauri starts checking the ACL for app commands on local pages too (`has_app_acl_manifest`).
- From then on a command that no capability allows fails for every window. `capabilities/default.json` lists the commands session windows use; `capabilities/settings.json` lists the few the settings window uses. No capability names `browser-*`, as today.
- tauri-build writes the generated permission files to `app/src-tauri/permissions/autogenerated/`. They are build output: add the folder to `.gitignore`, and check that the Nix build (`nix/package.nix`) and the release build can write there. If the Nix sandbox cannot, commit the generated files instead and have the test below compare them.

**Keeping three lists in step.** The command names are now in `generate_handler![…]`, in `build.rs` and in the capabilities. A missing entry fails at run time, so a test must catch it before:

- Put the names in one file, `app/src-tauri/commands.txt`, one per line. `build.rs` reads it for the manifest (`include_str!` plus a leaked slice, or a generated `const`).
- A unit test in `lib.rs` reads the same file and fails unless (a) every name appears in the `generate_handler!` list, read from the source text of `lib.rs`, and the reverse; and (b) every name is allowed, as `allow-<name-with-dashes>`, by `default.json` or `settings.json`.
- The existing `no_capability_covers_browser_windows` test stays.

*Alternative: check the window label inside each command.* Rejected: 81 call sites to get right, and the next command forgets it. The ACL fails closed.

*Alternative: move commands into an inlined plugin.* Same lists, more renaming.

*Cost.* Every new command now needs a line in `commands.txt` and in a capability. The test names the missing one, so the cost is a minute per command.

### 4. A join link from outside asks in the window that takes it

- `JoinLink` already tells the window whether it names another service. `takeJoinLink` in `App.tsx` stops joining at once: it opens a dialog, *Join this session?*, with the formatted code, the name the viewer joins with, and **Join** and **Cancel**. Cancel returns to the start screen and connects to nothing.
- A link that names another service keeps `JoinThroughDialog`, which gains the code and the name, so there is still one question.
- With no name set, the name step comes first, as today, and the question follows.
- Links pasted into the join field (`join(code, pasted)`) do not ask: pasting is the viewer's own act.
- The web viewer is not changed: its join page already waits for **Watch in this browser**.

The question is asked in the UI, not in Rust, because nothing connects until the UI sets the viewer screen. `open_link` in Rust already only stores the link.

### 5. One filter for names, in the protocol crate

- Move `invisible()` from `crates/core/src/program_status.rs` to `crates/protocol` as `control::shown_name(&str) -> Option<String>`: drops zero-width and text-direction characters, trims, and returns `None` when nothing is left. `program_status::clean` calls the shared function.
- `decode_viewer` refuses a `Hello` whose name is empty after the filter (`InvalidUsername`), as it already does for an empty one.
- The host applies the filter at admission (`sharing.rs`, where it now trims) and to its own name, so the participant list, timeline steps, notices and notifications all get the filtered name. The *(2)* suffix is computed after the filter.
- The app's UI applies the same filter when the user sets a name, so what they type is what others see.

No protocol version changes: the name is filtered where it is received.

### 6. Notification bodies on Linux are escaped

The freedesktop notification spec lets servers render a small set of markup in the body (KDE, dunst and GNOME do). In `notify.rs`, on Linux only, replace `&`, `<` and `>` in the body with `&amp;`, `&lt;` and `&gt;` before showing it. Titles are plain text in the spec and stay as they are. macOS and Windows show bodies as plain text already.

This covers every body, not only names: bodies also carry commands and questions from the agent.

## Risks / Trade-offs

- **A command missing from a capability breaks a feature at run time.** → The test in decision 3 fails in `just check`; the UI tests do not cover it, so the test is the guard. Run the app once through hosting, viewing and settings before release (task 7.2).
- **The app's own origin differs per platform and in dev.** A wrong comparison blanks every window. → Derive the origin instead of listing it, and test the predicate with the three known forms.
- **A Linux notification server without markup support shows `&lt;` literally.** → Only for bodies that hold `<`, `>` or `&`, which is rare; the alternative, asking the server for its capabilities, is more code for little gain. Revisit if someone reports it.
- **Links to `localhost` in documents no longer open for the host.** → Before this change they replaced the session view, so nothing useful is lost; the address is shown and can be copied. See decision 2.
- **Hosts on 1.8 or older do not filter names.** A viewer on the new release still sees unfiltered names from an old host. → The viewer's UI also applies `shown_name` to names it shows (task 5.3).
- **Generated permission files in the source tree.** → Decision 3; settled by trying the Nix build in task 3.1.

## Migration Plan

One release. Nothing is stored and no message changes, so there is nothing to migrate and a rollback is the previous release. Session processes of the previous release are not affected: all changes are in the app, the UI and the admission path of new sessions.

## Open Questions

- **Does WKWebView follow a redirect from an `http` page to `tauri://localhost`?** Not tested. Task 1.1 tests it on macOS before the fix, to record whether the hole was real. The answer changes the wording of the release note and of the security page, not the work: the handler and the manifest are added either way.
- **Does a channel message still reach a page the window navigated to, on WebView2 and WebKitGTK?** Read from Tauri's source, not run. Task 1.2 records it for macOS; with decision 1 in place the question no longer matters on any platform.
