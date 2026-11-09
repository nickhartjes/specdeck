// The diagnostic log, shared with the session processes.
pub(crate) use specdeck_diag as diagnostics;
// The hosted session, shared with the session process (`session-processes`).
use specdeck_session::runtime as host;
mod adapter;
mod browser;
mod client;
mod links;
mod menu;
mod migrate;
mod notify;
mod running;
mod services;
mod sessions;
mod settings;
mod tray;
mod updates;
mod viewer;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use serde::Serialize;
use specdeck_core::agent;
use specdeck_core::environment::{Environment, PROBE_TIMEOUT};
use specdeck_core::peer::PeerConfig;
use specdeck_core::sandbox;

use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::service;
use specdeck_protocol::{AgentId, AgentStatus};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::menu::MenuEvent;
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::StateFlags;

use crate::client::{Paths, SessionClient};
use crate::diagnostics::{Diagnostics, ExportHeader, Heartbeat, SessionStatus, TerminalSnapshot};
use crate::host::{HostNotice, SharingState, Summary};
use crate::services::{ServiceCheck, Services};
use crate::sessions::{MAIN_WINDOW, Session, SessionId, Sessions, is_session_window};
use crate::settings::{Appearance, Settings, View, ZOOM_DEFAULT, zoom_in, zoom_out};
use crate::updates::{GitHubFeed, Outcome, UpdateState, Updates};
use crate::viewer::{EndReason, ViewEvent, ViewerRuntime};

/// An agent's version, with its executable's modification time when it was read.
type ReadVersion = (Option<SystemTime>, Option<String>);

struct AppState {
    settings_path: PathBuf,
    settings: Mutex<Settings>,
    diagnostics: Diagnostics,
    /// Where session processes keep their files (`session-processes`).
    paths: Paths,
    services: Services,
    /// The user's login-shell environment, read once in the background at start.
    environment: Arc<OnceLock<Environment>>,
    /// Every hosted and watched session, and the window that shows each.
    sessions: Mutex<Sessions<SessionClient, ViewerRuntime>>,
    /// The hosted sessions as the tray shows them, in the order they started.
    tray: tray::Entries,
    /// Which session each notification was about, for a click on it.
    notices: Mutex<notify::Notices>,
    /// The session windows show the desktop through them (macOS vibrancy, Windows 11 Mica).
    translucent: AtomicBool,
    /// Join links the windows have not handled yet, by window label.
    join_links: Mutex<HashMap<String, JoinLink>>,
    /// A start command a new window opens with (host, join, a recent folder), by window label.
    start_commands: Mutex<HashMap<String, MenuCommand>>,
    /// New releases: whether one is being fetched or is ready to install.
    updates: Arc<Updates<tauri_plugin_updater::Update>>,
    versions: Mutex<HashMap<PathBuf, ReadVersion>>,
}

impl AppState {
    fn environment(&self) -> Environment {
        self.environment.get().cloned().unwrap_or_else(|| Environment::detect(PROBE_TIMEOUT))
    }

    /// The sharing service a session started now uses.
    fn sharing_service(&self) -> String {
        self.services.effective(self.settings.lock().unwrap().sharing_service.as_deref())
    }

    /// Whether any window hosts or watches a session.
    fn in_session(&self) -> bool {
        self.sessions.lock().unwrap().count() > 0
    }

    fn hosting(&self) -> bool {
        !self.sessions.lock().unwrap().hosts().is_empty()
    }

    /// The hosted session `window` shows, or an error to return.
    fn host_of(&self, window: &str) -> Result<Arc<SessionClient>, String> {
        self.sessions.lock().unwrap().host_of(window).ok_or_else(|| "No session is running.".into())
    }

