//! Updates (auto-update design decisions 1, 5 and 6): whether this install updates itself, the
//! state shown in the windows, and the checks.
//!
//! A check asks the release feed for a newer final release and downloads it; the plugin verifies
//! the download's signature, so a tampered one fails like a broken connection. The verified
//! download is kept in memory until the user restarts, and a background check that fails stays
//! silent. The feed is a trait so the checks can be tested without a release server.

use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

/// How often the app checks again while it runs.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Whether this install replaces itself (design decision 5): never a development build; the macOS
/// app and the Windows installer always; on Linux only the AppImage, whose runtime sets
/// `APPIMAGE`. The `.deb` and Nix builds leave updating to their package manager.
pub fn updates_itself(debug: bool, os: &str, appimage: bool) -> bool {
    !debug
        && match os {
            "macos" | "windows" => true,
            "linux" => appimage,
            _ => false,
        }
}

/// The running app's install, as `updates_itself` sees it.
pub fn this_install_updates_itself() -> bool {
    updates_itself(
        cfg!(debug_assertions),
        std::env::consts::OS,
        std::env::var_os("APPIMAGE").is_some_and(|path| !path.is_empty()),
    )
}

/// What the windows show (design decision 6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum UpdateState {
    /// This install does not update itself.
    Disabled,
    Idle,
    Checking,
    Downloading {
        version: String,
    },
    /// Downloaded and verified; installed when the user restarts.
    Ready {
        version: String,
    },
}

/// What a check from the menu found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    /// A newer release was found and is being downloaded.
    Downloading(String),
    Ready(String),
    /// Another check is still looking.
    Busy,
    Failed,
}

/// Why a check or a download failed, as the log records it: a fixed reason and an HTTP status,
/// never the error's text, which can hold the feed's address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateError {
    Offline,
    /// The server answered with an error; its status when it is known.
    HttpStatus(Option<u16>),
    /// The download does not match its signature.
    Signature,
    Other,
}

impl UpdateError {
    pub fn from_updater(error: &tauri_plugin_updater::Error) -> Self {
        use tauri_plugin_updater::Error;
        match error {
            Error::Reqwest(error) => match error.status() {
                Some(status) => UpdateError::HttpStatus(Some(status.as_u16())),
                None if error.is_connect() || error.is_timeout() => UpdateError::Offline,
                None => UpdateError::Other,
            },
            // No release in the feed: the plugin drops the status of an answer that failed.
            Error::ReleaseNotFound => UpdateError::HttpStatus(None),
            // "Download request failed with status: 404 Not Found": only the number is kept.
            Error::Network(text) => UpdateError::HttpStatus(
                text.split_once("status: ").and_then(|(_, rest)| rest.get(..3)?.parse().ok()),
            ),
            Error::Minisign(_) | Error::SignatureUtf8(_) | Error::Base64(_) => {
                UpdateError::Signature
            }
            _ => UpdateError::Other,
        }
    }

    fn reason(self) -> &'static str {
        match self {
            UpdateError::Offline => "offline",
            UpdateError::HttpStatus(_) => "http_status",
            UpdateError::Signature => "signature",
            UpdateError::Other => "other",
        }
    }

    /// The HTTP status, 0 when there was none or it is not known.
    fn status(self) -> u64 {
        match self {
            UpdateError::HttpStatus(status) => status.map_or(0, u64::from),
            _ => 0,
        }
    }
}

/// The running app's version, as the log records it.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// A newer release the feed announced.
pub trait Release: Send + Sync + 'static {
    fn version(&self) -> String;
    /// The verified update; an error when it cannot be fetched or does not match its signature.
    fn download(&self) -> impl Future<Output = Result<Vec<u8>, UpdateError>> + Send;
}

/// Where releases are announced.
pub trait Feed: Send + Sync {
    type Release: Release;
    /// The newest final release if it is newer than the running app.
    fn latest(&self) -> impl Future<Output = Result<Option<Self::Release>, UpdateError>> + Send;
}

/// The update state and the download that is ready, shared by the background checks, the menu
/// and the windows.
pub struct Updates<R> {
    state: Mutex<UpdateState>,
    ready: Mutex<Option<(R, Vec<u8>)>>,
    on_change: Box<dyn Fn(&UpdateState) + Send + Sync>,
}

