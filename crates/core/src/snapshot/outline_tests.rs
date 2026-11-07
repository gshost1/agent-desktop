use super::*;
use serde_json::json;

fn sample_tree() -> Value {
    json!({
        "role": "window",
        "name": "Weather",
        "native_id": {"kind": "ax_identifier", "value": "SceneWindow"},
        "children": [
            {"role": "group", "available_actions": ["RightClick", "ScrollTo"], "children": [
                {"role": "group", "available_actions": ["SetFocus"], "children": [
                    {"role": "textfield", "name": "Search", "ref_id": "@s8f3k2p9:e3",
                     "value": "Lon", "available_actions": ["Click", "SetValue", "SetFocus"]}
                ]}
            ]},
            {"role": "group", "name": "Location List", "ref_id": "@s8f3k2p9:e4",
             "available_actions": ["Scroll"], "children": [
                {"role": "button", "name": "London, 46°", "ref_id": "@s8f3k2p9:e16",
                 "available_actions": ["Click", "Scroll"], "states": ["selected"]}
            ]},
            {"role": "group", "children_count": 12, "ref_id": "@s8f3k2p9:e20"}
        ]
    })
}

#[test]
fn renders_one_line_per_node_with_local_refs() {
    let outline = render_outline(&sample_tree());
    assert_eq!(
        outline,
        "window \"Weather\" #SceneWindow\n\
         \x20 textfield \"Search\" [e3] = \"Lon\" (SetValue)\n\
         \x20 group \"Location List\" [e4] (Scroll)\n\
         \x20   button \"London, 46°\" [e16] {selected}\n\
         \x20 group [e20] +12\n"
    );
}

#[test]
fn keeps_every_ref_of_the_tree() {
    let outline = render_outline(&sample_tree());
    for local in ["[e3]", "[e4]", "[e16]", "[e20]"] {
        assert!(outline.contains(local), "{local} missing from {outline}");
    }
    assert!(!outline.contains("s8f3k2p9"));
}

#[test]
fn does_not_collapse_a_group_that_carries_information() {
    let tree = json!({"role": "group", "states": ["focused"], "children": [{"role": "button", "name": "OK", "ref_id": "@s1:e1"}]});
    assert_eq!(
        render_outline(&tree),
        "group {focused}\n  button \"OK\" [e1]\n"
    );
}

#[test]
fn does_not_collapse_a_group_with_several_children() {
    let tree = json!({"role": "group", "children": [
        {"role": "button", "name": "A", "ref_id": "@s1:e1"},
        {"role": "button", "name": "B", "ref_id": "@s1:e2"}
    ]});
    assert_eq!(
        render_outline(&tree),
        "group\n  button \"A\" [e1]\n  button \"B\" [e2]\n"
    );
}

#[test]
fn escapes_names_and_marks_truncation_and_bounds() {
    let tree = json!({"role": "statictext", "name": "say \"hi\"\nnow", "subtree_truncated": true,
                      "bounds": {"x": 10.4, "y": 20.6, "width": 100.0, "height": 30.0}});
    assert_eq!(
        render_outline(&tree),
        "statictext \"say \\\"hi\\\"\\nnow\" @10,21 100x30 +…\n"
    );
}

#[test]
fn replaces_tree_with_outline_and_keeps_other_fields() {
    let mut body = json!({"app": "Weather", "snapshot_id": "s8f3k2p9", "ref_count": 4, "complete": true, "tree": sample_tree()});
    replace_tree_with_outline(&mut body);
    assert!(body.get("tree").is_none());
    assert_eq!(body["snapshot_id"], "s8f3k2p9");
    assert!(
        body["outline"]
            .as_str()
            .is_some_and(|text| text.starts_with("window \"Weather\""))
    );
}

#[test]
fn leaves_a_body_without_tree_unchanged() {
    let mut body = json!({"after_action": {"action": "click"}});
    replace_tree_with_outline(&mut body);
    assert_eq!(body, json!({"after_action": {"action": "click"}}));
}