    fn save(&self, settings: &Settings) {
        // Failing to remember settings is not worth interrupting the user for.
        let _ = settings.save(&self.settings_path);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInfo {
    id: AgentId,
    name: &'static str,
    installed: bool,
    has_timeline: bool,
    reports_status: bool,
    /// It can run in a folder's Docker Sandbox, so the start screen offers the setting.
    sandboxable: bool,
    /// The executable, or for Terminal the shell it starts.
    path: Option<PathBuf>,
    /// Where to read how to install it.
    install_page: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentFolder {
    path: PathBuf,
    name: String,
    /// The folder has a Docker Sandbox for Claude Code (`claude-in-nma-sandbox`).
    has_sandbox: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    username: Option<String>,
    recent_folders: Vec<RecentFolder>,
    last_agent: Option<AgentId>,
    agents: Vec<AgentInfo>,
    /// Set while this window shows a hosted session, so a reopened window goes straight back to it.
    hosting: bool,
    /// The sessions hosted and watched in all windows, for the question before restarting.
    hosted_sessions: usize,
    watched_sessions: usize,
    notifications: bool,
    viewer_notifications: settings::ViewerNotifications,
    host_notifications: settings::HostNotifications,
    /// How the user followed sessions before layouts, to make their first layout from.
    view: Option<View>,
    /// How the user arranges the session window; `None` until they answered the first-join question.
    layout: Option<serde_json::Value>,
    /// The window has a material behind it, so the UI may let it show through.
    translucent: bool,
    platform: &'static str,
    /// The user's sharing service setting; `None` for the built-in one.
    sharing_service: Option<String>,
    /// The service a session started now would use.
    effective_service: String,
    built_in_service: String,
    /// `SPECDECK_SIGNALLING_URL` decides the service, so the setting has no effect.
    service_overridden: bool,
    /// Hosting or viewing in any window; the sharing service cannot change meanwhile.
    in_session: bool,
    /// This install updates itself, so Settings offers the update setting.
    updates_itself: bool,
    check_updates: bool,
    /// Screen readers read the terminal.
    screen_reader: bool,
    /// The diagnostic log records traffic too.
    verbose_logging: bool,
    /// New Claude Code sessions get SpecDeck's own browser.
    browser: bool,
    /// Light, dark or the OS's appearance.
    appearance: Appearance,
    /// New sessions show screenshots in the timeline.
    screenshots: bool,
    /// Claude Code runs in the folder's Docker Sandbox when it has one.
    claude_sandbox: bool,
}

/// The agents to choose from, in display order: the coding agents, then Terminal.
fn agent_infos(env: &Environment) -> Vec<AgentInfo> {
    agent::detect(env)
        .into_iter()
        .map(|found| AgentInfo {
            id: found.agent,
            name: found.agent.display_name(),
            installed: found.executable.is_some(),
            has_timeline: found.agent.has_timeline(),
            reports_status: found.agent.reports_status(),
            sandboxable: sandbox::runs(found.agent),
            path: found.executable,
            install_page: agent::install_page(found.agent),
        })
        .collect()
}

fn recent_folder(path: PathBuf) -> RecentFolder {
    RecentFolder {
        name: path.file_name().map_or_else(
            || path.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
        has_sandbox: sandbox::launcher(&path).is_some(),
        path,
    }
}

/// The profile as `window` sees it.
fn profile(state: &AppState, window: &str) -> Profile {
    let settings = state.settings.lock().unwrap().clone();
    let agents = agent_infos(&state.environment());
    let (hosting, hosted_sessions, watched_sessions) = {
        let sessions = state.sessions.lock().unwrap();
        (sessions.host_of(window).is_some(), sessions.hosts().len(), sessions.viewers().len())
    };
    let in_session = hosted_sessions + watched_sessions > 0;
    Profile {
        username: settings.username.clone(),
        recent_folders: settings.existing_recent().into_iter().map(recent_folder).collect(),
        last_agent: settings.last_agent,
        agents,
        hosting,
        hosted_sessions,
        watched_sessions,
        notifications: settings.notifications,
        viewer_notifications: settings.viewer_notifications,
        host_notifications: settings.host_notifications,
        view: settings.view,
        layout: settings.layout,
        translucent: state.translucent.load(Ordering::Relaxed),
        platform: std::env::consts::OS,
        effective_service: state.services.effective(settings.sharing_service.as_deref()),
        sharing_service: settings.sharing_service,
        built_in_service: state.services.built_in.clone(),
        service_overridden: state.services.overridden.is_some(),
        in_session,
        updates_itself: state.updates.state() != UpdateState::Disabled,
        check_updates: settings.check_updates,
        screen_reader: settings.screen_reader,
        verbose_logging: settings.verbose_logging,
        browser: settings.browser,
        appearance: settings.appearance,
        screenshots: settings.screenshots,
        claude_sandbox: settings.claude_sandbox,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentVersion {
    agent: AgentId,
    /// `None` when the agent printed none in time.
    version: Option<String>,
}

/// The versions of the installed agents, read in parallel. Each executable is asked once, and
/// again only after it changed, so the start screen asks for them after it is shown.
#[tauri::command]
async fn agent_versions(state: State<'_, AppState>) -> Result<Vec<AgentVersion>, String> {
    let env = state.environment();
    let installed: Vec<(AgentId, PathBuf, Option<SystemTime>)> = agent::detect(&env)
        .into_iter()
        .filter(|found| found.agent != AgentId::Terminal)
        .filter_map(|found| {
            let executable = found.executable?;
            let modified = executable.metadata().and_then(|meta| meta.modified()).ok();
            Some((found.agent, executable, modified))
        })
        .collect();
    let missing: Vec<(PathBuf, Option<SystemTime>)> = {
        let cache = state.versions.lock().unwrap();
        installed
            .iter()
            .filter(|(_, path, modified)| cache.get(path).is_none_or(|(at, _)| at != modified))
            .map(|(_, path, modified)| (path.clone(), *modified))
            .collect()
    };
    if !missing.is_empty() {
        let read = tauri::async_runtime::spawn_blocking(move || {
            std::thread::scope(|scope| {
                let readers: Vec<_> = missing
                    .into_iter()
                    .map(|(path, modified)| {
                        let env = &env;
                        scope.spawn(move || {
                            let version = agent::read_version(&path, env, agent::VERSION_TIMEOUT);
                            (path, (modified, version))
                        })
                    })
                    .collect();
                readers.into_iter().filter_map(|reader| reader.join().ok()).collect::<Vec<_>>()
            })
        })
        .await
        .map_err(|e| e.to_string())?;
        state.versions.lock().unwrap().extend(read);
    }
    let cache = state.versions.lock().unwrap();
    Ok(installed
        .into_iter()
        .map(|(agent, path, _)| AgentVersion {
            agent,
            version: cache.get(&path).and_then(|(_, version)| version.clone()),
        })
        .collect())
}

/// Opens the install page of `agent` in the browser; only the pages the app lists.
#[tauri::command]
async fn open_install_page(app: AppHandle, agent: AgentId) -> Result<(), String> {
    let page = agent::install_page(agent).ok_or("This choice needs nothing installed.")?;
    app.opener().open_url(page, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_profile(window: WebviewWindow, state: State<'_, AppState>) -> Result<Profile, String> {
    Ok(profile(&state, window.label()))
}

#[tauri::command]
async fn set_username(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    username: String,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.set_username(&username);
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

#[tauri::command]
async fn set_notifications(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.notifications = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

#[tauri::command]
async fn set_screen_reader(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.screen_reader = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

#[tauri::command]
async fn set_appearance(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    appearance: Appearance,
) -> Result<Profile, String> {
    change_appearance(&app, appearance);
    Ok(profile(&state, window.label()))
}

/// This app's side of a session; a hosted session's, with its latest events, comes from its
/// process.
async fn status_of(
    session: &Session<SessionClient, ViewerRuntime>,
) -> (SessionStatus, Vec<serde_json::Value>) {
    match session {
        Session::Host(host) => {
            let status = host.status().await;
            let (sharing, events) =
                status.map_or((serde_json::Value::Null, Vec::new()), |s| (s.sharing, s.events));
            (SessionStatus::Host { sharing }, events)
        }
        Session::Viewer(viewer) => (SessionStatus::Viewer { viewer: viewer.status() }, Vec::new()),
    }
}

/// Everything the Connection status panel of `window` shows, with the events of the session
/// process it hosts.
async fn window_status(state: &AppState, window: &str) -> diagnostics::Status {
    let session = state.sessions.lock().unwrap().of_window(window);
    let (session, events) = match session {
        Some((_, session)) => status_of(&session).await,
        None => (SessionStatus::None, Vec::new()),
    };
    state.diagnostics.status(window, session).with_session_events(events)
}

/// What the Connection status panel of `window` shows.
#[tauri::command]
async fn diagnostics_status(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<diagnostics::Status, String> {
    Ok(window_status(&state, window.label()).await)
}

/// The terminal's state from the window: enums and numbers only.
#[tauri::command]
async fn diagnostics_report_terminal(
    window: WebviewWindow,
    state: State<'_, AppState>,
    snapshot: TerminalSnapshot,
) -> Result<(), String> {
    state.diagnostics.report_terminal(window.label(), snapshot);
    Ok(())
}

/// Drops the viewer's connection and reconnects at once.
#[tauri::command]
async fn reconnect_viewer(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    let session = state.sessions.lock().unwrap().of_window(window.label());
    match session {
        Some((_, Session::Host(_))) => diagnostics::check_reconnect(true, None),
        Some((_, Session::Viewer(viewer))) => viewer.reconnect(),
        None => diagnostics::check_reconnect(false, None),
    }
}

/// Saves the diagnostics to `path`, which the user chose. Nothing is uploaded.
#[tauri::command]
async fn diagnostics_export(
    window: WebviewWindow,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<(), String> {
    export_diagnostics(&state, window.label(), &path).await
}

/// Exports the diagnostics as `window` sees them, with the sessions of the other windows.
async fn export_diagnostics(state: &AppState, window: &str, path: &Path) -> Result<(), String> {
    let others: Vec<_> = {
        let sessions = state.sessions.lock().unwrap();
        let own = sessions.of_window(window).map(|(id, _)| id);
        let hosts = sessions.hosts().into_iter().map(|(id, host)| (id, Session::Host(host)));
        let viewers = sessions.viewers().into_iter().map(|(id, v)| (id, Session::Viewer(v)));
        hosts.chain(viewers).filter(|(id, _)| Some(*id) != own).map(|(_, s)| s).collect()
    };
    let mut other_statuses = Vec::new();
    for session in &others {
        other_statuses.push(status_of(session).await.0);
    }
    // The log files hold every process's events; the header needs no list of them.
    let status = window_status(state, window).await;
    let header = ExportHeader::new(status, &state.sharing_service(), &state.services.built_in)
        .with_other_sessions(other_statuses);
    state.diagnostics.export(path, &header)
}

/// Help › Export Diagnostics: entirely in Rust, with the native save dialog, so it works
/// while the window's UI does not respond. Exported as the focused window sees it.
fn export_diagnostics_from_menu(app: &AppHandle) {
    let handle = app.clone();
    let window = app.state::<AppState>().sessions.lock().unwrap().focused().map(str::to_owned);
    let window = window.unwrap_or_else(|| MAIN_WINDOW.to_owned());
    app.dialog()
        .file()
        .set_file_name(diagnostics::export_file_name())
        .add_filter("Diagnostics", &["jsonl"])
        .save_file(move |path| {
            let Some(path) = path.and_then(|path| path.into_path().ok()) else { return };
            // Asks the session processes for their state, off the dialog's thread.
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let (kind, text) = match export_diagnostics(&state, &window, &path).await {
                    Ok(()) => (MessageDialogKind::Info, "Diagnostics saved.".to_owned()),
                    Err(error) => (MessageDialogKind::Error, error),
                };
                handle.dialog().message(text).title("Export Diagnostics").kind(kind).show(|_| {});
            });
        });
}

/// A session window's heartbeat, once a second while it is visible.
#[tauri::command]
async fn diagnostics_heartbeat(
    window: WebviewWindow,
    state: State<'_, AppState>,
    beat: Heartbeat,
) -> Result<(), String> {
    state.diagnostics.heartbeat(window.label(), beat);
    Ok(())
}

/// Gives new Claude Code sessions a browser, or not; running sessions keep what they started with.
#[tauri::command]
async fn set_browser(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.browser = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// Runs new Claude Code sessions in the folder's Docker Sandbox when it has one, or on the host;
/// running sessions keep where they started.
#[tauri::command]
async fn set_claude_sandbox(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.claude_sandbox = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// Shows screenshots in the timelines of new sessions, or not; running sessions keep what they
/// started with.
#[tauri::command]
async fn set_screenshots(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.screenshots = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// Turns Verbose logging on or off, at once.
#[tauri::command]
async fn set_verbose_logging(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.verbose_logging = enabled;
        state.save(&settings);
    }
    state.diagnostics.set_verbose(enabled);
    // Session processes keep their own logs, under the same setting.
    for (_, host) in state.sessions.lock().unwrap().hosts() {
        host.set_verbose(enabled);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// Turns the background update checks on or off; turning them on checks at once.
#[tauri::command]
async fn set_check_updates(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.check_updates = enabled;
        state.save(&settings);
    }
    if enabled {
        let updates = state.updates.clone();
        let feed = GitHubFeed(app.clone());
        tauri::async_runtime::spawn(async move { updates.check(&feed).await });
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// What the windows show about updates, for a window that opens after the last change.
#[tauri::command]
async fn update_status(state: State<'_, AppState>) -> Result<UpdateState, String> {
    Ok(state.updates.state())
}

/// How long a session that ends for an update has to tell its viewers before the app stops;
/// the sharing service gives its last messages half a second to leave.
const SESSION_END_GRACE: std::time::Duration = std::time::Duration::from_secs(1);

/// Installs the ready update and starts the new version (auto-update design decision 7). Every
/// watched session is left first; the window has already asked. Hosted sessions keep running in
/// their own processes and the new version reopens their windows (`session-processes`). On
/// Windows the installer exits the app and starts the new version itself.
#[tauri::command]
async fn restart_to_update(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some((update, bytes)) = state.updates.take_ready() else {
        return Err("No update is ready.".into());
    };
    let installed = updates::install_after_session(
        || leave_all_watched(&app),
        SESSION_END_GRACE,
        || update.install(&bytes).map_err(|error| error.to_string()),
    )
    .await;
    if let Err(error) = installed {
        state.updates.restore(update, bytes);
        return Err(format!("The update could not be installed: {error}"));
    }
    app.restart();
}

/// Leaves every watched session; whether there was any.
fn leave_all_watched(app: &AppHandle) -> bool {
    let viewers = app.state::<AppState>().sessions.lock().unwrap().viewers();
    for (id, _) in &viewers {
        leave(app, *id);
    }
    !viewers.is_empty()
}

/// Ends every hosted session and waits until each has told its viewers, then leaves every
/// watched one: before quitting, so no session process is left without its app.
async fn end_all(app: &AppHandle) {
    let hosts = app.state::<AppState>().sessions.lock().unwrap().hosts();
    let mut ending = Vec::new();
    for (id, host) in hosts {
        app.state::<AppState>().sessions.lock().unwrap().remove(id);
        update_tray(app, id, None);
        ending.push(tauri::async_runtime::spawn(async move { host.end(SESSION_END_WAIT).await }));
    }
    for end in ending {
        let _ = end.await;
    }
    leave_all_watched(app);
}

/// Remembers how the user arranges the session window, as the UI gives it.
#[tauri::command]
async fn set_layout(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    layout: serde_json::Value,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.layout = Some(layout);
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// Sets the sharing service, from a domain or URL; empty means the built-in one. Takes effect for
/// the next session, and cannot change while any window has one.
#[tauri::command]
async fn set_sharing_service(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    service: String,
) -> Result<Profile, String> {
    if state.in_session() {
        return Err("The sharing service can be changed when no session runs.".into());
    }
    let service = match service.trim() {
        "" => None,
        input => Some(service::normalize(input).map_err(|error| error.to_string())?),
    };
    {
        let mut settings = state.settings.lock().unwrap();
        settings.sharing_service = service;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

#[tauri::command]
async fn set_viewer_notifications(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    kinds: settings::ViewerNotifications,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.viewer_notifications = kinds;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

#[tauri::command]
async fn set_host_notifications(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    kinds: settings::HostNotifications,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.host_notifications = kinds;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state, window.label()))
}

/// Notifies a viewer about the session its window watches (`viewer-notifications` design
/// decision 4), unless they turned that kind off; the rest is up to `notify`. A click opens
/// `open` in the window (`notification-open` design decision 2).
#[tauri::command]
async fn viewer_notify(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    kind: settings::ViewerNotificationKind,
    title: String,
    body: String,
    open: Option<notify::NoticeOpen>,
) -> Result<(), String> {
    let id = match state.sessions.lock().unwrap().of_window(window.label()) {
        Some((id, Session::Viewer(_))) => id,
        _ => return Err("This window does not watch a session.".into()),
    };
    // Only the kind and what became of it are recorded, never the text.
    let outcome = if !state.settings.lock().unwrap().viewer_notifications.allows(kind) {
        "kind-off"
    } else {
        let (title, body) = notify::viewer_text(&title, &body);
        let open = open.and_then(notify::NoticeOpen::checked);
        if try_notify(&app, id, &title, &body, open) { "shown" } else { "held" }
    };
    tracing::info!(kind = kind.name(), outcome, "viewer.notified");
    Ok(())
}

/// Whether a sharing service, as a normalized URL, can be reached.
#[tauri::command]
async fn check_sharing_service(service: String) -> Result<ServiceCheck, String> {
    Ok(services::check(&service).await)
}

/// Shows a notification about session `id`, if the user wants them and the session's window is
/// not in front. With several sessions open, the title says which one.
fn notify(app: &AppHandle, id: SessionId, title: &str, body: &str) {
    try_notify(app, id, title, body, None);
}

/// As `notify`, with what a click opens, saying whether the notification was shown.
fn try_notify(
    app: &AppHandle,
    id: SessionId,
    title: &str,
    body: &str,
    open: Option<notify::NoticeOpen>,
) -> bool {
    let state = app.state::<AppState>();
    let enabled = state.settings.lock().unwrap().notifications;
    let (window, title) = {
        let sessions = state.sessions.lock().unwrap();
        let title = notify::title(title, sessions.name(id), sessions.count());
        (sessions.window_of(id).map(str::to_owned), title)
    };
    notify::notify(app, &state.notices, id, window.as_deref(), enabled, &title, body, open)
}

#[tauri::command]
async fn remove_recent_folder(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.remove_recent(&path);
        state.save(&settings);
    }
    refresh_menu(&app);
    Ok(profile(&state, window.label()))
}

/// The name a folder goes by in titles, the tray and notifications.
fn folder_name(folder: &Path) -> String {
    folder.file_name().map_or_else(
        || folder.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Puts hosted session `id` in the tray, brings it up to date, or with `None` takes it out.
fn update_tray(app: &AppHandle, id: SessionId, entry: Option<tray::Entry>) {
    app.state::<AppState>().tray.set(id, entry);
    refresh_tray(app);
    running_changed(app);
}

/// Shows the tray's entries, on the main thread without waiting for it: building the menu waits
/// on the main thread, which may be waiting on a lock this thread holds (issue #74). Each refresh
/// reads the list when it runs, and they run there one after another, so the newest list shows
/// last.
fn refresh_tray(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let entries = handle.state::<AppState>().tray.list();
        let _ = tray::refresh(&handle, &entries);
    });
}

/// The hosted sessions without a window, for the start screen (`running-sessions-on-start`).
fn running_list(app: &AppHandle) -> Vec<running::RunningSession> {
    let state = app.state::<AppState>();
    // One lock at a time: the tray's entries first, then the sessions.
    let entries = state.tray.by_session();
    let sessions = state.sessions.lock().unwrap();
    running::list(&entries, &sessions)
}

/// Tells every window the list may have changed; start screens follow it (design decision 2).
fn running_changed(app: &AppHandle) {
    let _ = app.emit("running-sessions", running_list(app));
}

/// The start screen's list of hosted sessions without a window.
#[tauri::command]
async fn running_sessions(app: AppHandle) -> Result<Vec<running::RunningSession>, String> {
    Ok(running_list(&app))
}

/// Shows listed session `id` in this window, in place of the start screen (design decision 3).
#[tauri::command]
async fn open_running_session(
    app: AppHandle,
    window: WebviewWindow,
    id: SessionId,
) -> Result<(), String> {
    let opened = {
        let state = app.state::<AppState>();
        let mut sessions = state.sessions.lock().unwrap();
        running::check_open(&sessions, window.label(), id).map(|()| {
            sessions.attach(window.label(), id);
            sessions.name(id).map(str::to_owned)
        })
    };
    // Also when refused: the list was out of date, so refresh it where it shows.
    running_changed(&app);
    let name = opened?;
    let _ = window.set_title(&session_window_title(name.as_deref()));
    refresh_menu(&app);
    session_changed(&app);
    Ok(())
}

/// Ends listed session `id`, one an older SpecDeck started, for everyone (design decision 4).
#[tauri::command]
async fn end_running_session(app: AppHandle, id: SessionId) -> Result<(), String> {
    let checked = running::check_end(&app.state::<AppState>().sessions.lock().unwrap(), id);
    match checked {
        // end_session updates the tray, which sends the new list.
        Ok(()) => end_session(&app, id),
        Err(_) => running_changed(&app),
    }
    checked
}

/// A hosted session's summary changed; ignored once the session has left the tray.
fn update_tray_summary(app: &AppHandle, id: SessionId, summary: Summary) {
    if app.state::<AppState>().tray.set_summary(id, summary) {
        refresh_tray(app);
        running_changed(app);
    }
}

#[tauri::command]
async fn start_hosting(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    agent: AgentId,
    folder: PathBuf,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    if !state.sessions.lock().unwrap().is_free(window.label()) {
        return Err("This window already shows a session. Open a new window for another.".into());
    }
    let (username, verbose, browser, screenshots, sandbox) = {
        let settings = state.settings.lock().unwrap();
        let username = settings.username.clone().ok_or("Set your name first.")?;
        let sandbox = settings.claude_sandbox;
        (username, settings.verbose_logging, settings.browser, settings.screenshots, sandbox)
    };
    let id = state.sessions.lock().unwrap().next_id();
    let launch = client::Launch {
        agent,
        folder: folder.clone(),
        cols,
        rows,
        username,
        signalling_url: state.sharing_service(),
        environment: state.environment(),
        verbose,
        browser: browser::offered(agent, browser).then(|| browser::runner(&app, id)).flatten(),
        screenshots,
        sandbox,
    };
    let name = folder_name(&folder);
    // The menu-bar or tray icon shows the session from the start, and follows it.
    let starting = Summary {
        status: AgentStatus::Starting,
        viewers: 0,
        code: None,
        sharing: SharingState::Connecting,
    };
    update_tray(&app, id, Some(tray_entry(id, name.clone(), agent, starting, true)));
    let service = launch.signalling_url.clone();
    let ended = app.clone();
    let on_ended: client::OnEnded = Box::new(move || {
        browser::close(&ended, id);
        session_gone(&ended, id);
    });
    // The session runs in a process of its own (`session-processes`), so it outlives the app.
    let observer = host_observer(&app, id, agent);
    let client = match client::launch(&state.paths, id, launch, observer, on_ended).await {
        Ok(client) => client,
        Err(error) => {
            update_tray(&app, id, None);
            return Err(error);
        }
    };
    // Join links open in a browser when the service serves the web viewer (`web-viewer` design
    // decision 6); the app asks, so the session needs no HTTP client.
    let checked = client.clone();
    tauri::async_runtime::spawn(async move {
        checked.set_web_viewer(services::web_viewer(&service).await);
    });
    let inserted = state.sessions.lock().unwrap().insert(
        window.label(),
        id,
        Session::Host(client.clone()),
        Some(name.clone()),
    );
    if let Err(error) = inserted {
        // Another session started in this window meanwhile.
        tauri::async_runtime::spawn(async move { client.end(SESSION_END_WAIT).await });
        update_tray(&app, id, None);
        return Err(error);
    }
    // Its tray entry went up before it had a window (design decision 2).
    running_changed(&app);
    set_window_title(&window, Some(&name));
    session_changed(&app);
    let mut settings = state.settings.lock().unwrap();
    settings.add_recent(folder);
    settings.last_agent = Some(agent);
    state.save(&settings);
    drop(settings);
    refresh_menu(&app);
    Ok(())
}

/// How long ending a session process waits for it to confirm, so its viewers are told.
const SESSION_END_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// What a hosted session's notices do in the app: the tray and notifications.
fn host_observer(app: &AppHandle, id: SessionId, agent: AgentId) -> client::Observer {
    let observed = app.clone();
    Arc::new(move |notice: HostNotice| match notice {
        HostNotice::Summary(summary) => update_tray_summary(&observed, id, summary),
        HostNotice::ViewerJoined(name) => {
            notify(
                &observed,
                id,
                &format!("{name} joined"),
                &format!("{name} is watching your session."),
            );
        }
        HostNotice::ViewerLeft(name) => {
            notify(&observed, id, &format!("{name} left"), &format!("{name} stopped watching."));
        }
        HostNotice::Suggestion(name) => {
            notify(
                &observed,
                id,
                &format!("{name} suggests a prompt"),
                "Open the session to send, edit or decline it.",
            );
        }
        HostNotice::AgentWaiting { message } => {
            let body =
                message.unwrap_or_else(|| "It finished and waits for your next prompt.".into());
            notify(&observed, id, &format!("{} is waiting for you", agent.display_name()), &body);
        }
        notice => {
            if let Some(text) = notify::ask_text(&notice) {
                host_ask_notify(&observed, id, text);
            }
        }
    })
}

/// Notifies the host about the answers to their asks, unless they turned that kind off
/// (`host-ask-notifications` design decision 1).
fn host_ask_notify(app: &AppHandle, id: SessionId, text: notify::AskText) {
    let allowed =
        app.state::<AppState>().settings.lock().unwrap().host_notifications.allows(text.kind);
    // Only the kind and what became of it are recorded, never the text.
    let outcome = if !allowed {
        "kind-off"
    } else if try_notify(app, id, &text.title, &text.body, text.open) {
        "shown"
    } else {
        "held"
    };
    tracing::info!(kind = text.kind.name(), outcome, "host.notified");
}

fn tray_entry(
    id: SessionId,
    folder: String,
    agent: AgentId,
    summary: Summary,
    controllable: bool,
) -> tray::Entry {
    tray::Entry { id, folder, agent, summary, controllable }
}

/// A hosted session ended without the app asking: its grace period ran out, or its process went
/// away. Everything that showed it goes; its window goes back to the start screen.
fn session_gone(app: &AppHandle, id: SessionId) {
    let state = app.state::<AppState>();
    let (window, removed) = {
        let mut sessions = state.sessions.lock().unwrap();
        (sessions.window_of(id).map(str::to_owned), sessions.remove(id))
    };
    if removed.is_none() {
        return; // the app ended it itself
    }
    tracing::info!(session = id, "session.gone");
    update_tray(app, id, None);
    refresh_menu(app);
    session_changed(app);
    if let Some(window) = window {
        let _ =
            app.emit_to(&window, "menu", MenuCommand { id: "session-ended".into(), path: None });
    }
}

/// Runs `f` on the hosted session `window` shows, or fails if it shows none. The lock is let go
/// first: a session may call back into the app while it works.
fn with_host<T>(
    state: &AppState,
    window: &WebviewWindow,
    f: impl FnOnce(&SessionClient) -> T,
) -> Result<T, String> {
    let host = state.host_of(window.label())?;
    Ok(f(&host))
}

#[tauri::command]
async fn host_attach(
    window: WebviewWindow,
    state: State<'_, AppState>,
    terminal: Channel<InvokeResponseBody>,
    events: Channel<serde_json::Value>,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.attach(terminal, events))
}

#[tauri::command]
async fn host_input(
    window: WebviewWindow,
    state: State<'_, AppState>,
    data: String,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.input(&data))?
}

#[tauri::command]
async fn host_resize(
    window: WebviewWindow,
    state: State<'_, AppState>,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.resize(cols, rows))
}

#[tauri::command]
async fn host_set_colors(
    window: WebviewWindow,
    state: State<'_, AppState>,
    foreground: String,
    background: String,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.set_colors_hex(&foreground, &background))?
}

#[tauri::command]
async fn host_new_code(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    with_host(&state, &window, SessionClient::new_code)
}

#[tauri::command]
async fn host_remove_viewer(
    window: WebviewWindow,
    state: State<'_, AppState>,
    viewer_id: u32,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.remove_viewer(viewer_id))
}

#[tauri::command]
async fn host_set_viewer_limit(
    window: WebviewWindow,
    state: State<'_, AppState>,
    limit: u8,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.set_viewer_limit(limit))
}

#[tauri::command]
async fn host_allow_suggestions(
    window: WebviewWindow,
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.allow_suggestions(on))
}

#[tauri::command]
async fn host_share_code(
    window: WebviewWindow,
    state: State<'_, AppState>,
    whole_files: bool,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.share_code(whole_files))
}

/// Sends a suggestion (edited or not), keeps it for later, or declines it.
#[tauri::command]
async fn host_answer_suggestion(
    window: WebviewWindow,
    state: State<'_, AppState>,
    viewer_id: u32,
    id: u32,
    answer: specdeck_session::messages::SuggestionAnswer,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.answer_suggestion(viewer_id, id, answer))
}

#[tauri::command]
async fn host_allow_files(
    window: WebviewWindow,
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.allow_files(on))
}

/// The bytes and name of a file a window hands over: the body raw, the name in the
/// `x-file-name` header, percent-encoded.
fn handed_file(request: &tauri::ipc::Request<'_>) -> Result<(String, Vec<u8>), String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("The file did not arrive.".into());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|name| name.to_str().ok())
        .map(percent_decode)
        .unwrap_or_default();
    Ok((name, bytes.clone()))
}

/// `text` with `%XX` sequences decoded, as `encodeURIComponent` writes them.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Shares a file the host chose or dropped in their window; answers with the file as listed.
#[tauri::command]
async fn host_share_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<serde_json::Value, String> {
    let (name, bytes) = handed_file(&request)?;
    let host = state.host_of(window.label())?;
    host.share_file(name, &bytes).await
}

#[tauri::command]
async fn host_remove_shared_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    file: u32,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.remove_shared_file(file))
}

#[tauri::command]
async fn host_read_shared_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    file: u32,
) -> Result<tauri::ipc::Response, String> {
    let host = state.host_of(window.label())?;
    Ok(tauri::ipc::Response::new(host.read_shared_file(file).await?))
}

/// Puts a shared file into `shared/` in the session folder; the path it was written to.
#[tauri::command]
async fn host_put_shared_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    file: u32,
) -> Result<String, String> {
    let host = state.host_of(window.label())?;
    host.put_shared_file(file).await
}

/// Adds a file the host chose or dropped to the braindump; answers with the file as listed.
#[tauri::command]
async fn host_add_to_braindump(
    window: WebviewWindow,
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<serde_json::Value, String> {
    let (name, bytes) = handed_file(&request)?;
    let host = state.host_of(window.label())?;
    host.add_to_braindump(name, &bytes).await
}

#[tauri::command]
async fn host_add_note(
    window: WebviewWindow,
    state: State<'_, AppState>,
    text: String,
) -> Result<serde_json::Value, String> {
    state.host_of(window.label())?.add_note(text).await
}

#[tauri::command]
async fn host_replace_note(
    window: WebviewWindow,
    state: State<'_, AppState>,
    file: u32,
    text: String,
) -> Result<serde_json::Value, String> {
    state.host_of(window.label())?.replace_note(file, text).await
}

/// Gives the braindump to the agent; the paths written in the session folder.
#[tauri::command]
async fn host_give_braindump(
    window: WebviewWindow,
    state: State<'_, AppState>,
    prompt: Option<String>,
) -> Result<Vec<String>, String> {
    state.host_of(window.label())?.give_braindump(prompt).await
}

fn viewer_of(state: &AppState, window: &WebviewWindow) -> Result<Arc<ViewerRuntime>, String> {
    match state.sessions.lock().unwrap().of_window(window.label()) {
        Some((_, Session::Viewer(viewer))) => Ok(viewer),
        _ => Err("This window does not watch a session.".into()),
    }
}

/// Shares a file the viewer chose or dropped, if the host allows it.
#[tauri::command]
async fn viewer_share_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let (name, bytes) = handed_file(&request)?;
    let viewer = viewer_of(&state, &window)?;
    viewer.share_file(name, bytes).await
}

/// Adds a file the viewer chose or dropped to the braindump, if the host allows it.
#[tauri::command]
async fn viewer_add_to_braindump(
    window: WebviewWindow,
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let (name, bytes) = handed_file(&request)?;
    viewer_of(&state, &window)?.add_to_braindump(name, bytes).await
}

/// Adds a note to the braindump, or replaces the viewer's own note `file`.
#[tauri::command]
async fn viewer_add_note(
    window: WebviewWindow,
    state: State<'_, AppState>,
    text: String,
    file: Option<u32>,
) -> Result<(), String> {
    viewer_of(&state, &window)?.add_note(text, file).await
}

/// A shared file's bytes, fetched from the host.
#[tauri::command]
async fn viewer_open_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    file: u32,
) -> Result<tauri::ipc::Response, String> {
    let viewer = viewer_of(&state, &window)?;
    Ok(tauri::ipc::Response::new(viewer.open_file(file).await?))
}

#[tauri::command]
async fn viewer_remove_file(
    window: WebviewWindow,
    state: State<'_, AppState>,
    file: u32,
) -> Result<(), String> {
    viewer_of(&state, &window)?.remove_file(file);
    Ok(())
}

/// Saves a copy of a shared file where the person chooses; false when they cancelled.
#[tauri::command]
async fn save_shared_file(
    app: AppHandle,
    window: WebviewWindow,
    request: tauri::ipc::Request<'_>,
) -> Result<bool, String> {
    let (name, bytes) = handed_file(&request)?;
    let dialog = app.dialog().file().set_parent(&window).set_file_name(&name);
    let chosen = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
        .await
        .map_err(|error| error.to_string())?;
    let Some(path) = chosen.and_then(|path| path.into_path().ok()) else { return Ok(false) };
    std::fs::write(&path, bytes)
        .map_err(|error| format!("The copy could not be saved: {error}"))?;
    Ok(true)
}

/// Suggests a prompt to the host of the session this window watches.
#[tauri::command]
async fn suggest_prompt(
    window: WebviewWindow,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    let session = state.sessions.lock().unwrap().of_window(window.label());
    match session {
        Some((_, Session::Viewer(viewer))) => viewer.suggest(text),
        _ => Err("This window does not watch a session.".into()),
    }
}

/// Shares the host's own link with everyone in the session.
#[tauri::command]
async fn host_share_link(
    window: WebviewWindow,
    state: State<'_, AppState>,
    url: String,
    title: Option<String>,
    session_only: bool,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.share_link(url, title, session_only))
}

/// Shares a viewer's waiting link with everyone, or declines it.
#[tauri::command]
async fn host_answer_link(
    window: WebviewWindow,
    state: State<'_, AppState>,
    viewer_id: u32,
    id: u32,
    share: bool,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.answer_link(viewer_id, id, share))
}

#[tauri::command]
async fn host_remove_link(
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: u32,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.remove_link(id))
}

