use std::time::Duration;

use agent_desktop_core::Deadline;
use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, GetPropertyReply, MapState, Window,
};
use x11rb::rust_connection::RustConnection;

use crate::system::display::display_server::{DisplayServer, session_server};
use crate::system::process::proc_stat::read_stat;
use crate::system::x11::control_display::open;
use crate::system::x11::stacking::bounded;

#[path = "launch_main_window.rs"]
mod main_window;

/// The longest one window read of the off-bus check may take. It may run as
/// the launch deadline expires, so it is bounded on its own and stays well
/// inside the second a command may overrun its timeout by.
const WINDOW_READ: Duration = Duration::from_millis(250);

/// How many parents a window owner's process is followed through to reach the
/// spawned process, which covers a wrapper or sandbox launcher chain.
const MAX_ANCESTRY: usize = 16;

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

/// Reads the evidence for `pid`: its environment, then whether it maps a
/// top-level X11 window, Xwayland's included.
pub(crate) fn observe(pid: u32) -> OffBus {
    let environ = std::fs::read(format!("/proc/{pid}/environ")).ok();
    classify(environ.as_deref(), || maps_window(pid))
}

/// Whether `pid`, or a process started from it, owns a mapped top-level
/// window on the session's X11 display. The owner is the client the X server
/// reports for the window, so a toolkit that never sets `_NET_WM_PID` is seen
/// too. A Wayland session
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

/// A trusted UTF-8 title of the largest mapped normal window owned by the
/// process or its descendants, absent when equally large windows disagree.
pub(in crate::system::launch) fn window_title(pid: u32) -> Option<String> {
    let display = match session_server() {
        DisplayServer::X11(display) => display,
        DisplayServer::Wayland => std::env::var("DISPLAY").ok()?,
        DisplayServer::Unavailable(..) => return None,
    };
    let deadline = Deadline::from_duration(WINDOW_READ).ok()?;
    bounded(deadline, None, move || {
        let (connection, root) = open(&display)?;
        let atoms = TitleAtoms::intern(&connection)?;
        let owned = toplevels(&connection, root)?
            .into_iter()
            .enumerate()
            .filter(|(_, window)| {
                connection
                    .get_window_attributes(*window)
                    .ok()
                    .and_then(|cookie| cookie.reply().ok())
                    .is_some_and(|attributes| attributes.map_state == MapState::VIEWABLE)
            })
            .filter(|(_, window)| owns_one(&connection, std::slice::from_ref(window), pid));
        main_window::select_title(&read_candidates(&connection, &atoms, owned))
    })
}

/// The atoms a title read compares against, interned once per connection
/// without creating any: one the server does not know yet is on no window.
struct TitleAtoms {
    window_type: Atom,
    normal: Atom,
    name: Atom,
    utf8: Atom,
}

impl TitleAtoms {
    fn intern(connection: &RustConnection) -> Option<Self> {
        let [window_type, normal, name, utf8] = intern_existing(
            connection,
            [
                b"_NET_WM_WINDOW_TYPE".as_slice(),
                b"_NET_WM_WINDOW_TYPE_NORMAL",
                b"_NET_WM_NAME",
                b"UTF8_STRING",
            ],
        )?;
        Some(Self {
            window_type,
            normal,
            name,
            utf8,
        })
    }
}

/// The atoms the server already knows by these names, [`x11rb::NONE`] for
/// one it does not, asked in one round trip and creating none.
fn intern_existing<const N: usize>(
    connection: &RustConnection,
    names: [&[u8]; N],
) -> Option<[Atom; N]> {
    let cookies = names.map(|name| connection.intern_atom(true, name).ok());
    let replies = cookies.map(|cookie| cookie.and_then(|cookie| cookie.reply().ok()));
    replies
        .iter()
        .all(Option::is_some)
        .then(|| replies.map(|reply| reply.map_or(x11rb::NONE, |reply| reply.atom)))
}

/// The title facts of each window that could be read whole, in stacking
/// order; a window that vanished or is malformed mid-read is left out.
fn read_candidates(
    connection: &RustConnection,
    atoms: &TitleAtoms,
    windows: impl IntoIterator<Item = (usize, Window)>,
) -> Vec<main_window::Candidate> {
    windows
        .into_iter()
        .filter_map(|(order, window)| read_candidate(connection, atoms, window, order))
        .collect()
}

