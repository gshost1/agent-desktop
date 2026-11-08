use agent_desktop_core::{AdapterError, Deadline, DownloadScope, ProcessIdentity};
use std::collections::{HashMap, HashSet};

use super::download::download_event::{event_guid, is_download_start};
use super::download::download_folder::DownloadFolder;
use crate::cdp::protocol_error::{is_protocol_error, target_gone};
use crate::{CdpConnection, CdpEvent, PageSession, attach_to_target};

/// One open browser-level DevTools connection for one browser process, the
/// page sessions attached through it, the folder the browser saves
/// downloads to when launch named one, and the downloads whose start this
/// connection saw. The adapter keeps it for the rest of the invocation so
/// each read does not reconnect.
pub struct BrowserLink {
    pub process: ProcessIdentity,
    pub connection: CdpConnection,
    pub download_folder: Option<DownloadFolder>,
    pub(crate) seen_downloads: HashSet<String>,
    sessions: HashMap<String, PageSession>,
}

impl BrowserLink {
    pub fn new(
        process: ProcessIdentity,
        connection: CdpConnection,
        download_folder: Option<DownloadFolder>,
    ) -> Self {
        Self {
            process,
            connection,
            download_folder,
            seen_downloads: HashSet::new(),
            sessions: HashMap::new(),
        }
    }

    /// The folder downloads must be renamed in from their guid: set only
    /// when DevTools connections alone point downloads at the folder.
    pub(crate) fn guid_folder(&self) -> Option<&std::path::Path> {
        self.download_folder
            .as_ref()
            .filter(|folder| folder.scope == DownloadScope::Connection)
            .map(|folder| folder.path.as_path())
    }

    /// Whether an action that may start a download must wait for it to
    /// begin before the connection closes: only when the folder lasts only as
    /// long as the connection.
    pub(crate) fn holds_downloads_open(&self) -> bool {
        self.guid_folder().is_some()
    }

    /// Remembers the downloads whose start is among `events`: this
    /// connection receives their progress, so a wait can follow them.
    pub(crate) fn note_seen(&mut self, events: &[CdpEvent]) {
        for event in events.iter().filter(|event| is_download_start(event)) {
            if let Some(guid) = event_guid(event) {
                self.seen_downloads.insert(guid.to_owned());
            }
        }
    }

    /// The flattened session for `target_id`, attaching once. A target the
    /// browser no longer has is `WINDOW_NOT_FOUND` with kind
    /// `cdp_target_gone`: the tab or frame is closed, so a new snapshot of it
    /// cannot help.
    pub fn session(
        &mut self,
        target_id: &str,
        deadline: Deadline,
    ) -> Result<PageSession, AdapterError> {
        if let Some(session) = self.sessions.get(target_id) {
            return Ok(session.clone());
        }
        let session =
            attach_to_target(&mut self.connection, target_id, deadline).map_err(|error| {
                if is_protocol_error(&error) {
                    let detail = error.platform_detail.clone().unwrap_or_default();
                    target_gone("Target.attachToTarget", detail)
                } else {
                    error
                }
            })?;
        self.sessions.insert(target_id.to_owned(), session.clone());
        Ok(session)
    }
}

#[cfg(test)]
#[path = "browser_link_tests.rs"]
mod tests;
