//! Serves one hosted session to the app (`session-processes` design decisions 3 to 5).
//!
//! One app is attached at a time; a new connection replaces the previous one. While no app is
//! attached the session keeps running and keeps the notices for the next app, and after the
//! grace period it ends itself as if the host had ended it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use specdeck_core::browser::{BrowserAction, BrowserHandler, BrowserResult};
use specdeck_core::peer::PeerConfig;
use specdeck_core::sharing::{Answer, AskCommand};
use specdeck_diag::Diagnostics;
use specdeck_local::{Compat, Connection, Frame, Frozen, Listener, Protocol, compat};
use specdeck_protocol::AgentId;
use tokio::sync::{Notify, mpsc, oneshot};

use crate::messages::{Command, Event, LinkAnswer, SuggestionAnswer};
use crate::runtime::{HostNotice, HostRuntime, Observer, StartOptions};

/// How long a session process waits for the app's `start` after it was launched.
pub const START_TIMEOUT: Duration = Duration::from_secs(60);

/// How long an ended session gives sharing to tell its viewers before the process exits.
const END_GRACE: Duration = Duration::from_secs(1);

/// How long the app may take to run a browser action (`claude-browser` design decision 6): a
/// page gets 30 seconds to load in the app, the rest answer at once.
const PAGE_TIMEOUT: Duration = Duration::from_secs(35);
const ACTION_TIMEOUT: Duration = Duration::from_secs(10);

const BROWSER_UNAVAILABLE: &str = "The browser isn't available until SpecDeck is open again.";

pub struct Options {
    /// The session's number in the app, which names its files.
    pub id: u32,
    /// Where `<id>.json` says the session runs (design decision 4).
    pub run_dir: PathBuf,
    /// The socket's path, or the pipe's name on Windows.
    pub endpoint: String,
    /// How long the session runs without an app before it ends itself.
    pub grace: Duration,
    pub start_timeout: Duration,
    pub peer: PeerConfig,
    pub diagnostics: Option<Arc<Diagnostics>>,
}

/// What `<run dir>/<id>.json` holds, for the app to find the session at its next start.
#[derive(Serialize)]
struct RunEntry<'a> {
    version: &'a str,
    protocol: Protocol,
    pid: u32,
    /// Unix seconds.
    started: u64,
    agent: AgentId,
    /// The folder's name only, for the tray.
    folder: String,
    endpoint: &'a str,
}

struct Shared {
    options: Options,
    runtime: Mutex<Option<Arc<HostRuntime>>>,
    /// The attached app's outgoing frames, if one is attached.
    app: Mutex<Option<mpsc::UnboundedSender<Frame>>>,
    /// Counts connections; the latest one is the attached app.
    connection: AtomicU64,
    /// Notices that came while no app was attached, for the next one.
    held: Mutex<Vec<HostNotice>>,
    /// Browser actions sent to the app, by id, waiting for its answer.
    browser: Mutex<HashMap<u64, oneshot::Sender<BrowserResult>>>,
    browser_ids: AtomicU64,
    ended: Notify,
}

impl Shared {
    fn send(&self, event: &Event) {
        if let (Some(app), Ok(frame)) = (&*self.app.lock().unwrap(), Frame::control(event)) {
            let _ = app.send(frame);
        }
    }

