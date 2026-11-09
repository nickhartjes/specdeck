//! What the app remembers between runs: the user's name, recent folders, last used agent,
//! which notifications to show, how the user arranges the session window, their sharing service, how
//! far the main window is zoomed, whether to check for updates, whether the terminal works
//! with screen readers, whether the diagnostic log is verbose, whether Claude gets a browser and
//! whether the app is light, dark or follows the OS.
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
    /// Which kinds of notification the user gets about sessions they watch, under `notifications`.
    pub viewer_notifications: ViewerNotifications,
    /// Which kinds of notification the host gets about their asks, under `notifications`.
    pub host_notifications: HostNotifications,
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
    /// New Claude Code sessions get SpecDeck's own browser (`agent-browser`); only on macOS.
    pub browser: bool,
    /// New sessions keep images from MCP tools, such as screenshots, in the timeline.
    pub screenshots: bool,
    /// Claude Code runs in the folder's Docker Sandbox when it has one (`claude-in-nma-sandbox`);
    /// only on macOS.
    pub claude_sandbox: bool,
    /// Light, dark or the OS's appearance; the terminal stays dark in each.
    pub appearance: Appearance,
}

/// The kinds of notification a viewer gets (`viewer-notifications` design decision 4). A file
/// from before a kind existed gets that kind's default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewerNotifications {
    /// A plan is ready for review.
    pub plans: bool,
    /// A document changed.
    pub documents: bool,
    /// The agent finished a turn; off by default, since it comes after every turn.
    pub finished: bool,
    /// The host asks everyone to read something (`read-and-estimate`).
    pub reads: bool,
    /// The host starts a vote, or a new round of one.
    pub estimates: bool,
    /// Someone else shares a file (`notification-bell`); off by default, like the two below.
    pub files: bool,
    /// The host shares a link.
    pub links: bool,
    /// Someone else adds to the braindump.
    pub braindump: bool,
}

impl Default for ViewerNotifications {
    fn default() -> Self {
        Self {
            plans: true,
            documents: true,
            finished: false,
            reads: true,
            estimates: true,
            files: false,
            links: false,
            braindump: false,
        }
    }
}

/// A kind of viewer notification, as the window names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewerNotificationKind {
    Plans,
    Documents,
    Finished,
    Reads,
    Estimates,
    Files,
    Links,
    Braindump,
}

impl ViewerNotificationKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Plans => "plans",
            Self::Documents => "documents",
            Self::Finished => "finished",
            Self::Reads => "reads",
            Self::Estimates => "estimates",
            Self::Files => "files",
            Self::Links => "links",
            Self::Braindump => "braindump",
        }
    }
}

impl ViewerNotifications {
    pub fn allows(&self, kind: ViewerNotificationKind) -> bool {
        match kind {
            ViewerNotificationKind::Plans => self.plans,
            ViewerNotificationKind::Documents => self.documents,
            ViewerNotificationKind::Finished => self.finished,
            ViewerNotificationKind::Reads => self.reads,
            ViewerNotificationKind::Estimates => self.estimates,
            ViewerNotificationKind::Files => self.files,
            ViewerNotificationKind::Links => self.links,
            ViewerNotificationKind::Braindump => self.braindump,
        }
    }
}

/// The kinds of notification the host gets about the answers to their read requests and votes
/// (`host-ask-notifications` design decision 4); all on, also in a file from before they existed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HostNotifications {
    /// A viewer has a question.
    pub questions: bool,
    /// Everyone has read, or the time is up while someone still reads.
    pub read: bool,
    /// Every viewer has voted.
    pub voted: bool,
}

impl Default for HostNotifications {
    fn default() -> Self {
        Self { questions: true, read: true, voted: true }
    }
}

/// A kind of the host's notification about their asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostNotificationKind {
    Questions,
    Read,
    Voted,
}

impl HostNotificationKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Questions => "questions",
            Self::Read => "read",
            Self::Voted => "voted",
        }
    }
}

impl HostNotifications {
    pub fn allows(&self, kind: HostNotificationKind) -> bool {
        match kind {
            HostNotificationKind::Questions => self.questions,
            HostNotificationKind::Read => self.read,
            HostNotificationKind::Voted => self.voted,
        }
    }
}

/// The app's appearance (`appearance-setting` design decision 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
    /// Follows the OS. Also what an unknown value, from a newer release, reads as, so the
    /// other settings are kept; `serde(other)` has to be on the last variant.
    #[default]
    #[serde(other)]
    System,
}

impl Appearance {
    /// The theme for the native windows; `None` follows the OS.
    pub fn theme(self) -> Option<tauri::Theme> {
        match self {
            Self::System => None,
            Self::Light => Some(tauri::Theme::Light),
            Self::Dark => Some(tauri::Theme::Dark),
        }
    }
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
            viewer_notifications: ViewerNotifications::default(),
            host_notifications: HostNotifications::default(),
            view: None,
            layout: None,
            sharing_service: None,
            zoom: ZOOM_DEFAULT,
            check_updates: true,
            screen_reader: false,
            verbose_logging: false,
            browser: true,
            appearance: Appearance::System,
            screenshots: true,
            claude_sandbox: false,
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
    fn the_browser_is_on_until_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before the browser existed still load, with it on.
        std::fs::write(&path, r#"{"username":"nick","notifications":false}"#).unwrap();
        assert!(Settings::load(&path).browser);
        let settings = Settings { browser: false, ..Settings::load(&path) };
        settings.save(&path).unwrap();
        assert!(!Settings::load(&path).browser);
    }

