use agent_desktop_core::{
    AdapterError, Deadline, OpenUrlRequest, OpenedPage, PlatformAdapter, ProcessIdentity,
    SurfaceInfo,
};

use super::browser_link::BrowserLink;
use super::link_access::LinkAccess;
use super::page_choice::choose_page;
use super::web_channel_adapter::WebChannelAdapter;
use crate::{create_background_tab, list_page_targets, navigate};

const BLANK_PAGE: &str = "about:blank";

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Loads a URL over CDP: in the tab the request's window shows, or in a
    /// new background tab. `Ok(None)` means no endpoint reaches the browser.
    pub(crate) fn open_web_url(
        &self,
        request: &OpenUrlRequest<'_>,
    ) -> Result<Option<OpenedPage>, AdapterError> {
        let deadline = request.deadline;
        self.with_link(&request.process, LinkAccess::prompting(deadline), |link| {
            let target_id = if request.new_tab {
                create_background_tab(&mut link.connection, BLANK_PAGE, deadline)?
            } else {
                choose_page(link, request.window, deadline)?.target_id
            };
            let session = link.session(&target_id, deadline)?;
            navigate(&mut link.connection, &session, request.url, deadline)?;
            opened_page(link, target_id, request.url, deadline)
        })
    }

    /// One `web` surface per page target. It never opens an endpoint that
    /// would ask the user to approve the connection, and an unreachable
    /// browser simply has no web surfaces.
    pub(crate) fn web_surfaces(
        &self,
        process: &ProcessIdentity,
        deadline: Deadline,
    ) -> Vec<SurfaceInfo> {
        let listed = self.with_link(process, LinkAccess::silent(deadline), |link| {
            list_page_targets(&mut link.connection, deadline)
        });
        let Ok(Some(targets)) = listed else {
            return Vec::new();
        };
        targets
            .into_iter()
            .map(|target| SurfaceInfo {
                id: target.target_id,
                kind: agent_desktop_core::SnapshotSurface::Web.as_str().into(),
                title: Some(if target.title.is_empty() {
                    target.url
                } else {
                    target.title
                }),
                item_count: None,
                unclassified: Vec::new(),
            })
            .collect()
    }
}

fn opened_page(
    link: &mut BrowserLink,
    target_id: String,
    requested_url: &str,
    deadline: Deadline,
) -> Result<OpenedPage, AdapterError> {
    let listed = list_page_targets(&mut link.connection, deadline)?
        .into_iter()
        .find(|target| target.target_id == target_id);
    let (url, title) = listed.map_or_else(
        || (requested_url.to_owned(), String::new()),
        |target| (target.url, target.title),
    );
    Ok(OpenedPage {
        url,
        title,
        channel: "cdp".into(),
        target_id,
    })
}

#[cfg(test)]
#[path = "web_pages_tests.rs"]
mod tests;