#[tauri::command]
async fn host_allow_links(
    window: WebviewWindow,
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.allow_links(on))
}

/// Asks everyone in the session to read something (`read-and-estimate`).
#[tauri::command]
async fn host_ask_to_read(
    window: WebviewWindow,
    state: State<'_, AppState>,
    target: specdeck_protocol::asks::ReadTarget,
    label: String,
    limit_s: u16,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.ask_to_read(target, label, limit_s))
}

/// Goes on: closes the open read request.
#[tauri::command]
async fn host_end_read(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    with_host(&state, &window, |host| host.end_read())
}

/// Asks everyone how big `task` is.
#[tauri::command]
async fn host_start_vote(
    window: WebviewWindow,
    state: State<'_, AppState>,
    task: String,
    cards: specdeck_protocol::asks::CardSet,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.start_vote(task, cards))
}

/// The host's own vote; none takes it back.
#[tauri::command]
async fn host_cast_vote(
    window: WebviewWindow,
    state: State<'_, AppState>,
    card: Option<specdeck_protocol::asks::Card>,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.cast_vote(card))
}

#[tauri::command]
async fn host_reveal_votes(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.reveal_votes())
}

#[tauri::command]
async fn host_vote_again(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    with_host(&state, &window, |host| host.vote_again())
}

