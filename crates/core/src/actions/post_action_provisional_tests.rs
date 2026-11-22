use super::*;

struct ProvisionalAdapter {
    inner: Adapter,
    reads: AtomicUsize,
    fail: bool,
}

impl ObservationOps for ProvisionalAdapter {
    fn get_live_element_provisional(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<LiveElement>, AdapterError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(AdapterError::not_supported("provisional read"));
        }
        self.inner.get_live_element(handle, deadline).map(Some)
    }

    fn get_live_element(&self, _: &NativeHandle, _: Deadline) -> Result<LiveElement, AdapterError> {
        panic!("full read must not run when provisional evidence is offered")
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
        fail: false,
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
        fail: true,
    };
    let error = execute_verified_action(
        &adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::Clear),
        &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 1);
}
