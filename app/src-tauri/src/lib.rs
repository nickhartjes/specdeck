mod host;
mod menu;
mod migrate;
mod notify;
mod services;
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
use specdeck_core::mirror::Colors;
use specdeck_core::peer::PeerConfig;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::service;
use specdeck_protocol::{AgentId, AgentStatus};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::menu::MenuEvent;
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::StateFlags;

use crate::host::{HostEvent, HostNotice, HostRuntime, SharingState, StartOptions, Summary};
use crate::services::{ServiceCheck, Services};
use crate::settings::{Settings, View, ZOOM_DEFAULT, zoom_in, zoom_out};
use crate::updates::{GitHubFeed, Outcome, UpdateState, Updates};
use crate::viewer::{EndReason, ViewEvent, ViewerRuntime};

/// An agent's version, with its executable's modification time when it was read.
type ReadVersion = (Option<SystemTime>, Option<String>);

struct AppState {
    settings_path: PathBuf,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
    services: Services,
    /// The user's login-shell environment, read once in the background at start.
    environment: Arc<OnceLock<Environment>>,
    host: Mutex<Option<HostRuntime>>,
    viewer: Mutex<Option<ViewerRuntime>>,
    /// The main window shows the desktop through it (macOS vibrancy, Windows 11 Mica).
    translucent: AtomicBool,
    /// A join link the window has not handled yet.
    join_link: Mutex<Option<JoinLink>>,
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

    fn in_session(&self) -> bool {
        self.host.lock().unwrap().is_some() || self.viewer.lock().unwrap().is_some()
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
    /// Set while a session is hosted, so a reopened window goes straight back to it.
    hosting: bool,
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
    /// Hosting or viewing; the sharing service cannot change meanwhile.
    in_session: bool,
    /// This install updates itself, so Settings offers the update setting.
    updates_itself: bool,
    check_updates: bool,
    /// Screen readers read the terminal.
    screen_reader: bool,
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

fn profile(state: &AppState) -> Profile {
    let settings = state.settings.lock().unwrap().clone();
    let agents = agent_infos(&state.environment());
    // Each takes the session locks itself, so not inside the struct expression below, where the
    // guards would live until its end.
    let hosting = state.host.lock().unwrap().is_some();
    let in_session = state.in_session();
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
async fn get_profile(state: State<'_, AppState>) -> Result<Profile, String> {
    Ok(profile(&state))
}

#[tauri::command]
async fn set_username(
    app: AppHandle,
    state: State<'_, AppState>,
    username: String,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.set_username(&username);
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state))
}

#[tauri::command]
async fn set_notifications(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.notifications = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state))
}

#[tauri::command]
async fn set_screen_reader(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.screen_reader = enabled;
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state))
}

/// Turns the background update checks on or off; turning them on checks at once.
#[tauri::command]
async fn set_check_updates(
    app: AppHandle,
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
    Ok(profile(&state))
}

/// What the windows show about updates, for a window that opens after the last change.
#[tauri::command]
async fn update_status(state: State<'_, AppState>) -> Result<UpdateState, String> {
    Ok(state.updates.state())
}

/// How long a session that ends for an update has to tell its viewers before the app stops;
/// the sharing service gives its last messages half a second to leave.
const SESSION_END_GRACE: std::time::Duration = std::time::Duration::from_secs(1);

/// Installs the ready update and starts the new version (auto-update design decision 7). A
/// hosted session ends for everyone and a watched one is left first; the window has already
/// asked. On Windows the installer exits the app and starts the new version itself.
#[tauri::command]
async fn restart_to_update(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some((update, bytes)) = state.updates.take_ready() else {
        return Err("No update is ready.".into());
    };
    let installed = updates::install_after_session(
        || end_or_leave_session(&app),
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

/// Ends a hosted session or leaves a watched one; whether there was one.
fn end_or_leave_session(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let hosting = state.host.lock().unwrap().is_some();
    if hosting {
        end_session(app);
    }
    let viewer = state.viewer.lock().unwrap().take();
    let watching = viewer.is_some();
    if let Some(viewer) = viewer {
        viewer.leave();
        session_changed(app);
    }
    hosting || watching
}

#[tauri::command]
async fn set_view(
    app: AppHandle,
    state: State<'_, AppState>,
    view: View,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.view = Some(view);
        state.save(&settings);
    }
    settings_changed(&app);
    Ok(profile(&state))
}

/// Sets the sharing service, from a domain or URL; empty means the built-in one. Takes effect for
/// the next session.
#[tauri::command]
async fn set_sharing_service(
    app: AppHandle,
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
    Ok(profile(&state))
}

/// Whether a sharing service, as a normalized URL, can be reached.
#[tauri::command]
async fn check_sharing_service(service: String) -> Result<ServiceCheck, String> {
    Ok(services::check(&service).await)
}

/// Shows a notification, if the user wants them and the window is not in front.
fn notify(app: &AppHandle, title: &str, body: &str) {
    let enabled = app.state::<AppState>().settings.lock().unwrap().notifications;
    notify::notify(app, enabled, title, body);
}

#[tauri::command]
async fn remove_recent_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<Profile, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.remove_recent(&path);
        state.save(&settings);
    }
    refresh_menu(&app);
    Ok(profile(&state))
}

