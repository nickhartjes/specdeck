//! The native menu (design decision 11): the application menu on macOS, a File, Edit, View and
//! Help menu bar on Windows and Linux.
//!
//! Most items are handled by the UI, which gets their id in a `menu` event. The host's terminal
//! gets first refusal on keys, so on Windows and Linux no item takes a key the terminal needs
//! (Ctrl+C, Ctrl+F, Ctrl+N, Ctrl+W and the like); the UI handles those keys itself. The zoom keys
//! (Ctrl+=, Ctrl+-, Ctrl+0) are the exception, as in Windows Terminal and GNOME Terminal; Ctrl+_
//! (undo) still reaches the terminal.

use std::path::PathBuf;

use tauri::menu::{
    AboutMetadata, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
};
use tauri::{AppHandle, Runtime};

use crate::settings::Appearance;

/// Menu items whose id starts with this open a recent folder; the rest is its index.
pub const RECENT_PREFIX: &str = "recent:";

/// The session window's panels in the View menu, numbered from left to right as they are
/// shown (`session-window-layout` design decision 6). Cmd/Ctrl+0 stays Actual Size.
const PANELS: [(&str, &str, &str); 3] = [
    (ids::FILES, "Files", "CmdOrCtrl+1"),
    (ids::TERMINAL, "Terminal", "CmdOrCtrl+2"),
    (ids::TIMELINE, "Timeline", "CmdOrCtrl+3"),
];

/// View > Appearance, in menu order (`appearance-setting` design decision 6). No keys: the
/// terminal keeps them.
const APPEARANCES: [(Appearance, &str, &str); 3] = [
    (Appearance::System, "appearance:system", "System"),
    (Appearance::Light, "appearance:light", "Light"),
    (Appearance::Dark, "appearance:dark", "Dark"),
];

/// The appearance a View > Appearance item chooses; handled in Rust.
pub fn appearance(id: &str) -> Option<Appearance> {
    APPEARANCES.iter().find(|(_, item, _)| *item == id).map(|(appearance, _, _)| *appearance)
}

/// Ids the UI handles, as sent in the `menu` event.
pub mod ids {
    /// Handled in Rust: opens another session window with the start screen.
    pub const NEW_WINDOW: &str = "new-window";
    pub const HOST: &str = "host";
    pub const JOIN: &str = "join";
    pub const COPY_CODE: &str = "copy-code";
    pub const FIND: &str = "find";
    pub const SETTINGS: &str = "settings";
    /// Open or close the session window's panels, numbered from left to right.
    pub const FILES: &str = "panel:files";
    pub const TERMINAL: &str = "panel:terminal";
    pub const TIMELINE: &str = "panel:timeline";
    pub const JUMP_TO_LIVE: &str = "jump-to-live";
    pub const SHORTCUTS: &str = "shortcuts";
    pub const CONNECTION_STATUS: &str = "connection-status";
    /// Handled in Rust, so it works while the window does not respond.
    pub const EXPORT_DIAGNOSTICS: &str = "export-diagnostics";
    /// Handled in Rust: zoom the main window and remember the level.
    pub const ZOOM_IN: &str = "zoom-in";
    pub const ZOOM_OUT: &str = "zoom-out";
    pub const ZOOM_RESET: &str = "zoom-reset";
    /// Handled in Rust: confirms first while a session is hosted.
    pub const QUIT: &str = "quit";
    /// Handled in Rust: checks at once and says what it found.
    pub const CHECK_UPDATES: &str = "check-updates";
    /// Edit commands on Windows and Linux, where the native ones would take terminal keys.
    pub const COPY: &str = "copy";
    pub const PASTE: &str = "paste";
    pub const SELECT_ALL: &str = "select-all";
}

/// What the menu depends on.
pub struct MenuState<'a> {
    pub recent: &'a [PathBuf],
    pub hosting: bool,
    /// This install updates itself.
    pub updates: bool,
    /// Checked in View > Appearance.
    pub appearance: Appearance,
}