#[tauri::command]
async fn host_stop_vote(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    with_host(&state, &window, |host| host.stop_vote())
}

/// Closes the vote keeping `card`.
#[tauri::command]
async fn host_keep_estimate(
    window: WebviewWindow,
    state: State<'_, AppState>,
    card: specdeck_protocol::asks::Card,
) -> Result<(), String> {
    with_host(&state, &window, |host| host.keep_estimate(card))
}

/// Answers the host's read request in the session this window watches.
#[tauri::command]
async fn answer_read(
    window: WebviewWindow,
    state: State<'_, AppState>,
    ask: u32,
    answer: specdeck_protocol::asks::ReadAnswer,
) -> Result<(), String> {
    let session = state.sessions.lock().unwrap().of_window(window.label());
    match session {
        Some((_, Session::Viewer(viewer))) => viewer.answer_read(ask, answer),
        _ => Err("This window does not watch a session.".into()),
    }
}

/// Votes in the host's vote in the session this window watches.
#[tauri::command]
async fn cast_vote(
    window: WebviewWindow,
    state: State<'_, AppState>,
    ask: u32,
    round: u16,
    card: Option<specdeck_protocol::asks::Card>,
) -> Result<(), String> {
    let session = state.sessions.lock().unwrap().of_window(window.label());
    match session {
        Some((_, Session::Viewer(viewer))) => {
            viewer.cast_vote(ask, round, card);
            Ok(())
        }
        _ => Err("This window does not watch a session.".into()),
    }
}

/// Sends the host of the session this window watches a link to share.
#[tauri::command]
async fn send_link(
    window: WebviewWindow,
    state: State<'_, AppState>,
    url: String,
    title: Option<String>,
    session_only: bool,
) -> Result<(), String> {
    let session = state.sessions.lock().unwrap().of_window(window.label());
    match session {
        Some((_, Session::Viewer(viewer))) => viewer.send_link(url, title, session_only),
        _ => Err("This window does not watch a session.".into()),
    }
}

