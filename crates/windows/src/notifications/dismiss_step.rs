use agent_desktop_core::NotificationInfo;

use super::verify::{departed, same_identity};

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
///
/// The dismiss is verified when the entries identical to the target dropped
/// in number. The clear-all stands in for an ignored dismiss only when the
/// target is still the center's sole entry; an unverified sole survivor means
/// the target had no identical twin when it was picked, because a listed twin
/// dropping to one is already a verified dismiss. A survivor beside a listed
/// twin is therefore never cleared: it cannot be told apart from that twin,
/// so clearing it could remove a notification nobody targeted.
pub(super) fn after_dismiss(
    target: &NotificationInfo,
    before: &[NotificationInfo],
    current: &[NotificationInfo],
) -> DismissStep {
    if departed(std::slice::from_ref(target), before, current) {
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
