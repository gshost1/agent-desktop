use agent_desktop_core::Deadline;
use serde_json::json;

use super::{attach_to_target, create_background_tab, list_page_targets};
use crate::CdpConnection;
use crate::transport::scripted_transport::ScriptedTransport;

fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

#[test]
fn only_page_targets_are_listed() {
    let transport = ScriptedTransport::new().expect(
        "Target.getTargets",
        json!({ "targetInfos": [
            { "targetId": "P1", "type": "page", "url": "https://a.test/", "title": "A", "attached": false },
            { "targetId": "W1", "type": "service_worker", "url": "https://a.test/sw.js", "title": "" },
            { "targetId": "P2", "type": "page", "url": "about:blank", "title": "", "attached": true, "browserContextId": "C" },
        ] }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let pages = list_page_targets(&mut connection, deadline()).unwrap();

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].target_id, "P1");
    assert_eq!(pages[0].title, "A");
    assert!(pages[1].attached);
    assert_eq!(pages[1].browser_context_id.as_deref(), Some("C"));
}

#[test]
fn attach_flattens_and_releases_a_paused_target() {
    let transport = ScriptedTransport::new()
        .expect("Target.attachToTarget", json!({ "sessionId": "S9" }))
        .expect("Runtime.runIfWaitingForDebugger", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let session = attach_to_target(&mut connection, "P1", deadline()).unwrap();

    assert_eq!(session.session_id, "S9");
    assert_eq!(session.target_id, "P1");
    let sent = sent.lock().unwrap();
    assert_eq!(
        sent[0]["params"],
        json!({ "targetId": "P1", "flatten": true })
    );
    assert_eq!(sent[1]["sessionId"], "S9");
}

fn attached(waiting: bool) -> serde_json::Value {
    json!({
        "method": "Target.attachedToTarget",
        "params": {
            "sessionId": "S9",
            "targetInfo": { "targetId": "P1", "type": "page" },
            "waitingForDebugger": waiting,
        },
    })
}

fn methods_sent_by_attach(transport: ScriptedTransport) -> Vec<String> {
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let session = attach_to_target(&mut connection, "P1", deadline()).unwrap();
    assert_eq!(session.session_id, "S9");
    let sent = sent.lock().unwrap();
    sent.iter()
        .filter_map(|request| request["method"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn a_target_the_browser_reports_running_is_not_released_again() {
    let transport = ScriptedTransport::new().expect_with_events(
        "Target.attachToTarget",
        vec![attached(false)],
        json!({ "result": { "sessionId": "S9" } }),
    );

    assert_eq!(methods_sent_by_attach(transport), ["Target.attachToTarget"]);
}

#[test]
fn a_target_the_browser_reports_paused_is_released() {
    let transport = ScriptedTransport::new()
        .expect_with_events(
            "Target.attachToTarget",
            vec![attached(true)],
            json!({ "result": { "sessionId": "S9" } }),
        )
        .expect("Runtime.runIfWaitingForDebugger", json!({}));

    assert_eq!(
        methods_sent_by_attach(transport),
        ["Target.attachToTarget", "Runtime.runIfWaitingForDebugger"]
    );
}

#[test]
fn an_attach_event_for_another_session_does_not_skip_the_release() {
    let mut other = attached(false);
    other["params"]["sessionId"] = json!("S1");
    let transport = ScriptedTransport::new()
        .expect_with_events(
            "Target.attachToTarget",
            vec![other],
            json!({ "result": { "sessionId": "S9" } }),
        )
        .expect("Runtime.runIfWaitingForDebugger", json!({}));

    assert_eq!(
        methods_sent_by_attach(transport),
        ["Target.attachToTarget", "Runtime.runIfWaitingForDebugger"]
    );
}

#[test]
fn new_tabs_open_in_the_background() {
    let transport =
        ScriptedTransport::new().expect("Target.createTarget", json!({ "targetId": "T7" }));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let target = create_background_tab(&mut connection, "about:blank", deadline()).unwrap();

    assert_eq!(target, "T7");
    assert_eq!(sent.lock().unwrap()[0]["params"]["background"], true);
}

#[test]
fn the_crate_never_raises_a_tab() {
    let forbidden = [
        concat!("Page.", "bringToFront"),
        concat!("Target.", "activateTarget"),
    ];
    let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut scanned = 0;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for method in forbidden {
                    assert!(
                        !source.contains(method),
                        "{} calls {method}",
                        path.display()
                    );
                }
                scanned += 1;
            }
        }
    }
    assert!(scanned > 20);
}

fn release_is_sent_after(event: serde_json::Value) {
    let transport = ScriptedTransport::new()
        .expect_with_events(
            "Target.attachToTarget",
            vec![event],
            json!({ "result": { "sessionId": "S9" } }),
        )
        .expect("Runtime.runIfWaitingForDebugger", json!({}));

    assert_eq!(
        methods_sent_by_attach(transport),
        ["Target.attachToTarget", "Runtime.runIfWaitingForDebugger"]
    );
}

#[test]
fn an_attach_event_that_does_not_say_whether_it_waits_does_not_skip_the_release() {
    let mut silent = attached(false);
    silent["params"]
        .as_object_mut()
        .unwrap()
        .remove("waitingForDebugger");
    release_is_sent_after(silent);
}

#[test]
fn an_attach_event_inside_another_session_does_not_skip_the_release() {
    let mut nested = attached(false);
    nested["sessionId"] = json!("S-outer");
    release_is_sent_after(nested);
}
