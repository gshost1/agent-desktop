use crate::system::registry::listed_window::ListedWindow;
use crate::system::x11::X11State;
use crate::system::x11::map::{ClientMatch, match_client};

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
    if let X11State::Ready { clients, .. } = state
        && windows
            .iter()
            .any(|window| match match_client(&window.frame, clients) {
                ClientMatch::One(id) => id == client,
                ClientMatch::Several(ids) => ids.contains(&client),
                ClientMatch::NoClient => false,
            })
    {
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
