use agent_desktop_core::{Deadline, WindowInfo};

use super::listed_twins::coherent_client;
use crate::system::SharedBus;
use crate::system::focus::focus_cause::Cause;
use crate::system::registry::read_registry_for;
use crate::system::signal::signal_inventory::read_selected;
use crate::system::x11::FrameFacts;
use crate::system::x11::X11Client;
use crate::system::x11::map::{ClientMatch, has_title_match};
use crate::system::x11::twin_tie::{Twins, client_among_twins};
use crate::tree::Extents;

/// What the mapping of a window to its X11 client reads from the world: the
/// window's toolkit decides whether its reported position is global.
pub(crate) trait ClientWorld {
    fn global_position(&self) -> bool;

    /// The process's listed frames with size-identity proofs and the index
    /// of the window identified by `win.id`.
    fn frames(&self, _win: &WindowInfo) -> Option<(Vec<FrameFacts>, usize)> {
        None
    }

    /// The frames of the window's process that share its title, when this
    /// world reads them. A world that does not leaves a tie between clients
    /// a tie.
    fn twins(&self, _win: &WindowInfo) -> Option<Twins> {
        None
    }
}

pub(crate) fn facts_of(win: &WindowInfo, global_position: bool) -> FrameFacts {
    FrameFacts {
        size_identity: false,
        pid: win.pid.get(),
        title: win.title.clone(),
        extents: win.bounds.map(|rect| Extents {
            x: rect.x as i32,
            y: rect.y as i32,
            width: rect.width as i32,
            height: rect.height as i32,
        }),
        global_position,
    }
}

pub(crate) fn client_of(
    world: &impl ClientWorld,
    win: &WindowInfo,
    clients: &[X11Client],
    active_client: impl FnOnce() -> Option<u32>,
) -> Result<u32, Cause> {
    let found = if let Some((frames, index)) = world.frames(win) {
        coherent_client(&frames, index, clients, active_client, || world.twins(win))
    } else {
        let frame = facts_of(win, world.global_position());
        if has_title_match(&frame, clients) {
            client_among_twins(&frame, clients, active_client(), || world.twins(win))
        } else {
            ClientMatch::NoClient
        }
    };
    match found {
        ClientMatch::One(id) => Ok(id),
        ClientMatch::Several(_) => Err(Cause::AmbiguousWindow),
        ClientMatch::NoClient => Err(Cause::WindowNotMapped),
    }
}

pub(crate) fn frames_on_bus(
    bus: &SharedBus,
    win: &WindowInfo,
    deadline: Deadline,
) -> Option<(Vec<FrameFacts>, usize)> {
    let registry = read_registry_for(bus, Some(win.pid.get()), deadline).ok()?;
    let (listed, outcomes) = read_selected(
        bus,
        &registry,
        |app| app.root.pid() == win.pid.get(),
        deadline,
    )
    .ok()?;
    if outcomes.is_empty()
        || outcomes
            .iter()
            .any(|outcome| outcome.read != crate::system::registry::registry_frames::RootRead::Read)
    {
        return None;
    }
    let index = listed.iter().position(|window| {
        window.info.id == win.id && window.info.process_instance == win.process_instance
    })?;
    Some((
        listed.into_iter().map(|window| window.frame).collect(),
        index,
    ))
}

#[cfg(test)]
#[path = "window_client_tests.rs"]
mod tests;
