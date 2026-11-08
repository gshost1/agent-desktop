use super::*;

#[test]
fn a_refusal_after_a_focus_call_records_the_post_screen_and_no_dispatch() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::Focuses, 1);
    let request = ActionRequest::headed(Action::Click).with_timeout_ms(Some(3_000));

    let error = click(&adapter, &context, request).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
    let event = only_event(&trace_dir);
    assert!(event["screenshot_post"].is_string(), "{event}");
    assert_eq!(event["delivery"], "not_delivered");
    assert!(event.get("skipped_post").is_none(), "{event}");
}

#[test]
fn a_refusal_after_a_preparatory_scroll_records_the_post_screen_and_no_dispatch() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::StaysOffscreen, 0);

    let error = click(&adapter, &context, ActionRequest::headless(Action::Click)).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.details.unwrap()["preparatory_scroll"].is_string());
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
    let event = only_event(&trace_dir);
    assert!(event["screenshot_post"].is_string(), "{event}");
    assert_eq!(event["delivery"], "not_delivered");
}

#[test]
fn an_unknown_dispatch_failure_records_the_post_screen_and_its_disposition() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::DispatchFails(DeliverySemantics::unknown()), 0);

    let error = click(&adapter, &context, headless(3_000)).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::unknown());
    let event = only_event(&trace_dir);
    assert!(event["screenshot_post"].is_string(), "{event}");
    assert_eq!(event["delivery"], "unknown");
}

#[test]
fn a_resolved_dispatch_that_delivered_nothing_takes_no_post_screenshot() {
    let (_home, context, trace_dir) = session();
    let adapter = PairingAdapter::new(Mode::DispatchFails(DeliverySemantics::not_delivered()), 0);
    let target_entry = entry();
    let deadline = crate::Deadline::standard().unwrap();
    let lease = crate::adapter::SystemOps::acquire_interaction_lease(&adapter, deadline).unwrap();
    let handle = NativeHandle::null();
    let target = crate::ref_action::ref_action_context::RefActionContext::new(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter: &adapter,
            entry: &target_entry,
            ref_id: "@e1",
            context: &context,
        },
        deadline,
    );

    let result = crate::ref_action::execute_resolved(
        crate::ref_action::ResolvedRefAction::new(target, &handle),
        ActionRequest::headless(Action::Click),
        &lease,
    );

    assert!(result.is_err());
    assert_eq!(adapter.screenshots.load(Ordering::SeqCst), 1);
    assert_eq!(only_event(&trace_dir)["skipped_post"], "not_dispatched");
}