    /// Asks the attached app to run `action`; answers at once when no app is attached.
    async fn ask_browser(&self, action: BrowserAction) -> BrowserResult {
        let id = self.browser_ids.fetch_add(1, Ordering::SeqCst);
        let timeout = if action.loads_page() { PAGE_TIMEOUT } else { ACTION_TIMEOUT };
        let (answer, answered) = oneshot::channel();
        {
            // Registered and sent under the app's lock, so a detach in between fails it.
            let app = self.app.lock().unwrap();
            let Some(app) = &*app else { return BrowserResult::error(BROWSER_UNAVAILABLE) };
            let Ok(frame) = Frame::control(&Event::Browser { id, action }) else {
                return BrowserResult::error("The browser action could not be sent.");
            };
            self.browser.lock().unwrap().insert(id, answer);
            if app.send(frame).is_err() {
                self.browser.lock().unwrap().remove(&id);
                return BrowserResult::error(BROWSER_UNAVAILABLE);
            }
        }
        match tokio::time::timeout(timeout, answered).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => BrowserResult::error(BROWSER_UNAVAILABLE),
            Err(_) => {
                self.browser.lock().unwrap().remove(&id);
                BrowserResult::error(format!(
                    "The browser did not answer within {} seconds.",
                    timeout.as_secs()
                ))
            }
        }
    }

    /// The app's answer to a browser action.
    fn browser_answered(&self, id: u64, result: BrowserResult) {
        if let Some(answer) = self.browser.lock().unwrap().remove(&id) {
            let _ = answer.send(result);
        }
    }

    /// The app is gone: what it was asked will not be answered.
    fn fail_browser_actions(&self) {
        self.browser.lock().unwrap().clear();
    }

    fn runtime(&self) -> Option<Arc<HostRuntime>> {
        self.runtime.lock().unwrap().clone()
    }

    fn entry_path(&self) -> PathBuf {
        self.options.run_dir.join(format!("{}.json", self.options.id))
    }

    /// Ends the session for everyone, once.
    fn end(&self) {
        let Some(runtime) = self.runtime.lock().unwrap().take() else {
            self.ended.notify_one();
            return;
        };
        tracing::info!(session = self.options.id, "session.ended");
        runtime.close();
        let _ = std::fs::remove_file(self.entry_path());
        self.send(&Event::Ended);
        self.ended.notify_one();
    }
}

/// Serves the session until it ends: by `end`, by the grace period, by a signal, or because no
/// app started it in time.
pub async fn serve(options: Options) -> std::io::Result<()> {
    #[cfg(unix)]
    let listener = Listener::bind(std::path::Path::new(&options.endpoint))?;
    #[cfg(windows)]
    let mut listener = Listener::bind(&options.endpoint)?;
    let start_timeout = options.start_timeout;
    let shared = Arc::new(Shared {
        options,
        runtime: Mutex::new(None),
        app: Mutex::new(None),
        connection: AtomicU64::new(0),
        held: Mutex::new(Vec::new()),
        browser: Mutex::new(HashMap::new()),
        browser_ids: AtomicU64::new(1),
        ended: Notify::new(),
    });

    // Nobody started the session in time: there is nothing to keep running.
    let unstarted = shared.clone();
    tokio::spawn(async move {
        tokio::time::sleep(start_timeout).await;
        if unstarted.runtime().is_none() {
            tracing::warn!(session = unstarted.options.id, "session.not_started");
            unstarted.end();
        }
    });
    spawn_signal_handler(shared.clone());

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let connection = accepted?;
                tokio::spawn(handle(connection, shared.clone()));
            }
            () = shared.ended.notified() => break,
        }
    }
    tokio::time::sleep(END_GRACE).await;
    Ok(())
}

#[cfg(unix)]
fn spawn_signal_handler(shared: Arc<Shared>) {
    use tokio::signal::unix::{SignalKind, signal};
    tokio::spawn(async move {
        let Ok(mut terminate) = signal(SignalKind::terminate()) else { return };
        terminate.recv().await;
        shared.end();
    });
}

#[cfg(windows)]
fn spawn_signal_handler(shared: Arc<Shared>) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            shared.end();
        }
    });
}

