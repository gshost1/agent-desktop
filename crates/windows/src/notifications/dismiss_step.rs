use agent_desktop_core::NotificationInfo;

use super::verify::{entry_gone, same_identity};

#[cfg(test)]
#[path = "dismiss_step_tests.rs"]
mod tests;

/// What a single-entry dismiss does after it re-reads the center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DismissStep {
    Verified,
    SubstituteClearAll,
    Survived,
}

/// The step the dismiss path takes once the entry's own dismiss control was
/// invoked, judged from the listing the target was picked from (`before`) and
/// the settled re-read (`current`).
pub(super) fn after_dismiss(
    target: &NotificationInfo,
    before: &[NotificationInfo],
    current: &[NotificationInfo],
) -> DismissStep {
    let _ = before;
    if entry_gone(target, current) {
        return DismissStep::Verified;
    }
    if is_sole_target(target, current) {
        return DismissStep::SubstituteClearAll;
    }
    DismissStep::Survived
}

pub(super) fn is_sole_target(target: &NotificationInfo, current: &[NotificationInfo]) -> bool {
    matches!(current, [only] if same_identity(target, only))
}
