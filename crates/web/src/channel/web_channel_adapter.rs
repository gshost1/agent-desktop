use std::sync::{Mutex, MutexGuard};

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, PlatformAdapter, ProcessIdentity, SnapshotSurface, StateFile,
};

use super::browser_link::BrowserLink;
use super::cdp_connector::CdpConnector;
use super::download::download_folder::DownloadFolder;
use super::download::download_ledger::ledger_file;
use super::link_access::LinkAccess;
use super::system_connector::SystemConnector;
use crate::CdpConnection;

/// A `PlatformAdapter` that wraps the platform adapter and answers web work
/// over the browser's DevTools endpoint. It intercepts only the web snapshot
/// surface, refs that carry a `cdp:` element token, handles that carry a
/// [`super::web_handle::WebHandle`], `open-url`, the web entries of
/// `list-surfaces`, `wait --download`, and launch's endpoint record.
/// Everything else goes to the platform adapter unchanged.
pub struct WebChannelAdapter<P> {
    pub(crate) inner: P,
    pub(crate) connector: Box<dyn CdpConnector>,
    link: Mutex<Option<BrowserLink>>,
    downloads: Option<StateFile>,
}

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    pub fn new(inner: P) -> Self {
        Self::with_connector(inner, Box::new(SystemConnector::new()))
    }

    pub fn with_connector(inner: P, connector: Box<dyn CdpConnector>) -> Self {
        Self {
            inner,
            connector,
            link: Mutex::new(None),
            downloads: ledger_file().ok(),
        }
    }

    /// Keeps the download ledger in `file` instead of the state root; `None`
    /// leaves downloads unrecorded.
    pub fn with_download_ledger(mut self, file: Option<StateFile>) -> Self {
        self.downloads = file;
        self
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }

    pub(crate) fn download_ledger(&self) -> Result<&StateFile, AdapterError> {
        self.downloads.as_ref().ok_or_else(|| {
            AdapterError::internal("The state root holds no download ledger for this invocation")
        })
    }

    /// Runs `op` over the open link to `process`, discovering and opening it
    /// first when needed. `Ok(None)` means no endpoint reaches the process.
    /// A transport failure drops the link so the next call reconnects.
    pub(crate) fn with_link<T>(
        &self,
        process: &ProcessIdentity,
        access: LinkAccess,
        op: impl FnOnce(&mut BrowserLink) -> Result<T, AdapterError>,
    ) -> Result<Option<T>, AdapterError> {
        let mut guard = self.lock_link();
        if guard.as_ref().is_some_and(|link| &link.process != process) {
            *guard = None;
        }
        if guard.is_none() {
            let Some((connection, download_folder)) = self.discover(process, access)? else {
                return Ok(None);
            };
            *guard = Some(BrowserLink::new(
                process.clone(),
                connection,
                download_folder,
            ));
        }
        let Some(link) = guard.as_mut() else {
            return Ok(None);
        };
        let result = op(link);
        if result.as_ref().is_err_and(is_transport_failure) {
            *guard = None;
        }
        result.map(Some)
    }

    /// [`Self::with_link`] for work that has no accessibility fallback, such
    /// as acting on a web ref: a missing endpoint is `STALE_REF`.
    pub(crate) fn with_required_link<T>(
        &self,
        process: &ProcessIdentity,
        deadline: Deadline,
        op: impl FnOnce(&mut BrowserLink) -> Result<T, AdapterError>,
    ) -> Result<T, AdapterError> {
        self.with_link(process, LinkAccess::prompting(deadline), op)?
            .ok_or_else(|| {
                AdapterError::stale_ref_because(
                    "No DevTools endpoint reaches the browser this web ref came from",
                )
                .with_suggestion(
                    "The browser was closed or relaunched without --cdp; take a new snapshot",
                )
                .with_details(serde_json::json!({ "kind": "cdp_endpoint_gone" }))
            })
    }

    /// The platform adapter's surface for a request the web channel does not
    /// serve: the web surface is the window it is drawn in.
    pub(crate) fn platform_surface(surface: SnapshotSurface) -> SnapshotSurface {
        if surface == SnapshotSurface::Web {
            SnapshotSurface::Window
        } else {
            surface
        }
    }

    fn discover(
        &self,
        process: &ProcessIdentity,
        access: LinkAccess,
    ) -> Result<Option<(CdpConnection, Option<DownloadFolder>)>, AdapterError> {
        let mut first_error = None;
        for candidate in self.connector.candidates(process) {
            let owned = self.inner.process_owns_tcp_listener(
                process.clone(),
                candidate.port,
                access.deadline,
            );
            if !matches!(owned, Ok(true)) {
                continue;
            }
            match self
                .connector
                .open(&candidate, access.deadline, access.may_prompt)
            {
                Ok(connection) => return Ok(Some((connection, candidate.download_folder()))),
                Err(error) if error.code == ErrorCode::PermDenied => return Err(error),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(None), Err)
    }

    fn lock_link(&self) -> MutexGuard<'_, Option<BrowserLink>> {
        match self.link.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn is_transport_failure(error: &AdapterError) -> bool {
    let kind = error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(serde_json::Value::as_str);
    matches!(kind, Some("cdp_transport_closed" | "cdp_transport_failed"))
}

#[cfg(test)]
#[path = "web_channel_adapter_tests.rs"]
mod tests;
