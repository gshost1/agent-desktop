use crate::{
    ActionRequest, ActionResult, AdapterError, AppError, Deadline,
    ref_action::ref_action_context::RefActionContext,
    ref_action::ref_action_poll::execute_poll_loop,
    ref_action::ref_action_poll_state::RefActionPollState,
    ref_action::ref_action_single::execute_single_shot,
    ref_action::ref_action_wait_context::RefActionWaitContext,
    ref_action::ref_action_wait_support::enrich_with_process_state,
};

#[cfg(test)]
use crate::{context::CommandContext, refs::RefEntry};
#[cfg(test)]
type RefActionWaitCtx<'a> = RefActionWaitContext<'a>;

pub(crate) fn execute_with_auto_wait(
    context: RefActionWaitContext<'_>,
    request: ActionRequest,
    dispatch: impl Fn(
        RefActionContext<'_>,
        ActionRequest,
        &crate::InteractionLease,
    ) -> Result<ActionResult, AppError>,
) -> Result<ActionResult, AdapterError> {
    let (result, lease, pre, deadline, _lease_started) =
        execute_with_auto_wait_and_lease(context, request, dispatch)?;
    drop(lease);
    crate::ref_action::finish_artifacts(RefActionContext::new(context, deadline), &pre);
    Ok(result)
}

pub(crate) fn execute_with_auto_wait_and_lease(
    context: RefActionWaitContext<'_>,
    request: ActionRequest,
    dispatch: impl Fn(
        RefActionContext<'_>,
        ActionRequest,
        &crate::InteractionLease,
    ) -> Result<ActionResult, AppError>,
) -> Result<
    (
        ActionResult,
        crate::InteractionLease,
        crate::trace::trace_artifacts::ArtifactOutcome,
        Deadline,
        std::time::Instant,
    ),
    AdapterError,
> {
    let deadline = operation_deadline(&request)?;
    let result = execute_with_deadline(context, request, deadline, dispatch);
    result.map_err(|error| {
        if deadline.is_expired() {
            error
        } else {
            enrich_with_process_state(context.adapter, context.entry, error, deadline)
        }
    })
}

fn execute_with_deadline(
    context: RefActionWaitContext<'_>,
    mut request: ActionRequest,
    deadline: Deadline,
    dispatch: impl Fn(
        RefActionContext<'_>,
        ActionRequest,
        &crate::InteractionLease,
    ) -> Result<ActionResult, AppError>,
) -> Result<
    (
        ActionResult,
        crate::InteractionLease,
        crate::trace::trace_artifacts::ArtifactOutcome,
        Deadline,
        std::time::Instant,
    ),
    AdapterError,