impl<R: Release> Updates<R> {
    /// `on_change` hears every new state, to pass it on to the windows.
    pub fn new(enabled: bool, on_change: impl Fn(&UpdateState) + Send + Sync + 'static) -> Self {
        let state = if enabled { UpdateState::Idle } else { UpdateState::Disabled };
        Self { state: Mutex::new(state), ready: Mutex::new(None), on_change: Box::new(on_change) }
    }

    pub fn state(&self) -> UpdateState {
        self.state.lock().unwrap().clone()
    }

    fn set(&self, state: UpdateState) {
        let mut current = self.state.lock().unwrap();
        if *current != state {
            *current = state.clone();
            drop(current);
            (self.on_change)(&state);
        }
    }

    /// The state to return to after a check that found nothing new: the ready update, if any.
    fn settled(&self) -> UpdateState {
        match self.ready.lock().unwrap().as_ref() {
            Some((release, _)) => UpdateState::Ready { version: release.version() },
            None => UpdateState::Idle,
        }
    }

    /// Asks the feed for a newer release. Returns one to download, or the outcome when there is
    /// nothing to download. A ready update stays offered while it looks.
    pub async fn find<F: Feed<Release = R>>(&self, feed: &F) -> Result<R, Outcome> {
        let ready = {
            let state = self.state.lock().unwrap();
            match &*state {
                UpdateState::Disabled | UpdateState::Checking => return Err(Outcome::Busy),
                UpdateState::Downloading { version } => {
                    return Err(Outcome::Downloading(version.clone()));
                }
                UpdateState::Ready { version } => Some(version.clone()),
                UpdateState::Idle => None,
            }
        };
        if ready.is_none() {
            self.set(UpdateState::Checking);
        }
        let found = feed.latest().await;
        match found {
            Ok(Some(release)) if Some(release.version()) != ready => {
                let offered = release.version();
                tracing::info!(outcome = "found", running = RUNNING, offered, "update.checked");
                Ok(release)
            }
            Ok(_) => {
                let outcome = if ready.is_some() { "already_ready" } else { "up_to_date" };
                tracing::info!(outcome, running = RUNNING, "update.checked");
                self.set(self.settled());
                Err(ready.map_or(Outcome::UpToDate, Outcome::Ready))
            }
            Err(error) => {
                let (reason, status) = (error.reason(), error.status());
                tracing::warn!(reason, status, running = RUNNING, "update.check_failed");
                self.set(self.settled());
                Err(Outcome::Failed)
            }
        }
    }

    /// Downloads and verifies `release`, and offers it once it is ready; a failed download
    /// goes back to what was offered before.
    pub async fn fetch(&self, release: R) -> Outcome {
        let version = release.version();
        self.set(UpdateState::Downloading { version: version.clone() });
        match release.download().await {
            Ok(bytes) => {
                tracing::info!(version, "update.ready");
                *self.ready.lock().unwrap() = Some((release, bytes));
                self.set(UpdateState::Ready { version: version.clone() });
                Outcome::Ready(version)
            }
            Err(error) => {
                let (reason, status) = (error.reason(), error.status());
                tracing::warn!(reason, status, offered = version, "update.download_failed");
                self.set(self.settled());
                Outcome::Failed
            }
        }
    }

    /// A background check: looks and downloads, quietly.
    pub async fn check<F: Feed<Release = R>>(&self, feed: &F) -> Outcome {
        match self.find(feed).await {
            Ok(release) => self.fetch(release).await,
            Err(outcome) => outcome,
        }
    }

    /// The ready update, to install; it is no longer offered.
    pub fn take_ready(&self) -> Option<(R, Vec<u8>)> {
        let ready = self.ready.lock().unwrap().take();
        if ready.is_some() {
            self.set(UpdateState::Idle);
        }
        ready
    }

    /// Puts an update back when installing it failed, so it stays offered.
    pub fn restore(&self, release: R, bytes: Vec<u8>) {
        let version = release.version();
        *self.ready.lock().unwrap() = Some((release, bytes));
        self.set(UpdateState::Ready { version });
    }
}

/// The background checks (spec: Background update checks): at start and after each `wait`,
/// each time only if `enabled` still says so, since the user can turn the setting off while the
/// app runs. `wait` sleeps `CHECK_INTERVAL` and says whether to go on.
pub async fn run_checks<F: Feed>(
    updates: &Updates<F::Release>,
    feed: &F,
    enabled: impl Fn() -> bool,
    mut wait: impl AsyncFnMut() -> bool,
) {
    loop {
        if enabled() {
            let _ = updates.check(feed).await;
        }
        if !wait().await {
            break;
        }
    }
}

