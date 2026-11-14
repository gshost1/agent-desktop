use std::sync::Mutex;

use serde_json::Value;

use super::*;
use crate::adapter::minimal_accessibility_node as node;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::context::WaitSelector;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AccessibilityNode, AdapterError, ErrorCode, SnapshotSurface, WindowInfo};

pub(super) struct ChangingWindow {
    pub(super) tree: Mutex<AccessibilityNode>,
}

impl ChangingWindow {
    pub(super) fn new(children: Vec<AccessibilityNode>) -> Self {
        Self {
            tree: Mutex::new(window(children)),
        }
    }

    pub(super) fn show(&self, children: Vec<AccessibilityNode>) {
        *self.tree.lock().unwrap() = window(children);
    }

    fn current(&self) -> AccessibilityNode {
        self.tree.lock().unwrap().clone()
    }
}

fn window(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = node("window", "Notes");
    root.children = children;
    root
}

impl ObservationOps for ChangingWindow {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        crate::adapter::observed_tree(&root, self.current())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Notes".into(),
            app: "FixtureApp".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
        _deadline: crate::Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        Ok(self.current())
    }
}

impl ActionOps for ChangingWindow {}

impl InputOps for ChangingWindow {}

impl SystemOps for ChangingWindow {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window]
    }
}

pub(super) fn args() -> SnapshotArgs {
    SnapshotArgs {
        app: Some("FixtureApp".into()),
        window_id: None,
        max_depth: 10,
        include_bounds: false,
        interactive_only: false,
        compact: false,
        surface: SnapshotSurface::Window,
        skeleton: false,
        root_ref: None,
        timeout_ms: None,
        force_electron_a11y: false,
    }
}

pub(super) fn title(value: &str, focused: bool) -> AccessibilityNode {
    let mut field = node("textfield", "Title");
    field.identity.value = Some(value.into());
    if focused {
        field.presentation.states = vec!["focused".into()];
    }
    field
}

pub(super) fn before() -> Vec<AccessibilityNode> {
    vec![
        title("draft", true),
        node("button", "Save"),
        node("statictext", "Unsaved"),
    ]
}

pub(super) fn after() -> Vec<AccessibilityNode> {
    vec![
        title("draft 2", true),
        node("button", "Save"),
        node("statictext", "Saved"),
        node("button", "Close"),
    ]
}

pub(super) fn snapshot_id(data: &Value) -> String {
    data["snapshot_id"].as_str().unwrap().to_owned()
}

fn code_of(err: AppError) -> ErrorCode {
    match err {
        AppError::Adapter(err) => err.code,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

#[test]
fn a_diff_is_a_new_snapshot_with_rows_instead_of_a_tree() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let context = CommandContext::default();
    let first = crate::commands::snapshot::execute(args(), &adapter, &context).unwrap();
    let since = snapshot_id(&first);
    adapter.show(after());

    let data = execute_since(args(), &since, &adapter, &context).unwrap();
    let now = snapshot_id(&data);
    assert_ne!(now, since);
    assert!(data.get("tree").is_none());
    assert_eq!(data["complete"], true);
    let diff = &data["diff"];
    assert_eq!(diff["since"], since.as_str());
    assert_eq!(diff["status"], "changed");
    assert_eq!(diff["changed"][0]["changed"], serde_json::json!(["value"]));
    assert_eq!(diff["changed"][0]["was"]["value"], "draft");
    let fresh = diff["changed"][0]["ref_id"].as_str().unwrap();
    assert!(fresh.starts_with(&format!("@{now}:e")));
    let added: Vec<&str> = diff["added"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["name"].as_str())
        .collect();
    assert_eq!(added, ["Saved", "Close"]);
    assert_eq!(diff["removed"][0]["name"], "Unsaved");
    assert!(diff["removed"][0].get("ref_id").is_none());
    assert_eq!(diff["focused"]["name"], "Title");

    let store = RefStore::for_session(None).unwrap();
    let (_, local) = crate::refs::ref_token::resolve_ref_target(fresh).unwrap();
    assert!(store.load(Some(&now)).unwrap().get(&local).is_some());
}

#[test]
fn an_unchanged_window_says_so_and_names_the_focused_element() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let context = CommandContext::default();
    let first = crate::commands::snapshot::execute(args(), &adapter, &context).unwrap();
    let data = execute_since(args(), &snapshot_id(&first), &adapter, &context).unwrap();
    let diff = &data["diff"];
    assert_eq!(diff["status"], "unchanged");
    assert!(diff.get("added").is_none() && diff.get("changed").is_none());
    assert!(diff.get("removed").is_none());
    assert_eq!(diff["focused"]["role"], "textfield");
}

