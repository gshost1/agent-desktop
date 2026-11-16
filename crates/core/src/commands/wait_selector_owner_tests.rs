use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::wait::deadline::{VirtualClock, now};
use crate::wait::window_baseline::WindowBaseline;
use crate::{
    AccessibilityNode, AdapterError, AppError, ErrorCode, SnapshotSurface, WindowFilter,
    WindowInfo, context::CommandContext, refs::refs_test_support::HomeGuard,
    system::process_state::ProcessState,
};

const OURS: &str = "test-instance";
const THEIRS: &str = "other-instance";
const WAIT_BUDGET_MS: u64 = 10_000;

/// Where the selector shows in one window, and from when.
#[derive(Clone, Copy)]
enum Field {
    Nowhere,
    InWindow(Duration),
    InSheet(Duration),
    BesideAnEmptySheet,
}

struct Pane {
    info: WindowInfo,
    field: Field,
    fails: Option<ErrorCode>,
    deep: bool,
}

/// Windows of two processes. `failure` fails every read, a pane's `fails`
/// only that window's, and each overlay probe costs `overlay_cost` of the
/// virtual clock; `late_probes` counts probes started past the wait budget.
struct TwoWindowApp {
    anchor: Instant,
    panes: Vec<Pane>,
    failure: Option<ErrorCode>,
    overlay_cost: Duration,
    late_probes: AtomicUsize,
}

impl TwoWindowApp {
    fn new(panes: Vec<Pane>) -> Self {
        Self {
            anchor: now(),
            panes,
            failure: None,
            overlay_cost: Duration::ZERO,
            late_probes: AtomicUsize::new(0),
        }
    }

    fn charge_overlay_probe(&self) {
        if now().saturating_duration_since(self.anchor) >= ms(WAIT_BUDGET_MS) {
            self.late_probes.fetch_add(1, Ordering::SeqCst);
        }
        crate::wait::deadline::sleep(self.overlay_cost);
    }

    fn shown(&self, since: Duration) -> bool {
        since <= now().saturating_duration_since(self.anchor)
    }

    fn failure(&self) -> Option<AdapterError> {
        self.failure.clone().map(error_for)
    }

    fn surface_tree(
        &self,
        id: &str,
        surface: SnapshotSurface,
        max_depth: u8,
    ) -> Result<AccessibilityNode, AdapterError> {
        if let Some(error) = self.failure() {
            return Err(error);
        }
        let pane = self
            .panes
            .iter()
            .find(|pane| pane.info.id == id)
            .ok_or_else(|| AdapterError::new(ErrorCode::WindowNotFound, "window closed"))?;
        if let Some(code) = pane.fails.clone() {
            return Err(error_for(code));
        }
        self.pane_tree(pane, surface, max_depth)
    }

    fn pane_tree(
        &self,
        pane: &Pane,
        surface: SnapshotSurface,
        max_depth: u8,
    ) -> Result<AccessibilityNode, AdapterError> {
        let width = |since| match (self.shown(since), pane.deep) {
            (true, true) => vec![depth_tests::buried(
                node("incrementor", "Width:"),
                max_depth,
            )],
            (true, false) => vec![node("incrementor", "Width:")],
            (false, _) => Vec::new(),
        };
        let children = match (surface, pane.field) {
            (SnapshotSurface::Sheet, Field::InSheet(since)) => width(since),
            (SnapshotSurface::Sheet, Field::BesideAnEmptySheet) => Vec::new(),
            (SnapshotSurface::Sheet, _) => {
                return Err(AdapterError::surface_absent(
                    SnapshotSurface::Sheet,
                    "no sheet",
                ));
            }
            (_, Field::InWindow(since)) => width(since),
            _ => vec![node("button", "Cancel")],
        };
        let mut root = node("window", &pane.info.id);
        root.children = children;
        Ok(root)
    }
}

fn error_for(code: ErrorCode) -> AdapterError {
    match code {
        ErrorCode::AppUnresponsive => AdapterError::app_unresponsive("TestApp")
            .with_details(serde_json::json!({ "retryable": true })),
        ErrorCode::PermDenied => AdapterError::permission_denied(),
        code => AdapterError::new(code, "read failed"),
    }
}

fn node(role: &str, name: &str) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    }
}