#[tauri::command]
async fn start_hosting(
    app: AppHandle,
    state: State<'_, AppState>,
    agent: AgentId,
    folder: PathBuf,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    if state.host.lock().unwrap().is_some() {
        return Err("A session is already running. End it before starting another.".into());
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
    // The menu-bar or tray icon shows the session from the start, and follows it.
    let folder_name = folder.file_name().map_or_else(
        || folder.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let starting = Summary {
        status: AgentStatus::Starting,
        viewers: 0,
        code: None,
        sharing: SharingState::Connecting,
    };
    let _ = tray::show(&app, &folder_name, agent, &starting);
    let observed = app.clone();
    let observer: host::Observer = Arc::new(move |notice: HostNotice| match notice {
        HostNotice::Summary(summary) => {
            tray::update(&observed, &folder_name, agent, &summary);
        }
        HostNotice::ViewerJoined(name) => {
            notify(
                &observed,
                &format!("{name} joined"),
                &format!("{name} is watching your session."),
            );
        }
        HostNotice::ViewerLeft(name) => {
            notify(&observed, &format!("{name} left"), &format!("{name} stopped watching."));
        }
        HostNotice::AgentWaiting { message } => {
            let body =
                message.unwrap_or_else(|| "It finished and waits for your next prompt.".into());
            notify(&observed, &format!("{} is waiting for you", agent.display_name()), &body);
        }
    });
    let runtime = match HostRuntime::start(options, &state.environment(), observer).await {
        Ok(runtime) => runtime,
        Err(error) => {
            tray::remove(&app);
            return Err(error);
        }
    };
    *state.host.lock().unwrap() = Some(runtime);
    session_changed(&app);
    let mut settings = state.settings.lock().unwrap();
    settings.add_recent(folder);
    settings.last_agent = Some(agent);
    state.save(&settings);
    drop(settings);
    refresh_menu(&app);
    Ok(())
}

/// Runs `f` on the hosted session, or fails if there is none.
fn with_host<T>(state: &AppState, f: impl FnOnce(&HostRuntime) -> T) -> Result<T, String> {
    state.host.lock().unwrap().as_ref().map(f).ok_or_else(|| "No session is running.".into())
}

#[tauri::command]
async fn host_attach(
    state: State<'_, AppState>,
    terminal: Channel<InvokeResponseBody>,
    events: Channel<HostEvent>,
) -> Result<(), String> {
    with_host(&state, |host| host.attach(terminal, events))
}

#[tauri::command]
async fn host_input(state: State<'_, AppState>, data: String) -> Result<(), String> {
    with_host(&state, |host| host.input(&data))?
}

#[tauri::command]
async fn host_resize(state: State<'_, AppState>, cols: u16, rows: u16) -> Result<(), String> {
    with_host(&state, |host| host.resize(cols, rows))
}

#[tauri::command]
async fn host_set_colors(
    state: State<'_, AppState>,
    foreground: String,
    background: String,
) -> Result<(), String> {
    let (Some(foreground), Some(background)) = (parse_hex(&foreground), parse_hex(&background))
    else {
        return Err("Colours must be #rrggbb.".into());
    };
    with_host(&state, |host| host.set_colors(Colors { foreground, background }))
}

#[tauri::command]
async fn host_new_code(state: State<'_, AppState>) -> Result<(), String> {
    with_host(&state, HostRuntime::new_code)
}

#[tauri::command]
async fn host_remove_viewer(state: State<'_, AppState>, viewer_id: u32) -> Result<(), String> {
    with_host(&state, |host| host.remove_viewer(viewer_id))
}

#[tauri::command]
async fn host_set_viewer_limit(state: State<'_, AppState>, limit: u8) -> Result<(), String> {
    with_host(&state, |host| host.set_viewer_limit(limit))
}

#[tauri::command]
async fn host_close(app: AppHandle) -> Result<(), String> {
    end_session(&app);
    Ok(())
}

/// Ends the hosted session, if any, and everything that showed it outside the window.
fn end_session(app: &AppHandle) {
    if let Some(host) = app.state::<AppState>().host.lock().unwrap().take() {
        host.close();
    }
    tray::remove(app);
    refresh_menu(app);
    session_changed(app);
}

#[tauri::command]
async fn join_session(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
    server: Option<String>,
    terminal: Channel<InvokeResponseBody>,
    events: Channel<ViewEvent>,
) -> Result<(), String> {
    let code = InviteCode::from_join_link(code.trim())
        .or_else(|_| InviteCode::parse(&code))
        .map_err(|_| "That is not an invite code. It looks like 7K2M-9QXH-4TPA.".to_string())?;
    let username = state.settings.lock().unwrap().username.clone().ok_or("Set your name first.")?;
    // A service from a join link the viewer agreed to join through; reconnects stay on it.
    let service = match server {
        Some(server) => service::normalize(&server).map_err(|error| error.to_string())?,
        None => state.sharing_service(),
    };
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
                "The host ended the session",
                "You can keep reading what was shared until you leave.",
            ),
            EndReason::Removed => notify(
                &notified,
                "The host removed you",
                "You are no longer connected to the session.",
            ),
            _ => {}
        },
    );
    if let Some(previous) = state.viewer.lock().unwrap().replace(runtime) {
        previous.leave();
    }
    session_changed(&app);
    Ok(())
}