/// Opens link `id` of this window's session in the default browser (`shared-links` design
/// decision 6). The window names the link, never an address, and the webview has no opener
/// permission of its own.
#[tauri::command]
async fn open_shared_link(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: u32,
) -> Result<(), String> {
    let session = state.sessions.lock().unwrap().of_window(window.label());
    let url = match session {
        Some((_, Session::Host(host))) => host.link(id)?,
        Some((_, Session::Viewer(viewer))) => viewer.link(id)?,
        None => return Err("This window shows no session.".into()),
    };
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
async fn host_close(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    let id = app.state::<AppState>().sessions.lock().unwrap().of_window(window.label());
    if let Some((id, Session::Host(_))) = id {
        end_session(&app, id);
    }
    set_window_title(&window, None);
    Ok(())
}

/// Ends hosted session `id`, if it runs, and everything that showed it outside its window.
fn end_session(app: &AppHandle, id: SessionId) {
    let removed = app.state::<AppState>().sessions.lock().unwrap().remove(id);
    match removed {
        Some(Session::Host(host)) => {
            tauri::async_runtime::spawn(async move { host.end(SESSION_END_WAIT).await });
        }
        Some(Session::Viewer(viewer)) => viewer.leave(),
        None => {}
    }
    update_tray(app, id, None);
    refresh_menu(app);
    session_changed(app);
}

/// Leaves watched session `id`, if it is still open.
fn leave(app: &AppHandle, id: SessionId) {
    let removed = app.state::<AppState>().sessions.lock().unwrap().remove(id);
    if let Some(Session::Viewer(viewer)) = removed {
        viewer.leave();
        session_changed(app);
    }
}

#[tauri::command]
async fn join_session(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    code: String,
    server: Option<String>,
    terminal: Channel<InvokeResponseBody>,
    events: Channel<ViewEvent>,
) -> Result<(), String> {
    if state.sessions.lock().unwrap().host_of(window.label()).is_some() {
        return Err("This window hosts a session. Open a new window to join another.".into());
    }
    let code = InviteCode::from_join_link(code.trim())
        .or_else(|_| InviteCode::parse(&code))
        .map_err(|_| "That is not an invite code. It looks like 7K2M-9QXH-4TPA.".to_string())?;
    let username = state.settings.lock().unwrap().username.clone().ok_or("Set your name first.")?;
    // A service from a join link the viewer agreed to join through; reconnects stay on it.
    let service = match server {
        Some(server) => service::normalize(&server).map_err(|error| error.to_string())?,
        None => state.sharing_service(),
    };
    let id = state.sessions.lock().unwrap().next_id();
    let notified = app.clone();
    let runtime = ViewerRuntime::join(
        code,
        username,
        service,
        PeerConfig::default(),
        terminal,
        events,
        move |reason| match reason {
            EndReason::Closed => notify(
                &notified,
                id,
                "The host ended the session",
                "You can keep reading what was shared until you leave.",
            ),
            EndReason::Removed => notify(
                &notified,
                id,
                "The host removed you",
                "You are no longer connected to the session.",
            ),
            _ => {}
        },
    );
    // Joining again in the same window (trying again after a failure) replaces the session.
    let previous = {
        let mut sessions = state.sessions.lock().unwrap();
        let previous = sessions.of_window(window.label()).map(|(previous, _)| previous);
        let previous = previous.and_then(|previous| sessions.remove(previous));
        sessions.insert(window.label(), id, Session::Viewer(Arc::new(runtime)), None)?;
        previous
    };
    if let Some(Session::Viewer(previous)) = previous {
        previous.leave();
    }
    session_changed(&app);
    Ok(())
}

#[tauri::command]
async fn leave_session(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let id = state.sessions.lock().unwrap().of_window(window.label()).map(|(id, _)| id);
    if let Some(id) = id {
        leave(&app, id);
    }
    set_window_title(&window, None);
    Ok(())
}

/// Names the session the calling window shows, for its title and its notifications; the
/// viewer's window knows the host's folder only once the session's first state arrives.
#[tauri::command]
async fn set_session_title(
    window: WebviewWindow,
    state: State<'_, AppState>,
    name: Option<String>,
) -> Result<(), String> {
    state.sessions.lock().unwrap().set_name(window.label(), name.clone());
    set_window_title(&window, name.as_deref());
    Ok(())
}

/// `shop — SpecDeck` while the window shows the session in `shop`, otherwise `SpecDeck`, so the
/// platform's window list tells session windows apart.
fn set_window_title(window: &WebviewWindow, session: Option<&str>) {
    let _ = window.set_title(&session_window_title(session));
}

fn session_window_title(session: Option<&str>) -> String {
    match session {
        Some(name) if !name.is_empty() => format!("{name} — {}", window_title()),
        _ => window_title(),
    }
}

/// Separate settings and data per profile, so two apps can run side by side for local testing
/// (`just local-dev`). Only debug builds read `SPECDECK_PROFILE`.
fn profile_dir(base: PathBuf) -> PathBuf {
    match test_profile() {
        Some(profile) => base.join("profiles").join(profile),
        None => base,
    }
}

/// Finds the hosted sessions whose processes still run (`session-processes` design decision 4):
/// sessions of this and the previous release join the app and are returned, to reopen their
/// windows; older ones go in the tray to be ended. Entries of sessions that are gone are removed,
/// and so are their files and the binaries no session uses any more.
async fn find_sessions(app: &AppHandle) -> Vec<SessionId> {
    let state = app.state::<AppState>();
    let paths = state.paths.clone();
    let mut reopen = Vec::new();
    let mut live = Vec::new();
    let mut versions = Vec::new();
    for entry in client::run_entries(&paths.run_dir) {
        let ended = app.clone();
        let id = entry.id;
        let observer = host_observer(app, id, entry.agent);
        let on_ended: client::OnEnded = Box::new(move || {
            browser::close(&ended, id);
            session_gone(&ended, id);
        });
        // A session this app's release started with a browser asks for it again; the browser
        // itself starts anew (`agent-browser`, *Browser without the app*).
        let runner = browser::runner(app, id);
        let found = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            client::connect(id, &entry.endpoint, entry.agent, observer, runner, on_ended),
        )
        .await;
        let Ok(Ok(client)) = found else {
            client::remove_entry(&paths.run_dir, &entry);
            continue;
        };
        tracing::info!(session = id, protocol = entry.protocol.0, "session.found");
        live.push(id);
        versions.push(entry.version.clone());
        let controllable = client.controllable();
        if controllable {
            // Sent at every attach, so the session always follows the setting.
            client.set_verbose(state.settings.lock().unwrap().verbose_logging);
        }
        let summary = client.summary().unwrap_or(Summary {
            status: AgentStatus::Unknown,
            viewers: 0,
            code: None,
            sharing: SharingState::Connecting,
        });
        update_tray(
            app,
            id,
            Some(tray_entry(id, entry.folder.clone(), entry.agent, summary, controllable)),
        );
        state.sessions.lock().unwrap().insert_windowless(
            id,
            Session::Host(client),
            Some(entry.folder.clone()),
        );
        if controllable {
            reopen.push(id);
        }
    }
    // Their tray entries went up before the app held them (design decision 2).
    running_changed(app);
    remove_leftover_session_files(&paths.data_dir, &live);
    for unused in client::unused_runtimes(&paths.runtime_dir, &versions) {
        let _ = std::fs::remove_dir_all(unused);
    }
    reopen
}

/// Removes the files of sessions that ended with a crash or a quit: their output log holds
/// everything the terminal printed, and is meant to live only as long as its session (design
/// decision 4). Runs at start. Each session process keeps its files in a folder named after its
/// number (`session-processes`), so the folders of the `live` sessions stay; loose files are
/// from releases before session processes.
fn remove_leftover_session_files(dir: &Path, live: &[SessionId]) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let ended_session = name.parse::<SessionId>().is_ok_and(|id| !live.contains(&id));
        if ended_session
            || ["session-", "hooks-", "statusline-", "snapshots-"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            let path = entry.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }
}

/// The `SPECDECK_PROFILE` of a debug build, unless it is the default one.
fn test_profile() -> Option<String> {
    std::env::var("SPECDECK_PROFILE")
        .ok()
        .filter(|profile| cfg!(debug_assertions) && !profile.is_empty() && profile != "default")
}

/// A join link the window has not handled yet: the code, or why the link does not work.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinLink {
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// The link's sharing service, only when it differs from the viewer's, so the window asks
    /// before joining through it (`configurable-sharing-service` design decision 5).
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    /// The service's host, for that question.
    #[serde(skip_serializing_if = "Option::is_none")]
    server_name: Option<String>,
}

/// Reads a join link for a viewer whose sharing service is `own`.
fn read_link(link: &str, own: &str) -> JoinLink {
    match InviteCode::parse_join_link(link.trim()) {
        Ok((code, server)) => {
            let server = server.filter(|server| server != own);
            JoinLink {
                code: Some(code.to_string()),
                error: None,
                server_name: server.as_deref().map(|url| service::display_name(url).to_owned()),
                server,
            }
        }
        Err(_) => JoinLink {
            error: Some("That link is not a SpecDeck invite. Ask the host for a new one.".into()),
            ..JoinLink::default()
        },
    }
}

/// A join link pasted into the start screen.
#[tauri::command]
async fn read_join_link(state: State<'_, AppState>, link: String) -> Result<JoinLink, String> {
    Ok(read_link(&link, &state.sharing_service()))
}

/// A `specdeck://` link was opened: keep it for a window, and bring that window to the front. The
/// focused window takes it when it shows no session; otherwise a new window opens for it
/// (`multiple-windows` design decision 10). The window takes it with `take_join_link`, at start
/// or when told about it.
fn open_link(app: &AppHandle, link: &str) {
    let state = app.state::<AppState>();
    let pending = read_link(link, &state.sharing_service());
    let free = state.sessions.lock().unwrap().free_window(&session_windows(app));
    let label = match free {
        Some(label) => label,
        None => new_window_label(app),
    };
    state.join_links.lock().unwrap().insert(label.clone(), pending);
    match app.get_webview_window(&label) {
        Some(window) => {
            show_window(&window);
            let _ = app.emit_to(&label, "menu", MenuCommand { id: "join-link".into(), path: None });
        }
        None => open_session_window(app, &label, None),
    }
}

/// The start screen is usable. With `SPECDECK_MEASURE_START` set, says so on standard output, for
/// `scripts/measure-footprint` (task 2.2 of release-and-verification).
#[tauri::command]
fn start_screen_shown() {
    if std::env::var_os("SPECDECK_MEASURE_START").is_some() {
        println!("specdeck: start screen shown");
    }
}

