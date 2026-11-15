use agent_desktop_core::{AppError, response::output::Response};
use serde_json::{Value, json};

use super::{apply_lines_output, present_lines, write_response};

fn snapshot_data() -> Value {
    json!({
        "snapshot_id": "s9",
        "ref_count": 1,
        "complete": true,
        "tree": { "role": "window", "name": "Main", "children": [
            { "ref_id": "@s9:e1", "role": "button", "name": "Send" }
        ]}
    })
}

fn written(response: &Response, pretty: bool) -> String {
    let mut out = Vec::new();
    write_response(&mut out, response, pretty).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn lines_replace_the_tree_and_indent_the_envelope() {
    let (result, pretty) = present_lines("snapshot", Ok(snapshot_data()), true);
    let data = result.unwrap();

    assert!(pretty);
    assert!(data.get("tree").is_none());
    assert_eq!(
        data["lines"],
        json!(["window \"Main\"", "  @s9:e1 button \"Send\""])
    );
    let text = written(&Response::ok("snapshot", data.clone()), pretty);
    assert!(text.contains("\n      \"  @s9:e1 button \\\"Send\\\"\"\n"));
    let parsed: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["data"], data);
}

#[test]
fn without_lines_the_tree_and_the_single_line_stay() {
    let (result, pretty) = present_lines("snapshot", Ok(snapshot_data()), false);
    let data = result.unwrap();

    assert!(!pretty);
    assert_eq!(data, snapshot_data());
    let text = written(&Response::ok("snapshot", data), pretty);
    assert_eq!(text.matches('\n').count(), 1);
}

#[test]
fn errors_and_treeless_results_stay_compact_under_lines() {
    let (error, error_pretty) =
        present_lines("click", Err(AppError::invalid_input("bad ref")), true);
    let (plain, plain_pretty) = present_lines("list-apps", Ok(json!({ "apps": [] })), true);

    assert!(error.is_err());
    assert!(!error_pretty);
    assert!(!plain_pretty);
    assert_eq!(plain.unwrap(), json!({ "apps": [] }));
}

#[test]
fn batch_entries_get_lines_and_their_neighbours_stay() {
    let mut data = json!({ "results": [
        { "ok": true, "command": "snapshot", "data": snapshot_data() },
        { "ok": true, "command": "click", "data": { "action": "click" } },
        { "ok": false, "command": "snapshot", "error": { "code": "STALE_REF" } }
    ]});

    assert!(apply_lines_output("batch", &mut data));
    assert!(data["results"][0]["data"].get("tree").is_none());
    assert_eq!(
        data["results"][0]["data"]["lines"][1],
        "  @s9:e1 button \"Send\""
    );
    assert_eq!(data["results"][1]["data"], json!({ "action": "click" }));
    assert_eq!(data["results"][2]["error"]["code"], "STALE_REF");
}

#[test]
fn an_unreadable_tree_keeps_the_result_and_names_why_lines_are_missing() {
    let data = json!({ "action": "click", "tree": { "name": "no role" } });
    let (result, pretty) = present_lines("click", Ok(data.clone()), true);
    let mut shown = result.unwrap();
    let reason = shown
        .as_object_mut()
        .and_then(|body| body.remove("lines_error"))
        .and_then(|reason| reason.as_str().map(str::to_owned));

    assert!(!pretty);
    assert!(reason.is_some_and(|reason| !reason.is_empty()));
    assert_eq!(shown, data);
}
