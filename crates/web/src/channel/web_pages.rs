use agent_desktop_core::{
    AdapterError, Deadline, OpenUrlRequest, OpenedPage, PlatformAdapter, ProcessIdentity,
    SurfaceInfo,
};

use super::browser_link::BrowserLink;
use super::link_access::LinkAccess;
use super::page_choice::{NO_PAGE_TARGET, choose_page};
use super::web_channel_adapter::WebChannelAdapter;
use crate::{create_background_tab, list_page_targets, navigate};

const BLANK_PAGE: &str = "about:blank";
const CURRENT_TAB: &str = "current_tab";
const NEW_TAB: &str = "new_tab";
const DOWNLOAD: &str = "download";

/// Where `open-url` put the page: its target and how it got there.
struct Placement {
    target_id: String,
    opened: &'static str,
}

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Loads a URL over CDP: in the tab the request's window shows, or in a
    /// new background tab. A URL the browser saves as a download instead of
    /// showing is recorded for `wait --download` and reported as
    /// `opened: "download"`. `Ok(None)` means no endpoint reaches the browser.
    pub(crate) fn open_web_url(
        &self,
        request: &OpenUrlRequest<'_>,
    ) -> Result<Option<OpenedPage>, AdapterError> {
        let deadline = request.deadline;
        self.with_link(&request.process, LinkAccess::prompting(deadline), |link| {
            let (target_id, opened) = if request.new_tab {
                (background_tab(link, deadline)?, NEW_TAB)
            } else {
                match choose_page(link, request.window, deadline) {
                    Ok(page) => (page.target_id, CURRENT_TAB),
                    Err(error) if is_no_page_target(&error) => {
                        (background_tab(link, deadline)?, NEW_TAB)
                    }
                    Err(error) => return Err(error),
                }
            };
            let session = link.session(&target_id, deadline)?;
            let navigated = navigate(&mut link.connection, &session, request.url, deadline);
            let linger = navigated.is_err() && link.download_folder.is_some();
            let download_began = self.note_downloads(link, linger, deadline);
            match navigated {
                Ok(()) => opened_page(link, Placement { target_id, opened }, request.url, deadline),
                Err(_) if download_began => Ok(OpenedPage {
                    url: request.url.to_owned(),
                    title: String::new(),
                    channel: "cdp".into(),
                    target_id,
                    opened: DOWNLOAD.into(),
                }),
                Err(error) => Err(error),
            }
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

/// A new background tab. A browser with no page at all, such as a fresh
/// profile showing only its first-run dialog, gets one this way too, so
/// `open-url` never needs a page to exist first.
fn background_tab(link: &mut BrowserLink, deadline: Deadline) -> Result<String, AdapterError> {
    create_background_tab(&mut link.connection, BLANK_PAGE, deadline)
}

fn is_no_page_target(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(serde_json::Value::as_str)
        == Some(NO_PAGE_TARGET)
}

fn opened_page(
    link: &mut BrowserLink,
    placement: Placement,
    requested_url: &str,
    deadline: Deadline,
) -> Result<OpenedPage, AdapterError> {
    let Placement { target_id, opened } = placement;
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
        opened: opened.into(),
    })
}

#[cfg(test)]
#[path = "web_pages_tests.rs"]
mod tests;
