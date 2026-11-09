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

pub(crate) fn is_new_raised_window(baseline: &MenuBaseline, raised_now: &[i64]) -> bool {
    raised_now
        .iter()
        .any(|number| !baseline.raised_windows.contains(number))
}

/// Reads at once, then every few milliseconds until the bounded settle
/// window or the operation deadline ends. A spent deadline reads nothing.
pub(crate) fn poll_within(
    deadline: Deadline,
    mut read: impl FnMut(&MenuBaseline) -> MenuRead,
) -> impl FnMut(&MenuBaseline) -> MenuRead {
    let settle_end = std::time::Instant::now() + std::time::Duration::from_millis(MENU_SETTLE_MS);
    let mut first = true;
    move |baseline| {
        if deadline.is_expired() {
            return MenuRead::Exhausted;
        }
        if !first {
            if std::time::Instant::now() >= settle_end {
                return MenuRead::Exhausted;
            }
            std::thread::sleep(
                deadline
                    .remaining()
                    .min(std::time::Duration::from_millis(MENU_POLL_MS)),
            );
            if deadline.is_expired() {
                return MenuRead::Exhausted;
            }
        }
        first = false;
        read(baseline)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::actions::interaction_policy::InteractionPolicy;
    use agent_desktop_core::{ActionStep, AdapterError, Deadline};

    use super::{is_new_raised_window, poll_within, settle_exhausted};
    use crate::actions::chain::menu_baseline::MenuBaseline;
    use crate::actions::chain::menu_read::MenuRead;
    use crate::actions::chain::{ChainContext, ChainDef, exhaustion_error, run_steps};
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
        let baseline = pid.and_then(|pid| read_baseline(pid, context.deadline));
        let (steps, finished) = run_steps(element, definition, context, policy)?;
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

    fn read_baseline(pid: i32, deadline: Deadline) -> Option<MenuBaseline> {
        let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
        if crate::tree::surfaces::is_menu_open(pid, instant).ok()? {
            return None;
        }
        let raised_windows =
            crate::system::window::raised_window::raised_window_numbers(pid).ok()?;
        Some(MenuBaseline { raised_windows })
    }

    /// The window server read goes first because it never waits on the
    /// application, which may stop answering while its menu tracks.
    fn read_once(pid: i32, deadline: Deadline, baseline: &MenuBaseline) -> MenuRead {
        if crate::system::window::raised_window::raised_window_numbers(pid)
            .is_ok_and(|raised| is_new_raised_window(baseline, &raised))
        {
            return MenuRead::Appeared;
        }
        let ax_open = crate::tree::locator_deadline::from_operation(deadline)
            .and_then(|instant| crate::tree::surfaces::is_menu_open(pid, instant));
        if matches!(ax_open, Ok(true)) {
            MenuRead::Appeared
        } else {
            MenuRead::Absent
        }
    }

    fn poll_menu(pid: i32, deadline: Deadline) -> impl FnMut(&MenuBaseline) -> MenuRead {
        poll_within(deadline, move |baseline| read_once(pid, deadline, baseline))
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::execute_menu_chain;

#[cfg(test)]
#[path = "menu_settle_tests.rs"]
mod tests;
