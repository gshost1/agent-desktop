use std::path::Path;

use agent_desktop_core::{
    Action, ActionOps, ActionRequest, AdapterError, DownloadInfo, ErrorCode, NativeHandle,
    StateFile, SystemOps,
};
use serde_json::{Value, json};

use crate::ElementToken;
use crate::channel::download::download_folder::DownloadFolder;
use crate::channel::download::download_ledger::update_records;
use crate::channel::download::download_record::DownloadRecord;
use crate::channel::download::download_state::DownloadState;
use crate::channel::test_dir::TestDir;
use crate::channel::test_support::{
    PORT, ScriptedConnector, StubPlatform, TOKEN, adapter, attach, lease, process, proven,
};
use crate::channel::web_handle::WebHandle;
use crate::transport::scripted_transport::ScriptedTransport;

const GUID: &str = "0bbf3529-01aa-49fa-8191-e85040f21a22";

struct Fixture {
    dir: TestDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: TestDir::new(),
        }
    }

    fn ledger(&self) -> StateFile {
        StateFile::at(self.dir.path().join("downloads.json"))
    }

    fn downloads(&self) -> std::path::PathBuf {
        let path = self.dir.path().join("downloads");
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn earlier(&self, state: DownloadState) {
        update_records(&self.ledger(), &process(), deadline_ms(1_000), |records| {
            records.push(DownloadRecord {
                guid: GUID.into(),
                url: "http://127.0.0.1/report.csv".into(),
                suggested_filename: "report.csv".into(),
                state,
                bytes: 0,
                path: None,
                delivered: false,
            });
        })
        .unwrap();
    }

    fn wait(
        &self,
        transport: ScriptedTransport,
        folder: bool,
        timeout_ms: u64,
    ) -> Result<DownloadInfo, AdapterError> {
        let mut connector = ScriptedConnector::with(transport);
        connector.download_folder =
            folder.then(|| DownloadFolder::per_connection(self.downloads()));
        let web = adapter(StubPlatform::owning(PORT), connector)
            .with_download_ledger(Some(self.ledger()));
        web.wait_for_download(process(), deadline_ms(timeout_ms))
    }
}

fn deadline_ms(ms: u64) -> agent_desktop_core::Deadline {
    agent_desktop_core::Deadline::after(ms).unwrap()
}

fn frame(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params })
}

fn begin() -> Value {
    frame(
        "Browser.downloadWillBegin",
        json!({ "guid": GUID, "url": "http://127.0.0.1/report.csv", "suggestedFilename": "report.csv" }),
    )
}

fn progress(state: &str, path: Option<&str>) -> Value {
    let mut params = json!({ "guid": GUID, "state": state, "receivedBytes": 8, "totalBytes": 8 });
    if let Some(path) = path {
        params["filePath"] = json!(path);
    }
    frame("Browser.downloadProgress", params)
}

fn kind(error: &AdapterError) -> &str {
    error
        .details
        .as_ref()
        .and_then(|details| details["kind"].as_str())
        .unwrap_or_default()
}

#[test]
fn a_download_that_finished_in_an_earlier_command_is_returned_under_its_suggested_name() {
    let fixture = Fixture::new();
    fixture.earlier(DownloadState::InProgress);
    std::fs::write(fixture.downloads().join(GUID), b"a,b\n1,2\n").unwrap();

    let info = fixture.wait(ScriptedTransport::new(), true, 1_000).unwrap();

    let placed = fixture.downloads().join("report.csv");
    assert_eq!(Path::new(&info.path), placed);
    assert_eq!(std::fs::read(&placed).unwrap(), b"a,b\n1,2\n");
    assert_eq!(info.state, "completed");
    assert_eq!(info.bytes, 8);
    assert_eq!(info.suggested_filename, "report.csv");
    assert_eq!(info.url.as_deref(), Some("http://127.0.0.1/report.csv"));
    assert!(!fixture.downloads().join(GUID).exists());

    let again = fixture
        .wait(ScriptedTransport::new(), true, 300)
        .unwrap_err();
    assert_eq!(again.code, ErrorCode::Timeout);
    assert_eq!(kind(&again), "cdp_download_not_started");
}

