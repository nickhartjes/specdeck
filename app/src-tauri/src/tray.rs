//! The menu-bar (macOS) or tray (Windows, Linux) icon while a session is hosted (design decision
//! 11). It shows each hosted session's state and viewers, and offers its window, its invite code
//! and ending it, also while the window is closed. One icon serves every hosted session
//! (`multiple-windows` design decision 6): one session gets a flat menu, several a submenu each.

use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::{AgentId, AgentStatus};
use tauri::image::Image;
use tauri::menu::{
    IsMenuItem, Menu, MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Runtime};

use crate::host::{SharingState, Summary};
use crate::sessions::SessionId;

const TRAY_ID: &str = "session";

/// Item ids start with this; the action and the session follow, as `tray:open:3`.
const PREFIX: &str = "tray:";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    CopyCode,
    End,
}

impl Action {
    const ALL: [Action; 3] = [Action::Open, Action::CopyCode, Action::End];

    fn as_str(self) -> &'static str {
        match self {
            Action::Open => "open",
            Action::CopyCode => "copy-code",
            Action::End => "end",
        }
    }
}

fn item_id(key: &str, session: SessionId) -> String {
    format!("{PREFIX}{key}:{session}")
}

/// The action and session of a tray item id, if it is one.
pub fn parse(id: &str) -> Option<(Action, SessionId)> {
    let (action, session) = id.strip_prefix(PREFIX)?.rsplit_once(':')?;
    let action = Action::ALL.into_iter().find(|known| known.as_str() == action)?;
    Some((action, session.parse().ok()?))
}

/// A hosted session as the tray shows it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: SessionId,
    pub folder: String,
    pub agent: AgentId,
    pub summary: Summary,
    /// False for a session an older SpecDeck started, which this one can only end
    /// (`session-processes` design decision 3).
    pub controllable: bool,
}

/// The first line of the menu: what the agent is doing.
fn state_line(agent: AgentId, summary: &Summary) -> String {
    let agent_name = agent.display_name();
    match summary.status {
        AgentStatus::Exited if agent == AgentId::Terminal => "The shell has exited".into(),
        AgentStatus::Starting => format!("{agent_name} is starting"),
        AgentStatus::Working => format!("{agent_name} is working"),
        AgentStatus::Waiting => format!("{agent_name} is waiting for you"),
        AgentStatus::Exited => format!("{agent_name} has exited"),
        AgentStatus::Unknown => "Session is live".into(),
    }
}

fn viewers_line(summary: &Summary) -> String {
    let viewers = match summary.viewers {
        0 => "No viewers yet".to_string(),
        1 => "1 viewer".to_string(),
        n => format!("{n} viewers"),
    };
    match summary.sharing {
        SharingState::Unavailable => format!("{viewers} · sharing unavailable"),
        _ => viewers,
    }
}

/// One menu line: an item, or a separator.
enum Line<R: Runtime> {
    Item(MenuItem<R>),
    Separator(PredefinedMenuItem<R>),
}

impl<R: Runtime> Line<R> {
    fn as_item(&self) -> &dyn IsMenuItem<R> {
        match self {
            Line::Item(item) => item,
            Line::Separator(separator) => separator,
        }
    }
}

/// A session's lines: what it is doing, then its window, its code and ending it. The folder line
/// is left out in a submenu, whose title already names the folder.
fn session_lines<R: Runtime>(
    app: &AppHandle<R>,
    entry: &Entry,
    with_folder: bool,
) -> tauri::Result<Vec<Line<R>>> {
    let item = |key: &str, text: String, enabled: bool| -> tauri::Result<Line<R>> {
        let item = MenuItemBuilder::with_id(item_id(key, entry.id), text).enabled(enabled);
        Ok(Line::Item(item.build(app)?))
    };
    let info = |key: &str, text: String| item(key, text, false);
    let action = |action: Action, text: String, enabled: bool| item(action.as_str(), text, enabled);
    let separator = || Ok::<_, tauri::Error>(Line::Separator(PredefinedMenuItem::separator(app)?));
    let code = entry.summary.code.as_deref().and_then(|code| InviteCode::parse(code).ok());
    let copy_text = match &code {
        Some(code) => format!("Copy Invite Code  {code}"),
        None => "Copy Invite Code".into(),
    };
    let mut lines = Vec::new();
    if with_folder {
        lines.push(info("folder", format!("Hosting {}", entry.folder))?);
    }
    if !entry.controllable {
        lines.extend([
            info("older", "Started by an older SpecDeck".into())?,
            separator()?,
            action(Action::End, "End Session…".into(), true)?,
        ]);
        return Ok(lines);
    }
    lines.extend([
        info("state", state_line(entry.agent, &entry.summary))?,
        info("viewers", viewers_line(&entry.summary))?,
        separator()?,
        action(Action::Open, "Open Window".into(), true)?,
        action(Action::CopyCode, copy_text, code.is_some())?,
        separator()?,
        action(Action::End, "End Session…".into(), true)?,
    ]);
    Ok(lines)
}

