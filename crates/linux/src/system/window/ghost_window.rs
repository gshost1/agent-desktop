use std::collections::HashSet;

use crate::system::registry::listed_window::ListedWindow;
use crate::system::window::listed_twins::listed_client;
use crate::system::x11::X11State;
use crate::system::x11::map::ClientMatch;

fn matched(found: ClientMatch) -> Vec<u32> {
    match found {
        ClientMatch::One(id) => vec![id],
        ClientMatch::Several(ids) => ids,
        ClientMatch::NoClient => Vec::new(),
    }
}

/// Removes the husks of closed windows. A toolkit that keeps a top-level
/// window for reuse leaves it on the registry after it hides or closes, with
/// neither `SHOWING` nor `ICONIFIED`. No live window measured lacks both: a
/// minimized one shows or is iconified, and one on another desktop shows.
/// An unshown window stays only when the display server shows it is not a
/// husk: its client is the only claimant of a managed X11 window of the same
/// process. A minimized window the window manager still manages is kept,
/// while the husk of a reopened dialog, whose client belongs to the live
/// dialog, is dropped.
pub(crate) fn without_ghosts(windows: Vec<ListedWindow>, state: &X11State) -> Vec<ListedWindow> {
    if !windows.iter().any(|window| window.unshown) {
        return windows;
    }
    let matches: Vec<_> = (0..windows.len())
        .map(|index| matched(listed_client(&windows, index, state)))
        .collect();
    let claimed: HashSet<u32> = windows
        .iter()
        .zip(&matches)
        .filter(|(window, _)| !window.unshown)
        .flat_map(|(_, found)| found.iter().copied())
        .collect();
    windows
        .into_iter()
        .zip(matches)
        .filter(|(window, found)| {
            !window.unshown || found.iter().any(|client| !claimed.contains(client))
        })
        .map(|(window, _)| window)
        .collect()
}

#[cfg(test)]
#[path = "ghost_window_tests.rs"]
mod tests;