#[tauri::command]
async fn leave_session(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(viewer) = state.viewer.lock().unwrap().take() {
        viewer.leave();
    }
    session_changed(&app);
    Ok(())
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

/// A `specdeck://` link was opened: keep it for the window and bring the window to the front.
/// The window takes it with `take_join_link`, at start or when told about it.
fn open_link(app: &AppHandle, link: &str) {
    let state = app.state::<AppState>();
    let pending = read_link(link, &state.sharing_service());
    *state.join_link.lock().unwrap() = Some(pending);
    open_main_window(app);
    let _ = app.emit_to("main", "menu", MenuCommand { id: "join-link".into(), path: None });
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
async fn take_join_link(state: State<'_, AppState>) -> Result<Option<JoinLink>, String> {
    Ok(state.join_link.lock().unwrap().take())
}

fn open_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title(window_title())
        .inner_size(1200.0, 800.0)
        .min_inner_size(720.0, 480.0);
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
    apply_zoom(&window, app.state::<AppState>().settings.lock().unwrap().zoom);
    let translucent = apply_material(&window);
    app.state::<AppState>().translucent.store(translucent, Ordering::Relaxed);
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

/// Rebuilds the menu from the recent folders and whether a session is hosted.
fn refresh_menu(app: &AppHandle) {
    let state = app.state::<AppState>();
    let recent = state.settings.lock().unwrap().existing_recent();
    let hosting = state.host.lock().unwrap().is_some();
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

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        menu::ids::QUIT => return quit(app),
        menu::ids::SETTINGS => return open_settings_window(app),
        menu::ids::ZOOM_IN => return change_zoom(app, zoom_in),
        menu::ids::ZOOM_OUT => return change_zoom(app, zoom_out),
        menu::ids::ZOOM_RESET => return change_zoom(app, |_| ZOOM_DEFAULT),
        menu::ids::CHECK_UPDATES => return check_for_updates(app),
        tray::ids::OPEN => return open_main_window(app),
        tray::ids::COPY_CODE => return copy_invite_code(app),
        tray::ids::END => return confirm_end_session(app),
        _ => {}
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
    // The menu works with the window closed too (macOS, while hosting).
    open_main_window(app);
    let _ = app.emit("menu", command);
}

/// Moves the zoom level, remembers it and applies it to the main window if it is open. The
/// settings window keeps its normal size: it cannot be resized to fit.
fn change_zoom(app: &AppHandle, step: impl FnOnce(u16) -> u16) {
    let state = app.state::<AppState>();
    let level = {
        let mut settings = state.settings.lock().unwrap();
        settings.zoom = step(settings.zoom);
        state.save(&settings);
        settings.zoom
    };
    if let Some(window) = app.get_webview_window("main") {
        apply_zoom(&window, level);
    }
}

fn apply_zoom(window: &tauri::WebviewWindow, level: u16) {
    // A refused level (WebView2) leaves the window as it was. On macOS this needs 11 or later,
    // the bundle's minimum: older WebKit has no page zoom and crashes instead of failing.
    let _ = window.set_zoom(f64::from(level) / 100.0);
}

/// Copies the invite code from the tray, which works with the window closed too.
fn copy_invite_code(app: &AppHandle) {
    let code = app.state::<AppState>().host.lock().unwrap().as_ref().and_then(HostRuntime::code);
    if let Some(code) = code {
        let _ = app.clipboard().write_text(code);
    }
}

/// What the hosted session runs, if anything.
fn hosted_agent(app: &AppHandle) -> Option<AgentId> {
    app.state::<AppState>().host.lock().unwrap().as_ref().map(|host| host.agent)
}

/// What ending a session stops, for the questions before ending it.
fn what_stops(agent: Option<AgentId>) -> &'static str {
    match agent {
        Some(AgentId::Terminal) => "the shell stops",
        _ => "the agent stops",
    }
}

/// Ends the session from the tray, after the same question the window asks.
fn confirm_end_session(app: &AppHandle) {
    let handle = app.clone();
    app.dialog()
        .message(format!(
            "Everyone watching is disconnected, {}, and the invite code stops working.",
            what_stops(hosted_agent(app))
        ))
        .title("End the session for everyone?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("End Session".into(), "Cancel".into()))
        .show(move |confirmed| {
            if confirmed {
                end_session(&handle);
                // An open window goes back to the start screen.
                let _ = handle.emit("menu", MenuCommand { id: "session-ended".into(), path: None });
            }
        });
}

/// Quits, after confirming when that would end a hosted session for everyone.
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
                        open_main_window(&handle);
                        let _ = handle.emit_to(
                            "main",
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

fn quit(app: &AppHandle) {
    if app.state::<AppState>().host.lock().unwrap().is_none() {
        return app.exit(0);
    }
    let handle = app.clone();
    app.dialog()
        .message(format!(
            "Quitting ends your session. Everyone watching is disconnected and {}.",
            what_stops(hosted_agent(app))
        ))
        .title("End the session and quit?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "End Session and Quit".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            if confirmed {
                end_session(&handle);
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

/// Tells the main window that the settings changed, so it shows the new name.
fn settings_changed(app: &AppHandle) {
    let _ = app.emit_to("main", "menu", MenuCommand { id: "profile-changed".into(), path: None });
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
            open_main_window(app);
        }));
    }
    // Window size and position per profile; the settings window always opens centred.
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
                .with_denylist(&["settings"])
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
            app.manage(AppState {
                settings: Mutex::new(Settings::load(&settings_path)),
                settings_path,
                data_dir,
                services: Services::from_environment(),
                environment,
                host: Mutex::new(None),
                viewer: Mutex::new(None),
                translucent: AtomicBool::new(false),
                join_link: Mutex::new(None),
                updates: updates.clone(),
                versions: Mutex::default(),
            });
            if updates.state() != UpdateState::Disabled {
                start_update_checks(app.handle(), updates);
            }
            open_main_window(app.handle());
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
            // A click on a notification brings the session to the front.
            let handle = app.handle().clone();
            let _ = app.notification().on_action(move |_| open_main_window(&handle));
            Ok(())
        })
        .on_menu_event(on_menu_event)
        .on_window_event(|window, event| {
            // Only the main window shows a session; the settings window comes and goes.
            if let WindowEvent::Destroyed = event
                && window.label() == "main"
            {
                let state = window.state::<AppState>();
                // A hosted session keeps running without its window; viewing ends with it.
                if let Some(host) = state.host.lock().unwrap().as_ref() {
                    host.detach();
                }
                if let Some(viewer) = state.viewer.lock().unwrap().take() {
                    viewer.leave();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_profile,
            set_username,
            set_notifications,
            set_check_updates,
            set_screen_reader,
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
            start_screen_shown,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SpecDeck");

    app.run(|app, event| match event {
        // Closing the last window while hosting keeps the session (and the app) running.
        RunEvent::ExitRequested { api, code: None, .. }
            if app.state::<AppState>().host.lock().unwrap().is_some() =>
        {
            api.prevent_exit();
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => open_main_window(app),
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
            data_dir: PathBuf::from("sessions"),
            services: Services {
                built_in: "wss://built-in.example.com/signal".into(),
                overridden: overridden.map(Into::into),
            },
            environment,
            host: Mutex::new(None),
            viewer: Mutex::new(None),
            translucent: AtomicBool::new(false),
            join_link: Mutex::new(None),
            updates: Arc::new(Updates::new(false, |_| {})),
            versions: Mutex::default(),
        }
    }

    #[test]
    fn the_profile_says_which_sharing_service_sessions_use() {
        let dir = tempfile::tempdir().unwrap();
        let state = |overridden, setting| state(dir.path(), overridden, setting);
        let built_in = profile(&state(None, None));
        assert_eq!(built_in.sharing_service, None);
        assert_eq!(built_in.effective_service, "wss://built-in.example.com/signal");
        assert_eq!(built_in.built_in_service, "wss://built-in.example.com/signal");
        assert!(!built_in.service_overridden);
        assert!(!built_in.in_session);

        let own = profile(&state(None, Some("wss://own.example.com/signal")));
        assert_eq!(own.sharing_service.as_deref(), Some("wss://own.example.com/signal"));
        assert_eq!(own.effective_service, "wss://own.example.com/signal");

        let overridden =
            profile(&state(Some("ws://127.0.0.1:1/signal"), Some("wss://own.example.com/signal")));
        assert_eq!(overridden.effective_service, "ws://127.0.0.1:1/signal");
        assert!(overridden.service_overridden);
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