async fn handle(connection: Connection, shared: Arc<Shared>) {
    let Connection { mut reader, mut writer } = connection;
    let (outgoing, mut queue) = mpsc::unbounded_channel::<Frame>();
    tokio::spawn(async move {
        while let Some(frame) = queue.recv().await {
            if writer.send(&frame).await.is_err() {
                break;
            }
        }
    });

    let own = Protocol::own();
    let hello = Frozen::Hello { protocol: own, release: env!("CARGO_PKG_VERSION").into() };
    let Ok(hello) = Frame::control(&hello) else { return };
    let _ = outgoing.send(hello);
    let peer = match reader.recv().await {
        Ok(Some(frame)) => match frame.parse::<Frozen>() {
            Ok(Frozen::Hello { protocol, .. }) => protocol,
            _ => return,
        },
        _ => return,
    };
    let compat = compat(own, peer);

    // This connection is the attached app from now on; the previous one is let go, with what it
    // was asked to run in the browser.
    let number = shared.connection.fetch_add(1, Ordering::SeqCst) + 1;
    *shared.app.lock().unwrap() = Some(outgoing.clone());
    shared.fail_browser_actions();
    tracing::info!(session = shared.options.id, protocol = peer.0, "session.app_attached");
    // What the tray shows now, then what happened while no app was attached.
    if let Some(summary) = shared.runtime().and_then(|runtime| runtime.summary()) {
        shared.send(&Event::Notice { notice: HostNotice::Summary(summary) });
    }
    for notice in std::mem::take(&mut *shared.held.lock().unwrap()) {
        shared.send(&Event::Notice { notice });
    }

    while let Ok(Some(frame)) = reader.recv().await {
        if shared.connection.load(Ordering::SeqCst) != number {
            return; // replaced by a newer connection
        }
        let kind = match &frame {
            Frame::Control(json) => serde_json::from_slice::<serde_json::Value>(json)
                .ok()
                .and_then(|value| value.get("type")?.as_str().map(str::to_owned))
                .unwrap_or_default(),
            Frame::Data(_) => String::new(),
        };
        if !compat.allows(&kind) {
            reply_failed(&outgoing, "This SpecDeck cannot control a session of that release.");
            continue;
        }
        match frame.parse::<Command>() {
            Ok(command) => run(command, &shared, &outgoing, compat).await,
            Err(error) => reply_failed(&outgoing, &error.to_string()),
        }
    }

    // The app went away. If it was the attached one, the session runs on without it.
    if shared.connection.load(Ordering::SeqCst) == number {
        *shared.app.lock().unwrap() = None;
        shared.fail_browser_actions();
        if let Some(runtime) = shared.runtime() {
            runtime.detach();
        }
        tracing::info!(session = shared.options.id, "session.app_detached");
        start_grace(shared, number);
    }
}

/// Ends the session if no app attaches within the grace period after connection `number` left.
fn start_grace(shared: Arc<Shared>, number: u64) {
    tokio::spawn(async move {
        tokio::time::sleep(shared.options.grace).await;
        if shared.connection.load(Ordering::SeqCst) == number
            && shared.app.lock().unwrap().is_none()
        {
            tracing::info!(session = shared.options.id, "session.grace_expired");
            shared.end();
        }
    });
}

/// Answers the command `id` with what it gave.
fn answer(
    outgoing: &mpsc::UnboundedSender<Frame>,
    id: u64,
    result: Result<serde_json::Value, String>,
) {
    let (value, error) = match result {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error)),
    };
    if let Ok(frame) = Frame::control(&Event::Answer { id, value, error }) {
        let _ = outgoing.send(frame);
    }
}

fn reply_failed(outgoing: &mpsc::UnboundedSender<Frame>, message: &str) {
    if let Ok(frame) = Frame::control(&Event::Failed { message: message.into() }) {
        let _ = outgoing.send(frame);
    }
}