/// One window's facts for the title choice, absent when the window went
/// away or carries a malformed window type, so it is left out of the choice.
fn read_candidate(
    connection: &RustConnection,
    atoms: &TitleAtoms,
    window: Window,
    order: usize,
) -> Option<main_window::Candidate> {
    let normal = match read_property(connection, window, atoms.window_type)? {
        None => None,
        Some(kind) => {
            if kind.type_ != u32::from(AtomEnum::ATOM) || kind.format != 32 {
                return None;
            }
            let first = kind.value32()?.next()?;
            Some(atoms.normal != x11rb::NONE && first == atoms.normal)
        }
    };
    let transient = read_property(connection, window, AtomEnum::WM_TRANSIENT_FOR.into())?;
    let geometry = connection.get_geometry(window).ok()?.reply().ok()?;
    Some(main_window::Candidate {
        normal,
        transient: transient.is_some(),
        area: u32::from(geometry.width) * u32::from(geometry.height),
        order,
        title: read_title(connection, atoms, window),
    })
}

/// A window's property: `Some(None)` when it is not set, `None` when the
/// window could not be read.
fn read_property(
    connection: &RustConnection,
    window: Window,
    atom: Atom,
) -> Option<Option<GetPropertyReply>> {
    if atom == x11rb::NONE {
        return Some(None);
    }
    let reply = connection
        .get_property(false, window, atom, AtomEnum::ANY, 0, 1_024)
        .ok()?
        .reply()
        .ok()?;
    Some((reply.type_ != u32::from(AtomEnum::NONE)).then_some(reply))
}

fn read_title(connection: &RustConnection, atoms: &TitleAtoms, window: Window) -> Option<String> {
    let property = read_property(connection, window, atoms.name)??;
    if atoms.utf8 == x11rb::NONE
        || property.type_ != atoms.utf8
        || property.format != 8
        || property.bytes_after != 0
    {
        return None;
    }
    decode_title(property.value)
}

fn decode_title(bytes: Vec<u8>) -> Option<String> {
    let title = String::from_utf8(bytes).ok()?;
    let title = title.trim_matches('\0').trim();
    (!title.is_empty() && !title.contains('\0')).then(|| title.to_owned())
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
                .filter_map(|id| id.value.first().copied())
                .any(|owner| descends_from(owner, pid))
        })
}

/// Whether `process` is `ancestor` or started from it, following the parent
/// in `/proc/<pid>/stat`.
pub(in crate::system::launch) fn descends_from(process: u32, ancestor: u32) -> bool {
    let mut current = process;
    for _ in 0..MAX_ANCESTRY {
        if current == ancestor {
            return true;
        }
        match read_stat(current) {
            Ok(Some(stat)) if stat.parent == ancestor => return true,
            Ok(Some(stat)) if stat.parent > 1 => current = stat.parent,
            _ => return false,
        }
    }
    false
}

/// The evidence from an environment block and a window check, which runs only
/// when the environment leaves the bridge on. `NO_AT_BRIDGE=1` stops the ATK
/// bridge of GTK 3 and `GTK_A11Y=none` (any case) stops GTK 4; Qt reads
/// neither, so a Qt process with them set is treated as bridge-off too and is
/// reported after the grace with that note.
pub(crate) fn classify(environ: Option<&[u8]>, windowed: impl FnOnce() -> bool) -> OffBus {
    let bridge_off =
        environ.is_some_and(|block| block.split(|byte| *byte == 0).any(turns_bridge_off));
    if bridge_off {
        OffBus::BridgeOff
    } else if windowed() {
        OffBus::Windowed
    } else {
        OffBus::Starting
    }
}

fn turns_bridge_off(entry: &[u8]) -> bool {
    entry == b"NO_AT_BRIDGE=1"
        || entry
            .strip_prefix(b"GTK_A11Y=")
            .is_some_and(|value| value.eq_ignore_ascii_case(b"none"))
}

#[cfg(test)]
#[path = "launch_off_bus_tests.rs"]
mod tests;