/// Restart to update (spec: Updating during a session): ends a hosted or watched session first,
/// if `end_session` finds one, and gives it `grace` to tell its viewers before installing, since
/// installing stops the app.
pub async fn install_after_session<T>(
    end_session: impl FnOnce() -> bool,
    grace: Duration,
    install: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if end_session() {
        tokio::time::sleep(grace).await;
    }
    install()
}

/// What the menu item's dialog says (spec: Check for updates on demand): a title and a message.
pub fn outcome_text(outcome: &Outcome, running: &str) -> (String, String) {
    match outcome {
        Outcome::UpToDate => (
            "SpecDeck is up to date".into(),
            format!("Version {running} is the newest version of SpecDeck."),
        ),
        Outcome::Downloading(version) => (
            format!("SpecDeck {version} is available"),
            "It is being downloaded. You can restart to update once it is ready.".into(),
        ),
        Outcome::Ready(version) => (
            format!("SpecDeck {version} is ready"),
            "Restart SpecDeck to update to the new version.".into(),
        ),
        Outcome::Busy => (
            "Checking for updates".into(),
            "SpecDeck is already checking for updates. Try again in a moment.".into(),
        ),
        Outcome::Failed => (
            "Could not check for updates".into(),
            "SpecDeck could not reach the update server. Try again later.".into(),
        ),
    }
}

/// The real feed: the `latest.json` of the newest GitHub release, through the updater plugin.
pub struct GitHubFeed<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> Feed for GitHubFeed<R> {
    type Release = tauri_plugin_updater::Update;

    async fn latest(&self) -> Result<Option<Self::Release>, UpdateError> {
        use tauri_plugin_updater::UpdaterExt;
        let updater = self.0.updater().map_err(|error| UpdateError::from_updater(&error))?;
        updater.check().await.map_err(|error| UpdateError::from_updater(&error))
    }
}

impl Release for tauri_plugin_updater::Update {
    fn version(&self) -> String {
        self.version.clone()
    }

    async fn download(&self) -> Result<Vec<u8>, UpdateError> {
        tauri_plugin_updater::Update::download(self, |_, _| {}, || {})
            .await
            .map_err(|error| UpdateError::from_updater(&error))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[derive(Clone)]
    struct FakeRelease {
        version: &'static str,
        /// `None` when the download fails, for example on a signature that does not match.
        bytes: Option<&'static [u8]>,
    }

    impl Release for FakeRelease {
        fn version(&self) -> String {
            self.version.into()
        }

        async fn download(&self) -> Result<Vec<u8>, UpdateError> {
            self.bytes.map(<[u8]>::to_vec).ok_or(UpdateError::Signature)
        }
    }

    /// Answers each `latest` call with the next of `answers`, then with no newer release.
    struct FakeFeed {
        answers: Mutex<Vec<Result<Option<FakeRelease>, UpdateError>>>,
        calls: AtomicUsize,
    }

    impl FakeFeed {
        fn new(mut answers: Vec<Result<Option<FakeRelease>, UpdateError>>) -> Self {
            answers.reverse();
            Self { answers: Mutex::new(answers), calls: AtomicUsize::new(0) }
        }
    }

    impl Feed for FakeFeed {
        type Release = FakeRelease;

        async fn latest(&self) -> Result<Option<FakeRelease>, UpdateError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.answers.lock().unwrap().pop().unwrap_or(Ok(None))
        }
    }

    fn release(version: &'static str) -> Result<Option<FakeRelease>, UpdateError> {
        Ok(Some(FakeRelease { version, bytes: Some(b"update") }))
    }

    /// Updates that record every state they announce.
    fn updates() -> (Updates<FakeRelease>, Arc<Mutex<Vec<UpdateState>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        (Updates::new(true, move |state| record.lock().unwrap().push(state.clone())), seen)
    }

    fn ready(version: &str) -> UpdateState {
        UpdateState::Ready { version: version.into() }
    }

    #[test]
    fn which_installs_update_themselves() {
        assert!(updates_itself(false, "macos", false));
        assert!(updates_itself(false, "windows", false));
        assert!(updates_itself(false, "linux", true), "AppImage");
        assert!(!updates_itself(false, "linux", false), ".deb or Nix");
        assert!(!updates_itself(true, "macos", false), "development build");
        assert!(!updates_itself(true, "linux", true), "development AppImage");
        assert!(!updates_itself(false, "freebsd", false));
    }

