use crate::{
    ActionRequest, ActionResult, AdapterError, ref_action::into_adapter_error,
    ref_action::ref_action_context::RefActionContext,
    ref_action::ref_action_wait_context::RefActionWaitContext,
};

/// The plain dispatch: the handle the action landed on is dropped.
pub(crate) fn dispatch_resolved(
    target: RefActionContext<'_>,
    request: ActionRequest,
    lease: &crate::InteractionLease,
) -> Result<ActionResult, crate::AppError> {
    crate::ref_action::dispatch_resolved_landing(target, request, lease).map(|(result, _)| result)
}

pub(crate) fn execute_single_shot(
    context: RefActionWaitContext<'_>,
    request: ActionRequest,
    deadline: crate::Deadline,
    lease: &crate::InteractionLease,
    dispatch: impl Fn(
        RefActionContext<'_>,
        ActionRequest,
        &crate::InteractionLease,
    ) -> Result<ActionResult, crate::AppError>,
) -> Result<ActionResult, AdapterError> {
    focus_for_headed_input(context, &request, deadline, lease).map_err(AdapterError::unsent)?;
    dispatch(RefActionContext::new(context, deadline), request, lease).map_err(into_adapter_error)
}

/// Raises the target application or window when headed input needs it.
fn focus_for_headed_input(
    context: RefActionWaitContext<'_>,
    request: &ActionRequest,
    deadline: crate::Deadline,
    lease: &crate::InteractionLease,
) -> Result<(), AdapterError> {
    let keyboard_is_aimed_at_the_element = request.action.delivered_by_keyboard()
        && !context.adapter.keyboard_needs_window_activation();
    if !request.headed_requirement().requires_focus() || keyboard_is_aimed_at_the_element {
        return Ok(());
    }
    let requirement =
        context
            .adapter
            .headed_requirement_for_ref(context.entry, request, deadline)?;
    if matches!(
        requirement,
        crate::HeadedRequirement::FocusedApplication
            | crate::HeadedRequirement::FocusedApplicationAndCursor
    ) {
        return crate::actions::headed_focus::focus_entry_app(
            context.entry,
            context.adapter,
            lease,
        );
    }
    if !requirement.requires_focus() {
        return Ok(());
    }
    crate::actions::headed_focus::focus_entry_window(
        context.entry,
        context.adapter,
        context.context,
        lease,
    )
    .map(|_| ())
    .map_err(into_adapter_error)
}