#[tauri::command]
async fn take_join_link(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<JoinLink>, String> {
    Ok(state.join_links.lock().unwrap().remove(window.label()))
}

/// The start command a new window opened with, if any: host, join or a recent folder.
#[tauri::command]
async fn take_start_command(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<MenuCommand>, String> {
    Ok(state.start_commands.lock().unwrap().remove(window.label()))
}

/// The labels of the open session windows.
fn session_windows(app: &AppHandle) -> Vec<String> {
    app.webview_windows().into_keys().filter(|label| is_session_window(label)).collect()
}

/// The label a new session window gets.
fn new_window_label(app: &AppHandle) -> String {
    let number = app.state::<AppState>().sessions.lock().unwrap().next_id();
    sessions::new_window_label(&session_windows(app), number)
}

fn show_window(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Brings the session window the user last used to the front, or another open one. With none
/// open, opens one: on a hosted session no window shows, if there is one, as the only window
/// did before. Returns the window's label.
fn focus_or_open(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let open = session_windows(app);
    let (focused, windowless) = {
        let sessions = state.sessions.lock().unwrap();
        (sessions.focused().map(str::to_owned), sessions.windowless_host())
    };
    let existing = focused.filter(|label| open.contains(label)).or_else(|| open.first().cloned());
    if let Some(window) = existing.as_deref().and_then(|label| app.get_webview_window(label)) {
        show_window(&window);
        return window.label().to_owned();
    }
    let label = new_window_label(app);
    open_session_window(app, &label, windowless);
    label
}

/// Brings session `id`'s window to the front, or reopens it on the hosted session.
fn show_session(app: &AppHandle, id: SessionId) {
    let window = app.state::<AppState>().sessions.lock().unwrap().window_of(id).map(str::to_owned);
    match window.and_then(|label| app.get_webview_window(&label)) {
        Some(window) => show_window(&window),
        None => {
            let label = new_window_label(app);
            open_session_window(app, &label, Some(id));
        }
    }
}

/// Opens a new window with `start` for a start command (host, join, a recent folder): the
/// command is kept for the window, which takes it when it has loaded.
fn open_window_for(app: &AppHandle, start: MenuCommand) {
    let label = new_window_label(app);
    app.state::<AppState>().start_commands.lock().unwrap().insert(label.clone(), start);
    open_session_window(app, &label, None);
}

/// Opens session window `label`, showing hosted session `attach` if given and otherwise the
/// start screen. It has the size of the focused window and opens a step right and down from it
/// (`multiple-windows` design decision 9); the first window's own place is restored by the
/// window-state plugin instead.
fn open_session_window(app: &AppHandle, label: &str, attach: Option<SessionId>) {
    let state = app.state::<AppState>();
    let name = attach.and_then(|id| {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.attach(label, id);
        sessions.name(id).map(str::to_owned)
    });
    if attach.is_some() {
        // It has a window again: start screens stop listing it.
        running_changed(app);
    }
    let appearance = state.settings.lock().unwrap().appearance;
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::default())
        .initialization_script(appearance_script(appearance))
        .title(session_window_title(name.as_deref()))
        .inner_size(1200.0, 800.0)
        .min_inner_size(720.0, 480.0);
    let from = state.sessions.lock().unwrap().focused().map(str::to_owned);
    let from = from.or_else(|| session_windows(app).into_iter().next());
    if let Some((size, position)) =
        from.and_then(|from| app.get_webview_window(&from)).and_then(|from| cascaded_from(&from))
    {
        builder = builder.inner_size(size.0, size.1).position(position.0, position.1);
    }
    // macOS: the toolbar is the title bar, with the window buttons centred in it.
    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(16.0, 20.0));
    }
    // Transparent only where a material can fill it; Linux stays opaque.
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        builder = builder.transparent(true);
    }
    let Ok(window) = builder.build() else { return };
    apply_zoom(&window, state.settings.lock().unwrap().zoom);
    let translucent = apply_material(&window);
    state.translucent.store(translucent, Ordering::Relaxed);
}

/// The size of `from` and the position a new window takes next to it, in logical pixels.
fn cascaded_from(from: &WebviewWindow) -> Option<((f64, f64), (f64, f64))> {
    let scale = from.scale_factor().ok()?;
    let size = from.inner_size().ok()?.to_logical::<f64>(scale);
    let position = from.outer_position().ok()?.to_logical::<f64>(scale);
    let monitor = from.current_monitor().ok()??;
    let screen_scale = monitor.scale_factor();
    let screen_position = monitor.position().to_logical::<f64>(screen_scale);
    let screen_size = monitor.size().to_logical::<f64>(screen_scale);
    let at = sessions::cascade(
        (position.x, position.y),
        (size.width, size.height),
        (screen_position.x, screen_position.y, screen_size.width, screen_size.height),
    );
    Some(((size.width, size.height), at))
}

/// Puts the platform's window material behind the page: the sidebar vibrancy on macOS, Mica on
/// Windows 11. Returns whether it worked; without it the page keeps an opaque background.
fn apply_material(window: &tauri::WebviewWindow) -> bool {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState};
        window_vibrancy::apply_vibrancy(
            window,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::FollowsWindowActiveState),
            None,
        )
        .is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        window_vibrancy::apply_mica(window, None).is_ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window;
        false
    }
}

/// Rebuilds the menu from the recent folders and whether any session is hosted.
fn refresh_menu(app: &AppHandle) {
    let state = app.state::<AppState>();
    let recent = state.settings.lock().unwrap().existing_recent();
    let hosting = state.hosting();
    let updates = state.updates.state() != UpdateState::Disabled;
    let appearance = state.settings.lock().unwrap().appearance;
    let menu_state = menu::MenuState { recent: &recent, hosting, updates, appearance };
    if let Ok(menu) = menu::build(app, &menu_state) {
        let _ = app.set_menu(menu);
    }
}

/// A menu command for the UI.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MenuCommand {
    id: String,
    /// The folder, for a recent folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
}

/// Menu commands that start a session, and so go to a window that shows none.
const START_COMMANDS: [&str; 3] = [menu::ids::HOST, menu::ids::JOIN, "recent"];

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        menu::ids::QUIT => return quit(app),
        menu::ids::SETTINGS => return open_settings_window(app),
        menu::ids::NEW_WINDOW => {
            let label = new_window_label(app);
            return open_session_window(app, &label, None);
        }
        menu::ids::ZOOM_IN => return change_zoom(app, zoom_in),
        menu::ids::ZOOM_OUT => return change_zoom(app, zoom_out),
        menu::ids::ZOOM_RESET => return change_zoom(app, |_| ZOOM_DEFAULT),
        menu::ids::CHECK_UPDATES => return check_for_updates(app),
        _ if let Some(appearance) = menu::appearance(id) => {
            return change_appearance(app, appearance);
        }
        menu::ids::EXPORT_DIAGNOSTICS => return export_diagnostics_from_menu(app),
        _ => {}
    }
    if let Some((action, session)) = tray::parse(id) {
        return match action {
            tray::Action::Open => show_session(app, session),
            tray::Action::CopyCode => copy_invite_code(app, session),
            tray::Action::End => confirm_end_session(app, session),
        };
    }
    let command = match id.strip_prefix(menu::RECENT_PREFIX) {
        Some(index) => {
            let recent = app.state::<AppState>().settings.lock().unwrap().existing_recent();
            let Some(path) = index.parse::<usize>().ok().and_then(|i| recent.get(i).cloned())
            else {
                return;
            };
            MenuCommand { id: "recent".into(), path: Some(path) }
        }
        None => MenuCommand { id: id.into(), path: None },
    };
    // Host, join and recent folders go to the focused window when it shows no session, and
    // otherwise to a new window (`multiple-windows` design decision 5).
    if START_COMMANDS.contains(&command.id.as_str()) {
        let free =
            app.state::<AppState>().sessions.lock().unwrap().free_window(&session_windows(app));
        match free.and_then(|label| app.get_webview_window(&label)) {
            Some(window) => {
                show_window(&window);
                let _ = app.emit_to(window.label(), "menu", command);
            }
            None => open_window_for(app, command),
        }
        return;
    }
    // Everything else concerns the focused window's session. The menu works with every window
    // closed too (macOS, while hosting).
    let label = focus_or_open(app);
    let _ = app.emit_to(&label, "menu", command);
}

/// Moves the zoom level, remembers it and applies it to every session window. The settings
/// window keeps its normal size: it cannot be resized to fit.
fn change_zoom(app: &AppHandle, step: impl FnOnce(u16) -> u16) {
    let state = app.state::<AppState>();
    let level = {
        let mut settings = state.settings.lock().unwrap();
        settings.zoom = step(settings.zoom);
        state.save(&settings);
        settings.zoom
    };
    for label in session_windows(app) {
        if let Some(window) = app.get_webview_window(&label) {
            apply_zoom(&window, level);
        }
    }
}

/// Saves the appearance and shows it everywhere at once (`appearance-setting` design decision 5).
fn change_appearance(app: &AppHandle, appearance: Appearance) {
    let state = app.state::<AppState>();
    {
        let mut settings = state.settings.lock().unwrap();
        settings.appearance = appearance;
        state.save(&settings);
    }
    // The window chrome, menus and dialogs, in every window.
    app.set_theme(appearance.theme());
    // The page's own colours, which the webview may not take from the window.
    for label in session_windows(app).into_iter().chain(["settings".to_owned()]) {
        let _ = app.emit_to(&label, "appearance", appearance);
    }
    // Settings shows a change made from the menu; the menu checks the new one.
    let _ =
        app.emit_to("settings", "menu", MenuCommand { id: "profile-changed".into(), path: None });
    refresh_menu(app);
}

/// Sets the page's appearance before its first paint, so a dark window never shows light.
fn appearance_script(appearance: Appearance) -> String {
    let theme = match appearance {
        Appearance::System => return String::new(),
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    };
    // At document start `<html>` may not exist yet.
    format!(
        "(function(){{var set=function(){{document.documentElement.dataset.theme='{theme}'}};\
         if(document.documentElement)return set();\
         new MutationObserver(function(_,o){{if(document.documentElement){{set();o.disconnect()}}}})\
         .observe(document,{{childList:true}})}})();"
    )
}

fn apply_zoom(window: &tauri::WebviewWindow, level: u16) {
    // A refused level (WebView2) leaves the window as it was. On macOS this needs 11 or later,
    // the bundle's minimum: older WebKit has no page zoom and crashes instead of failing.
    let _ = window.set_zoom(f64::from(level) / 100.0);
}

/// The hosted session `id`, if it still runs.
fn hosted(app: &AppHandle, id: SessionId) -> Option<Arc<SessionClient>> {
    match app.state::<AppState>().sessions.lock().unwrap().get(id)? {
        Session::Host(host) => Some(host),
        Session::Viewer(_) => None,
    }
}

/// Copies session `id`'s invite code from the tray, which works with its window closed too.
fn copy_invite_code(app: &AppHandle, id: SessionId) {
    if let Some(code) = hosted(app, id).and_then(|host| host.code()) {
        let _ = app.clipboard().write_text(code);
    }
}

/// What ending a session stops, for the questions before ending it.
fn what_stops(agent: Option<AgentId>) -> &'static str {
    match agent {
        Some(AgentId::Terminal) => "the shell stops",
        _ => "the agent stops",
    }
}

/// Ends session `id` from the tray, after the same question the window asks.
fn confirm_end_session(app: &AppHandle, id: SessionId) {
    let Some(host) = hosted(app, id) else { return };
    let handle = app.clone();
    app.dialog()
        .message(format!(
            "Everyone watching is disconnected, {}, and the invite code stops working.",
            what_stops(Some(host.agent))
        ))
        .title("End the session for everyone?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("End Session".into(), "Cancel".into()))
        .show(move |confirmed| {
            if !confirmed {
                return;
            }
            let window = handle
                .state::<AppState>()
                .sessions
                .lock()
                .unwrap()
                .window_of(id)
                .map(str::to_owned);
            end_session(&handle, id);
            // Its open window goes back to the start screen.
            if let Some(window) = window {
                let ended = MenuCommand { id: "session-ended".into(), path: None };
                let _ = handle.emit_to(&window, "menu", ended);
            }
        });
}

/// The background update checks, for as long as the app runs (spec: Background update checks).
fn start_update_checks(app: &AppHandle, updates: Arc<Updates<tauri_plugin_updater::Update>>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let feed = GitHubFeed(app.clone());
        let enabled = || app.state::<AppState>().settings.lock().unwrap().check_updates;
        let wait = async || {
            tokio::time::sleep(updates::CHECK_INTERVAL).await;
            true
        };
        updates::run_checks(&updates, &feed, enabled, wait).await;
    });
}

