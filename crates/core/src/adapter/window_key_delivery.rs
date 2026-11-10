use crate::{
    ActionResult, AdapterError, InteractionLease, InteractionPolicy, KeyCombo, ProcessIdentity,
    SystemOps, WindowInfo,
};

/// The default `press_key_for_window`: confirms the exact window holds focus,
/// then delivers the key through the process-scoped path.
pub(crate) fn press_key_for_focused_window<A: SystemOps + ?Sized>(
    adapter: &A,
    window: &WindowInfo,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    lease: &InteractionLease,
) -> Result<ActionResult, AdapterError> {
    let refusal = || {
        AdapterError::stale_ref_because(
            "The named window's exact focus could not be confirmed, so no key was sent",
        )
        .with_suggestion(if policy.is_headed() {
            "Ensure the named window remains active before retrying; a sibling window or another application's menu can hold the keyboard"
        } else {
            "Retry with --headed so the named window is brought forward first, or name the active window"
        })
        .with_details(serde_json::json!({
            "check": "window_active",
            "cause": "exact_window_focus_unconfirmed",
            "physical_delivery_started": false,
            "window_id": window.id,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
    };
    let instance = window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(refusal)?;
    let focused = adapter
        .focused_window(lease.deadline())
        .map_err(AdapterError::unsent)?;
    if !focused.is_some_and(|focused| {
        focused.id == window.id
            && focused.pid == window.pid
            && focused.process_instance.as_deref() == Some(instance)
    }) {
        return Err(refusal());
    }
    adapter.press_key_for_app(
        ProcessIdentity::new(window.pid, instance),
        combo,
        policy,
        lease,
    )
}
