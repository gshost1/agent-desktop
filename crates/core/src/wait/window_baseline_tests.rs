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

fn long() -> Deadline {
    Deadline::after(5_000).unwrap()
}

fn process() -> ProcessIdentity {
    ProcessIdentity::new(1, "test-instance")
}

fn appear(gone: bool) -> crate::CommandContext {
    crate::CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":Width".into(),
        gone,
        timeout_ms: 1_000,
    }))
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
    assert!(WindowBaseline::before_action(&windows, &entry, &appear(false), long()).is_some());
    assert!(WindowBaseline::before_action(&windows, &entry, &appear(true), long()).is_none());
    assert!(
        WindowBaseline::before_action(&windows, &entry, &crate::CommandContext::default(), long())
            .is_none()
    );

    let mut process_owned = entry.clone();
    process_owned.source.source_window_id = None;
    process_owned.source.source_window_title = Some(String::new());
    assert!(
        WindowBaseline::before_action(&windows, &process_owned, &appear(false), long()).is_none()
    );
}

#[test]
fn a_platform_that_cannot_list_windows_leaves_no_baseline() {
    struct NoWindows;
    impl ObservationOps for NoWindows {}
    impl ActionOps for NoWindows {}
    impl InputOps for NoWindows {}
    impl SystemOps for NoWindows {}

    assert!(WindowBaseline::before_action(&NoWindows, &entry(), &appear(false), long()).is_none());
}

#[test]
fn with_no_window_focused_the_command_acts_in_the_first_shown_window_not_a_minimized_one() {
    let mut minimized = window("w-minimized", 1, Some("instance"));
    minimized.state.visible = Some(true);
    minimized.state.minimized = Some(true);
    let mut shown = window("w-shown", 1, Some("instance"));
    shown.state.visible = Some(true);
    let mut behind = window("w-behind", 1, Some("instance"));
    behind.state.visible = Some(true);
    let adapter = Windows {
        listed: Mutex::new(vec![minimized, shown, behind]),
    };
    let baseline = WindowBaseline::capture(
        &adapter,
        "TestApp",
        ProcessIdentity::new(crate::ProcessId::new(1), "instance"),
        long(),
    )
    .unwrap();

    let acted_on: Vec<_> = baseline
        .acted_on(&adapter, long())
        .unwrap()
        .into_iter()
        .map(|window| window.id)
        .collect();

    assert_eq!(acted_on, ["w-shown"]);
}

/// Windows whose ids say what the adapter reports: a `panel` is auxiliary,
/// and the first `main` window it lists is the application's main window,
/// named even when it is not among the windows it is asked about.
struct PanelWindows(Vec<WindowInfo>);

impl ObservationOps for PanelWindows {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.0.clone())
    }

    fn is_auxiliary_window(&self, window: &WindowInfo, _deadline: Deadline) -> bool {
        window.id.contains("panel")
    }

    fn main_window(&self, _windows: &[WindowInfo], _deadline: Deadline) -> Option<String> {
        self.0
            .iter()
            .find(|window| window.id.starts_with("main"))
            .map(|window| window.id.clone())
    }
}

impl ActionOps for PanelWindows {}

impl InputOps for PanelWindows {}

impl SystemOps for PanelWindows {}

fn acted_on_ids(windows: Vec<WindowInfo>) -> Vec<String> {
    let adapter = PanelWindows(windows);
    let process = ProcessIdentity::new(crate::ProcessId::new(1), "instance");
    let baseline = WindowBaseline::capture(&adapter, "TestApp", process, long()).unwrap();
    baseline
        .acted_on(&adapter, long())
        .unwrap()
        .into_iter()
        .map(|window| window.id)
        .collect()
}

fn shown(id: &str, visible: Option<bool>) -> WindowInfo {
    let mut window = window(id, 1, Some("instance"));
    window.state.visible = visible;
    window
}

#[test]
fn with_no_window_focused_the_command_acts_in_the_document_behind_a_floating_panel() {
    let acted = acted_on_ids(vec![
        shown("panel-inspector", Some(true)),
        shown("w-document", Some(true)),
    ]);

    assert_eq!(acted, ["w-document"]);
}

#[test]
fn a_panel_stays_the_front_window_beside_a_hidden_document_or_other_panels() {
    let beside_hidden = acted_on_ids(vec![
        shown("panel-inspector", Some(true)),
        shown("w-document", Some(false)),
    ]);
    let only_panels = acted_on_ids(vec![
        shown("panel-inspector", Some(true)),
        shown("panel-colors", Some(true)),
    ]);

    assert_eq!(beside_hidden, ["panel-inspector"]);
    assert_eq!(only_panels, ["panel-inspector"]);
}

#[test]
fn with_no_window_focused_the_command_acts_in_the_main_window_among_documents() {
    let acted = acted_on_ids(vec![
        shown("panel-inspector", Some(true)),
        shown("w-first", Some(true)),
        shown("main-document", Some(true)),
    ]);

    assert_eq!(acted, ["main-document"]);
}

#[test]
fn without_a_main_window_the_command_acts_in_the_first_listed_document() {
    let acted = acted_on_ids(vec![
        shown("w-first", Some(true)),
        shown("w-second", Some(true)),
    ]);

    assert_eq!(acted, ["w-first"]);
}

#[test]
fn a_main_window_that_is_a_panel_or_hidden_falls_back_to_the_first_listed_document() {
    let named_panel = acted_on_ids(vec![
        shown("main-panel", Some(true)),
        shown("w-first", Some(true)),
        shown("w-second", Some(true)),
    ]);
    let named_hidden = acted_on_ids(vec![
        shown("main-hidden", Some(false)),
        shown("w-first", Some(true)),
        shown("w-second", Some(true)),
    ]);

    assert_eq!(named_panel, ["w-first"]);
    assert_eq!(named_hidden, ["w-first"]);
}
