//! What the app remembers between runs: the user's name, recent folders, last used agent,
//! whether to show notifications, how the user arranges the session window, their sharing service, how
//! far the main window is zoomed, whether to check for updates, whether the terminal works
//! with screen readers and whether the diagnostic log is verbose.
//! Stored as JSON in the app's config directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specdeck_protocol::AgentId;

/// Recent folders listed on the start screen.
pub const MAX_RECENT: usize = 5;

/// The zoom levels of the main window, in percent, as in browsers.
pub const ZOOM_STEPS: [u16; 11] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];
pub const ZOOM_DEFAULT: u16 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub username: Option<String>,
    /// Newest first, each once.
    pub recent_folders: Vec<PathBuf>,
    pub last_agent: Option<AgentId>,
    pub notifications: bool,
    /// How the user followed sessions before layouts; read once to make their first layout, and
    /// kept for older versions.
    pub view: Option<View>,
    /// How the user arranges the session window (`session-window-layout` design decision 1),
    /// as the UI stores it; `None` until they answered the first-join question.
    pub layout: Option<serde_json::Value>,
    /// The sharing service as a normalized URL; `None` for the one built into the app.
    pub sharing_service: Option<String>,
    /// The main window's zoom in percent; one level whether hosting or viewing.
    pub zoom: u16,
    /// Check for and download new releases in the background; the menu item checks anyway.
    pub check_updates: bool,
    /// Lets screen readers read the terminal; off by default, because xterm's screen reader
    /// mode can send typed text to the terminal twice.
    pub screen_reader: bool,
    /// The diagnostic log also records traffic, summed per second.
    pub verbose_logging: bool,
}

/// How a participant follows a session (`viewer-views` design decision 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// Whether the terminal is shown; only for viewers, since the host types into it.
    pub terminal: bool,
    pub steps: Steps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Steps {
    /// What the agent did, in plain words.
    Summary,
    /// Tool calls, commands and diffs.
    Details,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            username: None,
            recent_folders: Vec::new(),
            last_agent: None,
            notifications: true,
            view: None,
            layout: None,
            sharing_service: None,
            zoom: ZOOM_DEFAULT,
            check_updates: true,
            screen_reader: false,
            verbose_logging: false,
        }
    }
}

impl Settings {
    /// Reads the settings, or starts fresh if there are none or they cannot be read.
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Write then rename, so a crash never leaves half a file.
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, path)
    }

    /// Puts `folder` at the top of the recent folders.
    pub fn add_recent(&mut self, folder: PathBuf) {
        self.recent_folders.retain(|existing| existing != &folder);
        self.recent_folders.insert(0, folder);
        self.recent_folders.truncate(MAX_RECENT);
    }

    pub fn remove_recent(&mut self, folder: &Path) {
        self.recent_folders.retain(|existing| existing != folder);
    }

    /// Recent folders that still exist.
    pub fn existing_recent(&self) -> Vec<PathBuf> {
        self.recent_folders.iter().filter(|folder| folder.is_dir()).cloned().collect()
    }

    /// Sets the username, trimmed; an empty name clears it.
    pub fn set_username(&mut self, username: &str) {
        let username = username.trim();
        self.username = (!username.is_empty()).then(|| username.to_owned());
    }
}

/// The next zoom step above `zoom`, or the largest.
pub fn zoom_in(zoom: u16) -> u16 {
    ZOOM_STEPS.into_iter().find(|&step| step > zoom).unwrap_or(ZOOM_STEPS[ZOOM_STEPS.len() - 1])
}