    #[test]
    fn screenshots_are_shown_until_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before screenshots existed still load, with them on.
        std::fs::write(&path, r#"{"username":"nick","browser":false}"#).unwrap();
        assert!(Settings::load(&path).screenshots);
        let settings = Settings { screenshots: false, ..Settings::load(&path) };
        settings.save(&path).unwrap();
        assert!(!Settings::load(&path).screenshots);
    }

    #[test]
    fn claude_runs_on_the_host_until_the_sandbox_is_turned_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before the sandbox setting existed still load, with it off.
        std::fs::write(&path, r#"{"username":"nick"}"#).unwrap();
        assert!(!Settings::load(&path).claude_sandbox);
        let settings = Settings { claude_sandbox: true, ..Settings::load(&path) };
        settings.save(&path).unwrap();
        assert!(Settings::load(&path).claude_sandbox);
    }

    #[test]
    fn the_appearance_follows_the_os_until_chosen_and_then_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings saved before the appearance existed still load, following the OS.
        std::fs::write(&path, r#"{"username":"nick","notifications":false}"#).unwrap();
        assert_eq!(Settings::load(&path).appearance, Appearance::System);

        for appearance in [Appearance::Light, Appearance::Dark, Appearance::System] {
            let settings = Settings { appearance, ..Settings::load(&path) };
            settings.save(&path).unwrap();
            assert_eq!(Settings::load(&path), settings);
        }
        assert_eq!(serde_json::to_value(Appearance::Dark).unwrap(), "dark");
    }

    #[test]
    fn an_unknown_appearance_follows_the_os_and_keeps_the_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"username":"nick","zoom":125,"appearance":"sepia"}"#).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.appearance, Appearance::System);
        assert_eq!(settings.username.as_deref(), Some("nick"));
        assert_eq!(settings.zoom, 125);
    }

    #[test]
    fn the_appearance_gives_the_native_theme() {
        assert_eq!(Appearance::System.theme(), None);
        assert_eq!(Appearance::Light.theme(), Some(tauri::Theme::Light));
        assert_eq!(Appearance::Dark.theme(), Some(tauri::Theme::Dark));
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
    fn host_notifications_are_on_until_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings from before the switches existed: all on, and everything else kept.
        let older =
            br#"{"username":"nick","notifications":false,"viewerNotifications":{"finished":true}}"#;
        std::fs::write(&path, older).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.host_notifications, HostNotifications::default());
        assert!(settings.host_notifications.allows(HostNotificationKind::Questions));
        assert!(settings.host_notifications.allows(HostNotificationKind::Read));
        assert!(settings.host_notifications.allows(HostNotificationKind::Voted));
        assert_eq!(settings.username.as_deref(), Some("nick"));
        assert!(!settings.notifications && settings.viewer_notifications.finished);

        // A kind the file does not have gets its default.
        std::fs::write(&path, br#"{"hostNotifications":{"voted":false}}"#).unwrap();
        let kinds = Settings::load(&path).host_notifications;
        assert_eq!(kinds, HostNotifications { questions: true, read: true, voted: false });

        let mut settings = Settings::default();
        settings.host_notifications.questions = false;
        settings.save(&path).unwrap();
        let kinds = Settings::load(&path).host_notifications;
        assert!(!kinds.allows(HostNotificationKind::Questions));
        assert!(
            kinds.allows(HostNotificationKind::Read) && kinds.allows(HostNotificationKind::Voted)
        );
    }

    #[test]
    fn viewer_notifications_default_to_plans_and_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Settings from before the option existed keep everything else.
        std::fs::write(&path, br#"{"username":"nick","notifications":false}"#).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.viewer_notifications, ViewerNotifications::default());
        assert_eq!(settings.username.as_deref(), Some("nick"));
        assert!(!settings.notifications);
        assert!(settings.viewer_notifications.allows(ViewerNotificationKind::Plans));
        assert!(settings.viewer_notifications.allows(ViewerNotificationKind::Documents));
        assert!(!settings.viewer_notifications.allows(ViewerNotificationKind::Finished));

        // A kind the file does not have gets its default.
        std::fs::write(&path, br#"{"viewerNotifications":{"finished":true}}"#).unwrap();
        let kinds = Settings::load(&path).viewer_notifications;
        assert_eq!(
            kinds,
            ViewerNotifications {
                plans: true,
                documents: true,
                finished: true,
                reads: true,
                estimates: true,
                files: false,
                links: false,
                braindump: false
            }
        );

        // A file with only the three kinds of the release before: the host's asks load as on,
        // and the rest is kept.
        let older = br#"{"viewerNotifications":{"plans":false,"documents":true,"finished":true}}"#;
        std::fs::write(&path, older).unwrap();
        let kinds = Settings::load(&path).viewer_notifications;
        assert!(!kinds.plans && kinds.documents && kinds.finished);
        assert!(kinds.allows(ViewerNotificationKind::Reads));
        assert!(kinds.allows(ViewerNotificationKind::Estimates));
        // Shared files, links and the braindump came later still, and load as off.
        assert!(!kinds.allows(ViewerNotificationKind::Files));
        assert!(!kinds.allows(ViewerNotificationKind::Links));
        assert!(!kinds.allows(ViewerNotificationKind::Braindump));

        // Only some of the three: the others keep their default.
        std::fs::write(&path, br#"{"viewerNotifications":{"links":true,"reads":false}}"#).unwrap();
        let kinds = Settings::load(&path).viewer_notifications;
        assert!(kinds.links && !kinds.files && !kinds.braindump && !kinds.reads && kinds.plans);

        let mut settings = Settings::default();
        settings.viewer_notifications.documents = false;
        settings.viewer_notifications.estimates = false;
        settings.viewer_notifications.braindump = true;
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
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
