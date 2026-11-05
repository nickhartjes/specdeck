# Tasks

Carried over from the archived `native-shared-session-mvp` (its task number in brackets). "Design" means that change's `design.md`.

## 1. GitHub, CI and releases

- [x] 1.1 [1.5] Push the repository to GitHub and get `ci.yml` green: `just check` and `just package` on macOS, Windows and Ubuntu, failing when an installer exceeds 25 MB, including the libdatachannel build on Windows
- [ ] 1.2 [8.6] Fill in the pnpm dependency hash in `nix/package.nix` from the first CI run of the Nix job; verify `nix build` and `nix run` on x86_64-linux in CI and on aarch64-linux in a local container, and that the binary starts
- [ ] 1.3 [8.4] Verify `release.yml` with a test tag (on a fork or as a prerelease): tag check against the app version, draft release, matrix builds uploaded under stable names, signing and notarization when secrets exist, publish when all uploaded; and that the asset names match what the cask and docs use
  - 2026-11-01: `v1.0.0-rc.1` failed on Windows (the build step runs in Git Bash, whose Perl lacks a module OpenSSL's Configure needs; fixed by pointing `OPENSSL_SRC_PERL` at Strawberry Perl). `v1.0.0-rc.2` passed: tag check, draft with release notes, all five installers uploaded under the names the cask and docs use, published as a prerelease, Homebrew skipped. Both tags are on test commits outside `v1` that set the version to `-rc.N`. Still open: signing and notarization, since no certificates are set as secrets (the jobs warn and continue), and `SPECDECK_SIGNALLING_URL` is not set either, so the rc apps point at the local test service. Cleanup: delete the `v1.0.0-rc.1` draft and tag
  - 2026-11-03: once a Windows certificate is set, also check that its signing config and the updater's `createUpdaterArtifacts` are both applied in one build (carried over from `auto-update` task 4.1): the installer is signed and its `.sig` is uploaded
- [ ] 1.4 [8.1] Verify the installers per platform (`.dmg` arm64 and x64, NSIS `.exe`, AppImage and `.deb`) with the `specdeck://` scheme registered: each installs and launches on a clean machine
- [x] 1.5 Give the size check a separate AppImage limit of 100 MB (installers that use the system's web engine stay at 25 MB) in `scripts/check-installer-size.sh`, `ci.yml` and `release.yml`; verify that the script passes and fails at both limits, and that the Ubuntu CI job passes

## 2. Measurements

- [ ] 2.1 [2.4] Record three real one-hour Claude Code sessions and report raw and zstd-compressed log size and xterm.js replay time in `docs/replay-measurements.md`; state whether the 5 s catch-up target holds and, if not, raise it with the user
- [ ] 2.2 [8.2] Add `scripts/measure-footprint` and record cold start and one-hour viewing memory on the reference machines; verify the targets (1.5 s, 250 MB) are met or report the gap
  - 2026-11-01: the script is added and results go in `docs/footprint-measurements.md`. On an M5 Pro (not a reference machine): start 1.08 s first, 0.95 s median; one hour viewing a terminal session peaked at 205 MB. Still to do: the three reference machines, a cold start after a reboot, and a Claude Code session with the timeline

## 3. Manual verification

- [x] 3.1 [6.7] Close the host window with a viewer connected, reopen it from the Dock or tray, and see full scrollback while the viewer stays connected
  - Done on 2026-11-01 on macOS with the packaged app as host and a viewer joined: with the window closed the viewer stayed live; reopened with `open` (as from the Dock) the window had the whole scrollback (a search found the first command of 3,000 lines) and both participants. A host whose app bundle was rebuilt while it ran could no longer take new viewers (their connection failed); a fresh start of the same build could, so replacing the app during a session is the likely cause, to be checked against a long session in 2.1
- [ ] 3.2 [6.9, 7.8] Read the user guide (`docs/site/src/content/docs/guide`) against the running app: hosting and joining, menus, tray, notifications and join links; fix what differs
  - 2026-11-01 on macOS with the packaged app: hosting and joining, the File, Edit, View and Help menus with their shortcuts, the menu-bar icon (present while hosting, tooltip "SpecDeck: hosting demo-repo · 2 viewers"), and a `specdeck://join/…` link opened from outside (SpecDeck came to the front and joined) match the guide. Still open: the menu-bar icon's menu (hidden by a full menu bar here) and notifications (none appeared and none show in the system log while a Focus mode was on; check with Focus off)
- [ ] 3.3 [7.7] Check keyboard-only use and screen-reader labels (VoiceOver, Narrator, Orca) for the start screen, host and viewer views; fix gaps and verify by joining a session using only the keyboard
  - 2026-11-01 on macOS: joining by keyboard alone works (Tab to the code field, type the code in lower case without dashes, Enter), and F6 moves between toolbar, terminal and side panel. Every control on the start screen and in the session has a name in the accessibility tree. Possible improvements: the agent choices are four Tab stops instead of one radio group moved with the arrow keys, and after joining the focus lands on the page rather than on the session. Still open: listening with VoiceOver, Narrator and Orca
- [ ] 3.4 [8.3] Run the cross-platform checklist and record the results in `docs/verification.md`: host and viewer on different operating systems, a viewer relayed through TURN only, 20 viewers, window close and reopen with viewers connected, viewer reconnect after network loss; on Windows and Linux the menus and shortcuts (Ctrl+F opens search, Ctrl+C without selection interrupts the agent), the tray icon (including starting the app again on GNOME without an AppIndicator extension, and that ending a session removes it without a crash), notifications with click-to-focus, File > Settings, Mica on Windows 11 with the opaque fallback on Windows 10 and Linux, single instance and join links from a browser, and window size and position restored after a restart (also on a Mac without a tiling window manager such as AeroSpace)

## 4. Deployment

- [ ] 4.1 Deploy signalling and coturn to a test namespace once the cluster operators confirm the relay port mapping, with the namespace enforcing the `restricted` Pod Security level; verify a session relayed through it

## 5. Checks carried over from shipped changes

These changes shipped (up to 1.5.0) and were archived with only checks on real machines left; the change and its task number are in brackets.

- [ ] 5.1 [auto-update 5.1] Publish a first release with the updater (1.2.0) and a second one after it (1.2.1); on macOS from the `.dmg`, confirm 1.2.0 offers 1.2.1, restarts into it, and that About shows 1.2.1; record in `docs/verification.md`
- [ ] 5.2 [auto-update 5.2] Same on Linux from the AppImage, and confirm the `.deb` never checks (no request to github.com while running); record in `docs/verification.md`
- [ ] 5.3 [auto-update 5.3] Same on Windows from the installer, including whether SmartScreen interrupts the passive install and whether the app restarts by itself (design, Open Questions); record in `docs/verification.md`
- [ ] 5.4 [auto-update 5.4] While hosting with a viewer connected, choose Restart to update, confirm, and check that the viewer sees the session end and the host restarts in the new version; record in `docs/verification.md`
- [ ] 5.5 [configurable-sharing-service 2.2, 3.3, 4.3] Host with each source of the sharing service set in turn (`SPECDECK_SIGNALLING_URL`, the setting, neither) and check the session uses the one that wins; read the Sharing service section of `guide/settings.md` and "Through another sharing service" in `guide/join.md` against the running app
- [ ] 5.6 [configurable-sharing-service 5.1] With the local stack (`just local-up`) and `specdeck.hartj.es`: host on one, open the host's link on a viewer set to the other, join once, check the viewer's setting is unchanged, then join with "from now on" and check it changed; record the result in `docs/verification.md`
- [ ] 5.7 [configurable-sharing-service 5.2] Open a link with `?server=` in the 1.0 release and record whether it joins, so the user guide's note for 1.0 viewers matches (design.md, Risks)
- [ ] 5.8 [diagnostics 9.2, 10.3] Check the whole feature in the running app with `just local-dev`. Host and join a session, open the panel on both sides, use Reconnect, turn on Verbose logging, and export. Confirm the verdicts match what happened, the export holds the matching events, and a search of the export for the invite code, the username and the session folder finds nothing. Then freeze the window on purpose from the developer tools (`while (true) {}`) and check that Help › Export Diagnostics… still exports
- [ ] 5.9 [rename-app-identifier 3.1] macOS: upgrade through Restart to update, and separately with `brew upgrade --cask` over the 1.3.0 cask; verify settings, recent folders and window position are kept, one `SpecDeck.app` exists, `specdeck://` links open it, and the permission prompts appear once
  - 2026-11-03: `v1.4.0-rc.1` `.dmg` over 1.3.0 (Homebrew install in `~/Applications`) on Apple silicon: the new config folder is an identical copy of the old one (settings, window states, `profiles/`), the old one untouched; one `SpecDeck.app`; `specdeck://` opens it (LaunchServices default handler, a test join link reached the running app, one instance). No Local Network prompt while hosting alone; System Settings lists SpecDeck under Local Network, switched on. Open: Restart to update and `brew upgrade --cask` from 1.3.0, both only possible with the final 1.4.0
- [ ] 5.10 [rename-app-identifier 3.2] Windows: upgrade through Restart to update and with the installer; verify settings are kept, Installed apps lists one SpecDeck, and `specdeck://` links open it
- [ ] 5.11 [rename-app-identifier 3.3] Linux: upgrade the AppImage through Restart to update and the `.deb` with `apt install ./SpecDeck-amd64.deb`; verify settings are kept and `xdg-mime query default x-scheme-handler/specdeck` names a `.desktop` file that exists and starts the renamed app
- [ ] 5.12 [rename-app-identifier 3.4] Fresh install on a machine without the old folder: verify default settings and that no `com.entrnce.specdeck` folder is created
- [ ] 5.13 [shift-enter-newline 2.2] On Windows, with at least Claude Code and one other agent, check that Shift+Enter starts a new line in the host's terminal. If it sends the prompt there, propose a change to revise the design. Record the result in `docs/verification.md`
- [ ] 5.14 [show-host-interactions 7.1] Host a real Claude Code session with a viewer joined, and go through a question, a plan rejection and approval, an allowed and a denied permission prompt, and a helper. Verify the viewer sees each outcome live and after a fresh join (replay), in both Overview and Developer views
- [ ] 5.15 [web-viewer 8.1] In `just local-dev`, open `http://127.0.0.1:18081/` in Chrome, Firefox and Safari, and join a Claude Code session hosted by the app. In each browser, check:
  - the terminal and the timeline;
  - a late join after some output;
  - Overview and Developer views;
  - search and copy;
  - a reconnect after restarting the signalling pod;
  - the session ending;
  - the host's participant list after closing the tab.
  Repeat once in relay-only mode (direct connections blocked). Record the results in `docs/verification.md`.
  - 2026-11-04, automated, without the app or coturn (see the archived `web-viewer` tasks for the setup): joining a running session with its history, the terminal and the timeline, the Developer view, the People tab, and **Leave** passed in Chromium, Firefox and WebKit. Still to do by hand with the app: Overview, search and copy, reconnect after a signalling restart, the session ending, closing the tab, and relay-only mode
- [ ] 5.16 [zoom-window 4.2] Verify on Windows and Linux (WebView2, WebKitGTK) that Ctrl+`=`, Ctrl+`-` and Ctrl+`0` zoom even while the host's terminal has focus, that Ctrl+Shift+`-` still reaches the terminal, and that the accelerators parse. Use `Equal` if `=` does not. Record the result in `docs/verification.md`
