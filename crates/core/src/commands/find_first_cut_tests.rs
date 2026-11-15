use super::test_support::LiveFindAdapter;
use super::*;
use crate::{
    AdapterError, WindowInfo,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter},
    locator::live_locator::{
        LocatorStats, ObservationRequest, ObservationRoot, ObservationSource, ObservedSubtree,
        ObservedTree,
    },
    refs::refs_test_support::HomeGuard,
};

/// A window whose read stops inside a long list after its first button.
struct CutWindow {
    first_button: &'static str,
}

impl ObservationOps for CutWindow {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let source = ObservationSource::from_root(&root, request.surface);
        if let ObservationRoot::Element { entry, .. } = &root {
            let own =
                LiveFindAdapter::evidence(&entry.identity.role, entry.identity.name.as_deref());
            return ObservedTree::from_roots(
                vec![ObservedSubtree::new(own, Vec::new(), true, None)],
                source,
                LocatorStats::default(),
                true,
            );
        }
        let button = ObservedSubtree::new(
            LiveFindAdapter::evidence("button", Some(self.first_button)),
            Vec::new(),
            true,
            None,
        );
        let list = ObservedSubtree::new(
            LiveFindAdapter::evidence("list", None),
            Vec::new(),
            false,
            Some(500),
        );
        let window = ObservedSubtree::new(
            LiveFindAdapter::evidence("window", Some("Cut")),
            vec![button, list],
            false,
            None,
        );
        ObservedTree::from_roots(vec![window], source, LocatorStats::default(), false)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Cut".into(),
            app: "FixtureApp".into(),
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

    fn resolve_locator_anchor(
        &self,
        _entry: &crate::refs::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }
}
impl ActionOps for CutWindow {}
impl InputOps for CutWindow {}
impl SystemOps for CutWindow {
    fn supported_surfaces(&self) -> Vec<crate::SnapshotSurface> {
        vec![crate::SnapshotSurface::Window]
    }
}

fn first_by_name(name: &str) -> FindArgs {
    FindArgs {
        scope: FindScopeArgs {
            app: None,
            window_id: Some("w-1".into()),
            root: None,
            surface: crate::SnapshotSurface::Window,
        },
        filter: FindFilterArgs {
            role: None,
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
            first: true,
            last: false,
            nth: None,
            limit: None,
            allow_partial: false,
        },
    }
}

fn find_first(name: &str, first_button: &'static str) -> Value {
    let _guard = HomeGuard::new();
    execute(
        first_by_name(name),
        &CutWindow { first_button },
        &CommandContext::default(),
    )
    .expect("a cut read still answers --first")
}

#[test]
fn a_cut_read_answers_first_with_the_longer_name_it_saw_and_says_the_search_did_not_finish() {
    let response = find_first("Save", "Save As");

    assert_eq!(response["match"]["name"], "Save As");
    assert_eq!(response["complete"], false);
    assert_eq!(response["truncated"], true);
}

#[test]
fn a_cut_read_answers_first_with_a_whole_name_it_proved_as_a_complete_answer() {
    let response = find_first("Save", "Save");

    assert_eq!(response["match"]["name"], "Save");
    assert!(response.get("complete").is_none());
    assert!(response.get("truncated").is_none());
}
