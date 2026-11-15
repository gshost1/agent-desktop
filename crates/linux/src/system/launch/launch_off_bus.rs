use std::time::Duration;

use agent_desktop_core::Deadline;
use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt as _};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, MapState, Window};
use x11rb::rust_connection::RustConnection;

use crate::system::display::display_server::{DisplayServer, session_server};
use crate::system::x11::control_display::open;
use crate::system::x11::stacking::bounded;

/// The longest one window read of the off-bus check may take. It may run as
/// the launch deadline expires, so it is bounded on its own and stays well
/// inside the second a command may overrun its timeout by.
const WINDOW_READ: Duration = Duration::from_millis(250);

/// The environment settings with which a toolkit never loads its
/// accessibility bridge.
const BRIDGE_OFF: [&[u8]; 2] = [b"NO_AT_BRIDGE=1", b"GTK_A11Y=none"];

/// What a launch observed about a spawned process that runs and is not on the
/// accessibility bus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OffBus {
    /// No window this tool can see yet: the application may still be starting.
    #[default]
    Starting,
    /// The process maps a top-level window, so its toolkit is up, and it is
    /// still not on the bus.
    Windowed,
    /// The process's environment turns its accessibility bridge off.
    BridgeOff,
}

impl OffBus {
    /// Whether the evidence shows the process will not join the bus however
    /// long the launch waits.
    pub(crate) fn never_joins(self) -> bool {
        self != Self::Starting
    }
}

/// Reads the evidence for `pid`: its environment, then whether it maps a
/// top-level X11 window, Xwayland's included.
pub(crate) fn observe(pid: u32) -> OffBus {
    let environ = std::fs::read(format!("/proc/{pid}/environ")).ok();
    classify(environ.as_deref(), || maps_window(pid))
}

/// Whether `pid` owns a mapped top-level window on the session's X11
/// display. The owner is the client the X server reports for the window, so a
/// toolkit that never sets `_NET_WM_PID` is seen too. A Wayland session
/// without Xwayland, or a display that does not answer in time, shows none.
fn maps_window(pid: u32) -> bool {
    let display = match session_server() {
        DisplayServer::X11(display) => Some(display),
        DisplayServer::Wayland => std::env::var("DISPLAY")
            .ok()
            .filter(|name| !name.is_empty()),
        DisplayServer::Unavailable(..) => None,
    };
    let Some(display) = display else {
        return false;
    };
    Deadline::from_duration(WINDOW_READ).is_ok_and(|deadline| {
        bounded(deadline, false, move || {
            open(&display).is_some_and(|(connection, root)| {
                toplevels(&connection, root)
                    .is_some_and(|windows| owns_one(&connection, &windows, pid))
            })
        })
    })
}

/// The window manager's client list, or without a manager the mapped
/// children of the root.
fn toplevels(connection: &RustConnection, root: Window) -> Option<Vec<Window>> {
    let list = connection
        .intern_atom(true, b"_NET_CLIENT_LIST")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let managed = connection
        .get_property(false, root, list, AtomEnum::WINDOW, 0, 4_096)
        .ok()?
        .reply()
        .ok()?
        .value32()
        .map(Iterator::collect::<Vec<_>>)
        .unwrap_or_default();
    if !managed.is_empty() {
        return Some(managed);
    }
    let children = connection.query_tree(root).ok()?.reply().ok()?.children;
    Some(
        children
            .into_iter()
            .filter(|window| {
                connection
                    .get_window_attributes(*window)
                    .ok()
                    .and_then(|cookie| cookie.reply().ok())
                    .is_some_and(|attributes| attributes.map_state == MapState::VIEWABLE)
            })
            .collect(),
    )
}

fn owns_one(connection: &RustConnection, windows: &[Window], pid: u32) -> bool {
    let specs: Vec<ClientIdSpec> = windows
        .iter()
        .map(|window| ClientIdSpec {
            client: *window,
            mask: ClientIdMask::LOCAL_CLIENT_PID,
        })
        .collect();
    if specs.is_empty() {
        return false;
    }
    connection
        .res_query_client_ids(&specs)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .is_some_and(|reply| {
            reply
                .ids
                .iter()
                .filter(|id| id.spec.mask == ClientIdMask::LOCAL_CLIENT_PID)
                .any(|id| id.value.first() == Some(&pid))
        })
}

/// The evidence from an environment block and a window check, which runs only
/// when the environment leaves the bridge on.
pub(crate) fn classify(environ: Option<&[u8]>, windowed: impl FnOnce() -> bool) -> OffBus {
    let bridge_off = environ.is_some_and(|block| {
        block
            .split(|byte| *byte == 0)
            .any(|entry| BRIDGE_OFF.contains(&entry))
    });
    if bridge_off {
        OffBus::BridgeOff
    } else if windowed() {
        OffBus::Windowed
    } else {
        OffBus::Starting
    }
}

#[cfg(test)]
#[path = "launch_off_bus_tests.rs"]
mod tests;
