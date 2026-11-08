use agent_desktop_core::{
    AccessibilityNode, AdapterError, ErrorCode, ObservationRequest, ObservationRoot, ObservedTree,
    PlatformAdapter, ProcessIdentity, SnapshotChannel, SnapshotSurface, WindowInfo,
};

use super::browser_link::BrowserLink;
use super::link_access::LinkAccess;
use super::observed_page::observed_page;
use super::page_choice::choose_page;
use super::ref_routing::window_process;
use super::web_channel_adapter::WebChannelAdapter;
use super::web_handle::WebHandle;
use crate::{PageTarget, snapshot_page};

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Reads the web surface of a browser window: the page the window shows,
    /// over CDP when an endpoint reaches the browser, and otherwise the
    /// window's accessibility tree, marked as read through `ax`.
    pub(crate) fn observe_web_window(
        &self,
        window: &WindowInfo,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let root = ObservationRoot::Window(window);
        let read = match window_process(window) {
            Some(process) => self.read_shown_page(&process, Some(window), &root, request)?,
            None => None,
        };
        match read {
            Some(observed) => Ok(observed),
            None => self.observe_through_accessibility(root, request, &window.app),
        }
    }

    /// Reads the subtree under a web ref, for a drill-down rooted at it.
    pub(crate) fn observe_web_element(
        &self,
        handle: &WebHandle,
        root: &ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let token = handle.token.to_string();
        self.with_required_link(&handle.process, request.deadline, |link| {
            let session = link.session(&handle.token.target_id, request.deadline)?;
            let snapshot = snapshot_page(&mut link.connection, &session, request.deadline)?;
            let subtree = find_subtree(snapshot.tree, &token).ok_or_else(|| {
                AdapterError::stale_ref_because("The web element is no longer in the page")
                    .with_details(serde_json::json!({ "kind": "cdp_node_gone" }))
            })?;
            Ok(observed_page(root, subtree, request.max_raw_depth)?
                .with_channel(cdp_channel(None, Vec::new())))
        })
    }

    fn read_shown_page(
        &self,
        process: &ProcessIdentity,
        window: Option<&WindowInfo>,
        root: &ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<Option<ObservedTree>, AdapterError> {
        let deadline = request.deadline;
        self.with_link(process, LinkAccess::prompting(deadline), |link| {
            let page = choose_page(link, window, deadline)?;
            read_page(link, &page, root, request)
        })
    }

    fn observe_through_accessibility(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
        app: &str,
    ) -> Result<ObservedTree, AdapterError> {
        let mut platform_request = *request;
        platform_request.surface = SnapshotSurface::Window;
        let observed = self.inner.observe_tree(root, &platform_request)?;
        Ok(observed.with_channel(SnapshotChannel {
            channel: "ax".into(),
            url: None,
            title: None,
            suggestion: Some(format!(
                "No DevTools endpoint reaches {app}, so the page was read through the \
                 accessibility tree. Relaunch it with `agent-desktop launch \"{app}\" --cdp` \
                 to read and act on the page over CDP."
            )),
        }))
    }
}

fn read_page(
    link: &mut BrowserLink,
    page: &PageTarget,
    root: &ObservationRoot<'_>,
    request: &ObservationRequest,
) -> Result<ObservedTree, AdapterError> {
    let session = link.session(&page.target_id, request.deadline)?;
    let snapshot = snapshot_page(&mut link.connection, &session, request.deadline)?;
    Ok(observed_page(root, snapshot.tree, request.max_raw_depth)?
        .with_channel(cdp_channel(Some(page), snapshot.unreadable_frames)))
}

fn cdp_channel(page: Option<&PageTarget>, unreadable_frames: Vec<String>) -> SnapshotChannel {
    SnapshotChannel {
        channel: "cdp".into(),
        url: page.map(|page| page.url.clone()),
        title: page.map(|page| page.title.clone()),
        suggestion: (!unreadable_frames.is_empty()).then(|| {
            format!(
                "{} cross-origin frame(s) could not be read and are left out of the tree",
                unreadable_frames.len()
            )
        }),
    }
}

fn find_subtree(node: AccessibilityNode, token: &str) -> Option<AccessibilityNode> {
    if node.identity.retained_object.as_deref() == Some(token) {
        return Some(node);
    }
    node.children
        .into_iter()
        .find_map(|child| find_subtree(child, token))
}

/// The error for a web observation asked of a root the web channel cannot
/// read: an application root has no single page.
pub(crate) fn web_root_unsupported() -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        "The web surface is read from one browser window",
    )
    .with_suggestion(
        "Pass --app with the browser's name, and --window-id when it has several windows",
    )
}

#[cfg(test)]
#[path = "web_observation_tests.rs"]
mod tests;
