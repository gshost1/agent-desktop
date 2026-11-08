use agent_desktop_core::DownloadScope;
use serde_json::json;

use super::enable_downloads;
use crate::CdpConnection;
use crate::act::act_fixture::deadline;
use crate::channel::download::download_folder::DownloadFolder;
use crate::transport::scripted_transport::ScriptedTransport;

fn sent_params(folder: Option<DownloadFolder>) -> serde_json::Value {
    let transport = ScriptedTransport::new().expect("Browser.setDownloadBehavior", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    enable_downloads(&mut connection, folder.as_ref(), deadline()).unwrap();
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].get("sessionId").is_none(), "{}", sent[0]);
    sent[0]["params"].clone()
}

fn folder(scope: DownloadScope) -> Option<DownloadFolder> {
    Some(DownloadFolder {
        path: "/tmp/agent-downloads".into(),
        scope,
    })
}

#[test]
fn a_folder_only_connections_point_at_saves_each_download_under_its_guid() {
    let params = sent_params(folder(DownloadScope::Connection));
    assert_eq!(params["behavior"], "allowAndName");
    assert_eq!(params["downloadPath"], "/tmp/agent-downloads");
    assert_eq!(params["eventsEnabled"], true);
}

#[test]
fn a_folder_the_profile_saves_to_is_left_to_the_profile() {
    let params = sent_params(folder(DownloadScope::Profile));
    assert_eq!(params["behavior"], "default");
    assert_eq!(params["eventsEnabled"], true);
    assert!(params.get("downloadPath").is_none());
}

#[test]
fn without_a_folder_the_browser_keeps_its_own_and_only_reports_events() {
    let params = sent_params(None);
    assert_eq!(params["behavior"], "default");
    assert_eq!(params["eventsEnabled"], true);
    assert!(params.get("downloadPath").is_none());
}
