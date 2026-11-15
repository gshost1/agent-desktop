use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::{AccessibilityNode, AdapterError, WindowInfo};

const LABEL_DEPTH: u8 = 22;

/// A window whose only copy of the text sits 22 levels down, as an editor's
/// document body does once its window title stops growing. The adapter cuts
/// the tree at the depth the request asks for, as every platform walk does.
struct DeepBodyAdapter;

fn level(depth: u8, max_depth: u8) -> AccessibilityNode {
    let (role, name, children) = if depth == LABEL_DEPTH {
        ("textfield", None, Vec::new())
    } else if depth >= max_depth {
        ("group", Some(format!("level {depth}")), Vec::new())
    } else {
        (
            "group",
            Some(format!("level {depth}")),
            vec![level(depth + 1, max_depth)],
        )
    };
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name,
            value: (depth == LABEL_DEPTH).then(|| "first line\nsecond line deepword".into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

impl ObservationOps for DeepBodyAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let window = AccessibilityNode {
            role: "window".into(),
            identity: crate::NodeIdentity {
                name: Some("first line (Draft)".into()),
                ..Default::default()
            },
            children: vec![level(1, request.max_logical_depth)],
            ..level(0, 0)
        };
        crate::adapter::observed_tree(&root, window)
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "first line (Draft)".into(),
            app: "Editor".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }
}

impl ActionOps for DeepBodyAdapter {}
impl InputOps for DeepBodyAdapter {}
impl SystemOps for DeepBodyAdapter {}

/// `find --text` searches 50 levels down; `wait --text` read a snapshot cut
/// at 10 and timed out on text `find --text` found at once.
#[test]
fn a_text_wait_finds_text_as_deep_as_find_text_does() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let result = execute(
        WaitArgs {
            mode: WaitModeArgs {
                text: Some("deepword".into()),
                ..WaitModeArgs::default()
            },
            predicate: WaitPredicateArgs {
                predicate: None,
                value: None,
                action: None,
                count: None,
            },
            timeout_ms: 1_000,
            app: Some("Editor".into()),
        },
        &DeepBodyAdapter,
        &CommandContext::default(),
    )
    .expect("the text is in the window, 22 levels down");

    assert_eq!(result["found"], true);
    assert_eq!(result["role"], "textfield");
}
