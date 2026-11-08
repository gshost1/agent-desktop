use serde_json::{Value, json};

use super::{settle_action, watch_navigation};
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, session};
use crate::channel::download::download_event::is_download_event;
use crate::transport::scripted_transport::ScriptedTransport;

fn page(method: &str) -> Value {
    json!({ "method": method, "params": { "frameId": "F" }, "sessionId": "S" })
}

fn browser(method: &str) -> Value {
    json!({ "method": method, "params": { "guid": "g", "url": "http://x/r.csv", "suggestedFilename": "r.csv" } })
}

fn settle(frames: &[Value]) -> (Vec<String>, usize) {
    let transport = frames
        .iter()
        .fold(ScriptedTransport::new(), |transport, frame| {
            transport.push_inbound(frame.clone())
        });
    let mut connection = CdpConnection::new(transport.boxed());
    let starts = settle_action(&mut connection, deadline()).unwrap();
    let left = connection.drain_events(is_download_event).len();
    (starts.into_iter().map(|event| event.method).collect(), left)
}

#[test]
fn an_action_that_starts_nothing_returns_at_once() {
    assert_eq!(settle(&[]), (Vec::new(), 0));
}

#[test]
fn a_navigation_that_turns_into_a_download_returns_its_start() {
    let (starts, progress_left) = settle(&[
        page("Page.frameRequestedNavigation"),
        page("Page.frameStartedLoading"),
        page("Page.lifecycleEvent"),
        page("Page.frameStoppedLoading"),
        browser("Browser.downloadWillBegin"),
        browser("Browser.downloadProgress"),
    ]);

    assert_eq!(starts, vec!["Browser.downloadWillBegin"]);
    assert_eq!(progress_left, 1, "progress stays buffered for the ledger");
}

#[test]
fn a_navigation_that_loads_a_page_returns_no_download() {
    let (starts, _) = settle(&[
        page("Page.frameRequestedNavigation"),
        page("Page.frameStartedNavigating"),
        page("Page.loadEventFired"),
        page("Page.frameStoppedLoading"),
    ]);

    assert!(starts.is_empty());
}

#[test]
fn a_download_that_begins_with_no_navigation_is_returned() {
    let (starts, _) = settle(&[browser("Browser.downloadWillBegin")]);

    assert_eq!(starts, vec!["Browser.downloadWillBegin"]);
}

#[test]
fn watching_navigation_enables_page_events_on_the_session() {
    let transport = ScriptedTransport::new().expect("Page.enable", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    watch_navigation(&mut connection, &session(), deadline()).unwrap();

    assert_eq!(sent.lock().unwrap()[0]["sessionId"], "S");
}
