# app-updates Specification

## Purpose
Keeps installed copies of SpecDeck current: the app learns about new releases, downloads and verifies them, and installs them when the user restarts, without disturbing a running session.

## Requirements

### Requirement: Installs that update themselves
The macOS app, the Windows installer and the Linux AppImage SHALL update themselves. The `.deb`, Nix builds and development builds SHALL NOT check for or offer updates, and SHALL NOT show the update setting or menu item, because their package manager or developer updates them.

#### Scenario: AppImage
- **WHEN** SpecDeck runs from the AppImage
- **THEN** it checks for updates

#### Scenario: Debian package or Nix
- **WHEN** SpecDeck runs from the `.deb` or a Nix build
- **THEN** it never contacts the update server, and neither the menu nor Settings mention updates

#### Scenario: Homebrew
- **WHEN** SpecDeck was installed with Homebrew on macOS
- **THEN** it updates itself like the `.dmg` install, and the cask declares that it does

### Requirement: Background update checks
While **Check for updates automatically** is on, the app SHALL check for a newer final release when it starts and every 6 hours while it runs, and download one it finds in the background. The setting SHALL be on by default. A failed check SHALL stay silent and be retried at the next check.

#### Scenario: New release at start
- **WHEN** the app starts and a newer final release exists
- **THEN** it downloads the update without asking, and shows that an update is ready once the download is verified

#### Scenario: Offline
- **WHEN** a background check cannot reach the update server
- **THEN** nothing is shown, and the app checks again 6 hours later

#### Scenario: Setting off
- **WHEN** the user turned Check for updates automatically off
- **THEN** the app does not check on start or periodically, until the user checks from the menu

### Requirement: Final releases only
The app SHALL only offer final releases that are newer than the running version. It SHALL NOT offer prereleases or older versions.

#### Scenario: Prerelease published
- **WHEN** the newest release is `v1.1.0-rc.1` and the app runs 1.0.1
- **THEN** no update is offered

#### Scenario: Already current
- **WHEN** the newest final release is the running version
- **THEN** no update is offered

### Requirement: Verified updates
The app SHALL install an update only after verifying its signature against the update public key the app was built with. An update that fails verification SHALL be discarded and SHALL NOT be offered.

#### Scenario: Tampered download
- **WHEN** the downloaded update does not match its signature
- **THEN** the app discards it, offers nothing, and tries again at the next check

### Requirement: Restart to update
When a verified update is ready, the app SHALL show a notice with the new version and a **Restart to update** action, and SHALL install the update only when the user chooses it. The app SHALL NOT restart on its own.

#### Scenario: Update ready on the start screen
- **WHEN** an update to 1.0.2 is ready and the start screen is shown
- **THEN** a notice says SpecDeck 1.0.2 is ready, with Restart to update

#### Scenario: Restart
- **WHEN** the user chooses Restart to update and no session is running
- **THEN** the app installs the update and starts again in the new version

#### Scenario: Notice dismissed
- **WHEN** the user dismisses the notice
- **THEN** the app does not show it again until it starts again or finds a newer version

### Requirement: Updating during a session
While the user hosts or watches a session, choosing Restart to update SHALL first ask, saying that restarting ends the hosted session for everyone or leaves the watched one. Cancelling SHALL keep the session and the update ready.

#### Scenario: Host restarts
- **WHEN** the host chooses Restart to update during a session
- **THEN** the app asks whether to end the session for everyone and restart, and only on confirmation ends it, installs and restarts

#### Scenario: Viewer cancels
- **WHEN** a viewer chooses Restart to update and then Cancel
- **THEN** the viewer stays in the session and the notice stays

### Requirement: Check for updates on demand
The menu SHALL offer **Check for Updates…** (in the SpecDeck menu on macOS, in Help on Windows and Linux). It SHALL check at once, whatever the setting, and report the outcome: up to date, an update that is being downloaded or is ready, or that the check failed.

#### Scenario: Up to date
- **WHEN** the user chooses Check for Updates… and no newer final release exists
- **THEN** the app says SpecDeck is up to date and names the running version

#### Scenario: Check fails
- **WHEN** the user chooses Check for Updates… and the update server cannot be reached
- **THEN** the app says the check failed and that it can be tried again later

### Requirement: Update setting
Settings SHALL offer **Check for updates automatically**, on by default, on installs that update themselves. Changing it SHALL apply at once.

#### Scenario: Turned off while running
- **WHEN** the user turns the setting off
- **THEN** no further background checks happen while the app runs, and Check for Updates… still works

### Requirement: What an update check sends
An update check SHALL contact only the project's release host, and SHALL send nothing beyond what fetching the update manifest and file requires: no identifier, name, session or usage data.

#### Scenario: Check request
- **WHEN** the app checks for an update
- **THEN** it requests the published manifest from the release host, and the request carries no user, device or session information
