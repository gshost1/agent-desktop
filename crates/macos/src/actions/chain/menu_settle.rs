//! Settles a right-click whose chain ran out of steps without a verified one.
//!
//! Some toolkits open a context menu that the application's accessibility
//! tree never exposes, so every step's own menu read misses it and the chain
//! reports exhaustion while the menu is on screen. The window server still
//! lists that menu as an on-screen window of the target's process above the
//! normal layer. A menu surface that a baseline read before the first step
//! positively showed absent, and that is present after the steps, is the
//! postcondition, so the delivery is verified. Anything less keeps the
//! exhaustion error unchanged.

use agent_desktop_core::{
    ActionStep, ActionStepOutcome, AdapterError, Deadline, DeliverySemantics,
};

use crate::actions::chain::exhaustion_error;
use crate::actions::chain::menu_baseline::MenuBaseline;
use crate::actions::chain::menu_read::MenuRead;

pub(crate) const MENU_SETTLE_MS: u64 = 600;
const MENU_POLL_MS: u64 = 25;
pub(crate) const SETTLE_STEP_LABEL: &str = "menu_surface_settle";

/// Reads until a menu appears or the reads are exhausted. Only a chain that
/// delivered something and a baseline that was positively closed can be
/// upgraded; every other path returns the exhaustion error the chain reports
/// without a settle.
pub(crate) fn settle_exhausted(
    steps: Vec<ActionStep>,
    baseline: Option<MenuBaseline>,
    mut observe: impl FnMut(&MenuBaseline) -> MenuRead,
    suggestion: &str,
) -> Result<Vec<ActionStep>, AdapterError> {
    let original = exhaustion_error(&steps, suggestion);
    if original.disposition != DeliverySemantics::delivered_unverified() {
        return Err(original);
    }
    let Some(baseline) = baseline else {
        return Err(original);
    };
    loop {
        match observe(&baseline) {
            MenuRead::Appeared => return Ok(verified_by_settle(steps)),
            MenuRead::Absent => {}
            MenuRead::Exhausted => return Err(original),
        }
    }
}

/// The menu was observed for the chain as a whole, not for one step, so the
/// delivered steps drop their per-step verdicts and the settle read carries
/// the verification.
fn verified_by_settle(mut steps: Vec<ActionStep>) -> Vec<ActionStep> {
    for step in &mut steps {
        if matches!(step.outcome, ActionStepOutcome::Succeeded) {
            step.verified = None;
        }
    }
    steps.push(ActionStep::succeeded(SETTLE_STEP_LABEL).with_verified(true));
    steps
}

pub(crate) fn is_new_menu_window(baseline: &MenuBaseline, menu_windows_now: &[i64]) -> bool {
    menu_windows_now
        .iter()
        .any(|number| !baseline.menu_windows.contains(number))
}

/// A baseline exists only when the accessibility read shows no open menu and
/// the window server answers; the window list is read only then. A menu
/// search that ran out of its node budget, as in a window with thousands of
/// elements, shows no open menu either: the baseline then rests on the window
/// list alone, and only a pop-up menu window that appears afterwards counts.
pub(crate) fn baseline_from(
    ax_open: Result<bool, AdapterError>,
    menu_windows: impl FnOnce() -> Result<Vec<i64>, AdapterError>,
) -> Option<MenuBaseline> {
    match ax_open {
        Ok(false) => {}
        Err(error) if crate::system::window::popup_menu_window::searched_past_budget(&error) => {}
        Ok(true) | Err(_) => return None,
    }
    Some(MenuBaseline {
        menu_windows: menu_windows().ok()?,
    })
}

/// Either read showing a menu counts. The window server read goes first
/// because it never waits on the application, which may stop answering while
/// its menu tracks; the accessibility read runs only when it saw nothing new.
pub(crate) fn read_from(
    baseline: &MenuBaseline,
    menu_windows_now: Result<Vec<i64>, AdapterError>,
    ax_open: impl FnOnce() -> Result<bool, AdapterError>,
) -> MenuRead {
    if menu_windows_now.is_ok_and(|now| is_new_menu_window(baseline, &now)) {
        return MenuRead::Appeared;
    }
    if matches!(ax_open(), Ok(true)) {
        MenuRead::Appeared
    } else {
        MenuRead::Absent
    }
}

/// Reads at once, then every few milliseconds until the settle window or the
/// operation deadline ends. Each read gets a deadline capped to what is left
/// of the settle window, so no read runs past it. A spent deadline reads
/// nothing.
pub(crate) fn poll_within(
    deadline: Deadline,
    settle: std::time::Duration,
    mut read: impl FnMut(&MenuBaseline, Deadline) -> MenuRead,
) -> impl FnMut(&MenuBaseline) -> MenuRead {
    let settle_end = std::time::Instant::now() + settle;
    let mut first = true;
    move |baseline| {
        let poll = std::time::Duration::from_millis(MENU_POLL_MS);
        if !first {
            let left = settle_end.saturating_duration_since(std::time::Instant::now());
            std::thread::sleep(deadline.remaining().min(left).min(poll));
        }
        first = false;
        let left = settle_end.saturating_duration_since(std::time::Instant::now());
        if deadline.is_expired() || left.is_zero() {
            return MenuRead::Exhausted;
        }
        read(baseline, deadline.capped(left))
    }
}

mod imp {
    use agent_desktop_core::actions::interaction_policy::InteractionPolicy;
    use agent_desktop_core::{ActionStep, AdapterError, Deadline};

    use super::{MENU_SETTLE_MS, baseline_from, poll_within, read_from, settle_exhausted};
    use crate::actions::chain::menu_baseline::MenuBaseline;
    use crate::actions::chain::menu_read::MenuRead;
    use crate::actions::chain::{ChainContext, ChainDef, exhaustion_error, run_steps};
    use crate::system::window::popup_menu_window::menu_window_numbers;
    use crate::tree::AXElement;

    /// Runs a menu-opening chain and settles it when it ends without a
    /// verified step.
    pub(crate) fn execute_menu_chain(
        element: &AXElement,
        definition: &ChainDef,
        context: &ChainContext,
        policy: InteractionPolicy,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let pid = crate::system::app::app_ops::pid_from_element(element, context.deadline);
        let baseline = pid.and_then(|pid| {
            baseline_from(ax_menu_open(pid, context.deadline), || {
                menu_window_numbers(pid)
            })
        });
        let (steps, finished) = run_steps(element, definition, context, policy, pid)?;
        if finished {
            return Ok(steps);
        }
        let Some(pid) = pid else {
            return Err(exhaustion_error(&steps, definition.suggestion));
        };
        settle_exhausted(
            steps,
            baseline,
            poll_menu(pid, context.deadline),
            definition.suggestion,
        )
    }

    fn ax_menu_open(pid: i32, deadline: Deadline) -> Result<bool, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        crate::tree::surfaces::is_menu_open(pid, instant)
    }

    fn poll_menu(pid: i32, deadline: Deadline) -> impl FnMut(&MenuBaseline) -> MenuRead {
        poll_within(
            deadline,
            std::time::Duration::from_millis(MENU_SETTLE_MS),
            move |baseline, read_deadline| {
                read_from(baseline, menu_window_numbers(pid), || {
                    ax_menu_open(pid, read_deadline)
                })
            },
        )
    }
}

pub(crate) use imp::execute_menu_chain;

#[cfg(test)]
#[path = "menu_settle_tests.rs"]
mod tests;
