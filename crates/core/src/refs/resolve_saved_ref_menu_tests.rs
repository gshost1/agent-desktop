use super::test_support::{RuleAdapter, entry, node, tree_with};
use crate::{ErrorCode, refs::refs_test_support::HomeGuard};

struct ClosedMenu(super::test_support::RuleAdapter);

impl crate::adapter::ObservationOps for ClosedMenu {
    fn list_apps_scoped(
        &self,
        name: &str,
        bundle_id: Option<&str>,
        deadline: crate::Deadline,
    ) -> Result<Vec<crate::AppInfo>, crate::AdapterError> {
        self.0.list_apps_scoped(name, bundle_id, deadline)
    }

    fn list_windows(
        &self,
        filter: &crate::adapter::WindowFilter,
        deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, crate::AdapterError> {
        self.0.list_windows(filter, deadline)
    }

    fn observe_tree(
        &self,
        _root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, crate::AdapterError> {
        Err(crate::AdapterError::surface_absent(
            crate::SnapshotSurface::Menu,
            "The window does not present an open menu",
        ))
    }
}

impl crate::adapter::ActionOps for ClosedMenu {}
impl crate::adapter::InputOps for ClosedMenu {}
impl crate::adapter::SystemOps for ClosedMenu {
    fn supported_surfaces(&self) -> Vec<crate::SnapshotSurface> {
        vec![crate::SnapshotSurface::Window, crate::SnapshotSurface::Menu]
    }
}

#[test]
fn a_menu_ref_whose_menu_a_complete_lookup_finds_closed_is_stale() {
    let _guard = HomeGuard::new();
    let adapter = ClosedMenu(RuleAdapter::new(tree_with(vec![node("button", "Run")])));
    let mut entry = entry();
    entry.source.source_surface = crate::SnapshotSurface::Menu;

    let Err(error) = crate::refs::resolve_saved_ref::resolve_saved_ref(
        &adapter,
        &entry,
        crate::Deadline::standard().unwrap(),
        None,
    ) else {
        panic!("a ref taken in a closed menu cannot resolve");
    };

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "menu_closed");
    assert_eq!(details["complete"], true);
    assert_eq!(details["retryable"], false);
}
