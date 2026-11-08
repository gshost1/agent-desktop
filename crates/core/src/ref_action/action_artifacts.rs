//! The pre/post screenshot pair of one ref action in a full-artifacts session.

use crate::ref_action::ref_action_wait_context::RefActionWaitContext;
use crate::trace::trace_artifacts::{
    ArtifactOutcome, capture_action_screenshot, emit_action_artifacts,
};
use std::time::Duration;

const TRACE_CAPTURE_BUDGET_MS: u64 = 1_000;
const PRE_CAPTURE_DISPATCH_RESERVE_MS: u64 = 100;
const NOT_DISPATCHED: &str = "not_dispatched";

/// The pre-action screenshot of one ref action and the `action.artifacts`
/// event that must reference it.
///
/// The pre screenshot is taken at most once per command, before the first
/// attempt, and kept across auto-wait refusals. The guard then ends exactly
/// once: [`ActionArtifacts::finish_delivered`] takes the post screenshot right
/// after dispatch on its own budget, before any post-action wait, and dropping
/// the guard without it records the pre screenshot with
/// `skipped_post: "not_dispatched"`. A written pre file therefore always has an
/// event that references it, whichever refusal ends the action.
pub(crate) struct ActionArtifacts<'a> {
    target: RefActionWaitContext<'a>,
    pre: Option<ArtifactOutcome>,
}

impl<'a> ActionArtifacts<'a> {
    /// A guard for `target` that holds no pre screenshot yet.
    pub(crate) fn new(target: RefActionWaitContext<'a>) -> Self {
        Self { target, pre: None }
    }

    /// Takes the pre screenshot unless this command already took one. It
    /// leaves `deadline` a dispatch reserve and spends at most one second.
    pub(crate) fn capture_pre_once(&mut self, deadline: crate::Deadline) {
        if self.pre.is_some() {
            return;
        }
        let pre = match pre_capture_deadline(deadline) {
            Some(capture_deadline) => self.capture("pre", capture_deadline),
            None => ArtifactOutcome::Skipped("dispatch_budget".into()),
        };
        self.pre = Some(pre);
    }

    /// Ends the pair after a dispatch: takes the post screenshot now, on a
    /// budget of its own rather than what the action's deadline has left, and
    /// emits the event.
    pub(crate) fn finish_delivered(mut self) {
        let Some(pre) = self.pre.take() else {
            return;
        };
        let post = match crate::Deadline::after(TRACE_CAPTURE_BUDGET_MS) {
            Ok(deadline) => self.capture("post", deadline),
            Err(_) => ArtifactOutcome::Skipped("deadline".into()),
        };
        self.emit(&pre, &post);
    }

    fn capture(&self, phase: &str, deadline: crate::Deadline) -> ArtifactOutcome {
        capture_action_screenshot(
            self.target.context,
            self.target.adapter,
            self.target.entry,
            phase,
            deadline,
        )
    }

    fn emit(&self, pre: &ArtifactOutcome, post: &ArtifactOutcome) {
        if let Err(error) =
            emit_action_artifacts(self.target.context, self.target.ref_id, pre, post)
        {
            tracing::warn!(
                error = %error,
                ref_id = self.target.ref_id,
                "action artifact emission failed"
            );
        }
    }
}

impl Drop for ActionArtifacts<'_> {
    fn drop(&mut self) {
        if let Some(pre) = self.pre.take() {
            self.emit(&pre, &ArtifactOutcome::Skipped(NOT_DISPATCHED.into()));
        }
    }
}

fn pre_capture_deadline(parent: crate::Deadline) -> Option<crate::Deadline> {
    let remaining = parent.remaining();
    let reserve = Duration::from_millis(PRE_CAPTURE_DISPATCH_RESERVE_MS);
    if remaining <= reserve {
        return None;
    }
    let capture_budget = remaining
        .saturating_sub(reserve)
        .min(Duration::from_millis(TRACE_CAPTURE_BUDGET_MS));
    Some(parent.capped(capture_budget))
}
