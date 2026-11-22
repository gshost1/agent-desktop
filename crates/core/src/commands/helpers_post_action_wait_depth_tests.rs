use super::*;
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};

const BUTTON_DEPTH: u8 = 22;

/// A window whose only `Done` button sits 22 levels down, as a dialog's
/// controls do inside a deeply nested web or toolkit layout. The adapter cuts
/// the tree at the depth the request asks for, as every platform walk does.
/// `labelled_by_child` leaves the button unnamed with its label in a child
/// text node, as web and GTK buttons often are.
struct DeepDialogAdapter {
    labelled_by_child: bool,
}

const NAMED: DeepDialogAdapter = DeepDialogAdapter {
    labelled_by_child: false,
};

fn level(depth: u8, max_depth: u8, labelled_by_child: bool) -> AccessibilityNode {
    if depth == BUTTON_DEPTH && !labelled_by_child {
        return crate::adapter::minimal_accessibility_node("button", "Done");
    }
    if depth == BUTTON_DEPTH {
        let label = crate::adapter::minimal_accessibility_node("text", "Done");
        return AccessibilityNode {
            identity: crate::NodeIdentity::default(),
            children: if depth < max_depth {
                vec![label]
            } else {
                Vec::new()
            },
            ..crate::adapter::minimal_accessibility_node("button", "")
        };
    }
    let children = if depth >= max_depth {
        Vec::new()
    } else {
        vec![level(depth + 1, max_depth, labelled_by_child)]
    };
    AccessibilityNode {
        children,
        ..crate::adapter::minimal_accessibility_node("group", &format!("level {depth}"))
    }
}

fn window() -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Dialog".into(),
        app: "TargetApp".into(),
        pid: crate::ProcessId::new(1),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            ..Default::default()
        },
    }
}

impl ObservationOps for DeepDialogAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Element { .. }) {
            return crate::adapter::saved_ref_element_tree(root);
        }
        let dialog = AccessibilityNode {
            children: vec![level(1, request.max_logical_depth, self.labelled_by_child)],
            ..crate::adapter::minimal_accessibility_node("window", "Dialog")
        };
        crate::adapter::observed_tree(&root, dialog)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![window()])
    }
}

impl ActionOps for DeepDialogAdapter {}
impl InputOps for DeepDialogAdapter {}

impl SystemOps for DeepDialogAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(expected.clone())
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<crate::system::process_state::ProcessState, AdapterError> {
        Ok(crate::system::process_state::ProcessState::Running)
    }
}

fn deep_button_wait(adapter: &DeepDialogAdapter) -> Result<Value, AppError> {
    let mut entry = entry();
    entry.source.source_app = Some("TargetApp".into());
    entry.source.source_window_id = Some("w-1".into());
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: "button:Done".into(),
        gone: false,
        timeout_ms: 300,
    }));
    apply_post_action_wait(
        json!({ "action": "ok" }),
        Some(&entry),
        None,
        adapter,
        &context,
    )
}

fn holds_done_button(node: &Value) -> bool {
    (node["role"] == "button" && (node["name"] == "Done" || node["children"][0]["name"] == "Done"))
        || node["children"]
            .as_array()
            .is_some_and(|children| children.iter().any(holds_done_button))
}

/// The wait observes 50 levels down and found the button at 22; the snapshot
/// it then returned was cut at the default 10, so the recheck missed the
/// button and the wait timed out on a dialog that already satisfied it.
#[test]
fn a_post_action_wait_returns_a_match_deeper_than_the_default_snapshot_depth() {
    let _guard = HomeGuard::new();

    let value = deep_button_wait(&NAMED).expect("the button is in the window, 22 levels down");

    assert_eq!(value["matched_selector"], "button:Done");
    assert_eq!(value["after_action"]["action"], "ok");
    assert!(holds_done_button(&value["tree"]), "{value:#}");
}

/// The same holds for a wait scoped by app rather than followed from a ref,
/// the read `snapshot --wait-for` and a scoped post-action wait make.
#[test]
fn a_scoped_wait_returns_a_match_deeper_than_the_default_snapshot_depth() {
    let _guard = HomeGuard::new();
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: "button:Done".into(),
        gone: false,
        timeout_ms: 300,
    }));

    let value = apply_scoped_post_action_wait(
        json!({ "action": "ok" }),
        Some("TargetApp".into()),
        None,
        &NAMED,
        &context,
    )
    .expect("the button is in the window, 22 levels down");

    assert!(holds_done_button(&value["tree"]), "{value:#}");
}

/// A skeleton presents three levels, so a match below them comes back as a
/// full tree to the match's depth rather than a timeout.
#[test]
fn a_skeleton_wait_returns_a_full_tree_to_a_deeper_match() {
    let _guard = HomeGuard::new();

    let value = crate::commands::wait_selector::execute(
        crate::commands::wait_selector::WaitSelectorInput {
            query_raw: "button:Done".into(),
            gone: false,
            app: Some("TargetApp".into()),
            window_id: None,
            opts: TreeOptions {
                max_depth: 3,
                skeleton: true,
                ..TreeOptions::default()
            },
            timeout_ms: 300,
        },
        &NAMED,
        &CommandContext::default(),
    )
    .expect("the button is in the window, 22 levels down");

    assert!(holds_done_button(&value["tree"]), "{value:#}");
}

/// A selector's text part matches a node through its descendants' text, so
/// the snapshot must reach the label below the button, not only the button.
#[test]
fn a_post_action_wait_returns_a_deep_button_labelled_by_a_child() {
    let _guard = HomeGuard::new();
    let adapter = DeepDialogAdapter {
        labelled_by_child: true,
    };

    let value = deep_button_wait(&adapter).expect("the labelled button is 22 levels down");

    assert!(holds_done_button(&value["tree"]), "{value:#}");
}