/// Check for Updates… in the menu: checks at once, whatever the setting, and says what it found
/// in a dialog (spec: Check for updates on demand). A newer release downloads after the dialog.
fn check_for_updates(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let updates = app.state::<AppState>().updates.clone();
        let feed = GitHubFeed(app.clone());
        match updates.find(&feed).await {
            Ok(release) => {
                show_update_outcome(&app, &Outcome::Downloading(release.version.clone()));
                updates.fetch(release).await;
            }
            Err(outcome) => show_update_outcome(&app, &outcome),
        }
    });
}

fn show_update_outcome(app: &AppHandle, outcome: &Outcome) {
    let (title, message) = updates::outcome_text(outcome, &app.package_info().version.to_string());
    let dialog = app.dialog().message(message).title(title);
    match outcome {
        Outcome::Ready(_) => {
            // Restarting goes through the window, which asks first during a session.
            let handle = app.clone();
            dialog
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Restart to Update".into(),
                    "Later".into(),
                ))
                .show(move |restart| {
                    if restart {
                        let label = focus_or_open(&handle);
                        let _ = handle.emit_to(
                            &label,
                            "menu",
                            MenuCommand { id: "restart-to-update".into(), path: None },
                        );
                    }
                });
        }
        Outcome::Failed => dialog.kind(MessageDialogKind::Warning).show(|_| {}),
        _ => dialog.kind(MessageDialogKind::Info).show(|_| {}),
    }
}

/// The question before quitting with hosted sessions: its title, message and confirm button.
/// One question covers every session in every window (`multiple-windows` design decision 8).
fn quit_question(hosted: &[AgentId], watched: usize) -> (String, String, String) {
    let also_watching = match watched {
        0 => String::new(),
        1 => " You also stop watching the session you watch.".into(),
        n => format!(" You also stop watching the {n} sessions you watch."),
    };
    if let [agent] = hosted {
        return (
            "End the session and quit?".into(),
            format!(
                "Quitting ends your session. Everyone watching is disconnected and {}.{also_watching}",
                what_stops(Some(*agent))
            ),
            "End Session and Quit".into(),
        );
    }
    let shells = hosted.iter().filter(|agent| **agent == AgentId::Terminal).count();
    let stops = match (shells, hosted.len() - shells) {
        (0, _) => "the agents stop",
        (_, 0) => "the shells stop",
        _ => "the agents and shells stop",
    };
    let n = hosted.len();
    (
        format!("End {n} sessions and quit?"),
        format!(
            "Quitting ends the {n} sessions you host. Everyone watching them is disconnected and \
             {stops}.{also_watching}"
        ),
        "End Sessions and Quit".into(),
    )
}

/// Quits, after confirming when that would end hosted sessions for everyone. Watched sessions
/// are left without asking, as before.
fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (hosted, watched) = {
        let sessions = state.sessions.lock().unwrap();
        let hosted: Vec<_> = sessions.hosts().into_iter().map(|(_, host)| host.agent).collect();
        (hosted, sessions.viewers().len())
    };
    if hosted.is_empty() {
        return app.exit(0);
    }
    let (title, message, confirm) = quit_question(&hosted, watched);
    let handle = app.clone();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(confirm, "Cancel".into()))
        .show(move |confirmed| {
            if confirmed {
                // Quit is the one way of stopping the app that ends hosted sessions
                // (`session-processes`); it waits until each has told its viewers.
                tauri::async_runtime::spawn(async move {
                    end_all(&handle).await;
                    handle.exit(0);
                });
            }
        });
}

/// The settings window: one at a time, brought to the front when open.
fn open_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let title = if cfg!(target_os = "macos") { "Settings" } else { "SpecDeck Settings" };
    let appearance = app.state::<AppState>().settings.lock().unwrap().appearance;
    let _ = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .initialization_script(appearance_script(appearance))
    .title(title)
    .inner_size(460.0, 420.0)
    .resizable(false)
    .minimizable(false)
    .maximizable(false)
    .center()
    .build();
}

/// Tells the session windows that the settings changed, so they show the new name.
fn settings_changed(app: &AppHandle) {
    for label in session_windows(app) {
        let changed = MenuCommand { id: "profile-changed".into(), path: None };
        let _ = app.emit_to(&label, "menu", changed);
    }
}

/// A session started or ended: Settings shows whether the sharing service can be changed.
fn session_changed(app: &AppHandle) {
    let _ =
        app.emit_to("settings", "menu", MenuCommand { id: "profile-changed".into(), path: None });
}

