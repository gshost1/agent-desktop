use agent_desktop_core::{AdapterError, Deadline};

use crate::actions::chain::chain_delivery::DeliveryOutcome;
use crate::actions::chain::menu_gate::closed_menu_pid;
use crate::actions::chain::menu_observation::MenuObservation;
use crate::actions::chain::menu_tracking::{self, observe_menu};
use crate::actions::chain::selection_write::SelectionWrite;
use crate::tree::AXElement;

const MAX_MENU_SEARCH_NODES: usize = 2_048;

pub(crate) fn show_menu(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    show_menu_on_element(element, deadline)
}

pub(crate) fn show_menu_on_ancestors(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    let mut current = parent(element, deadline)?;
    for _ in 0..3 {
        let Some(ancestor) = current else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        let outcome = show_menu_on_element(&ancestor, deadline)?;
        if outcome.terminates_chain() {
            return Ok(outcome);
        }
        current = parent(&ancestor, deadline)?;
    }
    Ok(DeliveryOutcome::NotDelivered)
}

pub(crate) fn show_menu_on_children(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    let instant = instant(deadline)?;
    for child in crate::tree::surface_read::elements(element, "AXChildren", instant)?
        .into_iter()
        .take(5)
    {
        let outcome = show_menu_on_element(&child, deadline)?;
        if outcome.terminates_chain() {
            return Ok(outcome);
        }
    }
    Ok(DeliveryOutcome::NotDelivered)
}

pub(crate) fn select_then_show_menu(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    if closed_menu_pid(element, deadline).is_none() {
        return Ok(DeliveryOutcome::NotDelivered);
    }
    after_selection(select_containing_item(element, deadline)?, || {
        show_menu_on_element(element, deadline)
    })
}

pub(crate) fn select_then_selected_items_menu(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    if closed_menu_pid(element, deadline).is_none() {
        return Ok(DeliveryOutcome::NotDelivered);
    }
    after_selection(select_containing_item(element, deadline)?, || {
        let Some(window) = window_ancestor(element, deadline)? else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        let Some(menu_button) = selected_items_menu_button(&window, deadline)? else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        show_menu_or_press(&menu_button, deadline)
    })
}

/// A step that has written the selection has changed the application, so
/// what it reports afterwards carries that delivery: a later "nothing
/// delivered" is still an unverified delivery and a later failure is never a
/// safe retry.
fn after_selection(
    selection: SelectionWrite,
    then: impl FnOnce() -> Result<DeliveryOutcome, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    match selection {
        SelectionWrite::NotWritten => Ok(DeliveryOutcome::NotDelivered),
        SelectionWrite::Unconfirmed => Ok(DeliveryOutcome::DeliveredUnverified),
        SelectionWrite::Selected => match then() {
            Ok(outcome) if outcome.was_delivered() => Ok(outcome),
            Ok(_) => Ok(DeliveryOutcome::DeliveredUnverified),
            Err(error) => Err(after_write(error)),
        },
    }
}

/// Reads the selection back after a write that the application accepted,
/// with the settle every selection write gets, since the application applies
/// it a moment after it answers. A read that keeps failing cannot undo the
/// write, so it leaves the selection unconfirmed, not unwritten.
fn selection_after_write(
    deadline: Deadline,
    mut read: impl FnMut() -> Result<bool, AdapterError>,
) -> SelectionWrite {
    if crate::actions::select::selection_settle::direct_selection_holds(deadline, || read().ok()) {
        SelectionWrite::Selected
    } else {
        SelectionWrite::Unconfirmed
    }
}

fn after_write(error: AdapterError) -> AdapterError {
    crate::actions::DeliveryTracker::from_delivered_units(1).annotate(error)
}

fn show_menu_on_element(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    let Some(pid) = closed_menu_pid(element, deadline) else {
        return Ok(DeliveryOutcome::NotDelivered);
    };
    let baseline = MenuObservation::Closed;
    perform_menu_action(
        element,
        "AXShowMenu",
        MenuTarget { pid, baseline },
        deadline,
    )
}

fn show_menu_or_press(
    element: &AXElement,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    let Some(pid) = closed_menu_pid(element, deadline) else {
        return Ok(DeliveryOutcome::NotDelivered);
    };
    let baseline = MenuObservation::Closed;
    for action in ["AXShowMenu", "AXPress"] {
        let outcome = perform_menu_action(element, action, MenuTarget { pid, baseline }, deadline)?;
        if outcome.was_delivered() {
            return Ok(outcome);
        }
    }
    Ok(DeliveryOutcome::NotDelivered)
}

#[derive(Clone, Copy)]
struct MenuTarget {
    pid: i32,
    baseline: MenuObservation,
}

