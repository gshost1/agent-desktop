use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::wait::deadline::VirtualClock;
use crate::{AdapterError, DeliverySemantics, ErrorCode, WindowInfo};
use std::sync::Mutex;
use std::time::Duration;

const DRAINED_LEASE_FLOOR: Duration = Duration::from_millis(30);
const DRAIN_STEP: Duration = Duration::from_millis(10);

struct FocusAdapter {
    windows: Vec<WindowInfo>,
    focused_windows: Vec<WindowInfo>,
    focused_window_calls: Mutex<u32>,
    read_error: Option<ErrorCode>,
    focus_error: Option<AdapterError>,
    drains_lease: bool,
    read_cost: Duration,
}

impl FocusAdapter {
    fn new(target: &WindowInfo, focused: Vec<WindowInfo>) -> Self {
        Self {
            windows: vec![target.clone()],
            focused_windows: focused,
            focused_window_calls: Mutex::new(0),
            read_error: None,
            focus_error: None,
            drains_lease: false,
            read_cost: Duration::ZERO,
        }
    }

    fn reads(&self) -> u32 {
        *self.focused_window_calls.lock().unwrap()
    }
}

impl ObservationOps for FocusAdapter {
    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        if filter.focused_only {
            Ok(self.focused_windows.clone())
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
        if let Some(error) = &self.focus_error {
            return Err(error.clone());
        }
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
        match &self.read_error {
            Some(ErrorCode::PlatformNotSupported) => {
                Err(AdapterError::not_supported("focused_window"))
            }
            Some(code) => Err(AdapterError::new(
                code.clone(),
                "focused window read failed",
            )),
            None => Ok(self.focused_windows.first().cloned()),
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

fn dialog() -> WindowInfo {
    let mut dialog = window("d1", true);
    dialog.title = "Save changes?".into();
    dialog
}

fn by_id(id: &str) -> FocusWindowArgs {
    FocusWindowArgs {
        window_id: Some(id.into()),
        app: None,
        title: None,
    }
}

#[test]
fn reports_focused_window_after_os_confirms_focus() {
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, vec![window("w1", true)]);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(value["focused"]["is_focused"], true);
    assert_eq!(adapter.reads(), 1);
}

#[test]
fn a_late_adapter_confirmation_is_a_focused_window() {
    let _clock = VirtualClock::freeze();
    let target = window("w1", false);
    let mut adapter = FocusAdapter::new(&target, vec![window("w1", true)]);
    adapter.drains_lease = true;
    adapter.read_cost = Duration::from_millis(20);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(value["focused"]["is_focused"], true);
}

#[test]
fn a_modal_dialog_seen_by_the_follow_up_read_is_reported_as_focused() {
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, vec![dialog()]);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "d1");
    assert_eq!(value["focused"]["title"], "Save changes?");
}

#[test]
fn a_failed_follow_up_read_reports_the_target_the_adapter_confirmed() {
    let target = window("w1", false);
    let mut adapter = FocusAdapter::new(&target, vec![dialog()]);
    adapter.read_error = Some(ErrorCode::Timeout);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(value["focused"]["is_focused"], true);
    assert_eq!(adapter.reads(), 1);
}

#[test]
fn a_follow_up_read_that_sees_no_window_reports_the_confirmed_target() {
    let target = window("w1", false);
    let adapter = FocusAdapter::new(&target, Vec::new());

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(value["focused"]["is_focused"], true);
}

#[test]
fn an_adapter_focus_error_passes_through_unchanged() {
    let target = window("w1", false);
    let mut adapter = FocusAdapter::new(&target, vec![window("w1", true)]);
    adapter.focus_error = Some(
        AdapterError::new(ErrorCode::ActionFailed, "activation refused")
            .with_disposition(DeliverySemantics::uncertain()),
    );

    let err = execute(by_id(&target.id), &adapter).unwrap_err();

    let AppError::Adapter(error) = err else {
        panic!("expected an adapter error, got {err:?}");
    };
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.message, "activation refused");
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    assert_eq!(adapter.reads(), 0);
}

#[test]
fn falls_back_to_focused_window_list_when_direct_observation_is_unsupported() {
    let target = window("w1", false);
    let mut adapter = FocusAdapter::new(&target, vec![window("w1", true)]);
    adapter.read_error = Some(ErrorCode::PlatformNotSupported);

    let value = execute(by_id(&target.id), &adapter).unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(adapter.reads(), 1);
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
