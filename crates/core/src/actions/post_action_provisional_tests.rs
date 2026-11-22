use super::*;

struct ProvisionalAdapter {
    inner: Adapter,
    reads: AtomicUsize,
    error: Option<ErrorCode>,
}

impl ObservationOps for ProvisionalAdapter {
    fn get_live_element_provisional(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<LiveElement>, AdapterError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(code) = &self.error {
            return Err(AdapterError::new(code.clone(), "provisional read failed"));
        }
        self.inner.get_live_element(handle, deadline).map(Some)
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        if self.error.is_some() {
            return self.inner.get_live_element(handle, deadline);
        }
        Err(
            AdapterError::new(ErrorCode::ActionFailed, "Blocked by in-window modal")
                .with_details(serde_json::json!({"cause": "blocked_by_modal_dialog"}))
                .with_disposition(DeliverySemantics::not_delivered()),
        )
    }
}

impl ActionOps for ProvisionalAdapter {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.inner.execute_action(handle, request, lease)
    }
}

impl InputOps for ProvisionalAdapter {}
impl SystemOps for ProvisionalAdapter {}

#[test]
fn post_action_before_after_and_settling_reads_use_provisional_when_offered() {
    let inner = adapter(element(None, &["checked"]), element(None, &[]));
    inner
        .settling
        .lock()
        .unwrap()
        .push_back(element(None, &["checked"]));
    let adapter = ProvisionalAdapter {
        inner,
        reads: AtomicUsize::new(0),
        error: None,
    };
    let result = execute_verified_action(
        &adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::Toggle),
        &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 3);
    assert_eq!(adapter.inner.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn post_action_provisional_error_preserves_failure_without_full_fallback() {
    let adapter = ProvisionalAdapter {
        inner: adapter(element(Some("old"), &[]), element(Some(""), &[])),
        reads: AtomicUsize::new(0),
        error: Some(ErrorCode::ActionFailed),
    };
    let error = execute_verified_action(
        &adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::Clear),
        &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 1);
}

#[test]
fn post_action_unsupported_provisional_reads_fall_back_to_full() {
    for code in [
        ErrorCode::PlatformNotSupported,
        ErrorCode::ActionNotSupported,
    ] {
        let adapter = ProvisionalAdapter {
            inner: adapter(element(Some("old"), &[]), element(Some(""), &[])),
            reads: AtomicUsize::new(0),
            error: Some(code),
        };
        let result = execute_verified_action(
            &adapter,
            &NativeHandle::null(),
            ActionRequest::headless(Action::Clear),
            &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            result.disposition(),
            DeliverySemantics::delivered_verified()
        );
        assert_eq!(adapter.reads.load(Ordering::SeqCst), 1);
        assert_eq!(adapter.inner.reads.lock().unwrap().len(), 1);
        assert_eq!(adapter.inner.calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn post_action_confirms_changed_target_when_full_read_would_be_blocked_by_in_window_modal() {
    let adapter = ProvisionalAdapter {
        inner: adapter(element(None, &[]), element(None, &["expanded"])),
        reads: AtomicUsize::new(0),
        error: None,
    };
    let result = execute_verified_action(
        &adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::Expand),
        &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        result.details.unwrap()["verification_scope"],
        "element_state"
    );
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.inner.calls.load(Ordering::SeqCst), 1);
    let error = adapter
        .get_live_element(&NativeHandle::null(), Deadline::standard().unwrap())
        .unwrap_err();
    assert_eq!(error.details.unwrap()["cause"], "blocked_by_modal_dialog");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