impl ObservationOps for TwoWindowApp {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Menubar
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if request.surface == SnapshotSurface::Sheet {
            self.charge_overlay_probe();
        }
        let tree = match &root {
            ObservationRoot::Window(window) => {
                self.surface_tree(&window.id, request.surface, request.max_logical_depth)?
            }
            ObservationRoot::Application { .. } => match self.failure() {
                Some(error) => return Err(error),
                None => node("menubar", "Menu Bar"),
            },
            _ => return Err(AdapterError::not_supported("observe_tree")),
        };
        crate::adapter::observed_tree(&root, tree)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.panes.iter().map(|pane| pane.info.clone()).collect())
    }

    fn is_auxiliary_window(&self, window: &WindowInfo, _deadline: crate::Deadline) -> bool {
        window.id.starts_with("panel")
    }

    fn get_tree(
        &self,
        window: &WindowInfo,
        opts: &crate::adapter::TreeOptions,
        _deadline: crate::Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        self.surface_tree(&window.id, opts.surface, opts.max_depth)
    }
}

impl ActionOps for TwoWindowApp {}

impl InputOps for TwoWindowApp {}

impl SystemOps for TwoWindowApp {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window, SnapshotSurface::Sheet]
    }

    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.panes
            .iter()
            .find(|pane| pane.info.id == expected.id)
            .map(|pane| pane.info.clone())
            .ok_or_else(|| AdapterError::new(ErrorCode::WindowNotFound, "window closed"))
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<ProcessState, AdapterError> {
        Ok(match self.failure {
            Some(ErrorCode::AppUnresponsive) => ProcessState::Unresponsive,
            _ => ProcessState::Running,
        })
    }
}

fn pane(id: &str, instance: &str, focused: bool, field: Field) -> Pane {
    Pane {
        info: WindowInfo {
            id: id.into(),
            title: id.into(),
            app: "TestApp".into(),
            pid: crate::ProcessId::new(if instance == OURS { 1 } else { 2 }),
            process_instance: Some(instance.into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: focused,
                visible: Some(true),
                ..Default::default()
            },
        },
        field,
        fails: None,
        deep: false,
    }
}

fn window_entry(window: &str) -> crate::RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Apply"));
    entry.source.source_app = Some("TestApp".into());
    entry.source.source_window_id = Some(window.into());
    entry.source.source_window_title = Some(window.into());
    entry
}

fn menubar_entry() -> crate::RefEntry {
    let mut entry = window_entry("");
    entry.source.source_surface = SnapshotSurface::Menubar;
    entry.source.source_window_id = None;
    entry
}

/// The click's own path: a delivered action, the baseline it took before
/// sending, and the post-action wait the command runs after it.
fn click_and_wait(
    app: &TwoWindowApp,
    entry: &crate::RefEntry,
    baseline: Option<&WindowBaseline>,
) -> Result<Value, AppError> {
    let context = CommandContext::default().with_wait_selector(Some(crate::WaitSelector {
        query_raw: ":Width".into(),
        gone: false,
        timeout_ms: WAIT_BUDGET_MS,
    }));
    let delivered =
        serde_json::to_value(crate::ActionResult::delivered_unverified("click")).unwrap();
    crate::commands::helpers::apply_post_action_wait(
        delivered,
        Some(entry),
        baseline,
        app,
        &context,
    )
}

fn baseline(app: &TwoWindowApp, entry: &crate::RefEntry) -> Option<WindowBaseline> {
    let context = CommandContext::default().with_wait_selector(Some(crate::WaitSelector {
        query_raw: ":Width".into(),
        gone: false,
        timeout_ms: 10_000,
    }));
    WindowBaseline::before_action(app, entry, &context, crate::Deadline::after(5_000).ok()?)
}

fn adapter_error(result: Result<Value, AppError>) -> AdapterError {
    match result {
        Err(AppError::Adapter(error)) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn a_sheet_on_the_second_window_is_the_one_materialized() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, true, Field::Nowhere),
        pane("w-2", OURS, false, Field::InSheet(ms(300))),
    ]);
    let entry = window_entry("w-1");
    let baseline = baseline(&app, &entry);

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-2");
    assert_eq!(result["matched_selector"], ":Width");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn an_empty_sheet_on_the_first_window_does_not_hide_the_second_windows_sheet() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, true, Field::BesideAnEmptySheet),
        pane("w-2", OURS, false, Field::InSheet(ms(300))),
    ]);
    let entry = window_entry("w-1");
    let baseline = baseline(&app, &entry);

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-2");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_sheet_of_another_process_with_the_same_name_never_satisfies_the_wait() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-theirs", THEIRS, true, Field::InSheet(Duration::ZERO)),
        pane("w-1", OURS, false, Field::Nowhere),
    ]);
    let entry = window_entry("w-1");
    let baseline = baseline(&app, &entry);

    let error = adapter_error(click_and_wait(&app, &entry, baseline.as_ref()));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= ms(10_000), "{:?}", clock.elapsed());
}

#[path = "wait_selector_unreadable_tests.rs"]
mod unreadable_tests;

#[path = "wait_selector_front_window_tests.rs"]
mod front_window_tests;

#[path = "wait_selector_depth_scope_tests.rs"]
mod depth_tests;