fn menu<R: Runtime>(app: &AppHandle<R>, entries: &[Entry]) -> tauri::Result<Menu<R>> {
    if let [entry] = entries {
        let lines = session_lines(app, entry, true)?;
        let items: Vec<_> = lines.iter().map(Line::as_item).collect();
        return MenuBuilder::new(app).items(&items).build();
    }
    let mut submenus = Vec::new();
    for entry in entries {
        let lines = session_lines(app, entry, false)?;
        let items: Vec<_> = lines.iter().map(Line::as_item).collect();
        submenus.push(SubmenuBuilder::new(app, &entry.folder).items(&items).build()?);
    }
    let items: Vec<&dyn IsMenuItem<R>> = submenus.iter().map(|s| s as &dyn IsMenuItem<R>).collect();
    MenuBuilder::new(app).items(&items).build()
}

fn tooltip(entries: &[Entry]) -> String {
    match entries {
        [entry] => format!("SpecDeck: hosting {} · {}", entry.folder, viewers_line(&entry.summary)),
        _ => format!("SpecDeck: hosting {} sessions", entries.len()),
    }
}

/// Shows the hosted sessions `entries`, in this order: adds the icon for the first one, brings
/// its menu up to date for a change, and removes it when none is left.
pub fn refresh<R: Runtime>(app: &AppHandle<R>, entries: &[Entry]) -> tauri::Result<()> {
    if entries.is_empty() {
        remove(app);
        return Ok(());
    }
    let menu = menu(app, entries)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
        return tray.set_tooltip(Some(tooltip(entries)));
    }
    let icon = if cfg!(target_os = "macos") {
        Image::from_bytes(include_bytes!("../icons/tray-template.png"))?
    } else {
        app.default_window_icon()
            .cloned()
            .unwrap_or(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
    };
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip(tooltip(entries))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    Ok(())
}

/// Removes the icon: no session is hosted.
///
/// On the main thread: AppKit aborts the app when a status item is removed from any other, and
/// ending a session runs on a command's worker thread.
fn remove<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || drop(handle.remove_tray_by_id(TRAY_ID)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(status: AgentStatus, viewers: usize, sharing: SharingState) -> Summary {
        Summary { status, viewers, code: None, sharing }
    }

    #[test]
    fn says_what_the_agent_does_and_who_watches() {
        let waiting = summary(AgentStatus::Waiting, 3, SharingState::Ready);
        assert_eq!(state_line(AgentId::ClaudeCode, &waiting), "Claude Code is waiting for you");
        assert_eq!(viewers_line(&waiting), "3 viewers");
        assert_eq!(
            viewers_line(&summary(AgentStatus::Unknown, 1, SharingState::Ready)),
            "1 viewer"
        );
        assert_eq!(
            viewers_line(&summary(AgentStatus::Unknown, 0, SharingState::Unavailable)),
            "No viewers yet · sharing unavailable"
        );
        assert_eq!(
            state_line(AgentId::OpenCode, &summary(AgentStatus::Unknown, 0, SharingState::Ready)),
            "Session is live"
        );
    }

    #[test]
    fn a_terminal_session_talks_about_its_shell() {
        let exited = summary(AgentStatus::Exited, 0, SharingState::Ready);
        assert_eq!(state_line(AgentId::Terminal, &exited), "The shell has exited");
        assert_eq!(state_line(AgentId::Omp, &exited), "omp has exited");
        let live = summary(AgentStatus::Unknown, 2, SharingState::Ready);
        assert_eq!(state_line(AgentId::Terminal, &live), "Session is live");
    }

    #[test]
    fn item_ids_name_the_action_and_the_session() {
        for action in Action::ALL {
            assert_eq!(parse(&item_id(action.as_str(), 12)), Some((action, 12)));
        }
        assert_eq!(parse("tray:open:3"), Some((Action::Open, 3)));
        assert_eq!(parse("tray:copy-code:7"), Some((Action::CopyCode, 7)));
        // Information lines and other menus' ids are not actions.
        assert_eq!(parse("tray:state:3"), None);
        assert_eq!(parse("tray:open"), None);
        assert_eq!(parse("host"), None);
    }

    #[test]
    fn the_tooltip_names_one_session_or_counts_several() {
        let entry = |id, folder: &str| Entry {
            id,
            folder: folder.into(),
            agent: AgentId::ClaudeCode,
            summary: summary(AgentStatus::Working, 2, SharingState::Ready),
            controllable: true,
        };
        assert_eq!(tooltip(&[entry(1, "shop")]), "SpecDeck: hosting shop · 2 viewers");
        assert_eq!(tooltip(&[entry(1, "shop"), entry(2, "api")]), "SpecDeck: hosting 2 sessions");
    }
}