/// The menu that offers Check for Updates…, if any (auto-update design decision 5): the
/// SpecDeck menu on macOS, Help elsewhere, and none on installs that do not update themselves.
pub fn check_updates_menu(mac: bool, updates: bool) -> Option<&'static str> {
    match (updates, mac) {
        (false, _) => None,
        (true, true) => Some("SpecDeck"),
        (true, false) => Some("Help"),
    }
}

/// An accelerator for macOS only, or one for every platform.
fn keys(mac: &'static str, others: Option<&'static str>) -> Option<&'static str> {
    if cfg!(target_os = "macos") { Some(mac) } else { others }
}

fn item<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    text: &str,
    accelerator: Option<&str>,
    enabled: bool,
) -> tauri::Result<tauri::menu::MenuItem<R>> {
    let mut builder = MenuItemBuilder::with_id(id, text).enabled(enabled);
    if let Some(accelerator) = accelerator {
        builder = builder.accelerator(accelerator);
    }
    builder.build(app)
}

pub fn build<R: Runtime>(app: &AppHandle<R>, state: &MenuState) -> tauri::Result<Menu<R>> {
    let mac = cfg!(target_os = "macos");
    let about = AboutMetadata {
        name: Some("SpecDeck".into()),
        version: Some(app.package_info().version.to_string()),
        comments: Some("Run a coding agent with your team watching along.".into()),
        ..Default::default()
    };
    let settings = item(app, ids::SETTINGS, "Settings…", Some("CmdOrCtrl+,"), true)?;
    let check_updates = item(app, ids::CHECK_UPDATES, "Check for Updates…", None, true)?;
    let updates_in = check_updates_menu(mac, state.updates);

    let mut recent = SubmenuBuilder::new(app, "Open Recent");
    for (i, folder) in state.recent.iter().enumerate() {
        let name = folder.file_name().map_or_else(
            || folder.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        recent =
            recent.text(format!("{RECENT_PREFIX}{i}"), format!("{name} — {}", folder.display()));
    }
    let recent = recent.enabled(!state.recent.is_empty()).build()?;

    // No New Window key on Windows and Linux: Ctrl+Shift+N is Host there, and Ctrl+N belongs to
    // the terminal.
    let mut file = SubmenuBuilder::new(app, "File")
        .item(&item(app, ids::NEW_WINDOW, "New Window", keys("Cmd+Shift+N", None), true)?)
        .separator()
        .item(&item(app, ids::HOST, "Host a Session…", keys("Cmd+N", Some("Ctrl+Shift+N")), true)?)
        .item(&recent)
        .item(&item(app, ids::JOIN, "Join a Session…", keys("Cmd+J", Some("Ctrl+Shift+J")), true)?)
        .separator()
        .item(&item(
            app,
            ids::COPY_CODE,
            "Copy Invite Code",
            keys("Cmd+Shift+C", None),
            state.hosting,
        )?)
        .separator();
    file = if mac {
        file.close_window()
    } else {
        file.item(&settings).separator().close_window().item(&item(
            app,
            ids::QUIT,
            "Quit",
            None,
            true,
        )?)
    };
    let file = file.build()?;

    let mut edit = SubmenuBuilder::new(app, "Edit");
    edit = if mac {
        edit.undo().redo().separator().cut().copy().paste().select_all()
    } else {
        edit.item(&item(app, ids::COPY, "Copy", None, true)?)
            .item(&item(app, ids::PASTE, "Paste", None, true)?)
            .item(&item(app, ids::SELECT_ALL, "Select All", None, true)?)
    };
    let edit = edit
        .separator()
        .item(&item(app, ids::FIND, "Find…", keys("Cmd+F", None), true)?)
        .build()?;

    let mut view = SubmenuBuilder::new(app, "View");
    for (id, label, accelerator) in PANELS {
        view = view.item(&item(app, id, label, Some(accelerator), true)?);
    }
    let mut appearance = SubmenuBuilder::new(app, "Appearance");
    for (value, id, label) in APPEARANCES {
        appearance = appearance.item(
            &CheckMenuItemBuilder::with_id(id, label)
                .checked(value == state.appearance)
                .build(app)?,
        );
    }
    let mut view = view
        .separator()
        .item(&item(app, ids::ZOOM_IN, "Zoom In", Some("CmdOrCtrl+="), true)?)
        .item(&item(app, ids::ZOOM_OUT, "Zoom Out", Some("CmdOrCtrl+-"), true)?)
        .item(&item(app, ids::ZOOM_RESET, "Actual Size", Some("CmdOrCtrl+0"), true)?)
        .item(&appearance.build()?)
        .separator()
        .item(&item(app, ids::JUMP_TO_LIVE, "Jump to Live", keys("Cmd+Down", None), true)?);
    if mac {
        view = view.separator().fullscreen();
    }
    let view = view.build()?;

    // Ctrl+Shift+letter, like the app's other commands, so no terminal key is taken.
    let mut help = SubmenuBuilder::new(app, "Help")
        .item(&item(app, ids::SHORTCUTS, "Keyboard Shortcuts", keys("Cmd+/", Some("F1")), true)?)
        .item(&item(
            app,
            ids::CONNECTION_STATUS,
            "Connection Status",
            keys("Cmd+Shift+D", Some("Ctrl+Shift+D")),
            true,
        )?)
        .item(&item(app, ids::EXPORT_DIAGNOSTICS, "Export Diagnostics…", None, true)?);
    if updates_in == Some("Help") {
        help = help.separator().item(&check_updates);
    }
    if !mac {
        help = help.separator().about(Some(about.clone()));
    }
    let help = help.build()?;

    let mut menu = MenuBuilder::new(app);
    if mac {
        let mut app_menu = SubmenuBuilder::new(app, "SpecDeck").about(Some(about));
        if updates_in == Some("SpecDeck") {
            app_menu = app_menu.item(&check_updates);
        }
        let app_menu = app_menu
            .separator()
            .item(&settings)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .item(&item(app, ids::QUIT, "Quit SpecDeck", Some("Cmd+Q"), true)?)
            .build()?;
        let window = SubmenuBuilder::new(app, "Window")
            .minimize()
            .maximize()
            .separator()
            .bring_all_to_front()
            .build()?;
        menu = menu.items(&[&app_menu, &file, &edit, &view, &window, &help]);
    } else {
        menu = menu.items(&[&file, &edit, &view, &help]);
    }
    menu.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panels_are_numbered_from_left_to_right_and_leave_zero_to_zoom() {
        let keys: Vec<_> = PANELS.iter().map(|(id, _, key)| (*id, *key)).collect();
        assert_eq!(
            keys,
            [
                ("panel:files", "CmdOrCtrl+1"),
                ("panel:terminal", "CmdOrCtrl+2"),
                ("panel:timeline", "CmdOrCtrl+3")
            ]
        );
        assert!(PANELS.iter().all(|(_, _, key)| *key != "CmdOrCtrl+0"));
    }

    #[test]
    fn each_appearance_item_chooses_its_appearance() {
        assert_eq!(appearance("appearance:system"), Some(Appearance::System));
        assert_eq!(appearance("appearance:light"), Some(Appearance::Light));
        assert_eq!(appearance("appearance:dark"), Some(Appearance::Dark));
        assert_eq!(appearance("zoom-in"), None);
        assert!(APPEARANCES.iter().all(|(_, id, _)| !id.starts_with(RECENT_PREFIX)));
    }

    #[test]
    fn check_for_updates_is_offered_only_where_the_install_updates_itself() {
        assert_eq!(check_updates_menu(true, true), Some("SpecDeck"));
        assert_eq!(check_updates_menu(false, true), Some("Help"));
        assert_eq!(check_updates_menu(true, false), None);
        assert_eq!(check_updates_menu(false, false), None);
    }
}
