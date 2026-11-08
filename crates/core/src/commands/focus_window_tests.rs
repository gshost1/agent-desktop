use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::wait::deadline::{DEFAULT_OPERATION_TIMEOUT_MS, VirtualClock};
use crate::{AdapterError, DeliverySemantics, WindowInfo};
use std::sync::Mutex;
use std::time::Duration;

const DRAINED_LEASE_FLOOR: Duration = Duration::from_millis(30);
const DRAIN_STEP: Duration = Duration::from_millis(10);

struct FocusAdapter {
    windows: Vec<WindowInfo>,
    focused_windows: Mutex<Vec<WindowInfo>>,
    focused_window_calls: Mutex<u32>,
    focused_window_supported: bool,
    drains_lease: bool,
    read_cost: Duration,
    adapter_budget: Mutex<Option<Duration>>,
}

impl FocusAdapter {
    fn new(target: &WindowInfo, focused: Vec<WindowInfo>) -> Self {
        Self {
            windows: vec![target.clone()],
            focused_windows: Mutex::new(focused),
            focused_window_calls: Mutex::new(0),
            focused_window_supported: true,
            drains_lease: false,
            read_cost: Duration::ZERO,
            adapter_budget: Mutex::new(None),
        }
    }
}

impl ObservationOps for FocusAdapter {
    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        if filter.focused_only {
            Ok(self.focused_windows.lock().unwrap().clone())
        } else {
            Ok(self.windows.clone())
        }
    }
}

impl ActionOps for FocusAdapter {}

impl InputOps for FocusAdapter {}

impl SystemOps for FocusAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn focus_window(
        &self,
        _win: &WindowInfo,
        lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        *self.adapter_budget.lock().unwrap() = Some(lease.deadline().remaining());
        while self.drains_lease && lease.deadline().remaining() >= DRAINED_LEASE_FLOOR {
            crate::wait::deadline::sleep(DRAIN_STEP);
        }
        Ok(())
    }

    fn focused_window(
        &self,
        deadline: crate::Deadline,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        crate::wait::deadline::sleep(self.read_cost);
        deadline.ensure_remaining()?;
        *self.focused_window_calls.lock().unwrap() += 1;
        if !self.focused_window_supported {
            return Err(AdapterError::not_supported("focused_window"));
        }
        let mut focused = self.focused_windows.lock().unwrap();
        if focused.len() > 1 {
            Ok(Some(focused.remove(0)))
        } else {
            Ok(focused.first().cloned())
        }
    }
}

fn window(id: &str, focused: bool) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Main".into(),
        app: "TextEdit".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: focused,
            ..Default::default()
        },
    }
}

fn by_id(id: &str) -> FocusWindowArgs {
    FocusWindowArgs {
        window_id: Some(id.into()),
        app: None,
        title: None,
    }
}

fn disposition(error: AppError) -> DeliverySemantics {
    match error {
        AppError::Adapter(error) => error.disposition,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

#[test]
fn reports_focused_window_after_os_confirms_focus() {
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, vec![window("w1", true)]);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(value["focused"]["is_focused"], true);
    assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 1);
}

#[test]
fn a_late_adapter_confirmation_still_leaves_room_for_the_corroborating_read() {
    let _clock = VirtualClock::freeze();
    let target = window("w1", false);
    let mut adapter = FocusAdapter::new(&target, vec![window("w1", true)]);
    adapter.drains_lease = true;
    adapter.read_cost = Duration::from_millis(20);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
}

#[test]
fn the_adapter_deadline_leaves_the_corroboration_slice_in_reserve() {
    let _clock = VirtualClock::freeze();
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, vec![window("w1", true)]);

    execute(by_id(&target.id), &adapter).unwrap();

    let reserve = Duration::from_millis(crate::system::window_focus::CORROBORATION_RESERVE_MS);
    assert_eq!(
        *adapter.adapter_budget.lock().unwrap(),
        Some(Duration::from_millis(DEFAULT_OPERATION_TIMEOUT_MS) - reserve)
    );
}

#[test]
fn a_corroborating_read_that_disagrees_reports_focus_delivered_unverified() {
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, vec![window("w2", true)]);

    let err = execute(by_id(&target.id), &adapter).unwrap_err();

    assert_eq!(err.code(), "ACTION_FAILED");
    assert_eq!(disposition(err), DeliverySemantics::delivered_unverified());
}

#[test]
fn errors_when_no_window_is_observed_focused_after_confirmation() {
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, Vec::new());

    let err = execute(by_id(&target.id), &adapter).unwrap_err();

    assert_eq!(err.code(), "ACTION_FAILED");
    assert_eq!(disposition(err), DeliverySemantics::delivered_unverified());
}

#[test]
fn falls_back_to_focused_window_list_when_direct_observation_is_unsupported() {
    let target = window("w1", false);
    let mut adapter = FocusAdapter::new(&target, vec![window("w1", true)]);
    adapter.focused_window_supported = false;

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 1);
}

#[test]
fn app_filter_results_are_not_discarded_by_display_name() {
    let mut target = window("w1", false);
    target.app = "WhatsApp".into();
    let mut focused = target.clone();
    focused.state.is_focused = true;
    let adapter = FocusAdapter::new(&target, vec![focused]);

    let value = execute(
        FocusWindowArgs {
            window_id: None,
            app: Some("net.whatsapp.WhatsApp".into()),
            title: None,
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(value["focused"]["id"], "w1");
}
