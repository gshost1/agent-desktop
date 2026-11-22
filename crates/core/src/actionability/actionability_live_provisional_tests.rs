use super::*;
use crate::{Deadline, DeliverySemantics, actionability::live::LiveCheckTarget};
use std::sync::Mutex;

struct ProvisionalAdapter {
    offered: Result<Option<LiveElement>, AdapterError>,
    full: Result<LiveElement, AdapterError>,
    reads: Mutex<Vec<&'static str>>,
}

impl ObservationOps for ProvisionalAdapter {
    fn get_live_element_provisional(
        &self,
        _: &NativeHandle,
        _: Deadline,
    ) -> Result<Option<LiveElement>, AdapterError> {
        self.reads.lock().unwrap().push("provisional");
        self.offered.clone()
    }

    fn get_live_element(&self, _: &NativeHandle, _: Deadline) -> Result<LiveElement, AdapterError> {
        self.reads.lock().unwrap().push("full");
        self.full.clone()
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: crate::Point,
        deadline: Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        CombinedLiveAdapter.hit_test(handle, point, deadline)
    }
}

impl ActionOps for ProvisionalAdapter {}
impl InputOps for ProvisionalAdapter {}
impl SystemOps for ProvisionalAdapter {}

fn adapter() -> ProvisionalAdapter {
    let live = CombinedLiveAdapter
        .get_live_element(&NativeHandle::null(), Deadline::standard().unwrap())
        .unwrap();
    ProvisionalAdapter {
        offered: Ok(Some(live.clone())),
        full: Ok(live),
        reads: Mutex::new(vec![]),
    }
}

type Check = fn(
    &LiveCheckTarget<'_>,
    &ActionRequest,
    StabilityExpectation,
) -> Result<ActionabilityReport, AdapterError>;

fn check(
    adapter: &dyn crate::PlatformAdapter,
    check: Check,
) -> Result<ActionabilityReport, AdapterError> {
    let entry = entry();
    check(
        &LiveCheckTarget {
            entry: &entry,
            handle: &NativeHandle::null(),
            adapter,
            deadline: Deadline::standard().unwrap(),
        },
        &ActionRequest::headed(Action::Click),
        StabilityExpectation::permissive(entry.geometry.bounds_hash),
    )
}

#[test]
fn before_focus_uses_provisional_when_offered_and_full_when_not() {
    for offered in [true, false] {
        let mut adapter = adapter();
        if !offered {
            adapter.offered = Ok(None);
        }
        assert!(check(&adapter, check_live_before_focus).unwrap().actionable);
        assert_eq!(
            *adapter.reads.lock().unwrap(),
            if offered {
                vec!["provisional"]
            } else {
                vec!["provisional", "full"]
            }
        );
    }
}

#[test]
fn or_gap_actionable_provisional_read_is_followed_by_full_read() {
    let adapter = adapter();
    assert!(
        check(&adapter, check_live_with_stability_or_gap)
            .unwrap()
            .actionable
    );
    assert_eq!(*adapter.reads.lock().unwrap(), vec!["provisional", "full"]);
}

#[test]
fn or_gap_catches_modal_shown_after_provisional_read_before_dispatch() {
    let mut adapter = adapter();
    let blocked = AdapterError::new(ErrorCode::ActionFailed, "Blocked by modal")
        .with_details(serde_json::json!({"cause": "blocked_by_modal_dialog"}))
        .with_disposition(DeliverySemantics::not_delivered());
    adapter.full = Err(blocked.clone());
    let error = check(&adapter, check_live_with_stability_or_gap).unwrap_err();
    assert_eq!(error.code, blocked.code);
    assert_eq!(error.details, blocked.details);
    assert_eq!(error.disposition, blocked.disposition);
    assert_eq!(*adapter.reads.lock().unwrap(), vec!["provisional", "full"]);
}

#[test]
fn or_gap_returns_report_evaluated_from_full_evidence() {
    let mut adapter = adapter();
    adapter.full.as_mut().unwrap().state.enabled = Some(false);
    assert!(check(&adapter, check_live_with_stability_or_gap).is_err());
    assert_eq!(*adapter.reads.lock().unwrap(), vec!["provisional", "full"]);
}

#[test]
fn or_gap_nonactionable_provisional_read_does_not_read_full() {
    let mut adapter = adapter();
    adapter
        .offered
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .bounds
        .as_mut()
        .unwrap()
        .x += 100.0;
    let entry = entry();
    let report = check_live_with_stability_or_gap(
        &LiveCheckTarget {
            entry: &entry,
            handle: &NativeHandle::null(),
            adapter: &adapter,
            deadline: Deadline::standard().unwrap(),
        },
        &ActionRequest::headed(Action::DoubleClick),
        StabilityExpectation::strict_hash(entry.geometry.bounds_hash),
    )
    .unwrap();
    assert!(!report.actionable);
    assert_eq!(*adapter.reads.lock().unwrap(), vec!["provisional"]);
}

#[test]
fn stability_check_never_calls_provisional_read() {
    let mut adapter = adapter();
    adapter.offered = Err(AdapterError::not_supported("must not read"));
    assert!(
        check(&adapter, check_live_with_stability)
            .unwrap()
            .actionable
    );
    assert_eq!(*adapter.reads.lock().unwrap(), vec!["full"]);
}

struct DefaultAdapter(ProvisionalAdapter);

impl ObservationOps for DefaultAdapter {
    fn get_live_element(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        self.0.get_live_element(handle, deadline)
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: crate::Point,
        deadline: Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        self.0.hit_test(handle, point, deadline)
    }
}

impl ActionOps for DefaultAdapter {}
impl InputOps for DefaultAdapter {}
impl SystemOps for DefaultAdapter {}

#[test]
fn default_adapter_gets_exactly_one_full_read_per_check() {
    for check_fn in [
        check_live_before_focus as Check,
        check_live_with_stability_or_gap,
        check_live_with_stability,
    ] {
        let adapter = DefaultAdapter(adapter());
        assert!(check(&adapter, check_fn).unwrap().actionable);
        assert_eq!(*adapter.0.reads.lock().unwrap(), vec!["full"]);
    }
}

#[test]
fn provisional_and_fallback_unsupported_errors_keep_observe_mapping() {
    for provisional in [true, false] {
        let mut adapter = adapter();
        let error = AdapterError::not_supported("live read");
        if provisional {
            adapter.offered = Err(error);
        } else {
            adapter.offered = Ok(None);
            adapter.full = Err(error);
        }
        let error = check(&adapter, check_live_before_focus).unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert!(error.details.unwrap().get("source_code").is_some());
    }
}
