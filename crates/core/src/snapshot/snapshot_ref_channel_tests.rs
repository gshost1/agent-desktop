use super::tests::{StubAdapter, drill_opts, named, save_latest, seed_skeleton_refmap};
use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::locator::live_locator::ObservedTree;
use crate::refs::refs_test_support::HomeGuard;

/// Reads like the stub, and reports the channel that read it, as a web
/// channel does for a page.
struct ChannelAdapter {
    inner: StubAdapter,
}

impl ObservationOps for ChannelAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let observed = self.inner.observe_tree(root, request)?;
        Ok(observed.with_channel(crate::SnapshotChannel {
            channel: "cdp".into(),
            url: Some("https://example.test/".into()),
            title: Some("Example".into()),
            suggestion: None,
        }))
    }

    fn list_apps(&self, deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        self.inner.list_apps(deadline)
    }

    fn list_windows(
        &self,
        filter: &crate::adapter::WindowFilter,
        deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.inner.list_windows(filter, deadline)
    }

    fn resolve_element_strict(
        &self,
        entry: &crate::refs::RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.inner.resolve_element_strict(entry, deadline)
    }
}

impl ActionOps for ChannelAdapter {}
impl InputOps for ChannelAdapter {}
impl SystemOps for ChannelAdapter {}

#[test]
fn a_drill_down_reports_the_channel_that_read_it() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_latest(seed_skeleton_refmap());
    let mut subtree_root = named("group", "Sidebar");
    subtree_root.children = vec![named("button", "Save")];
    let adapter = ChannelAdapter {
        inner: StubAdapter::new(subtree_root),
    };

    let result = run_from_ref(&adapter, &drill_opts(), "@e1", Some(&snapshot_id)).unwrap();

    let channel = result
        .notes
        .channel
        .expect("the drill-down keeps its channel");
    assert_eq!(channel.channel, "cdp");
    assert_eq!(channel.url.as_deref(), Some("https://example.test/"));
}
