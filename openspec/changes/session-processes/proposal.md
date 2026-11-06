# Proposal

## Why

A hosted session lives inside the app's process, so anything that stops the app ends it for everyone: Restart to update, a crash, or a window that hangs until the app is killed. Every release interrupts every viewer, and with several sessions per app (`multiple-windows`) one failure now ends all of them. Running each hosted session in its own process, outside the app, removes that coupling.

## What Changes

- Each hosted session runs in its own **session process**: the agent's terminal, the output log, the hooks, sharing and the viewers' connections. The app becomes the session's window: it attaches to the process, shows it and sends the host's input.
- **Restart to update no longer ends hosted sessions.** Their processes keep running in the version they started with; viewers stay connected and notice nothing. After the restart the app reopens a window for each running session. Watched sessions are still left. **BREAKING** (spec level): "Updating during a session" changes.
- **A crash or a killed app no longer ends hosted sessions.** When the app starts again it finds running sessions and reopens their windows.
- A session process that no app has attached to for **10 minutes** ends itself, which its viewers see as the host ending the session, so a session never stays shared without anything on the host's screen showing it.
- **Quit still ends every hosted session**, after asking once, as today.
- The app controls session processes of **its own and the previous release**. A running session of an older release can only be ended.
- Each session process keeps its own diagnostic log in the app's log folder, and Export diagnostics includes it.
- Watching stays inside the app: a watched session ends with the app, as today.

Out of scope: sessions surviving a reboot or logging out (the processes end with the user's login session), resuming an agent's conversation in a new session, and moving watched sessions out of the app.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-hosting`: hosted sessions survive the app stopping, are reopened when it starts, end themselves after the grace period without an app, and are controllable across one release.
- `app-updates`: Restart to update keeps hosted sessions running and asks only about watched ones.
- `diagnostics`: session processes keep their own log, and the export includes it.

## Impact

- New binary `specdeck-session` (crate `crates/session`), shipped next to the app in every package. The session engine (`crates/core`) is unchanged apart from what a separate process needs; `HostRuntime` in `app/src-tauri/src/host.rs` moves into it.
- New local protocol between the app and session processes (crate `crates/local`): a Unix socket in a private folder on macOS and Linux, a named pipe on Windows, versioned per release.
- `app/src-tauri`: sessions are reached through a client instead of in-process runtimes; start, discovery at launch, reopening windows, Quit, Restart to update and the tray change accordingly.
- Packaging: the release workflow, the Homebrew cask, the Nix package, the `.deb` and the AppImage include the second binary. Installers (7–14 MB in 1.6.0) grow by a few MB and stay under the 25 MB limit. The AppImage is already 87 MB because it bundles WebKitGTK, which conflicts with that requirement today; this change does not address that.
- No change to the session protocol between hosts and viewers, or to the sharing service.
