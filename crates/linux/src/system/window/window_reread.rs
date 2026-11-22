use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use super::window_focus::active_client;
use crate::system::signal::signal_inventory::Pass;
use crate::system::x11::X11State;

const REREAD_LIMIT: usize = 3;
const REREAD_BUDGET: Duration = Duration::from_millis(300);
const REREAD_INTERVAL: Duration = Duration::from_millis(25);

fn disagrees(pass: &Pass, state: &X11State, selected: &[u32]) -> bool {
    pass.focused.is_none()
        && active_client(state).is_some_and(|(_, pid)| {
            selected.contains(&pid)
                || pass
                    .windows
                    .iter()
                    .any(|window| window.info.pid.get() == pid)
        })
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
        current = read(budget)?;
    }
    Ok(current)
}

#[cfg(test)]
#[path = "window_reread_tests.rs"]
mod tests;
