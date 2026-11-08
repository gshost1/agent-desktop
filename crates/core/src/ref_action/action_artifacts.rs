//! The pre/post screenshot pair of one ref action in a full-artifacts session.

use crate::DeliverySemantics;
use crate::ref_action::ref_action_wait_context::RefActionWaitContext;
use crate::trace::trace_artifacts::{
    ArtifactOutcome, capture_action_screenshot, emit_action_artifacts,
};
use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

const TRACE_CAPTURE_BUDGET_MS: u64 = 1_000;
const PRE_CAPTURE_DISPATCH_RESERVE_MS: u64 = 100;
const NOT_DISPATCHED: &str = "not_dispatched";
const OUT_OF_TIME: [&str; 3] = ["dispatch_budget", "deadline", "adapter: TIMEOUT"];

/// The pre-action screenshot of one ref action and the `action.artifacts`
/// event that must reference it.
///
/// The pre screenshot is taken at most once per command and kept across
/// auto-wait refusals; one skipped only because it ran out of time is tried
/// again on the next attempt. The guard then ends exactly once. When the screen
/// may have changed (a dispatch of any disposition but `not_delivered`, or a
/// focus call or preparatory scroll that ran before a refusal),
/// [`ActionArtifacts::finish_with_post`]
/// takes the post screenshot within what the action's deadline has left, at
/// most one second, and records the disposition. Dropping the guard without it
/// records the pre screenshot with `skipped_post: "not_dispatched"`. A written
/// pre file therefore always has an event that references it.
pub(crate) struct ActionArtifacts<'a> {
    target: RefActionWaitContext<'a>,
    deadline: crate::Deadline,
    pre: Option<ArtifactOutcome>,
    pre_taken_at: Option<Instant>,
    attempted_at: Option<Instant>,
}

impl<'a> ActionArtifacts<'a> {
    /// A guard for `target` under the action's `deadline`, holding no pre
    /// screenshot yet.
    pub(crate) fn new(target: RefActionWaitContext<'a>, deadline: crate::Deadline) -> Self {
        Self {
            target,
            deadline,
            pre: None,
            pre_taken_at: None,
            attempted_at: None,
        }
    }

    /// Takes the pre screenshot unless this command already has one, or has
    /// one that was skipped for a reason time cannot fix. It leaves the
    /// deadline a dispatch reserve and spends at most one second.
    pub(crate) fn capture_pre_once(&mut self) {
        let retry = match &self.pre {
            None => true,
            Some(ArtifactOutcome::Skipped(reason)) => OUT_OF_TIME.contains(&reason.as_str()),
            Some(ArtifactOutcome::Captured(_)) => false,
        };
        if !retry {
            return;
        }
        let pre = match pre_capture_deadline(self.deadline) {
            Some(capture_deadline) => self.capture("pre", capture_deadline),
            None => ArtifactOutcome::Skipped("dispatch_budget".into()),
        };
        self.pre_taken_at = Some(crate::wait::deadline::now());
        self.pre = Some(pre);
    }

    /// Notes that an attempt under the lease starts now, which is what the
    /// pre screenshot's age is measured to.
    pub(crate) fn attempting(&mut self) {
        self.attempted_at = Some(crate::wait::deadline::now());
    }

    /// Ends the pair when the screen may have changed: takes the post
    /// screenshot now, within the action's deadline, and records `delivery`.
    pub(crate) fn finish_with_post(mut self, delivery: DeliverySemantics) {
        let Some(pre) = self.pre.take() else {
            return;
        };
        let budget = Duration::from_millis(TRACE_CAPTURE_BUDGET_MS);
        let post = self.capture("post", self.deadline.capped(budget));
        self.emit(&pre, &post, delivery);
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

    fn emit(&self, pre: &ArtifactOutcome, post: &ArtifactOutcome, delivery: DeliverySemantics) {
        let mut extra = Map::new();
        extra.insert("delivery".into(), json!(delivery.delivery()));
        if let (ArtifactOutcome::Captured(_), Some(taken), Some(attempted)) =
            (pre, self.pre_taken_at, self.attempted_at)
        {
            let age = attempted.saturating_duration_since(taken).as_millis();
            extra.insert(
                "screenshot_pre_age_ms".into(),
                Value::from(u64::try_from(age).unwrap_or(u64::MAX)),
            );
        }
        let result =
            emit_action_artifacts(self.target.context, self.target.ref_id, pre, post, extra);
        if let Err(error) = result {
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
            let post = ArtifactOutcome::Skipped(NOT_DISPATCHED.into());
            self.emit(&pre, &post, DeliverySemantics::not_delivered());
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

#[cfg(test)]
#[path = "action_artifacts_tests.rs"]
mod tests;
