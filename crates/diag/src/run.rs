//! The app's runs (`diagnostics-launch-and-crashes` design decisions 1 and 2): what started, and
//! whether the run before it stopped cleanly.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::system::{self, Os};

/// The marker of the run in progress, in the log folder.
pub const MARKER: &str = "run.json";
/// How often a running app says it is still there, so a run that ends without a word is known
/// to have lasted at least that long.
pub const ALIVE_EVERY: std::time::Duration = std::time::Duration::from_secs(10);

/// What the app runs as and on, recorded at each start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub version: String,
    pub os: Os,
    /// The system's web view: WebView2, WKWebView or WebKitGTK.
    pub webview: String,
    /// Whether this install updates itself (`app-updates`).
    pub self_updating: bool,
    pub verbose: bool,
}

impl Launch {
    /// This process on this system; `webview` as the web view reported it.
    pub fn here(webview: &str, self_updating: bool, verbose: bool) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            os: system::os(),
            webview: system::plain(webview),
            self_updating,
            verbose,
        }
    }

    /// `app.started`.
    pub fn record(&self) {
        tracing::info!(
            target: "specdeck_diag",
            version = self.version.as_str(),
            os = std::env::consts::OS,
            os_name = self.os.name.as_str(),
            os_version = self.os.version.as_str(),
            arch = std::env::consts::ARCH,
            webview = self.webview.as_str(),
            self_updating = self.self_updating,
            verbose = self.verbose,
            "app.started"
        );
    }
}

/// What `run.json` holds: the run's version, when it started and when it was last known to be
/// running, in seconds since 1970.
#[derive(Debug, Deserialize, Serialize)]
struct Marker {
    version: String,
    started_at: u64,
    alive_at: u64,
}

/// The run in progress, from its start to its clean stop.
#[derive(Debug)]
pub struct Run {
    path: PathBuf,
    version: String,
    started_at: u64,
}

/// A run that ended without stopping cleanly: by a crash, being killed or power loss.
#[derive(Debug, PartialEq, Eq)]
pub struct Unclean {
    pub version: String,
    /// About how long it ran, to within [`ALIVE_EVERY`].
    pub ran_s: u64,
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

impl Run {
    /// Starts this run in log folder `dir`, and returns the run before it if that one did not
    /// stop cleanly. A marker that cannot be read is replaced without a word.
    pub fn begin(dir: &Path, version: &str, now: SystemTime) -> (Self, Option<Unclean>) {
        let path = dir.join(MARKER);
        let previous = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Marker>(&bytes).ok())
            .map(|marker| Unclean {
                ran_s: marker.alive_at.saturating_sub(marker.started_at),
                version: system::plain(&marker.version),
            });
        let run = Self { path, version: version.into(), started_at: seconds(now) };
        run.alive(now);
        (run, previous)
    }

    /// Says the run is still going; called every [`ALIVE_EVERY`]. Written whole and then
    /// renamed, so a crash halfway leaves the previous marker.
    pub fn alive(&self, now: SystemTime) {
        let marker = Marker {
            version: self.version.clone(),
            started_at: self.started_at,
            alive_at: seconds(now),
        };
        let next = self.path.with_extension("json.next");
        let written = serde_json::to_vec(&marker)
            .ok()
            .is_some_and(|bytes| std::fs::write(&next, bytes).is_ok());
        if written {
            let _ = std::fs::rename(&next, &self.path);
        }
    }

    /// `app.stopped`, and the marker goes: the next start finds a clean stop.
    pub fn end(&self, now: SystemTime) {
        tracing::info!(
            target: "specdeck_diag",
            ran_s = seconds(now).saturating_sub(self.started_at),
            "app.stopped"
        );
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Unclean {
    /// `app.ended_unexpectedly`.
    pub fn record(&self) {
        tracing::warn!(
            target: "specdeck_diag",
            version = self.version.as_str(),
            ran_s = self.ran_s,
            "app.ended_unexpectedly"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{APP_LOG, Diagnostics};

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds)
    }

    #[test]
    fn a_clean_stop_leaves_nothing_for_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        let (run, previous) = Run::begin(dir.path(), "1.9.0", at(0));
        assert_eq!(previous, None);
        assert!(dir.path().join(MARKER).exists());
        run.end(at(30));
        assert!(!dir.path().join(MARKER).exists());
        assert_eq!(Run::begin(dir.path(), "1.9.0", at(60)).1, None);
    }

    #[test]
    fn a_run_left_behind_is_found_with_its_version_and_length() {
        let dir = tempfile::tempdir().unwrap();
        let (run, _) = Run::begin(dir.path(), "1.8.0", at(0));
        run.alive(at(10));
        run.alive(at(20));
        // The process ends here without `end`.
        let (_, previous) = Run::begin(dir.path(), "1.9.0", at(100));
        assert_eq!(previous, Some(Unclean { version: "1.8.0".into(), ran_s: 20 }));
        // The new run's marker replaced it.
        assert_eq!(Run::begin(dir.path(), "1.9.0", at(200)).1.unwrap().version, "1.9.0");
    }

    #[test]
    fn a_marker_that_cannot_be_read_is_replaced_without_a_word() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MARKER), b"{\"version\":").unwrap();
        let (run, previous) = Run::begin(dir.path(), "1.9.0", at(0));
        assert_eq!(previous, None);
        drop(run);
        let marker: Marker =
            serde_json::from_slice(&std::fs::read(dir.path().join(MARKER)).unwrap()).unwrap();
        assert_eq!(marker.version, "1.9.0");
    }

    #[test]
    fn stops_and_unclean_ends_are_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, subscriber) = Diagnostics::new(dir.path().to_owned(), APP_LOG);
        tracing::subscriber::with_default(subscriber, || {
            Unclean { version: "1.8.0".into(), ran_s: 20 }.record();
            Run::begin(dir.path(), "1.9.0", at(0)).0.end(at(42));
        });
        let recent = diagnostics.recent();
        assert_eq!(recent[1]["event"], "app.ended_unexpectedly");
        assert_eq!(recent[1]["level"], "WARN");
        assert_eq!(recent[1]["version"], "1.8.0");
        assert_eq!(recent[1]["ran_s"], 20);
        assert_eq!(recent[0]["event"], "app.stopped");
        assert_eq!(recent[0]["ran_s"], 42);
    }

    #[test]
    fn a_start_is_recorded_with_what_runs_where() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, subscriber) = Diagnostics::new(dir.path().to_owned(), APP_LOG);
        let launch = Launch::here("131.0.2903.70 <marker>", true, false);
        tracing::subscriber::with_default(subscriber, || launch.record());
        let started = &diagnostics.recent()[0];
        assert_eq!(started["event"], "app.started");
        assert_eq!(started["level"], "INFO");
        assert_eq!(started["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(started["os"], std::env::consts::OS);
        assert_eq!(started["os_name"], launch.os.name.as_str());
        assert_eq!(started["os_version"], launch.os.version.as_str());
        assert_eq!(started["arch"], std::env::consts::ARCH);
        assert_eq!(started["webview"], "131.0.2903.70marker");
        assert_eq!(started["self_updating"], true);
        assert_eq!(started["verbose"], false);
    }
}