async fn run(
    command: Command,
    shared: &Arc<Shared>,
    outgoing: &mpsc::UnboundedSender<Frame>,
    compat: Compat,
) {
    if let Command::End = command {
        return shared.end();
    }
    if let Command::Start { .. } = command {
        return start(command, shared, outgoing).await;
    }
    if let Command::BrowserResult { id, result } = command {
        return shared.browser_answered(id, result);
    }
    if compat == Compat::EndOnly {
        return;
    }
    let Some(runtime) = shared.runtime() else {
        return reply_failed(outgoing, "The session has not started.");
    };
    let result = match command {
        Command::Attach => {
            let terminal = outgoing.clone();
            let events = outgoing.clone();
            runtime.attach(
                Box::new(move |records| terminal.send(Frame::Data(records)).is_ok()),
                Box::new(move |event| {
                    let Ok(event) = serde_json::to_value(event) else { return true };
                    let Ok(frame) = Frame::control(&Event::HostEvent { event }) else {
                        return true;
                    };
                    events.send(frame).is_ok()
                }),
            );
            Ok(())
        }
        Command::Detach => {
            runtime.detach();
            Ok(())
        }
        Command::Input { data } => runtime.input(&data),
        Command::Resize { cols, rows } => {
            runtime.resize(cols, rows);
            Ok(())
        }
        Command::Colors { foreground, background } => {
            runtime.set_colors_hex(&foreground, &background)
        }
        Command::NewCode => {
            runtime.new_code();
            Ok(())
        }
        Command::RemoveViewer { viewer_id } => {
            runtime.remove_viewer(viewer_id);
            Ok(())
        }
        Command::ViewerLimit { limit } => {
            runtime.set_viewer_limit(limit);
            Ok(())
        }
        Command::WebViewer { available } => {
            runtime.set_web_viewer(available);
            Ok(())
        }
        Command::Verbose { enabled } => {
            if let Some(diagnostics) = &shared.options.diagnostics {
                diagnostics.set_verbose(enabled);
            }
            Ok(())
        }
        Command::AllowRequests { suggestions } => {
            runtime.allow_suggestions(suggestions);
            Ok(())
        }
        Command::ShareCode { whole_files } => {
            runtime.share_whole_files(whole_files);
            Ok(())
        }
        Command::AllowFiles { files } => {
            runtime.allow_viewer_files(files);
            Ok(())
        }
        Command::RemoveSharedFile { file } => {
            runtime.remove_shared_file(file);
            Ok(())
        }
        Command::ShareFile { id, name, data } => {
            let result = runtime
                .share_file(&name, &data)
                .map(|info| serde_json::to_value(info).unwrap_or_default());
            answer(outgoing, id, result);
            Ok(())
        }
        Command::ReadSharedFile { id, file } => {
            answer(outgoing, id, runtime.read_shared_file(file).map(serde_json::Value::String));
            Ok(())
        }
        Command::PutSharedFile { id, file } => {
            answer(outgoing, id, runtime.put_shared_file(file).map(serde_json::Value::String));
            Ok(())
        }
        Command::AddToBraindump { id, name, data } => {
            let result = runtime
                .add_to_braindump(&name, &data)
                .map(|info| serde_json::to_value(info).unwrap_or_default());
            answer(outgoing, id, result);
            Ok(())
        }
        Command::AddNote { id, text } => {
            let result =
                runtime.add_note(&text).map(|info| serde_json::to_value(info).unwrap_or_default());
            answer(outgoing, id, result);
            Ok(())
        }
        Command::ReplaceNote { id, file, text } => {
            let result = runtime
                .replace_note(file, &text)
                .map(|info| serde_json::to_value(info).unwrap_or_default());
            answer(outgoing, id, result);
            Ok(())
        }
        Command::GiveBraindump { id, prompt } => {
            let result = runtime.give_braindump(prompt).await.map(serde_json::Value::from);
            answer(outgoing, id, result);
            Ok(())
        }
        Command::AnswerSuggestion { viewer_id, id, answer } => {
            let answer = match answer {
                SuggestionAnswer::Send { text } => Answer::Sent { text },
                SuggestionAnswer::Keep => Answer::Kept,
                SuggestionAnswer::Decline => Answer::Declined,
            };
            runtime.answer_suggestion(viewer_id, id, answer).await
        }
        Command::ShareLink { url, title, session_only } => {
            runtime.share_link(&url, title.as_deref(), session_only)
        }
        Command::AnswerLink { viewer_id, id, answer } => {
            runtime.answer_link(viewer_id, id, answer == LinkAnswer::Share).await
        }
        Command::RemoveLink { id } => {
            runtime.remove_link(id);
            Ok(())
        }
        Command::AllowLinks { links } => {
            runtime.allow_links(links);
            Ok(())
        }
        Command::AskToRead { target, label, limit_s } => {
            runtime.ask(AskCommand::Read { target, label, limit_s }).await
        }
        Command::EndRead => runtime.ask(AskCommand::EndRead).await,
        Command::StartVote { task, cards } => {
            runtime.ask(AskCommand::StartVote { task, cards }).await
        }
        Command::CastVote { card } => runtime.ask(AskCommand::CastVote(card)).await,
        Command::RevealVotes => runtime.ask(AskCommand::Reveal).await,
        Command::VoteAgain => runtime.ask(AskCommand::VoteAgain).await,
        Command::StopVote => runtime.ask(AskCommand::StopVote).await,
        Command::KeepEstimate { card } => runtime.ask(AskCommand::Keep(card)).await,
        Command::Status => {
            let sharing = serde_json::to_value(runtime.sharing_status()).unwrap_or_default();
            let events =
                shared.options.diagnostics.as_ref().map(|d| d.recent()).unwrap_or_default();
            shared.send(&Event::Status { sharing, events });
            Ok(())
        }
        Command::Start { .. } | Command::End | Command::BrowserResult { .. } => Ok(()),
    };
    if let Err(message) = result {
        reply_failed(outgoing, &message);
    }
}

