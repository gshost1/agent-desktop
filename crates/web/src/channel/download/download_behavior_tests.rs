use std::path::Path;

use serde_json::json;

use super::enable_downloads;
use crate::CdpConnection;
use crate::act::act_fixture::deadline;
use crate::transport::scripted_transport::ScriptedTransport;

fn sent_params(dir: Option<&Path>) -> serde_json::Value {
    let transport = ScriptedTransport::new().expect("Browser.setDownloadBehavior", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    enable_downloads(&mut connection, dir, deadline()).unwrap();
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].get("sessionId").is_none(), "{}", sent[0]);
    sent[0]["params"].clone()
}

#[test]
fn a_download_folder_saves_each_download_under_its_guid_there() {
    let params = sent_params(Some(Path::new("/tmp/agent-downloads")));
    assert_eq!(params["behavior"], "allowAndName");
    assert_eq!(params["downloadPath"], "/tmp/agent-downloads");
    assert_eq!(params["eventsEnabled"], true);
}

#[test]
fn without_a_folder_the_browser_keeps_its_own_and_only_reports_events() {
    let params = sent_params(None);
    assert_eq!(params["behavior"], "default");
    assert_eq!(params["eventsEnabled"], true);
    assert!(params.get("downloadPath").is_none());
}
