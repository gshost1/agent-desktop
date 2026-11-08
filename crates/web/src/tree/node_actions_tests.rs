use serde_json::json;

use super::actions_of;
use crate::tree::cdp_ax_node::CdpAxNode;

fn node(properties: serde_json::Value) -> CdpAxNode {
    serde_json::from_value(json!({ "nodeId": "1", "properties": properties })).unwrap()
}

#[test]
fn controls_advertise_the_actions_the_crate_delivers() {
    let focusable = node(json!([{ "name": "focusable", "value": { "value": true } }]));
    assert_eq!(actions_of("button", &focusable), ["Click", "SetFocus"]);
    assert_eq!(
        actions_of("checkbox", &node(json!([]))),
        ["Click", "Toggle"]
    );
    assert_eq!(
        actions_of("combobox", &node(json!([]))),
        ["Click", "Select"]
    );
    assert_eq!(
        actions_of("textfield", &node(json!([]))),
        ["SetValue", "TypeText"]
    );
}

#[test]
fn editable_content_is_fillable_and_static_content_is_not() {
    let editable =
        node(json!([{ "name": "editable", "value": { "type": "token", "value": "richtext" } }]));
    assert_eq!(actions_of("group", &editable), ["SetValue", "TypeText"]);
    assert_eq!(
        actions_of("combobox", &editable),
        ["Click", "SetValue", "TypeText"]
    );
    assert!(actions_of("statictext", &node(json!([]))).is_empty());
    assert!(actions_of("heading", &node(json!([]))).is_empty());
}