#[test]
fn a_diff_can_be_the_baseline_of_the_next_diff() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let context = CommandContext::default();
    let first = crate::commands::snapshot::execute(args(), &adapter, &context).unwrap();
    adapter.show(after());
    let second = execute_since(args(), &snapshot_id(&first), &adapter, &context).unwrap();
    let third = execute_since(args(), &snapshot_id(&second), &adapter, &context).unwrap();
    assert_eq!(third["diff"]["status"], "unchanged");
}

#[test]
fn an_unknown_id_fails_before_the_window_is_read() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let err = execute_since(args(), "s0000000", &adapter, &CommandContext::default());
    assert_eq!(code_of(err.unwrap_err()), ErrorCode::SnapshotNotFound);
    let saved = RefStore::for_session(None)
        .unwrap()
        .latest_snapshot_id()
        .unwrap();
    assert!(saved.is_none(), "no snapshot may be taken for a refused diff");
}

#[test]
fn a_snapshot_of_another_session_is_not_found() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let alpha = CommandContext::new(Some("since-alpha".into()), None, false).unwrap();
    let beta = CommandContext::new(Some("since-beta".into()), None, false).unwrap();
    let first = crate::commands::snapshot::execute(args(), &adapter, &alpha).unwrap();
    let since = snapshot_id(&first);
    let err = execute_since(args(), &since, &adapter, &beta).unwrap_err();
    assert_eq!(code_of(err), ErrorCode::SnapshotNotFound);
    let global = execute_since(args(), &since, &adapter, &CommandContext::default());
    assert_eq!(code_of(global.unwrap_err()), ErrorCode::SnapshotNotFound);
    assert!(execute_since(args(), &since, &adapter, &alpha).is_ok());
}

#[test]
fn a_differently_shaped_snapshot_returns_the_whole_tree() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let context = CommandContext::default();
    let first = crate::commands::snapshot::execute(args(), &adapter, &context).unwrap();
    let interactive = SnapshotArgs {
        interactive_only: true,
        ..args()
    };
    let data = execute_since(interactive, &snapshot_id(&first), &adapter, &context).unwrap();
    assert_eq!(data["diff"]["status"], "not_comparable");
    assert_eq!(data["diff"]["reason"], "presentation_changed");
    assert!(data["tree"].is_object());
}

#[test]
fn since_refuses_root_and_wait_selectors() {
    let _home = HomeGuard::new();
    let adapter = ChangingWindow::new(before());
    let rooted = SnapshotArgs {
        root_ref: Some("@s1abc:e1".into()),
        ..args()
    };
    let err = execute_since(rooted, "s1abc", &adapter, &CommandContext::default());
    assert_eq!(code_of(err.unwrap_err()), ErrorCode::InvalidArgs);
    let waiting = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: "button".into(),
        gone: false,
        timeout_ms: 100,
    }));
    let err = execute_since(args(), "s1abc", &adapter, &waiting);
    assert_eq!(code_of(err.unwrap_err()), ErrorCode::InvalidArgs);
}

#[path = "snapshot_since_fixture_tests.rs"]
mod fixture_tests;
