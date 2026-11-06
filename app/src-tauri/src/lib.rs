// The diagnostic log, shared with the session processes.
pub(crate) use specdeck_diag as diagnostics;
mod host;
mod menu;
mod migrate;
mod notify;
mod services;
mod sessions;
mod settings;
mod tray;
mod updates;
mod viewer;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use serde::Serialize;
use specdeck_core::agent;
use specdeck_core::environment::{Environment, PROBE_TIMEOUT};
use specdeck_core::mirror::Colors;
use specdeck_core::peer::PeerConfig;
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

use crate::diagnostics::{Diagnostics, ExportHeader, Heartbeat, SessionStatus, TerminalSnapshot};
use crate::host::{HostEvent, HostNotice, HostRuntime, SharingState, StartOptions, Summary};
use crate::services::{ServiceCheck, Services};
use crate::sessions::{MAIN_WINDOW, Session, SessionId, Sessions, is_session_window};
use crate::settings::{Settings, View, ZOOM_DEFAULT, zoom_in, zoom_out};
use crate::updates::{GitHubFeed, Outcome, UpdateState, Updates};
use crate::viewer::{EndReason, ViewEvent, ViewerRuntime};

/// An agent's version, with its executable's modification time when it was read.
type ReadVersion = (Option<SystemTime>, Option<String>);

struct AppState {
    settings_path: PathBuf,
    settings: Mutex<Settings>,
    diagnostics: Diagnostics,
    data_dir: PathBuf,
    services: Services,
    /// The user's login-shell environment, read once in the background at start.
    environment: Arc<OnceLock<Environment>>,
    /// Every hosted and watched session, and the window that shows each.
    sessions: Mutex<Sessions<HostRuntime, ViewerRuntime>>,
    /// The hosted sessions as the tray shows them, in the order they started.
    tray: Mutex<BTreeMap<SessionId, tray::Entry>>,
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
    fn host_of(&self, window: &str) -> Result<Arc<HostRuntime>, String> {
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
    /// How the user follows sessions; `None` until they answered the first-join question.
    view: Option<View>,
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
            path: found.executable,
            install_page: agent::install_page(found.agent),
        })
        .collect()
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
        recent_folders: settings
            .existing_recent()
            .into_iter()
            .map(|path| RecentFolder {
                name: path.file_name().map_or_else(
                    || path.to_string_lossy().into_owned(),
                    |name| name.to_string_lossy().into_owned(),
                ),
                path,
            })
            .collect(),
        last_agent: settings.last_agent,
        agents,
        hosting,
        hosted_sessions,
        watched_sessions,
        notifications: settings.notifications,
        view: settings.view,
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

/// This app's own side of a session.
fn status_of(session: &Session<HostRuntime, ViewerRuntime>) -> SessionStatus {
    match session {
        Session::Host(host) => SessionStatus::Host { sharing: host.sharing_status() },
        Session::Viewer(viewer) => SessionStatus::Viewer { viewer: viewer.status() },
    }
}

/// This app's own side of the session `window` shows.
fn session_status(state: &AppState, window: &str) -> SessionStatus {
    let session = state.sessions.lock().unwrap().of_window(window);
    session.map_or(SessionStatus::None, |(_, session)| status_of(&session))
}