#[test]
fn a_download_that_begins_during_the_wait_is_followed_to_its_saved_path() {
    let fixture = Fixture::new();
    let transport = ScriptedTransport::new()
        .push_inbound(begin())
        .push_inbound(progress("inProgress", None))
        .push_inbound(progress(
            "completed",
            Some("/Users/me/Downloads/report.csv"),
        ));

    let info = fixture.wait(transport, false, 1_000).unwrap();

    assert_eq!(info.path, "/Users/me/Downloads/report.csv");
    assert_eq!(info.bytes, 8);
}

#[test]
fn a_canceled_download_is_an_error_not_a_path() {
    let fixture = Fixture::new();
    let transport = ScriptedTransport::new()
        .push_inbound(begin())
        .push_inbound(progress("canceled", None));

    let error = fixture.wait(transport, false, 1_000).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(kind(&error), "cdp_download_canceled");
    let next = fixture
        .wait(ScriptedTransport::new(), false, 200)
        .unwrap_err();
    assert_eq!(kind(&next), "cdp_download_not_started");
}

/// The wait calls a vanished partial file interrupted once it has been
/// missing for the grace period, and returns then. The budget only has to
/// outlast that grace on a stalled host, so it is far longer than the grace.
#[test]
fn a_partial_file_that_vanished_is_an_interrupted_download() {
    let fixture = Fixture::new();
    fixture.earlier(DownloadState::InProgress);

    let error = fixture
        .wait(ScriptedTransport::new(), true, 30_000)
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(kind(&error), "cdp_download_interrupted");
}

#[test]
fn a_download_still_receiving_at_the_timeout_says_so() {
    let fixture = Fixture::new();
    fixture.earlier(DownloadState::InProgress);
    std::fs::write(
        fixture.downloads().join(format!("{GUID}.crdownload")),
        b"a,",
    )
    .unwrap();

    let error = fixture
        .wait(ScriptedTransport::new(), true, 400)
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(kind(&error), "cdp_download_in_progress");
}

#[test]
fn an_earlier_download_without_a_folder_cannot_be_followed() {
    let fixture = Fixture::new();
    fixture.earlier(DownloadState::InProgress);

    let error = fixture
        .wait(ScriptedTransport::new(), false, 1_000)
        .unwrap_err();

    assert_eq!(kind(&error), "cdp_download_untracked", "{error:?}");
}

#[test]
fn a_browser_no_endpoint_reaches_is_refused() {
    let fixture = Fixture::new();
    let web = adapter(StubPlatform::default(), ScriptedConnector::default())
        .with_download_ledger(Some(fixture.ledger()));

    let error = web
        .wait_for_download(process(), deadline_ms(500))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(kind(&error), "cdp_endpoint_missing");
}

#[test]
fn a_click_that_starts_a_download_is_returned_by_a_later_wait_in_the_same_process() {
    let fixture = Fixture::new();
    let transport = proven(attach(ScriptedTransport::new(), "T"))
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect(
            "Page.getLayoutMetrics",
            crate::act::act_fixture::metrics(0.0),
        )
        .expect(
            "DOM.getContentQuads",
            json!({ "quads": [[0, 0, 10, 0, 10, 10, 0, 10]] }),
        )
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 7 }))
        .expect("Input.dispatchMouseEvent", json!({}))
        .expect_with_events(
            "Input.dispatchMouseEvent",
            vec![
                begin(),
                progress("completed", Some("/Users/me/Downloads/report.csv")),
            ],
            json!({ "result": {} }),
        );
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    )
    .with_download_ledger(Some(fixture.ledger()));
    let handle = NativeHandle::new(WebHandle {
        process: process(),
        token: ElementToken::parse(TOKEN).unwrap(),
    });

    web.execute_action(&handle, ActionRequest::headless(Action::Click), &lease())
        .unwrap();
    let info = web
        .wait_for_download(process(), deadline_ms(1_000))
        .unwrap();

    assert_eq!(info.path, "/Users/me/Downloads/report.csv");
}
