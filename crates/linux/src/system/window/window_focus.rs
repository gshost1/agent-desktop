use crate::system::registry::listed_window::ListedWindow;
use crate::system::x11::{X11Evidence, X11State, evidence};

pub(super) fn active_client(state: &X11State) -> Option<(u32, u32)> {
    let X11State::Ready {
        active: Some(active),
        clients,
    } = state
    else {
        return None;
    };
    let client = clients.iter().find(|client| client.id == *active)?;
    Some((*active, client.pid?))
}

pub(super) fn reconcile_dialog(
    windows: &[ListedWindow],
    state: &X11State,
    input_focus_in: impl FnOnce(u32) -> bool,
) -> Option<usize> {
    let (client, pid) = active_client(state)?;
    let frames: Vec<_> = windows.iter().map(|window| window.frame.clone()).collect();
    if evidence(state, &frames) != X11Evidence::Unmapped {
        return None;
    }
    let mut active = windows
        .iter()
        .enumerate()
        .filter(|(_, window)| window.info.pid.get() == pid && window.active);
    let (index, window) = active.next()?;
    if active.next().is_some() || !(window.modal || window.dialog) || !input_focus_in(client) {
        return None;
    }
    Some(index)
}

#[cfg(test)]
#[path = "window_focus_tests.rs"]
mod tests;
