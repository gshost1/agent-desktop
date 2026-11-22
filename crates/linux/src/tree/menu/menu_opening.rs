use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use crate::system::menu::menu_surface::MenuPopup;

const OPENING_BUDGET: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn await_popup(
    deadline: Deadline,
    mut read: impl FnMut(Deadline) -> Result<Vec<MenuPopup>, AdapterError>,
    mut pending: impl FnMut(Deadline) -> bool,
) -> Result<MenuPopup, AdapterError> {
    let first = read(deadline)?;
    if !first.is_empty() {
        return super::observe_menu::outermost(first);
    }
    let opening = deadline.capped(OPENING_BUDGET);
    while !opening.remaining().is_zero() && pending(opening) {
        std::thread::sleep(opening.remaining().min(POLL_INTERVAL));
        if opening.remaining().is_zero() || !pending(opening) {
            break;
        }
        let popups = read(opening)?;
        if !popups.is_empty() {
            return super::observe_menu::outermost(popups);
        }
    }
    super::observe_menu::outermost(Vec::new())
}

#[cfg(test)]
#[path = "menu_opening_tests.rs"]
mod tests;
