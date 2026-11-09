use agent_desktop_core::{DownloadScope, ProcessIdentity};

use super::BrowserLink;
use crate::CdpConnection;
use crate::channel::download::download_folder::DownloadFolder;
use crate::transport::scripted_transport::ScriptedTransport;

fn link(scope: Option<DownloadScope>) -> BrowserLink {
    BrowserLink::new(
        ProcessIdentity::new(1u32, "one"),
        CdpConnection::new(ScriptedTransport::new().boxed()),
        scope.map(|scope| DownloadFolder {
            path: "/tmp/downloads".into(),
            scope,
        }),
    )
}

#[test]
fn only_a_folder_that_lasts_as_long_as_the_connection_holds_a_click_open() {
    assert!(link(Some(DownloadScope::Connection)).holds_downloads_open());
    assert!(!link(Some(DownloadScope::Profile)).holds_downloads_open());
    assert!(!link(None).holds_downloads_open());
}

#[test]
fn only_a_per_connection_folder_names_downloads_by_guid() {
    assert_eq!(
        link(Some(DownloadScope::Connection)).guid_folder(),
        Some(std::path::Path::new("/tmp/downloads"))
    );
    assert_eq!(link(Some(DownloadScope::Profile)).guid_folder(), None);
}
