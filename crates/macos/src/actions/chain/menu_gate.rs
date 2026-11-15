//! The menu state a right-click step must read before it writes the selection
//! or opens a menu.

use agent_desktop_core::{AdapterError, Deadline};

use crate::actions::chain::menu_observation::MenuObservation;
use crate::tree::AXElement;

const MENU_GATE_READS: usize = 3;
const MENU_GATE_PAUSE_MS: u64 = 25;

/// Only a menu read that positively answered "closed" lets a step open one:
/// an open menu would be opened twice, and an unreadable read is what an
/// application tracking a menu returns, so it is treated as one being up.
fn may_open_menu(observation: MenuObservation) -> bool {
    observation == MenuObservation::Closed
}

/// Reads the menu state again while it is unreadable, a few times within the
/// deadline, so one failed read of a responsive application does not refuse
/// the step; a read that keeps failing is the answer.
fn settled_menu_observation(
    mut observe: impl FnMut() -> MenuObservation,
    deadline: Deadline,
) -> MenuObservation {
    let mut observation = observe();
    for _ in 1..MENU_GATE_READS {
        if observation != MenuObservation::Unreadable || deadline.is_expired() {
            break;
        }
        std::thread::sleep(
            deadline
                .remaining()
                .min(std::time::Duration::from_millis(MENU_GATE_PAUSE_MS)),
        );
        observation = observe();
    }
    observation
}

/// The target's pid when its menu state reads as closed, the one state from
/// which a step may write the selection or open a menu.
pub(crate) fn closed_menu_pid(element: &AXElement, deadline: Deadline) -> Option<i32> {
    let pid = crate::system::app::app_ops::pid_from_element(element, deadline)?;
    may_open_menu(settled_menu_observation(
        || gate_read(pid, deadline),
        deadline,
    ))
    .then_some(pid)
}

fn gate_read(pid: i32, deadline: Deadline) -> MenuObservation {
    gate_answer(
        crate::system::window::popup_menu_window::menu_window_numbers(pid),
        || {
            crate::tree::locator_deadline::from_operation(deadline)
                .and_then(|instant| crate::tree::surfaces::is_menu_open(pid, instant))
        },
    )
}

/// Answers from two reads. The window server goes first: it never waits on
/// the application, and a pop-up menu window of the process is a menu on
/// screen whatever the accessibility tree says. The accessibility read then
/// decides, except that a menu search that ran out of its node budget proves
/// nothing alone; it counts as no menu only when the window server answered
/// and listed no menu window, since the menu bar was read before the search
/// and showed none. Every other failed read refuses.
fn gate_answer(
    menu_windows: Result<Vec<i64>, AdapterError>,
    accessibility: impl FnOnce() -> Result<bool, AdapterError>,
) -> MenuObservation {
    let window_server_saw_none = match &menu_windows {
        Ok(windows) if !windows.is_empty() => return MenuObservation::Open,
        Ok(_) => true,
        Err(_) => false,
    };
    match accessibility() {
        Ok(true) => MenuObservation::Open,
        Ok(false) => MenuObservation::Closed,
        Err(error) if window_server_saw_none && searched_past_budget(&error) => {
            MenuObservation::Closed
        }
        Err(_) => MenuObservation::Unreadable,
    }
}

fn searched_past_budget(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(serde_json::Value::as_str)
        == Some("surface_search_limit")
}

#[cfg(test)]
#[path = "menu_gate_tests.rs"]
mod tests;