/// What the Connection status panel of `window` shows.
#[tauri::command]
async fn diagnostics_status(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<diagnostics::Status, String> {
    Ok(state.diagnostics.status(window.label(), session_status(&state, window.label())))
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
    export_diagnostics(&state, window.label(), &path)
}

/// Exports the diagnostics as `window` sees them, with the sessions of the other windows.
fn export_diagnostics(state: &AppState, window: &str, path: &Path) -> Result<(), String> {
    let others: Vec<_> = {
        let sessions = state.sessions.lock().unwrap();
        let own = sessions.of_window(window).map(|(id, _)| id);
        let hosts = sessions.hosts().into_iter().map(|(id, host)| (id, Session::Host(host)));
        let viewers = sessions.viewers().into_iter().map(|(id, v)| (id, Session::Viewer(v)));
        hosts.chain(viewers).filter(|(id, _)| Some(*id) != own).map(|(_, s)| s).collect()
    };
    let status = state.diagnostics.status(window, session_status(state, window));
    let header = ExportHeader::new(status, &state.sharing_service(), &state.services.built_in)
        .with_other_sessions(others.iter().map(status_of).collect());
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
            let state = handle.state::<AppState>();
            let (kind, text) = match export_diagnostics(&state, &window, &path) {
                Ok(()) => (MessageDialogKind::Info, "Diagnostics saved.".to_owned()),
                Err(error) => (MessageDialogKind::Error, error),
            };
            handle.dialog().message(text).title("Export Diagnostics").kind(kind).show(|_| {});
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
/// hosted session ends for everyone and every watched one is left first; the window has already
/// asked. On Windows the installer exits the app and starts the new version itself.
#[tauri::command]
async fn restart_to_update(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some((update, bytes)) = state.updates.take_ready() else {
        return Err("No update is ready.".into());
    };
    let installed = updates::install_after_session(
        || end_or_leave_all(&app),
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

/// Ends every hosted session and leaves every watched one; whether there was any.
fn end_or_leave_all(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let (hosts, viewers) = {
        let sessions = state.sessions.lock().unwrap();
        (sessions.hosts(), sessions.viewers())
    };
    for (id, _) in &hosts {
        end_session(app, *id);
    }
    for (id, _) in &viewers {
        leave(app, *id);
    }
    !hosts.is_empty() || !viewers.is_empty()
}

#[tauri::command]
async fn set_view(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    view: View,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.view = Some(view);
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

/// Whether a sharing service, as a normalized URL, can be reached.
#[tauri::command]
async fn check_sharing_service(service: String) -> Result<ServiceCheck, String> {
    Ok(services::check(&service).await)
}

/// Shows a notification about session `id`, if the user wants them and the session's window is
/// not in front. With several sessions open, the title says which one.
fn notify(app: &AppHandle, id: SessionId, title: &str, body: &str) {
    let state = app.state::<AppState>();
    let enabled = state.settings.lock().unwrap().notifications;
    let (window, title) = {
        let sessions = state.sessions.lock().unwrap();
        let title = notify::title(title, sessions.name(id), sessions.count());
        (sessions.window_of(id).map(str::to_owned), title)
    };
    notify::notify(app, &state.notices, id, window.as_deref(), enabled, &title, body);
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
    let state = app.state::<AppState>();
    let mut entries = state.tray.lock().unwrap();
    match entry {
        Some(entry) => drop(entries.insert(id, entry)),
        None => drop(entries.remove(&id)),
    }
    // Under the lock, so two sessions changing at once cannot show an older list last.
    let entries: Vec<_> = entries.values().cloned().collect::<Vec<_>>();
    let _ = tray::refresh(app, &entries);
}

/// A hosted session's summary changed; ignored once the session has left the tray.
fn update_tray_summary(app: &AppHandle, id: SessionId, summary: Summary) {
    let entry = {
        let state = app.state::<AppState>();
        let mut entries = state.tray.lock().unwrap();
        let Some(entry) = entries.get_mut(&id) else { return };
        entry.summary = summary;
        entry.clone()
    };
    update_tray(app, id, Some(entry));
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
    let username = state.settings.lock().unwrap().username.clone().ok_or("Set your name first.")?;
    let options = StartOptions {
        agent,
        folder: folder.clone(),
        cols,
        rows,
        username,
        data_dir: state.data_dir.clone(),
        signalling_url: state.sharing_service(),
        peer: PeerConfig::default(),
    };
    let id = state.sessions.lock().unwrap().next_id();
    let name = folder_name(&folder);
    // The menu-bar or tray icon shows the session from the start, and follows it.
    let starting = Summary {
        status: AgentStatus::Starting,
        viewers: 0,
        code: None,
        sharing: SharingState::Connecting,
    };
    update_tray(&app, id, Some(tray::Entry { id, folder: name.clone(), agent, summary: starting }));
    let observed = app.clone();
    let observer: host::Observer = Arc::new(move |notice: HostNotice| match notice {
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
        HostNotice::AgentWaiting { message } => {
            let body =
                message.unwrap_or_else(|| "It finished and waits for your next prompt.".into());
            notify(&observed, id, &format!("{} is waiting for you", agent.display_name()), &body);
        }
    });
    let runtime = match HostRuntime::start(options, &state.environment(), observer).await {
        Ok(runtime) => Arc::new(runtime),
        Err(error) => {
            update_tray(&app, id, None);
            return Err(error);
        }
    };
    let inserted = state.sessions.lock().unwrap().insert(
        window.label(),
        id,
        Session::Host(runtime.clone()),
        Some(name.clone()),
    );
    if let Err(error) = inserted {
        // Another session started in this window meanwhile.
        runtime.close();
        update_tray(&app, id, None);
        return Err(error);
    }
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

/// Runs `f` on the hosted session `window` shows, or fails if it shows none. The lock is let go
/// first: a session may call back into the app while it works.
fn with_host<T>(
    state: &AppState,
    window: &WebviewWindow,
    f: impl FnOnce(&HostRuntime) -> T,
) -> Result<T, String> {
    let host = state.host_of(window.label())?;
    Ok(f(&host))
}

#[tauri::command]
async fn host_attach(
    window: WebviewWindow,
    state: State<'_, AppState>,
    terminal: Channel<InvokeResponseBody>,
    events: Channel<HostEvent>,
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
    let (Some(foreground), Some(background)) = (parse_hex(&foreground), parse_hex(&background))
    else {
        return Err("Colours must be #rrggbb.".into());
    };
    with_host(&state, &window, |host| host.set_colors(Colors { foreground, background }))
}

#[tauri::command]
async fn host_new_code(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    with_host(&state, &window, HostRuntime::new_code)
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
        Some(Session::Host(host)) => host.close(),
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

fn parse_hex(color: &str) -> Option<specdeck_core::mirror::Rgb> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(specdeck_core::mirror::Rgb { r: channel(0)?, g: channel(2)?, b: channel(4)? })
}

/// Separate settings and data per profile, so two apps can run side by side for local testing
/// (`just local-dev`). Only debug builds read `SPECDECK_PROFILE`.
fn profile_dir(base: PathBuf) -> PathBuf {
    match test_profile() {
        Some(profile) => base.join("profiles").join(profile),
        None => base,
    }
}

/// Removes the files of sessions that ended with a crash or a quit: their output log holds
/// everything the terminal printed, and is meant to live only as long as its session (design
/// decision 4). Runs at start, when no session of this profile can be running.
fn remove_leftover_session_files(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if ["session-", "hooks-", "statusline-", "snapshots-"]
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
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::default())
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
    if let Ok(menu) = menu::build(app, &menu::MenuState { recent: &recent, hosting, updates }) {
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

fn apply_zoom(window: &tauri::WebviewWindow, level: u16) {
    // A refused level (WebView2) leaves the window as it was. On macOS this needs 11 or later,
    // the bundle's minimum: older WebKit has no page zoom and crashes instead of failing.
    let _ = window.set_zoom(f64::from(level) / 100.0);
}

/// The hosted session `id`, if it still runs.
fn hosted(app: &AppHandle, id: SessionId) -> Option<Arc<HostRuntime>> {
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
                end_or_leave_all(&handle);
                handle.exit(0);
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
    let _ = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
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
            std::thread::spawn(move || {
                loop {
                    diagnostics::sweep(&log_dir, SystemTime::now());
                    std::thread::sleep(diagnostics::SWEEP_EVERY);
                }
            });
            let config_dir = profile_dir(app.path().app_config_dir()?);
            let data_dir = profile_dir(app.path().app_cache_dir()?).join("sessions");
            remove_leftover_session_files(&data_dir);
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
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                diagnostics,
                data_dir,
                services: Services::from_environment(),
                environment,
                sessions: Mutex::default(),
                tray: Mutex::default(),
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
            // Notices a window that stopped beating: its own thread cannot say so.
            let watched = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    watched.state::<AppState>().diagnostics.check_windows();
                }
            });
            open_session_window(app.handle(), MAIN_WINDOW, None);
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
            // A click on a notification brings its session's window to the front.
            let handle = app.handle().clone();
            let _ = app.notification().on_action(move |performed| {
                let clicked = performed.notification().map(|notification| notification.id());
                let state = handle.state::<AppState>();
                let session = state.notices.lock().unwrap().clicked(clicked);
                let running =
                    session.filter(|id| state.sessions.lock().unwrap().get(*id).is_some());
                match running {
                    Some(id) => show_session(&handle, id),
                    None => drop(focus_or_open(&handle)),
                }
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
                        Some(Session::Host(host)) => host.detach(),
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
            set_check_updates,
            set_screen_reader,
            set_verbose_logging,
            diagnostics_status,
            diagnostics_report_terminal,
            reconnect_viewer,
            diagnostics_export,
            diagnostics_heartbeat,
            agent_versions,
            open_install_page,
            update_status,
            restart_to_update,
            set_view,
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
            host_close,
            join_session,
            leave_session,
            take_join_link,
            take_start_command,
            set_session_title,
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
        remove_leftover_session_files(dir.path());
        let left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(left, ["notes.txt"]);
        // A folder that does not exist yet is fine.
        remove_leftover_session_files(&dir.path().join("missing"));
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
            data_dir: PathBuf::from("sessions"),
            services: Services {
                built_in: "wss://built-in.example.com/signal".into(),
                overridden: overridden.map(Into::into),
            },
            environment,
            sessions: Mutex::default(),
            tray: Mutex::default(),
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
