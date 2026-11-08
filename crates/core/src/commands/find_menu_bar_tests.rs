use super::super::{FindArgs, FindFilterArgs, FindSelectionArgs, execute};
use super::*;
use crate::{
    AdapterError, AppInfo, WindowInfo,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter},
    locator::live_locator::{
        IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
        ObservationRequest, ObservationRoot, ObservationSource, ObservedSubtree, ObservedTree,
    },
    refs::refs_test_support::HomeGuard,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const APP: &str = "FixtureApp";

#[derive(Default)]
struct MenuBarAdapter {
    menu_item_in_window: bool,
    empty_menu_bar: bool,
    menu_bar_reads: AtomicUsize,
}

fn evidence(role: &str, name: &str) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known(role.into()),
        name: LocatorField::Known(name.into()),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Known(Vec::new()),
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Known(vec!["Click".into()]),
            descriptors: Default::default(),
        },
    }
}

fn node(role: &str, name: &str, children: Vec<ObservedSubtree>) -> ObservedSubtree {
    ObservedSubtree::new(evidence(role, name), children, true, None)
}

fn app_info() -> AppInfo {
    AppInfo {
        name: APP.into(),
        pid: crate::ProcessId::new(101),
        bundle_id: None,
        process_instance: Some("test-instance".into()),
        presentation: None,
    }
}

impl ObservationOps for MenuBarAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let tree = match &root {
            ObservationRoot::Application { surface, .. }
                if *surface == SnapshotSurface::Menubar =>
            {
                self.menu_bar_reads.fetch_add(1, Ordering::SeqCst);
                let items = if self.empty_menu_bar {
                    Vec::new()
                } else {
                    vec![node("menuitem", "Go to Folder\u{2026}", Vec::new())]
                };
                node("menubar", "", vec![node("menubaritem", "Go", items)])
            }
            ObservationRoot::Window(window) => {
                let mut children = vec![node("button", "Open", Vec::new())];
                if self.menu_item_in_window {
                    children.push(node("menuitem", "Go to Folder\u{2026}", Vec::new()));
                }
                node("window", &window.title, children)
            }
            _ => return Err(AdapterError::internal("unexpected observation root")),
        };
        ObservedTree::from_roots(
            vec![tree],
            ObservationSource::from_root(&root, request.surface),
            LocatorStats::default(),
            true,
        )
    }

    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Menubar
    }

    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![app_info()])
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Documents".into(),
            app: APP.into(),
            pid: crate::ProcessId::new(101),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    fn resolve_element_strict(
        &self,
        _entry: &crate::refs::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }
}

impl ActionOps for MenuBarAdapter {}
impl InputOps for MenuBarAdapter {}
impl SystemOps for MenuBarAdapter {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window, SnapshotSurface::Menubar]
    }
}

fn find(role: &str, name: &str) -> FindArgs {
    FindArgs {
        app: Some(APP.into()),
        window_id: None,
        root: None,
        surface: SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: Some(role.into()),
            name: Some(name.into()),
            description: None,
            native_id: None,
            value: None,
            text: None,
            exact: false,
        },
        states: Vec::new(),
        timeout_ms: None,
        selection: FindSelectionArgs {
            count: false,
            first: false,
            last: false,
            nth: None,
            limit: None,
        },
    }
}

fn run(adapter: &MenuBarAdapter, args: FindArgs) -> Value {
    execute(args, adapter, &CommandContext::default()).expect("find succeeds")
}

#[test]
fn a_menu_item_missing_from_the_window_is_answered_from_the_menu_bar() {
    let _guard = HomeGuard::new();
    let adapter = MenuBarAdapter::default();
    let response = run(&adapter, find("menuitem", "go to folder"));
    assert_eq!(response["surface"], "menubar", "{response}");
    let matches = response["matches"].as_array().expect("matches");
    assert_eq!(matches.len(), 1, "{response}");
    assert_eq!(matches[0]["name"], "Go to Folder\u{2026}");
    let snapshot_id = response["snapshot_id"].as_str().expect("snapshot id");
    let ref_id = matches[0]["ref_id"].as_str().expect("ref");
    assert!(ref_id.starts_with(&format!("@{snapshot_id}:")), "{ref_id}");
}

#[test]
fn a_first_match_menu_item_query_is_answered_from_the_menu_bar() {
    let _guard = HomeGuard::new();
    let adapter = MenuBarAdapter::default();
    let mut args = find("menuitem", "go to folder");
    args.selection.first = true;
    let response = run(&adapter, args);
    assert_eq!(response["surface"], "menubar", "{response}");
    assert_eq!(response["match"]["name"], "Go to Folder\u{2026}");
}

#[test]
fn a_menu_item_found_in_the_window_never_reads_the_menu_bar() {
    let _guard = HomeGuard::new();
    let adapter = MenuBarAdapter {
        menu_item_in_window: true,
        ..Default::default()
    };
    let response = run(&adapter, find("menuitem", "go to folder"));
    assert!(response.get("surface").is_none(), "{response}");
    assert_eq!(response["total_matches"], 1);
    assert_eq!(adapter.menu_bar_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn a_find_for_another_role_never_reads_the_menu_bar() {
    let _guard = HomeGuard::new();
    let adapter = MenuBarAdapter::default();
    let response = run(&adapter, find("button", "go to folder"));
    assert_eq!(response["matches"], json!([]));
    assert!(response.get("surface").is_none(), "{response}");
    assert_eq!(adapter.menu_bar_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn an_empty_menu_bar_returns_the_window_answer() {
    let _guard = HomeGuard::new();
    let adapter = MenuBarAdapter {
        empty_menu_bar: true,
        ..Default::default()
    };
    let response = run(&adapter, find("menuitem", "go to folder"));
    assert_eq!(response["matches"], json!([]));
    assert!(response.get("surface").is_none(), "{response}");
    assert!(response["roles_present"].is_array(), "{response}");
    assert_eq!(adapter.menu_bar_reads.load(Ordering::SeqCst), 1);
}

#[test]
fn a_rooted_or_explicitly_surfaced_find_is_left_alone() {
    let query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("menuitem".into()),
            ..Default::default()
        },
        ..LocatorQuery::default()
    };
    let mut rooted = find("menuitem", "x");
    rooted.root = Some("@s1:e1".into());
    assert!(!searches_window_for_menu_item(&rooted, &query));
    let mut sheet = find("menuitem", "x");
    sheet.surface = SnapshotSurface::Sheet;
    assert!(!searches_window_for_menu_item(&sheet, &query));
    assert!(searches_window_for_menu_item(
        &find("menuitem", "x"),
        &query
    ));
}

#[test]
fn every_response_shape_reports_emptiness() {
    assert!(is_empty(&json!({ "count": 0 })));
    assert!(!is_empty(&json!({ "count": 2 })));
    assert!(is_empty(&json!({ "match": null })));
    assert!(!is_empty(&json!({ "match": { "role": "menuitem" } })));
    assert!(is_empty(&json!({ "matches": [] })));
    assert!(!is_empty(&json!({ "matches": [{}] })));
}

#[test]
fn only_a_missing_window_counts_as_finding_nothing() {
    let missing = Err(AppError::Adapter(AdapterError::app_has_no_window(APP)));
    assert!(window_found_nothing(&missing));
    let timeout = Err(AppError::Adapter(AdapterError::timeout("slow")));
    assert!(!window_found_nothing(&timeout));
}