fn window_title() -> String {
    match std::env::var("SPECDECK_PROFILE").ok().filter(|p| cfg!(debug_assertions) && !p.is_empty())
    {
        Some(profile) => format!("SpecDeck ({profile})"),
        None => "SpecDeck".into(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    // One instance per user: starting the app again, or a join link on Windows and Linux, goes
    // to the running one (links through the deep-link plugin). The test profiles of
    // `just local-dev` run side by side instead.
    if test_profile().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            focus_or_open(app);
        }));
    }
    // Window size and position per profile, for the first session window only: further windows
    // open next to the focused one, and the settings window always opens centred.
    let window_state_file = match test_profile() {
        Some(profile) => format!(".window-state-{profile}.json"),
        None => ".window-state.json".into(),
    };
    let app = builder
        // Before window state, which reads its file when it is set up.
        .plugin(migrate::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_filename(window_state_file)
                .with_filter(|label| label == MAIN_WINDOW)
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // First, so everything after it is recorded.
            let log_dir = profile_dir(app.path().app_log_dir()?);
            let (diagnostics, subscriber) = Diagnostics::new(log_dir.clone(), diagnostics::APP_LOG);
            let _ = tracing::subscriber::set_global_default(subscriber);
            diagnostics::install_panic_hook("app.panicked");
            let swept = log_dir.clone();
            std::thread::spawn(move || {
                loop {
                    diagnostics::sweep(&swept, SystemTime::now());
                    std::thread::sleep(diagnostics::SWEEP_EVERY);
                }
            });
            let config_dir = profile_dir(app.path().app_config_dir()?);
            let paths = Paths {
                run_dir: profile_dir(app.path().app_data_dir()?).join("run"),
                log_dir,
                data_dir: profile_dir(app.path().app_cache_dir()?).join("sessions"),
                runtime_dir: profile_dir(app.path().app_data_dir()?).join("session-runtime"),
            };
            let settings_path = config_dir.join("settings.json");
            let environment = Arc::new(OnceLock::new());
            let background = environment.clone();
            std::thread::spawn(move || {
                let _ = background.set(Environment::detect(PROBE_TIMEOUT));
            });
            let windows = app.handle().clone();
            let updates = Arc::new(Updates::new(
                updates::this_install_updates_itself(),
                move |state: &UpdateState| {
                    let _ = windows.emit("update-changed", state);
                },
            ));
            let settings = Settings::load(&settings_path);
            diagnostics.set_verbose(settings.verbose_logging);
            // Before the first window, so it opens in the chosen appearance.
            app.handle().set_theme(settings.appearance.theme());
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                diagnostics,
                paths,
                services: Services::from_environment(),
                environment,
                sessions: Mutex::default(),
                tray: tray::Entries::default(),
                notices: Mutex::default(),
                translucent: AtomicBool::new(false),
                join_links: Mutex::default(),
                start_commands: Mutex::default(),
                updates: updates.clone(),
                versions: Mutex::default(),
            });
            if updates.state() != UpdateState::Disabled {
                start_update_checks(app.handle(), updates);
            }
            // Notices a window that stopped beating: its own thread cannot say so. Also records
            // what the flood limit left out.
            let watched = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let diagnostics = &watched.state::<AppState>().diagnostics;
                    diagnostics.check_windows();
                    diagnostics.tick();
                }
            });
            // Hosted sessions that outlived the last run get their windows back; without any,
            // the first window shows the start screen.
            let reopened = tauri::async_runtime::block_on(find_sessions(app.handle()));
            match reopened.split_first() {
                Some((first, rest)) => {
                    open_session_window(app.handle(), MAIN_WINDOW, Some(*first));
                    for id in rest {
                        let label = new_window_label(app.handle());
                        open_session_window(app.handle(), &label, Some(*id));
                    }
                }
                None => open_session_window(app.handle(), MAIN_WINDOW, None),
            }
            refresh_menu(app.handle());
            // Join links: the one the app was started with, and any opened while it runs.
            #[cfg(any(windows, target_os = "linux"))]
            let _ = app.deep_link().register_all();
            if let Ok(Some(urls)) = app.deep_link().get_current()
                && let Some(url) = urls.first()
            {
                open_link(app.handle(), url.as_str());
            }
            let links = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                if let Some(url) = event.urls().first() {
                    open_link(&links, url.as_str());
                }
            });
            // A click on a notification brings its session's window to the front, and opens
            // there what a viewer's notification is about.
            let handle = app.handle().clone();
            let _ = app.notification().on_action(move |performed| {
                let clicked = performed.notification().map(|notification| notification.id());
                let state = handle.state::<AppState>();
                let notice = state.notices.lock().unwrap().clicked(clicked);
                let running =
                    notice.filter(|(id, _)| state.sessions.lock().unwrap().get(*id).is_some());
                let opened = match running {
                    Some((id, open)) => {
                        show_session(&handle, id);
                        let window =
                            state.sessions.lock().unwrap().window_of(id).map(str::to_owned);
                        match (open, window) {
                            (Some(open), Some(window)) => {
                                handle.emit_to(&window, "notice-open", open).is_ok()
                            }
                            _ => false,
                        }
                    }
                    None => {
                        drop(focus_or_open(&handle));
                        false
                    }
                };
                // Whether something was opened, never what.
                tracing::info!(opened, "notice.clicked");
            });
            Ok(())
        })
        .on_menu_event(on_menu_event)
        .on_window_event(|window, event| {
            // Session windows only; the settings window comes and goes.
            if !is_session_window(window.label()) {
                return;
            }
            let state = window.state::<AppState>();
            match event {
                WindowEvent::Focused(true) => {
                    state.sessions.lock().unwrap().set_focused(window.label());
                }
                WindowEvent::Destroyed => {
                    state.diagnostics.window_closed(window.label());
                    state.join_links.lock().unwrap().remove(window.label());
                    state.start_commands.lock().unwrap().remove(window.label());
                    // A hosted session keeps running without its window; watching ends with it.
                    let closed = state.sessions.lock().unwrap().window_closed(window.label());
                    match closed {
                        Some(Session::Host(host)) => {
                            host.detach();
                            // Now it runs without a window: start screens list it.
                            running_changed(window.app_handle());
                        }
                        Some(Session::Viewer(viewer)) => {
                            viewer.leave();
                            session_changed(window.app_handle());
                        }
                        None => {}
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_profile,
            set_username,
            set_notifications,
            set_viewer_notifications,
            set_host_notifications,
            viewer_notify,
            set_check_updates,
            set_screen_reader,
            set_appearance,
            set_verbose_logging,
            set_browser,
            set_screenshots,
            set_claude_sandbox,
            diagnostics_status,
            diagnostics_report_terminal,
            reconnect_viewer,
            diagnostics_export,
            diagnostics_heartbeat,
            agent_versions,
            open_install_page,
            update_status,
            restart_to_update,
            set_layout,
            set_sharing_service,
            check_sharing_service,
            read_join_link,
            remove_recent_folder,
            start_hosting,
            host_attach,
            host_input,
            host_resize,
            host_set_colors,
            host_new_code,
            host_remove_viewer,
            host_set_viewer_limit,
            host_allow_suggestions,
            host_share_code,
            host_answer_suggestion,
            host_allow_files,
            host_share_file,
            host_remove_shared_file,
            host_read_shared_file,
            host_put_shared_file,
            viewer_share_file,
            host_add_to_braindump,
            host_add_note,
            host_replace_note,
            host_give_braindump,
            viewer_add_to_braindump,
            viewer_add_note,
            viewer_open_file,
            viewer_remove_file,
            save_shared_file,
            suggest_prompt,
            host_share_link,
            host_answer_link,
            host_remove_link,
            host_allow_links,
            host_ask_to_read,
            host_end_read,
            host_start_vote,
            host_cast_vote,
            host_reveal_votes,
            host_vote_again,
            host_stop_vote,
            host_keep_estimate,
            answer_read,
            cast_vote,
            send_link,
            open_shared_link,
            host_close,
            join_session,
            leave_session,
            take_join_link,
            take_start_command,
            set_session_title,
            running_sessions,
            open_running_session,
            end_running_session,
            start_screen_shown,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SpecDeck");

    app.run(|app, event| match event {
        // Closing the last window while hosting keeps the sessions (and the app) running.
        RunEvent::ExitRequested { api, code: None, .. } if app.state::<AppState>().hosting() => {
            api.prevent_exit();
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => drop(focus_or_open(app)),
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_from_the_window_are_decoded() {
        assert_eq!(percent_decode("ci%20failure%20%C3%A9.log"), "ci failure é.log");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%zz"), "a%zz");
    }

    /// An nma-platform checkout's root has a Docker Sandbox on macOS; other folders never do.
    #[cfg(unix)]
    #[test]
    fn recent_folders_say_whether_they_have_a_sandbox() {
        use std::os::unix::fs::PermissionsExt;

        let checkout = tempfile::tempdir().unwrap();
        std::fs::create_dir(checkout.path().join(".git")).unwrap();
        let launcher = checkout.path().join(sandbox::LAUNCHER);
        std::fs::create_dir_all(launcher.parent().unwrap()).unwrap();
        std::fs::write(&launcher, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(checkout.path().join(sandbox::SANDBOX_CONFIG), "").unwrap();

        assert_eq!(recent_folder(checkout.path().to_owned()).has_sandbox, sandbox::SUPPORTED);
        assert!(!recent_folder(launcher.parent().unwrap().to_owned()).has_sandbox);
    }

    #[test]
    fn terminal_is_offered_last_and_always_installed() {
        let dir = tempfile::tempdir().unwrap();
        let agents = agent_infos(&empty_environment(dir.path()));
        let terminal = agents.last().unwrap();
        assert_eq!(
            (terminal.id, terminal.name, terminal.installed),
            (AgentId::Terminal, "Terminal", true)
        );
        assert!(!terminal.has_timeline);
        assert!(agents[..agents.len() - 1].iter().all(|agent| !agent.installed));
    }

    #[test]
    fn removes_what_earlier_sessions_left_behind() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["session-a1.log", "hooks-b2.json", "statusline-c3.sh", "notes.txt"] {
            std::fs::write(dir.path().join(name), "secret output").unwrap();
        }
        std::fs::create_dir_all(dir.path().join("snapshots-d4/objects")).unwrap();
        // Session processes keep their files in a folder per session: 3 still runs, 2 ended.
        for session in ["2", "3"] {
            std::fs::create_dir_all(dir.path().join(session)).unwrap();
            std::fs::write(dir.path().join(session).join("session-x.log"), "output").unwrap();
        }
        remove_leftover_session_files(dir.path(), &[3]);
        let mut left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["3", "notes.txt"]);
        assert!(dir.path().join("3/session-x.log").exists(), "a running session keeps its log");
        // A folder that does not exist yet is fine.
        remove_leftover_session_files(&dir.path().join("missing"), &[]);
    }

    /// No agents on this PATH; the shell is found anyway.
    fn empty_environment(dir: &Path) -> Environment {
        let mut env: Environment = [("PATH", dir)].into_iter().collect();
        if cfg!(windows) {
            env.set("SystemRoot", std::env::var_os("SystemRoot").unwrap_or_default());
        }
        env
    }

    fn state(dir: &Path, overridden: Option<&str>, setting: Option<&str>) -> AppState {
        let environment = Arc::new(OnceLock::new());
        let _ = environment.set(empty_environment(dir));
        let settings = Settings { sharing_service: setting.map(Into::into), ..Settings::default() };
        AppState {
            settings_path: PathBuf::from("settings.json"),
            settings: Mutex::new(settings),
            diagnostics: Diagnostics::new(dir.join("logs"), diagnostics::APP_LOG).0,
            paths: Paths {
                run_dir: dir.join("run"),
                log_dir: dir.join("logs"),
                data_dir: dir.join("sessions"),
                runtime_dir: dir.join("session-runtime"),
            },
            services: Services {
                built_in: "wss://built-in.example.com/signal".into(),
                overridden: overridden.map(Into::into),
            },
            environment,
            sessions: Mutex::default(),
            tray: tray::Entries::default(),
            notices: Mutex::default(),
            translucent: AtomicBool::new(false),
            join_links: Mutex::default(),
            start_commands: Mutex::default(),
            updates: Arc::new(Updates::new(false, |_| {})),
            versions: Mutex::default(),
        }
    }

    #[test]
    fn the_profile_says_which_sharing_service_sessions_use() {
        let dir = tempfile::tempdir().unwrap();
        let state = |overridden, setting| state(dir.path(), overridden, setting);
        let built_in = profile(&state(None, None), MAIN_WINDOW);
        assert_eq!(built_in.sharing_service, None);
        assert_eq!(built_in.effective_service, "wss://built-in.example.com/signal");
        assert_eq!(built_in.built_in_service, "wss://built-in.example.com/signal");
        assert!(!built_in.service_overridden);
        assert!(!built_in.in_session);

        let own = profile(&state(None, Some("wss://own.example.com/signal")), MAIN_WINDOW);
        assert_eq!(own.sharing_service.as_deref(), Some("wss://own.example.com/signal"));
        assert_eq!(own.effective_service, "wss://own.example.com/signal");

        let overridden = profile(
            &state(Some("ws://127.0.0.1:1/signal"), Some("wss://own.example.com/signal")),
            MAIN_WINDOW,
        );
        assert_eq!(overridden.effective_service, "ws://127.0.0.1:1/signal");
        assert!(overridden.service_overridden);
    }

    #[test]
    fn quitting_asks_once_for_every_session() {
        let (title, message, confirm) = quit_question(&[AgentId::ClaudeCode], 0);
        assert_eq!(title, "End the session and quit?");
        assert_eq!(
            message,
            "Quitting ends your session. Everyone watching is disconnected and the agent stops."
        );
        assert_eq!(confirm, "End Session and Quit");

        let (_, message, _) = quit_question(&[AgentId::Terminal], 1);
        assert_eq!(
            message,
            "Quitting ends your session. Everyone watching is disconnected and the shell stops. \
             You also stop watching the session you watch."
        );

        let (title, message, confirm) = quit_question(&[AgentId::ClaudeCode, AgentId::Terminal], 1);
        assert_eq!(title, "End 2 sessions and quit?");
        assert_eq!(
            message,
            "Quitting ends the 2 sessions you host. Everyone watching them is disconnected and \
             the agents and shells stop. You also stop watching the session you watch."
        );
        assert_eq!(confirm, "End Sessions and Quit");

        let (_, message, _) = quit_question(&[AgentId::Codex, AgentId::Omp, AgentId::Pi], 2);
        assert!(message.contains("the 3 sessions you host"), "{message}");
        assert!(message.contains("the agents stop."), "{message}");
        assert!(message.ends_with("the 2 sessions you watch."), "{message}");
    }

    #[test]
    fn a_window_title_names_its_session() {
        assert_eq!(session_window_title(None), window_title());
        assert_eq!(session_window_title(Some("")), window_title());
        assert_eq!(session_window_title(Some("shop")), format!("shop — {}", window_title()));
    }

    #[test]
    fn the_profile_says_whether_this_window_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path(), None, None);
        let fresh = profile(&state, MAIN_WINDOW);
        assert!(!fresh.hosting && !fresh.in_session);
        assert_eq!((fresh.hosted_sessions, fresh.watched_sessions), (0, 0));
    }

    #[test]
    fn a_join_link_names_its_service_only_when_it_differs() {
        let own = "wss://own.example.com/signal";
        let same = read_link(
            "specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fown.example.com%2Fsignal",
            own,
        );
        assert_eq!(same, JoinLink { code: Some("7K2M-9QXH-4TPA".into()), ..JoinLink::default() });

        let other = read_link(
            "specdeck://join/7k2m9qxh4tpa?server=wss%3A%2F%2Fother.example.com%2Fsignal",
            own,
        );
        assert_eq!(other.code.as_deref(), Some("7K2M-9QXH-4TPA"));
        assert_eq!(other.server.as_deref(), Some("wss://other.example.com/signal"));
        assert_eq!(other.server_name.as_deref(), Some("other.example.com"));

        // Links from before services were named join through the viewer's own.
        let old = read_link("specdeck://join/7K2M-9QXH-4TPA", own);
        assert_eq!(old, JoinLink { code: Some("7K2M-9QXH-4TPA".into()), ..JoinLink::default() });

        assert!(read_link("https://example.com/7K2M-9QXH-4TPA", own).error.is_some());
    }

    #[test]
    fn web_join_links_are_read_like_app_links() {
        // The link a host copies when its service serves the web viewer (web-viewer decision 6).
        let own = "wss://own.example.com/signal";
        let same = read_link("https://own.example.com/join#7K2M-9QXH-4TPA", own);
        assert_eq!(same, JoinLink { code: Some("7K2M-9QXH-4TPA".into()), ..JoinLink::default() });

        // Another service's: the app asks before joining through it, as for other links.
        let other = read_link("https://other.example.com/join#7k2m9qxh4tpa", own);
        assert_eq!(other.code.as_deref(), Some("7K2M-9QXH-4TPA"));
        assert_eq!(other.server.as_deref(), Some("wss://other.example.com/signal"));
        assert_eq!(other.server_name.as_deref(), Some("other.example.com"));

        assert!(read_link("https://own.example.com/join?code=7K2M-9QXH-4TPA", own).error.is_some());
    }

    #[test]
    fn says_what_ending_a_session_stops() {
        assert_eq!(what_stops(Some(AgentId::Terminal)), "the shell stops");
        assert_eq!(what_stops(Some(AgentId::ClaudeCode)), "the agent stops");
        assert_eq!(what_stops(None), "the agent stops");
    }
}