async fn start(command: Command, shared: &Arc<Shared>, outgoing: &mpsc::UnboundedSender<Frame>) {
    let Command::Start {
        agent,
        folder,
        cols,
        rows,
        username,
        data_dir,
        signalling_url,
        environment,
        verbose,
        browser,
        screenshots,
        sandbox,
    } = command
    else {
        return;
    };
    if shared.runtime().is_some() {
        return reply_failed(outgoing, "The session has already started.");
    }
    if let Some(diagnostics) = &shared.options.diagnostics {
        diagnostics.set_verbose(verbose);
    }
    let observed = shared.clone();
    let observer: Observer = Arc::new(move |notice: HostNotice| {
        let attached = observed.app.lock().unwrap().is_some();
        if attached {
            observed.send(&Event::Notice { notice });
        } else {
            observed.held.lock().unwrap().push(notice);
        }
    });
    let options = StartOptions {
        agent,
        folder: folder.clone(),
        cols,
        rows,
        username,
        data_dir,
        signalling_url,
        peer: shared.options.peer.clone(),
        browser: browser.then(|| browser_handler(Arc::downgrade(shared))),
        screenshots,
        sandbox,
    };
    match HostRuntime::start(options, &environment, observer).await {
        Ok(runtime) => {
            *shared.runtime.lock().unwrap() = Some(Arc::new(runtime));
            write_entry(shared, agent, &folder);
            tracing::info!(
                session = shared.options.id,
                agent = agent.display_name(),
                "session.started"
            );
            shared.send(&Event::Started);
        }
        Err(message) => {
            reply_failed(outgoing, &message);
            // Nothing runs: the process has no reason to stay.
            shared.end();
        }
    }
}

/// Passes the agent's browser actions to the attached app. Holds the session weakly: the runtime
/// that holds the handler is part of it.
fn browser_handler(shared: Weak<Shared>) -> BrowserHandler {
    Arc::new(move |action| {
        let shared = shared.clone();
        Box::pin(async move {
            match shared.upgrade() {
                Some(shared) => shared.ask_browser(action).await,
                None => BrowserResult::error(BROWSER_UNAVAILABLE),
            }
        })
    })
}

fn write_entry(shared: &Shared, agent: AgentId, folder: &std::path::Path) {
    let entry = RunEntry {
        version: env!("CARGO_PKG_VERSION"),
        protocol: Protocol::own(),
        pid: std::process::id(),
        started: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
        agent,
        folder: folder.file_name().map_or_else(
            || folder.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
        endpoint: &shared.options.endpoint,
    };
    if let Ok(json) = serde_json::to_vec(&entry) {
        let _ = std::fs::write(shared.entry_path(), json);
    }
}
