## ADDED Requirements

### Requirement: Upgrade from the old identifier keeps settings
When SpecDeck starts for the first time after an upgrade from a release with the identifier `com.entrnce.specdeck`, it SHALL take over that release's settings and the main window's size and position, and the upgrade SHALL replace the old install instead of adding a second one. Pane sizes MAY reset once. The old settings SHALL be left on disk and SHALL NOT be read again once taken over.

#### Scenario: In-app update
- **WHEN** a user of a release with the old identifier updates to the renamed release through Restart to update
- **THEN** the renamed SpecDeck starts with their name, sharing service, notification, zoom, screen reader and update settings, their recent folders, and the window where they left it

#### Scenario: Installer, Homebrew or package over an old install
- **WHEN** a user installs the renamed release over the old one with the `.dmg`, Homebrew, the Windows installer, the AppImage or the `.deb`
- **THEN** one SpecDeck is installed, its settings are taken over, and `specdeck://` links open it

#### Scenario: Settings already present
- **WHEN** the renamed SpecDeck starts and already has settings of its own
- **THEN** it keeps them and ignores the old ones

#### Scenario: Fresh install
- **WHEN** SpecDeck is installed on a machine that never had the old identifier
- **THEN** it starts with default settings, as before
