use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use super::window_focus::active_client;
use crate::system::signal::signal_inventory::Pass;
use crate::system::x11::X11State;
use crate::system::x11::map::{ClientMatch, match_frames};

const REREAD_LIMIT: usize = 3;
const REREAD_BUDGET: Duration = Duration::from_millis(300);
const REREAD_INTERVAL: Duration = Duration::from_millis(25);

fn disagrees(pass: &Pass, state: &X11State, selected: &[u32]) -> bool {
    let Some((active, pid)) = active_client(state).filter(|_| pass.focused.is_none()) else {
        return false;
    };
    if !selected.contains(&pid)
        && !pass
            .windows
            .iter()
            .any(|window| window.info.pid.get() == pid)
    {
        return false;
    }
    let X11State::Ready { clients, .. } = state else {
        return false;
    };
    let frames: Vec<_> = pass
        .windows
        .iter()
        .map(|window| window.frame.clone())
        .collect();
    let matches = match_frames(&frames, clients);
    !matches.iter().any(|found| match found {
        ClientMatch::One(id) => *id == active,
        ClientMatch::Several(ids) => ids.contains(&active),
        ClientMatch::NoClient => false,
    }) && frames
        .iter()
        .zip(matches)
        .filter(|(frame, _)| frame.pid == pid)
        .all(|(_, found)| match found {
            ClientMatch::One(id) => id != active,
            ClientMatch::Several(ids) => ids.iter().any(|id| *id != active),
            ClientMatch::NoClient => false,
        })
}

fn stable(previous: &(Pass, X11State), next: &(Pass, X11State)) -> bool {
    let ids = |pass: &Pass| -> std::collections::BTreeSet<String> {
        pass.windows
            .iter()
            .map(|window| window.info.id.clone())
            .collect()
    };
    active_client(&previous.1) == active_client(&next.1) && ids(&previous.0) == ids(&next.0)
}

pub(super) fn reread_with(
    mut current: (Pass, X11State),
    deadline: Deadline,
    selected: &[u32],
    mut read: impl FnMut(Deadline) -> Result<(Pass, X11State), AdapterError>,
    mut pause: impl FnMut(Duration),
) -> Result<(Pass, X11State), AdapterError> {
    let budget = deadline.capped(REREAD_BUDGET);
    for _ in 0..REREAD_LIMIT {
        if !disagrees(&current.0, &current.1, selected) || budget.remaining() <= REREAD_INTERVAL {
            break;
        }
        pause(REREAD_INTERVAL);
        if budget.is_expired() {
            break;
        }
        match read(budget) {
            Ok(next) => {
                let settled = stable(&current, &next);
                current = next;
                if settled {
                    break;
                }
            }
            Err(error) if error.code == ErrorCode::Timeout => break,
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

#[cfg(test)]
#[path = "window_reread_tests.rs"]
mod tests;
