use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::system::menu::menu_surface::MenuPopup;

const OPENING_BUDGET: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn await_popup(
    deadline: Deadline,
    mut read: impl FnMut(Deadline) -> Result<Vec<MenuPopup>, AdapterError>,
) -> Result<MenuPopup, AdapterError> {
    let first = read(deadline)?;
    if !first.is_empty() {
        return super::observe_menu::outermost(first);
    }
    let opening = deadline.capped(OPENING_BUDGET);
    while !opening.remaining().is_zero() {
        std::thread::sleep(opening.remaining().min(POLL_INTERVAL));
        if opening.remaining().is_zero() {
            break;
        }
        let popups = match read(opening) {
            Ok(popups) => popups,
            Err(error) if error.code == ErrorCode::Timeout && opening.remaining().is_zero() => {
                break;
            }
            Err(error) => return Err(error),
        };
        if !popups.is_empty() {
            return super::observe_menu::outermost(popups);
        }
    }
    super::observe_menu::outermost(Vec::new())
}

#[cfg(test)]
#[path = "menu_opening_tests.rs"]
mod tests;