    #[tokio::test]
    async fn a_newer_release_is_downloaded_and_offered() {
        let (updates, seen) = updates();
        let outcome = updates.check(&FakeFeed::new(vec![release("1.2.1")])).await;
        assert_eq!(outcome, Outcome::Ready("1.2.1".into()));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                UpdateState::Checking,
                UpdateState::Downloading { version: "1.2.1".into() },
                ready("1.2.1")
            ]
        );
        let (release, bytes) = updates.take_ready().unwrap();
        assert_eq!((release.version, bytes.as_slice()), ("1.2.1", b"update".as_slice()));
        assert_eq!(updates.state(), UpdateState::Idle);
    }

    #[tokio::test]
    async fn nothing_newer_goes_back_to_idle() {
        let (updates, seen) = updates();
        assert_eq!(updates.check(&FakeFeed::new(vec![Ok(None)])).await, Outcome::UpToDate);
        assert_eq!(*seen.lock().unwrap(), [UpdateState::Checking, UpdateState::Idle]);
    }

    #[tokio::test]
    async fn a_failed_check_is_silent_and_tried_again() {
        let (updates, seen) = updates();
        let feed = FakeFeed::new(vec![Err(UpdateError::Offline), release("1.2.1")]);
        assert_eq!(updates.check(&feed).await, Outcome::Failed);
        assert_eq!(updates.state(), UpdateState::Idle);
        assert_eq!(updates.check(&feed).await, Outcome::Ready("1.2.1".into()));
        // Nothing but checking and back marks the failure.
        assert_eq!(seen.lock().unwrap()[..2], [UpdateState::Checking, UpdateState::Idle]);
    }

    #[tokio::test]
    async fn a_download_that_fails_verification_is_discarded() {
        let (updates, _) = updates();
        let tampered = Ok(Some(FakeRelease { version: "1.2.1", bytes: None }));
        assert_eq!(updates.check(&FakeFeed::new(vec![tampered])).await, Outcome::Failed);
        assert_eq!(updates.state(), UpdateState::Idle);
        assert!(updates.take_ready().is_none());
    }

    #[tokio::test]
    async fn a_ready_update_stays_offered_until_a_newer_one_is_ready() {
        let (updates, seen) = updates();
        let feed = FakeFeed::new(vec![
            release("1.2.1"),
            release("1.2.1"),
            Err(UpdateError::Offline),
            release("1.2.2"),
        ]);
        updates.check(&feed).await;
        seen.lock().unwrap().clear();
        // The same release again, then a failed check: still ready, and nothing announced.
        assert_eq!(updates.check(&feed).await, Outcome::Ready("1.2.1".into()));
        assert_eq!(updates.check(&feed).await, Outcome::Failed);
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(updates.state(), ready("1.2.1"));
        // A newer one replaces it.
        assert_eq!(updates.check(&feed).await, Outcome::Ready("1.2.2".into()));
        assert_eq!(updates.take_ready().unwrap().0.version, "1.2.2");
    }

    #[tokio::test]
    async fn the_menu_hears_about_a_download_under_way_and_checks_already_running() {
        let (updates, _) = updates();
        updates.set(UpdateState::Downloading { version: "1.2.1".into() });
        let feed = FakeFeed::new(vec![]);
        assert_eq!(updates.find(&feed).await.err(), Some(Outcome::Downloading("1.2.1".into())));
        updates.set(UpdateState::Checking);
        assert_eq!(updates.find(&feed).await.err(), Some(Outcome::Busy));
        assert_eq!(feed.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_install_that_failed_keeps_the_update_offered() {
        let (updates, _) = updates();
        updates.check(&FakeFeed::new(vec![release("1.2.1")])).await;
        let (release, bytes) = updates.take_ready().unwrap();
        updates.restore(release, bytes);
        assert_eq!(updates.state(), ready("1.2.1"));
    }

    #[tokio::test]
    async fn disabled_installs_never_check() {
        let updates: Updates<FakeRelease> = Updates::new(false, |_| {});
        let feed = FakeFeed::new(vec![release("1.2.1")]);
        assert_eq!(updates.check(&feed).await, Outcome::Busy);
        assert_eq!(updates.state(), UpdateState::Disabled);
        assert_eq!(feed.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn background_checks_follow_the_setting_and_repeat() {
        let (updates, _) = updates();
        let feed = FakeFeed::new(vec![Ok(None), release("1.2.1")]);
        // The setting is on at start, turned off before the second round, on for the third.
        let settings = [true, false, true];
        let round = AtomicUsize::new(0);
        run_checks(
            &updates,
            &feed,
            || settings[round.load(Ordering::SeqCst)],
            async || round.fetch_add(1, Ordering::SeqCst) + 1 < settings.len(),
        )
        .await;
        // Two checks, not three: the first found nothing, the third the release.
        assert_eq!(feed.calls.load(Ordering::SeqCst), 2);
        assert_eq!(updates.state(), ready("1.2.1"));
    }

    #[tokio::test]
    async fn a_session_ends_and_gets_time_to_tell_its_viewers_before_the_install() {
        let steps = Mutex::new(Vec::new());
        let grace = Duration::from_millis(50);
        let started = std::time::Instant::now();
        let installed = install_after_session(
            || {
                steps.lock().unwrap().push("end session");
                true
            },
            grace,
            || {
                steps.lock().unwrap().push("install");
                Ok(started.elapsed())
            },
        )
        .await;
        assert_eq!(*steps.lock().unwrap(), ["end session", "install"]);
        assert!(installed.unwrap() >= grace);

        // Without a session there is nothing to wait for.
        let started = std::time::Instant::now();
        let waited =
            install_after_session(|| false, Duration::from_secs(5), || Ok(started.elapsed()));
        assert!(waited.await.unwrap() < Duration::from_secs(1));
    }

    /// The app's log of what `run` did.
    async fn logged(run: impl AsyncFnOnce()) -> Vec<serde_json::Value> {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, subscriber) =
            specdeck_diag::Diagnostics::new(dir.path().to_owned(), specdeck_diag::APP_LOG);
        let _default = tracing::subscriber::set_default(subscriber);
        run().await;
        let mut events = diagnostics.recent();
        events.reverse();
        events
    }

    #[tokio::test]
    async fn checks_and_downloads_are_logged_with_a_fixed_reason() {
        let events = logged(async || {
            let (updates, _) = updates();
            let feed = FakeFeed::new(vec![
                Err(UpdateError::Offline),
                Ok(None),
                Ok(Some(FakeRelease { version: "1.2.1", bytes: None })),
                release("1.2.2"),
            ]);
            for _ in 0..4 {
                updates.check(&feed).await;
            }
        })
        .await;
        let names: Vec<&str> = events.iter().map(|e| e["event"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                "update.check_failed",
                "update.checked",
                "update.checked",
                "update.download_failed",
                "update.checked",
                "update.ready"
            ]
        );
        assert_eq!(events[0]["reason"], "offline");
        assert_eq!(events[0]["running"], env!("CARGO_PKG_VERSION"));
        assert_eq!(events[1]["outcome"], "up_to_date");
        assert_eq!(events[2]["outcome"], "found");
        assert_eq!(events[2]["offered"], "1.2.1");
        assert_eq!(events[3]["reason"], "signature");
        assert_eq!(events[3]["offered"], "1.2.1");
        assert_eq!(events[5]["version"], "1.2.2");
    }

    #[test]
    fn updater_errors_become_a_fixed_reason() {
        use tauri_plugin_updater::Error;
        let signature = Error::SignatureUtf8("Marker".into());
        assert_eq!(UpdateError::from_updater(&signature), UpdateError::Signature);
        assert_eq!(
            UpdateError::from_updater(&Error::ReleaseNotFound),
            UpdateError::HttpStatus(None)
        );
        let network = Error::Network(
            "Download request failed with status: 404 Not Found https://marker.example".into(),
        );
        assert_eq!(UpdateError::from_updater(&network), UpdateError::HttpStatus(Some(404)));
        assert_eq!(UpdateError::from_updater(&Error::EmptyEndpoints), UpdateError::Other);
    }

    #[test]
    fn the_menu_dialog_names_the_version() {
        let (title, message) = outcome_text(&Outcome::UpToDate, "1.2.0");
        assert_eq!(title, "SpecDeck is up to date");
        assert!(message.contains("1.2.0"));
        assert_eq!(
            outcome_text(&Outcome::Ready("1.2.1".into()), "1.2.0").0,
            "SpecDeck 1.2.1 is ready"
        );
        assert!(
            outcome_text(&Outcome::Downloading("1.2.1".into()), "1.2.0").1.contains("downloaded")
        );
        assert!(outcome_text(&Outcome::Failed, "1.2.0").1.contains("Try again later"));
    }
}
