use crate::system::registry::listed_window::ListedWindow;
use crate::system::x11::map::{ClientMatch, has_title_match, match_frames};
use crate::system::x11::twin_tie::{Twins, client_among_twins};
use crate::system::x11::{FrameFacts, X11Client, X11State};

/// The listed windows of one process that share the title of window `index`,
/// and whether the toolkit reports each active.
pub(crate) fn twins_of(listed: &[ListedWindow], index: usize) -> Option<Twins> {
    let own = &listed.get(index)?.frame;
    let mut active = Vec::new();
    let mut me = None;
    for (at, window) in listed.iter().enumerate() {
        if window.frame.pid == own.pid && window.frame.title == own.title {
            if at == index {
                me = Some(active.len());
            }
            active.push(window.active);
        }
    }
    Some(Twins { active, me: me? })
}

/// The X11 client of listed window `index`: its one client by process and
/// title or, among same-titled windows, the client `twin_tie` proves it is.
pub(crate) fn listed_client(
    listed: &[ListedWindow],
    index: usize,
    state: &X11State,
) -> ClientMatch {
    let X11State::Ready { active, clients } = state else {
        return ClientMatch::NoClient;
    };
    let frames: Vec<_> = listed.iter().map(|window| window.frame.clone()).collect();
    coherent_client(
        &frames,
        index,
        clients,
        || *active,
        || twins_of(listed, index),
    )
}

pub(crate) fn coherent_client(
    frames: &[FrameFacts],
    index: usize,
    clients: &[X11Client],
    active: impl FnOnce() -> Option<u32>,
    twins: impl FnOnce() -> Option<Twins>,
) -> ClientMatch {
    let Some(frame) = frames.get(index) else {
        return ClientMatch::NoClient;
    };
    if has_title_match(frame, clients) {
        return client_among_twins(frame, clients, active(), twins);
    }
    match_frames(frames, clients)
        .get(index)
        .cloned()
        .unwrap_or(ClientMatch::NoClient)
}

#[cfg(test)]
#[path = "listed_twins_tests.rs"]
mod tests;
