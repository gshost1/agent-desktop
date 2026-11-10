use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::{AccessibilityNode, AdapterError, WindowInfo};

const NAME_PAD: usize = 400;
pub(crate) const BUTTON_COUNT: usize = 2_000;
pub(crate) const LAST_BUTTON: usize = BUTTON_COUNT - 1;

/// A window whose refs serialize well past the 1 MB refmap cap, so a saved
/// snapshot of it keeps roughly the first two thirds of its buttons. The
/// buttons at `needles` carry the word "needle" in their names.
pub(crate) fn oversized_window(needles: &[usize]) -> AccessibilityNode {
    let mut window = crate::adapter::minimal_accessibility_node("window", "Doc");
    window.children = (0..BUTTON_COUNT)
        .map(|index| {
            let word = if needles.contains(&index) {
                "needle"
            } else {
                "item"
            };
            let name = format!("{word} {index:05} {}", "x".repeat(NAME_PAD));
            crate::adapter::minimal_accessibility_node("button", &name)
        })
        .collect();
    window
}

pub(crate) struct OversizedTreeAdapter {
    pub(crate) tree: AccessibilityNode,
}

impl ObservationOps for OversizedTreeAdapter {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        crate::adapter::observed_tree(&root, self.tree.clone())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TestApp".into(),
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

impl ActionOps for OversizedTreeAdapter {}
impl InputOps for OversizedTreeAdapter {}
impl SystemOps for OversizedTreeAdapter {}
