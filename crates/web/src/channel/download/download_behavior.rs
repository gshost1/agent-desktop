use agent_desktop_core::{AdapterError, Deadline, DownloadScope};
use serde_json::json;

use super::download_folder::DownloadFolder;
use crate::CdpConnection;

/// Turns on the browser's download events for this connection. A folder
/// the profile already saves to is left to the profile (`default`): the
/// profile's own download path numbers a taken name as `name (1).ext`, while
/// `allow` overwrites it. A folder only connections point at uses
/// `allowAndName`: the browser saves each download under its guid, so a
/// later invocation can find it by name. Without a folder the browser keeps
/// its own folder and naming, so a real profile's downloads are never
/// redirected. The browser resets this when the connection closes, so every
/// connection sets it again.
pub(crate) fn enable_downloads(
    connection: &mut CdpConnection,
    folder: Option<&DownloadFolder>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let params = match folder {
        Some(folder) if folder.scope == DownloadScope::Connection => json!({
            "behavior": "allowAndName",
            "downloadPath": folder.path,
            "eventsEnabled": true,
        }),
        _ => json!({ "behavior": "default", "eventsEnabled": true }),
    };
    connection
        .call("Browser.setDownloadBehavior", params, None, deadline)
        .map(|_| ())
}

#[cfg(test)]
#[path = "download_behavior_tests.rs"]
mod tests;
