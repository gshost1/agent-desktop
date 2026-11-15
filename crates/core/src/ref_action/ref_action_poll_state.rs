use serde_json::{Value, json};

use crate::{
    ActionResult, AdapterError, actionability::StabilityExpectation,
    refs::resolve_verdict::ResolveVerdict,
};

#[derive(Default)]
pub(crate) struct RefActionPollState {
    pub(crate) last_report: Option<Value>,
    pub(crate) last_unsettled_answer: Option<AdapterError>,
    pub(crate) saw_ambiguity: bool,
    pub(crate) expected_bounds_hash: Option<u64>,
    pub(crate) resolve_attempts: u64,
    pub(crate) preflight_attempts: u64,
    pub(crate) live_read_incomplete_only: Option<bool>,
}

impl RefActionPollState {
    /// The poll's starting state. A headless action compares live bounds with
    /// the snapshot's hash, but only a screen-space one: a window-space hash
    /// never matches the live screen rectangle, so such a ref is primed from
    /// its first live read instead (see [`Self::prime_from_live`]).
    pub(crate) fn for_entry(entry: &crate::RefEntry, request: &crate::ActionRequest) -> Self {
        Self {
            expected_bounds_hash: screen_bounds_hash(entry).filter(|_| is_headless(request)),
            ..Default::default()
        }
    }

    /// Takes the live screen bounds as the expected hash before the first
    /// preflight of a headless window-space ref, so the stability check
    /// compares two live screen reads instead of failing on a hash measured
    /// in another frame.
    pub(crate) fn prime_from_live(
        &mut self,
        target: (&crate::RefEntry, &crate::ActionRequest),
        adapter: &dyn crate::PlatformAdapter,
        handle: &crate::NativeHandle,
        deadline: crate::Deadline,
    ) {
        let (entry, request) = target;
        if self.preflight_attempts > 0
            || self.expected_bounds_hash.is_some()
            || !is_headless(request)
            || !is_window_space(entry)
        {
            return;
        }
        self.expected_bounds_hash = adapter
            .get_element_bounds(handle, deadline)
            .ok()
            .flatten()
            .and_then(|bounds| bounds.bounds_hash());
    }

    pub(crate) fn stability(&self) -> StabilityExpectation {
        StabilityExpectation::strict_hash(self.expected_bounds_hash)
    }

    pub(crate) fn record_preflight_error(&mut self, error: &AdapterError) {
        self.live_read_incomplete_only =
            Some(self.live_read_incomplete_only.unwrap_or(true) && is_live_read_incomplete(error));
        if let Some(observed) =
            crate::ref_action::ref_action_wait_evidence::observed_bounds_hash(error)
        {
            self.expected_bounds_hash = Some(observed);
        }
        if let Some(report) = error.details.clone() {
            self.last_report = Some(json!({ "phase": "preflight", "report": report }));
        }
    }

    /// Records a refusal the wait is about to retry. An unsettled one is a
    /// complete answer, kept so a wait that runs out reports it instead of a
    /// bare timeout; an incomplete one leaves the kept answer in place.
    pub(crate) fn record_resolve_error(&mut self, error: &AdapterError, verdict: ResolveVerdict) {
        self.live_read_incomplete_only = Some(false);
        if verdict == ResolveVerdict::Unsettled {
            self.last_unsettled_answer = Some(error.clone());
        }
        self.last_report = Some(json!({
            "phase": "resolve",
            "code": error.code.as_str(),
            "message": error.message,
            "details": error.details,
        }));
    }

    pub(crate) fn note_resolved(&mut self) {
        self.last_unsettled_answer = None;
    }

    pub(crate) fn unsettled_answer(&self) -> Option<&AdapterError> {
        self.last_unsettled_answer.as_ref()
    }

    pub(crate) fn only_live_read_incomplete(&self) -> bool {
        self.live_read_incomplete_only == Some(true)
    }

    pub(crate) fn attach_transient_ambiguity(&self, result: &mut ActionResult) {
        if !self.saw_ambiguity {
            return;
        }
        let mut details = result.details.take().unwrap_or_else(|| json!({}));
        if let Some(object) = details.as_object_mut() {
            object.insert("transient_ambiguity".into(), json!(true));
            result.details = Some(details);
        } else {
            result.details = Some(json!({
                "action_details": details,
                "transient_ambiguity": true,
            }));
        }
    }

    pub(crate) fn attach_wait_metrics(
        &self,
        result: &mut ActionResult,
        lease: &crate::InteractionLease,
        lease_hold_ms: u64,
    ) {
        let mut details = result.details.take().unwrap_or_else(|| json!({}));
        let metrics = json!({
            "read_only_resolve_attempts": self.resolve_attempts,
            "read_only_preflight_attempts": self.preflight_attempts,
            "lease_contention_count": lease.contention_count(),
            "lease_hold_ms": lease_hold_ms,
        });
        if let Some(object) = details.as_object_mut() {
            object.insert("auto_wait".into(), metrics);
            result.details = Some(details);
        } else {
            result.details = Some(json!({
                "action_details": details,
                "auto_wait": metrics,
            }));
        }
    }

    pub(crate) fn attach_error_metrics(&self, mut error: AdapterError) -> AdapterError {
        let mut details = error.details.take().unwrap_or_else(|| json!({}));
        let metrics = json!({
            "read_only_resolve_attempts": self.resolve_attempts,
            "read_only_preflight_attempts": self.preflight_attempts,
        });
        if let Some(object) = details.as_object_mut() {
            object.insert("auto_wait".into(), metrics);
            error.details = Some(details);
        } else {
            error.details = Some(json!({
                "error_details": details,
                "auto_wait": metrics,
            }));
        }
        error
    }
}

fn is_live_read_incomplete(error: &AdapterError) -> bool {
    error.details.as_ref().is_some_and(|details| {
        details.get("kind").and_then(Value::as_str) == Some("live_element_evidence")
            && details.get("complete").and_then(Value::as_bool) == Some(false)
    })
}

/// The snapshot's bounds hash when it was measured on screen. A refmap entry
/// without a space tag is a screen rectangle.
pub(crate) fn screen_bounds_hash(entry: &crate::RefEntry) -> Option<u64> {
    (!is_window_space(entry))
        .then_some(entry.geometry.bounds_hash)
        .flatten()
}

fn is_window_space(entry: &crate::RefEntry) -> bool {
    entry.geometry.bounds_space == Some(crate::BoundsSpace::Window)
}

fn is_headless(request: &crate::ActionRequest) -> bool {
    request.policy == crate::InteractionPolicy::headless()
}