/// The next zoom step below `zoom`, or the smallest.
pub fn zoom_out(zoom: u16) -> u16 {
    ZOOM_STEPS.into_iter().rev().find(|&step| step < zoom).unwrap_or(ZOOM_STEPS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_folders_newest_first_each_once_at_most_five() {
        let mut settings = Settings::default();
        for name in ["a", "b", "a", "c", "d", "e", "f"] {
            settings.add_recent(PathBuf::from(name));
        }
        let names: Vec<_> = settings.recent_folders.iter().map(|p| p.to_str().unwrap()).collect();
        assert_eq!(names, ["f", "e", "d", "c", "a"]);
    }

    #[test]
    fn removed_and_missing_folders_are_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.add_recent(dir.path().join("gone"));
        settings.add_recent(dir.path().to_owned());
        assert_eq!(settings.existing_recent(), [dir.path().to_owned()]);
        settings.remove_recent(dir.path());
        assert!(settings.existing_recent().is_empty());
        // Removing from the list does not touch the folder.
        assert!(dir.path().exists());
    }

    #[test]
    fn zoom_is_100_until_changed_and_then_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before zoom existed still load, at 100%.
        std::fs::write(&path, r#"{"username":"nick","notifications":false}"#).unwrap();
        assert_eq!(Settings::load(&path).zoom, 100);

        let mut settings = Settings::load(&path);
        settings.zoom = 125;
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).zoom, 125);
    }

    #[test]
    fn zoom_moves_one_step_and_stops_at_the_limits() {
        assert_eq!(zoom_in(100), 110);
        assert_eq!(zoom_out(100), 90);
        assert_eq!(zoom_in(200), 200);
        assert_eq!(zoom_out(50), 50);
        // A level between steps, from a hand-edited file, moves to the step beside it.
        assert_eq!(zoom_in(130), 150);
        assert_eq!(zoom_out(130), 125);
        // Out of range, it comes back inside.
        assert_eq!(zoom_in(10), 50);
        assert_eq!(zoom_out(900), 200);
    }

    #[test]
    fn screen_reader_support_is_off_until_turned_on_and_then_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before the setting existed still load, with it off.
        std::fs::write(&path, r#"{"username":"nick","notifications":false}"#).unwrap();
        assert!(!Settings::load(&path).screen_reader);

        let mut settings = Settings::load(&path);
        settings.screen_reader = true;
        settings.save(&path).unwrap();
        assert!(Settings::load(&path).screen_reader);
    }

    #[test]
    fn verbose_logging_is_off_until_turned_on_and_then_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"username":"nick"}"#).unwrap();
        assert!(!Settings::load(&path).verbose_logging);

        let mut settings = Settings::load(&path);
        settings.verbose_logging = true;
        settings.save(&path).unwrap();
        assert!(Settings::load(&path).verbose_logging);
    }

    #[test]
    fn the_view_is_unset_until_chosen_and_then_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before views existed still load, with no view chosen.
        std::fs::write(&path, r#"{"username":"nick","notifications":false}"#).unwrap();
        assert_eq!(Settings::load(&path).view, None);

        let mut settings = Settings::load(&path);
        settings.view = Some(View { terminal: false, steps: Steps::Summary });
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        assert_eq!(
            serde_json::to_value(settings.view).unwrap(),
            serde_json::json!({ "terminal": false, "steps": "summary" })
        );
    }

    #[test]
    fn the_layout_is_unset_until_chosen_and_then_remembered_as_given() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before layouts existed still load, with their view and no layout.
        std::fs::write(&path, r#"{"username":"nick","view":{"terminal":true,"steps":"details"}}"#)
            .unwrap();
        let mut settings = Settings::load(&path);
        assert_eq!(settings.layout, None);
        assert_eq!(settings.view, Some(View { terminal: true, steps: Steps::Details }));

        let layout = serde_json::json!({ "preset": "present", "files": false, "widths": { "present": { "files": 250 } } });
        settings.layout = Some(layout.clone());
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.layout, Some(layout));
        // The view stays for older versions.
        assert_eq!(loaded.view, settings.view);
    }

    #[test]
    fn saves_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let mut settings = Settings::default();
        settings.set_username("  nick ");
        settings.last_agent = Some(AgentId::OpenCode);
        settings.add_recent(PathBuf::from("/projects/shop"));
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        assert_eq!(settings.username.as_deref(), Some("nick"));
        // A terminal session is remembered as the last choice too.
        settings.last_agent = Some(AgentId::Terminal);
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).last_agent, Some(AgentId::Terminal));
    }

    #[test]
    fn unreadable_settings_start_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn notifications_are_on_unless_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings from before the option existed.
        std::fs::write(&path, br#"{"username":"nick"}"#).unwrap();
        assert!(Settings::load(&path).notifications);
        std::fs::write(&path, br#"{"notifications":false}"#).unwrap();
        assert!(!Settings::load(&path).notifications);
    }

    #[test]
    fn the_sharing_service_is_built_in_until_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before the option existed.
        std::fs::write(&path, br#"{"username":"nick","notifications":false}"#).unwrap();
        let mut settings = Settings::load(&path);
        assert_eq!(settings.sharing_service, None);
        assert_eq!(settings.username.as_deref(), Some("nick"));
        assert!(!settings.notifications);

        settings.sharing_service = Some("wss://specdeck.example.com/signal".into());
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
    }

    #[test]
    fn update_checks_are_on_unless_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before the option existed.
        std::fs::write(&path, br#"{"username":"nick","notifications":false}"#).unwrap();
        let mut settings = Settings::load(&path);
        assert!(settings.check_updates);

        settings.check_updates = false;
        settings.save(&path).unwrap();
        assert!(!Settings::load(&path).check_updates);
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["checkUpdates"], false);
    }

    #[test]
    fn empty_username_clears_it() {
        let mut settings = Settings::default();
        settings.set_username("nick");
        settings.set_username("   ");
        assert_eq!(settings.username, None);
    }
}