> {
    let polls = request.timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    let mut saw_ambiguity = false;
    let mut refusal: Option<AdapterError> = None;
    loop {
        let mut state = if polls {
            execute_poll_loop(context, &request, deadline)
                .map_err(|error| with_refusal(error, refusal.as_ref()))?
        } else {
            RefActionPollState::default()
        };
        state.saw_ambiguity |= saw_ambiguity;
        let pre = crate::ref_action::capture_pre_artifact(
            context.context,
            context.adapter,
            context.entry,
            deadline,
        );
        let cursor = crate::cursor_overlay::hold_cursor(context.context, deadline);
        crate::ref_action::presentation::before_lease(context, &request, deadline, cursor.is_ok());
        let lease = acquire_lease(context.adapter, deadline)
            .map_err(AdapterError::unsent)
            .map_err(|error| with_refusal(state.attach_error_metrics(error), refusal.as_ref()))?;
        if polls {
            request.timeout_ms = Some(deadline.remaining_ms());
        }
        let lease_started = std::time::Instant::now();
        let shot = execute_single_shot(context, request.clone(), deadline, &lease, &dispatch);
        match shot.result {
            Ok(mut result) => {
                state.attach_transient_ambiguity(&mut result);
                state.attach_wait_metrics(
                    &mut result,
                    &lease,
                    u64::try_from(lease_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
                return Ok((result, lease, pre, deadline, lease_started));
            }
            Err(error) => {
                drop(lease);
                crate::ref_action::finish_artifacts(RefActionContext::new(context, deadline), &pre);
                if !(polls && resolves_again(&error, deadline)) {
                    return Err(error);
                }
                if shot.focused {
                    return Err(stopped_after_focus(error, &state, refusal.as_ref()));
                }
                saw_ambiguity |= error.code == crate::ErrorCode::AmbiguousTarget;
                refusal = Some(error);
                crate::wait::deadline::sleep(
                    crate::refs::ref_resolve_deadline::POLL_INTERVAL.min(deadline.remaining()),
                );
            }
        }
    }
}

/// Takes the interaction lease, refusing with the deadline's `TIMEOUT` when
/// the wait for it spent the budget. A failure here does not go back to a
/// wait: the lease already waits out the whole budget, so a lease timeout, or
/// a budget spent while it was taken, leaves none to wait with, and the other
/// lease failures (an unsupported lease, a broken lock file) do not clear by
/// waiting. Callers mark the refusal `not_delivered`, since nothing was sent.
pub(crate) fn acquire_lease(
    adapter: &dyn crate::PlatformAdapter,
    deadline: Deadline,
) -> Result<crate::InteractionLease, AdapterError> {
    let lease = adapter.acquire_interaction_lease(deadline)?;
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    Ok(lease)
}

/// Ends the wait after a refusal that would go back to it but came from an
/// attempt that ran a focus call, so a command raises a window at most once.
/// It keeps what the other ends of the wait report: the wait metrics, a
/// transient ambiguity, and the earlier refusal.
fn stopped_after_focus(
    error: AdapterError,
    state: &RefActionPollState,
    refusal: Option<&AdapterError>,
) -> AdapterError {
    let error = state.attach_error_metrics(error);
    let error = if state.saw_ambiguity {
        error.with_detail("transient_ambiguity", serde_json::json!(true))
    } else {
        error
    };
    with_refusal(error, refusal)
}

/// Whether a dispatch-time refusal goes back to the wait: the full resolution
/// under the lease failed in a way that may clear, nothing was sent, and the
/// caller's budget is not spent.
fn resolves_again(error: &AdapterError, deadline: Deadline) -> bool {
    error.disposition == crate::DeliverySemantics::not_delivered()
        && crate::refs::resolve_verdict::ResolveVerdict::for_ref_action(error).retries()
        && !deadline.is_expired()
}

/// Carries the dispatch-time refusal that sent the action back to the wait as
/// `details.refusal`, so a budget that runs out there still reports why the
/// ref was not acted on.
fn with_refusal(error: AdapterError, refusal: Option<&AdapterError>) -> AdapterError {
    let Some(refusal) = refusal else {
        return error;
    };
    let mut recorded = serde_json::json!({
        "code": refusal.code.as_str(),
        "message": refusal.message,
    });
    if let (Some(object), Some(details)) = (recorded.as_object_mut(), &refusal.details) {
        object.insert("details".into(), details.clone());
    }
    error.with_detail("refusal", recorded)
}

pub(crate) fn operation_deadline(request: &ActionRequest) -> Result<Deadline, AdapterError> {
    request
        .timeout_ms
        .filter(|timeout_ms| *timeout_ms > 0)
        .map_or_else(Deadline::standard, Deadline::after)
}

#[cfg(test)]
#[path = "ref_action_wait_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ref_action_wait_process_state_tests.rs"]
mod process_state_tests;

#[cfg(test)]
#[path = "ref_action_wait_app_not_found_tests.rs"]
mod app_not_found_tests;

#[cfg(test)]
#[path = "ref_action_wait_success_tests.rs"]
mod success_tests;

#[cfg(test)]
#[path = "ref_action_exactly_once_tests.rs"]
mod exactly_once_tests;

#[cfg(test)]
#[path = "ref_action_focus_order_tests.rs"]
mod focus_order_tests;

#[cfg(test)]
#[path = "ref_action_pre_dispatch_tests.rs"]
mod pre_dispatch_tests;