/// A menu-opening action can time out because the menu it opened is still
/// tracking, so a `kAXErrorCannotComplete` is settled by reading the menu
/// surface rather than reported as an unresponsive application.
fn perform_menu_action(
    element: &AXElement,
    action: &str,
    target: MenuTarget,
    deadline: Deadline,
) -> Result<DeliveryOutcome, AdapterError> {
    let call_deadline = menu_tracking::menu_action_deadline(deadline);
    prepare(element, call_deadline)?;
    match crate::actions::ax_helpers::try_ax_action_or_err(element, action, call_deadline) {
        Ok(false) => Ok(DeliveryOutcome::NotDelivered),
        Ok(true) => Ok(DeliveryOutcome::from_delivery(
            true,
            wait_for_new_menu(target.pid, deadline),
        )),
        Err(failure) if menu_tracking::is_tracking_timeout(&failure) => {
            menu_tracking::settle_tracking_timeout(
                failure,
                target.baseline,
                menu_tracking::poll_menu(target.pid, deadline),
            )
        }
        Err(failure) => Err(failure),
    }
}

/// Whether a new menu shows up after a posted menu action. The action is
/// already delivered, so a spent deadline ends the wait unverified instead of
/// turning the delivery into an error.
fn wait_for_new_menu(pid: i32, deadline: Deadline) -> bool {
    let local_end =
        std::time::Instant::now() + std::time::Duration::from_millis(menu_tracking::MENU_SETTLE_MS);
    loop {
        if menu_probe(pid, deadline) {
            return true;
        }
        if deadline.is_expired() || std::time::Instant::now() >= local_end {
            return false;
        }
        let Ok(pause) = deadline.remaining_slice(std::time::Duration::from_millis(25)) else {
            return false;
        };
        std::thread::sleep(pause.min(std::time::Duration::from_millis(25)));
    }
}

/// An application in menu tracking stops answering child reads, so a failed
/// probe is the expected shape of "a menu is up" rather than an application
/// fault. Verification must never turn a delivered action into an error.
fn menu_probe(pid: i32, deadline: Deadline) -> bool {
    observe_menu(pid, deadline) == MenuObservation::Open
}

fn select_containing_item(
    element: &AXElement,
    deadline: Deadline,
) -> Result<SelectionWrite, AdapterError> {
    let mut current = Some(element.clone());
    for _ in 0..4 {
        let Some(candidate) = current else {
            return Ok(SelectionWrite::NotWritten);
        };
        prepare(&candidate, deadline)?;
        if crate::actions::ax_helpers::is_attr_settable(&candidate, "AXSelected", deadline)? {
            prepare(&candidate, deadline)?;
            if crate::actions::ax_helpers::set_ax_bool_or_err(
                &candidate,
                "AXSelected",
                true,
                deadline,
            )? {
                return Ok(selection_after_write(deadline, || {
                    selected(&candidate, deadline)
                }));
            }
        }
        current = parent(&candidate, deadline)?;
    }
    Ok(SelectionWrite::NotWritten)
}

fn selected(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    Ok(
        crate::tree::surface_read::boolean(element, "AXSelected", instant(deadline)?)?
            == Some(true),
    )
}

fn window_ancestor(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let mut current = parent(element, deadline)?;
    for _ in 0..20 {
        let Some(ancestor) = current else {
            return Ok(None);
        };
        if crate::tree::surface_read::string(&ancestor, "AXRole", instant(deadline)?)?.as_deref()
            == Some("AXWindow")
        {
            return Ok(Some(ancestor));
        }
        current = parent(&ancestor, deadline)?;
    }
    Ok(None)
}

fn selected_items_menu_button(
    root: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let mut stack = vec![(root.clone(), 0_u8)];
    let mut visited = 0_usize;
    while let Some((candidate, depth)) = stack.pop() {
        visited = visited.saturating_add(1);
        if visited > MAX_MENU_SEARCH_NODES {
            return Err(AdapterError::new(
                agent_desktop_core::ErrorCode::AppUnresponsive,
                "Selected-items menu search exceeded its accessibility-node budget",
            )
            .with_details(serde_json::json!({
                "kind": "selected_items_menu_node_limit",
                "limit": MAX_MENU_SEARCH_NODES,
                "complete": false,
            })));
        }
        if is_selected_items_control(&candidate, deadline)? {
            return Ok(Some(candidate));
        }
        if depth >= 8 {
            continue;
        }
        stack.extend(
            crate::tree::surface_read::elements(&candidate, "AXChildren", instant(deadline)?)?
                .into_iter()
                .rev()
                .map(|child| (child, depth.saturating_add(1))),
        );
    }
    Ok(None)
}

fn is_selected_items_control(
    element: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if crate::tree::surface_read::string(element, "AXRole", instant(deadline)?)?.as_deref()
        != Some("AXMenuButton")
    {
        return Ok(false);
    }
    for attribute in ["AXHelp", "AXDescription", "AXTitle"] {
        if crate::tree::surface_read::string(element, attribute, instant(deadline)?)?
            .is_some_and(|value| value.to_ascii_lowercase().contains("selected item"))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parent(element: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    crate::tree::surface_read::element(element, "AXParent", instant(deadline)?)
}

fn instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    crate::tree::locator_deadline::from_operation(deadline)
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(test)]
#[path = "chain_menu_steps_tests.rs"]
mod tests;
