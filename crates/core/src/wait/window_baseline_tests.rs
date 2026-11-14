use super::WindowBaseline;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, Deadline, ProcessIdentity, WaitSelector, WindowFilter, WindowInfo};
use std::sync::Mutex;

struct Windows {
    listed: Mutex<Vec<WindowInfo>>,
}

impl ObservationOps for Windows {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.listed.lock().unwrap().clone())
    }
}

impl ActionOps for Windows {}

impl InputOps for Windows {}

impl SystemOps for Windows {}

fn window(id: &str, pid: u32, instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: id.into(),
        app: "TestApp".into(),
        pid: crate::ProcessId::new(pid),
        process_instance: instance.map(String::from),
        bounds: None,
        state: Default::default(),
    }
}

fn process() -> ProcessIdentity {
    ProcessIdentity::new(1, "test-instance")
}

fn appear(gone: bool) -> WaitSelector {
    WaitSelector {
        query_raw: ":Width".into(),
        gone,
        timeout_ms: 1_000,
    }
}

fn entry() -> crate::RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Open"));
    entry.source.source_app = Some("TestApp".into());
    entry.source.source_window_id = Some("w-main".into());
    entry.source.source_window_title = Some("Main".into());
    entry
}

#[test]
fn only_windows_opened_after_the_baseline_are_reported() {
    let windows = Windows {
        listed: Mutex::new(vec![window("w-main", 1, Some("test-instance"))]),
    };
    let baseline = WindowBaseline::capture(
        &windows,
        "TestApp",
        process(),
        Deadline::after(500).unwrap(),
    )
    .unwrap();
    windows.listed.lock().unwrap().extend([
        window("w-dialog", 1, Some("test-instance")),
        window("w-reused-pid", 1, Some("old-instance")),
        window("w-unproven", 1, None),
        window("w-other-process", 2, Some("test-instance")),
    ]);

    let opened = baseline
        .opened_since(&windows, Deadline::after(500).unwrap())
        .unwrap();

    let ids: Vec<_> = opened.iter().map(|window| window.id.as_str()).collect();
    assert_eq!(ids, ["w-dialog"]);
}

#[test]
fn a_baseline_is_taken_only_for_an_appear_wait_on_a_window_ref() {
    let windows = Windows {
        listed: Mutex::new(vec![window("w-main", 1, Some("test-instance"))]),
    };
    let entry = entry();
    assert!(WindowBaseline::before_action(&windows, &entry, Some(&appear(false))).is_some());
    assert!(WindowBaseline::before_action(&windows, &entry, Some(&appear(true))).is_none());
    assert!(WindowBaseline::before_action(&windows, &entry, None).is_none());

    let mut process_owned = entry.clone();
    process_owned.source.source_window_id = None;
    process_owned.source.source_window_title = Some(String::new());
    assert!(
        WindowBaseline::before_action(&windows, &process_owned, Some(&appear(false))).is_none()
    );
}

#[test]
fn a_platform_that_cannot_list_windows_leaves_no_baseline() {
    struct NoWindows;
    impl ObservationOps for NoWindows {}
    impl ActionOps for NoWindows {}
    impl InputOps for NoWindows {}
    impl SystemOps for NoWindows {}

    assert!(WindowBaseline::before_action(&NoWindows, &entry(), Some(&appear(false))).is_none());
}
