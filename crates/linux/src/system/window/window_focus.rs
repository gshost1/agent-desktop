use super::dialog_evidence::DialogEvidence;
use crate::system::registry::listed_window::ListedWindow;
use crate::system::x11::X11State;
use crate::system::x11::map::{
    ClientMatch, exact_geometry_matches, geometry_client, match_frames, size_matches,
};

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
    evidence: impl FnOnce(u32) -> DialogEvidence,
) -> Option<usize> {
    let (client, pid) = active_client(state)?;
    let mut active = windows
        .iter()
        .enumerate()
        .filter(|(_, window)| window.info.pid.get() == pid && window.active);
    let (index, window) = active.next()?;
    if active.next().is_some() || !(window.modal || window.dialog) {
        return None;
    }
    let X11State::Ready { clients, .. } = state else {
        return None;
    };
    let native = clients.iter().find(|native| native.id == client)?;
    let frames: Vec<_> = windows.iter().map(|window| window.frame.clone()).collect();
    let matches = match_frames(&frames, clients);
    if matches.iter().enumerate().any(|(other, found)| {
        other != index
            && match found {
                ClientMatch::One(id) => *id == client,
                ClientMatch::Several(ids) => ids.contains(&client),
                ClientMatch::NoClient => false,
            }
    }) {
        return None;
    }
    let geometry = matches[index] == ClientMatch::One(client)
        && geometry_client(&window.frame, std::slice::from_ref(native)) == ClientMatch::One(client);
    let evidence = evidence(client);
    let transient = evidence.transient_for.is_some_and(|owner| {
        owner != client
            && clients
                .iter()
                .any(|other| other.id == owner && other.pid == Some(pid))
            && size_matches(&window.frame, native)
            && (window.frame.size_identity || exact_geometry_matches(&window.frame, native))
            && !windows.iter().enumerate().any(|(other, candidate)| {
                other != index
                    && candidate.frame.pid == pid
                    && candidate
                        .frame
                        .extents
                        .zip(window.frame.extents)
                        .is_some_and(|(candidate, frame)| {
                            candidate.width == frame.width && candidate.height == frame.height
                        })
            })
    });
    (evidence.input_focus && (geometry || transient)).then_some(index)
}

#[cfg(test)]
#[path = "window_focus_tests.rs"]
mod tests;
