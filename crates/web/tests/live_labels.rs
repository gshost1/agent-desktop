//! Live headless-Chrome check that a visually hidden checkbox drawn by its
//! label is checked through the label, while an overlay still blocks it.
//! Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_labels -- --ignored`.

mod common;

use agent_desktop_core::ErrorCode;
use agent_desktop_web::{set_checked, snapshot_page, wait_for_load};
use common::{connect_to, deadline, find, launch, open_tab, serve};
use serde_json::{Value, json};

const SR_ONLY: &str = "position:absolute;width:1px;height:1px;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);border:0";

fn page(cover: bool) -> String {
    let overlay = if cover {
        r#"<div style="position:fixed;inset:0;background:white">Overlay</div>"#
    } else {
        ""
    };
    format!(
        r#"<!doctype html><html><head><title>Label</title></head><body>
<label style="display:inline-block;padding:8px 12px;border:1px solid #888">
<input type="checkbox" id="terms" style="{SR_ONLY}"><span>Accept terms</span></label>{overlay}
</body></html>"#
    )
}

fn checked_after(cover: bool) -> (Result<(), agent_desktop_core::AdapterError>, bool) {
    let port = serve(page(cover));
    let chrome = launch(if cover { "label-covered" } else { "label" });
    let mut connection = connect_to(&chrome);
    let tab = open_tab(&mut connection, &format!("http://127.0.0.1:{port}/"));
    wait_for_load(&mut connection, &tab, deadline()).unwrap();
    let snapshot = snapshot_page(&mut connection, &tab, deadline()).unwrap();
    let (_, checkbox) = find(&snapshot, "checkbox", "Accept terms");
    let outcome = set_checked(&mut connection, &tab, &checkbox, true, deadline()).map(|_| ());
    let state = connection
        .call(
            "Runtime.evaluate",
            json!({ "expression": "document.getElementById('terms').checked", "returnByValue": true }),
            Some(&tab.session_id),
            deadline(),
        )
        .unwrap();
    let checked = state.pointer("/result/value").and_then(Value::as_bool) == Some(true);
    (outcome, checked)
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_hidden_checkbox_its_label_draws_is_checked_through_the_label() {
    let (outcome, checked) = checked_after(false);
    outcome.unwrap_or_else(|error| panic!("{error:?}"));
    assert!(checked);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_hidden_checkbox_under_an_overlay_is_still_refused() {
    let (outcome, checked) = checked_after(true);
    let error = outcome.unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(!checked);
}
