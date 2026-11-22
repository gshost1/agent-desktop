use agent_desktop_core::Deadline;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, MapState};

use super::DisplayReader;
use super::control_display::{atoms, longs, property};
use super::stacking::{bounded, capped};
use crate::system::display::display_server::{DisplayServer, session_server};

pub(crate) fn is_pending(pid: u32, caller: Deadline) -> bool {
    let Some(deadline) = capped(caller) else {
        return false;
    };
    read_on(session_server(), move |display| {
        bounded(deadline, None, move || read(&display, pid))
    })
}

fn read_on(server: DisplayServer, read: impl FnOnce(String) -> Option<bool>) -> bool {
    match server {
        DisplayServer::X11(display) => read(display).unwrap_or(false),
        DisplayServer::Wayland | DisplayServer::Unavailable(_) => false,
    }
}

fn has_menu_type(types: Option<&[Atom]>, menus: &[Atom]) -> bool {
    types.is_some_and(|types| types.iter().any(|kind| menus.contains(kind)))
}

fn read(display: &str, pid: u32) -> Option<bool> {
    let (connection, screen) = DisplayReader::connect(display).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let atoms = atoms(
        &connection,
        &[
            b"_NET_WM_PID",
            b"_NET_WM_WINDOW_TYPE",
            b"_NET_WM_WINDOW_TYPE_MENU",
            b"_NET_WM_WINDOW_TYPE_DROPDOWN_MENU",
            b"_NET_WM_WINDOW_TYPE_POPUP_MENU",
        ],
    )?;
    let windows = connection.query_tree(root).ok()?.reply().ok()?.children;
    for window in windows {
        let attributes = connection
            .get_window_attributes(window)
            .ok()?
            .reply()
            .ok()?;
        if !attributes.override_redirect || attributes.map_state != MapState::VIEWABLE {
            continue;
        }
        let owner = property(&connection, window, atoms[0], AtomEnum::CARDINAL, 1)
            .and_then(|reply| longs(reply.format, &reply.value, reply.bytes_after))
            .and_then(|values| values.first().copied());
        if owner != Some(pid) {
            continue;
        }
        let types = property(&connection, window, atoms[1], AtomEnum::ATOM, 16)
            .and_then(|reply| longs(reply.format, &reply.value, reply.bytes_after));
        if has_menu_type(types.as_deref(), &atoms[2..]) {
            return Some(true);
        }
    }
    Some(false)
}

#[cfg(test)]
#[path = "menu_pending_tests.rs"]
mod tests;
